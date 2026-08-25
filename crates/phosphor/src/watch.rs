//! The disk watcher (`T069`, screen `1d`).
//!
//! One background thread, `notify-debouncer-full`, and a [`Poster`]. It answers
//! exactly one question — *did this file change under an open buffer?* — and
//! posts `note-disk-change` when it did.
//!
//! # It reports; it never refreshes
//!
//! Invariant 3 is the reason this module is so small. *"Buffer holds stable;
//! nothing moves unless you asked"* — so the watcher's whole job is to make the
//! disagreement **visible**, and closing it is `reload-from-disk`, which only a
//! keystroke or a door can call. There is deliberately no path from an event to
//! a buffer mutation in this file, and that absence is the feature: a watcher
//! that could refresh is one bug away from moving your cursor while you type.
//!
//! # Debouncing is the feature, not a nicety
//!
//! `T069`'s own line calls it load-bearing, and the shape of the problem is
//! specific: **one save is not one event.** An agent writing a file produces a
//! burst — `notify` reports the truncate, each write and the metadata touch
//! separately, and an editor that writes to a temp file and renames produces a
//! `Create`/`Remove` pair on top. Raw `notify` would flash `✱` several times
//! for a single save and each flash would be equally true, which is what makes
//! the raw signal useless rather than merely noisy.
//!
//! [`notify_debouncer_full`] collapses a burst into one delivery, and
//! [`DEBOUNCE`] is how long it waits for the burst to finish.
//!
//! # Why the binary and not a library crate
//!
//! `crate::picker`'s `nucleo` note makes the argument and it applies unchanged:
//! this owns a thread, and a crate that spawns one outlives a frame. The binary
//! owns the loop that drains what this posts.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify_debouncer_full::notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use phosphor_core::action::{Action, FileAction};
use phosphor_core::request::Actor;

use crate::events::{AppEvent, Posted, Poster};

/// How long a burst has to go quiet before it counts as one change.
///
/// **250ms, and the number is a trade with two named ends.** Too short and one
/// save arrives as two `✱`s, which is the defect debouncing exists to prevent;
/// too long and the indicator lags a change you can already see in another
/// window. `notify-debouncer-full`'s own examples use two seconds, which is
/// tuned for bulk filesystem sync rather than for a person watching an agent
/// edit one file, and two seconds of silence after claude writes would read as
/// the editor having missed it.
const DEBOUNCE: Duration = Duration::from_millis(250);

/// What [`AppEvent::Posted`] carries from this producer.
const SOURCE: &str = "disk";

/// A handle that keeps the watcher alive.
///
/// **Dropping this stops the watching**, which is why the loop holds one rather
/// than letting `spawn` return `()`. `notify`'s watcher shuts its own thread
/// down on drop, so an editor that closes every buffer stops paying for a
/// thread it is not using.
pub(crate) struct Watch {
    /// Paths currently watched, so [`Watch::follow`] can tell a new file from
    /// one already covered.
    watched: Vec<PathBuf>,
    /// The command channel to the watcher thread. [`None`] once the thread has
    /// gone, which makes every later call a no-op rather than a panic.
    commands: Option<mpsc::Sender<Command>>,
}

/// What the loop asks the watcher thread to do.
enum Command {
    Follow(PathBuf),
    Drop(PathBuf),
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch")
            .field("watched", &self.watched)
            .field("live", &self.commands.is_some())
            .finish()
    }
}

impl Watch {
    /// A watcher that watches nothing, for a session that never opens a file
    /// and for tests that do not want a thread.
    pub(crate) fn idle() -> Self {
        Self {
            watched: Vec::new(),
            commands: None,
        }
    }

    /// Starts the watcher thread and hands back the handle (`T069`).
    ///
    /// **A watcher that fails to start is a working editor**, which is why this
    /// answers [`Watch::idle`] rather than an error. The same call
    /// `Shared::opened` makes for the seen journal, for the reason it gives:
    /// *"a history is not the file"*. Here it is smaller still — the cost of no
    /// watcher is that `✱` never appears, and every other thing the editor does
    /// is unaffected.
    pub(crate) fn spawn(poster: Poster) -> Self {
        let (commands, orders) = mpsc::channel();
        match std::thread::Builder::new()
            .name("phosphor-disk-watch".to_owned())
            .spawn(move || run(&poster, &orders))
        {
            Ok(_) => Self {
                watched: Vec::new(),
                commands: Some(commands),
            },
            // **The failure path is the idle watcher, named rather than
            // reconstructed.** A `Self { commands: None }` here would be the
            // same three fields written twice, and the second copy is where a
            // later field gets forgotten.
            Err(_) => Self::idle(),
        }
    }

    /// Watch this file, if it is not already watched.
    pub(crate) fn follow(&mut self, path: &Path) {
        if self.watched.iter().any(|held| held == path) {
            return;
        }
        let Some(commands) = &self.commands else {
            return;
        };
        if commands.send(Command::Follow(path.to_path_buf())).is_ok() {
            self.watched.push(path.to_path_buf());
        }
    }

    /// Stop watching this file.
    pub(crate) fn unfollow(&mut self, path: &Path) {
        let Some(index) = self.watched.iter().position(|held| held == path) else {
            return;
        };
        self.watched.remove(index);
        if let Some(commands) = &self.commands {
            let _ = commands.send(Command::Drop(path.to_path_buf()));
        }
    }
}

/// The watcher thread.
///
/// **The file's parent is watched, not the file**, and that is not a
/// simplification. An editor saving a file typically writes a temporary and
/// renames it over the target, which replaces the inode — a watch on the old
/// inode then reports nothing at all, because the thing it was watching still
/// exists and simply is not the file any more. Watching the directory
/// non-recursively sees the rename as an event on a name, which is what a
/// buffer actually cares about. `notify`'s own README names this as the
/// portable approach and it is the one thing about this module that would look
/// like a mistake without a comment.
fn run(poster: &Poster, orders: &mpsc::Receiver<Command>) {
    let (events, incoming) = mpsc::channel();
    let Ok(mut debouncer) = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
        // A send failure means the loop is gone. Nothing to do but stop
        // caring; the thread ends when its orders channel closes.
        let _ = events.send(result);
    }) else {
        return;
    };

    // Which names we actually care about, since a directory watch reports its
    // whole directory. Kept here rather than on `Watch` because this is the
    // thread that has to filter, and shipping the set across per event would
    // be a lock for something one thread owns.
    let mut wanted: Vec<PathBuf> = Vec::new();

    loop {
        // Orders first, so a file opened this frame is watched before the next
        // event is judged against the set. Draining here rather than once per
        // event is also the only place that can see the handle go: a closed
        // orders channel means `Watch` was dropped, and this thread ends with
        // it rather than watching for a loop that no longer exists.
        loop {
            let order = match orders.try_recv() {
                Ok(order) => order,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            };
            match order {
                Command::Follow(path) => {
                    if let Some(parent) = path.parent() {
                        let _ = debouncer.watch(parent, RecursiveMode::NonRecursive);
                    }
                    if !wanted.contains(&path) {
                        wanted.push(path);
                    }
                }
                Command::Drop(path) => {
                    wanted.retain(|held| held != &path);
                    // The parent stays watched. Two buffers in one directory
                    // share a watch, and unwatching on the first close would
                    // silently stop reporting the second.
                }
            }
        }

        match incoming.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(debounced)) => {
                for event in debounced {
                    for path in &event.paths {
                        if !wanted.iter().any(|held| held == path) {
                            continue;
                        }
                        // **`Actor::System`, always, from this thread.** The
                        // watcher cannot see an author and must not invent one;
                        // the loop is what knows whether a turn was running and
                        // re-attributes on the way in. See `store::DiskChange`.
                        let action = Action::File(FileAction::NoteDiskChange {
                            path: path.clone(),
                            changed_by: Actor::System,
                        });
                        if !poster.post(AppEvent::Posted(Posted {
                            source: SOURCE,
                            action,
                        })) {
                            return;
                        }
                    }
                }
            }
            // A watch error is not a reason to stop watching everything else.
            Ok(Err(_)) => {}
            // **Nothing happened, so nothing is posted.** The timeout exists
            // only to go round and pick up orders; an editor sitting idle must
            // cost no frames, and a `Woke` here would redraw the screen five
            // times a second forever.
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
    }
}
