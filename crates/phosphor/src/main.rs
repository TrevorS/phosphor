//! The phosphor binary: terminal setup, the event loop, input routing and panes.
//!
//! This is the app layer — the one place `ratatui` and `crossterm` are allowed.
//! It owns the frame: Steel decides what is on screen, this decides when pixels
//! land (Q12). Input is decoded here into Actions; nothing else emits them.
//!
//! Owned by `spine`. The loop lands in Window C; what is here now is `T090`.
//!
//! # `T090` — the S1 host, and its demolition date
//!
//! `CP-1`'s run line is `cargo run -- src/some_real_file.rs`. Windows A and B
//! built the whole widget layer around a `fn main() {}`, so that line drew
//! nothing, every tape died on `Require phosphor`, and the checkpoint's manual
//! half had nothing to open. This file is the application those widgets were
//! missing, and **it is deliberately the thinnest thing that makes `CP-1`
//! judgeable**:
//!
//! * open the file named on argv, pick a theme by slug, build one frame out of
//!   [`BufferView`] + [`StatusLine`] + [`FloatSlot`], draw it through `T014`'s
//!   synchronized-output wrapper, and quit restoring the terminal;
//! * input rides the vendored core's own `editor_crossterm` handler, exactly as
//!   `TASKS.md`'s S1 preamble says S1 does.
//!
//! **Nothing above it may grow to depend on it.** There is no `Action` enum
//! (`T019`), no Steel (`T020`+), no input machine (`T026`), no panes (`T088`),
//! and no state that outlives the loop below. `T026` deletes this file's event
//! handling in one commit; the two lines of `Cargo.toml` that turn the fork's
//! `crossterm` feature on go with it.
//!
//! # What `T019` has to carry, learned here
//!
//! Recorded because the S1 host is the first thing to actually run the widget
//! layer, and three of these are only visible from a loop:
//!
//! 1. **Scroll is a request, and the fork disagrees.**
//!    `buffer_view::apply_scroll` is invariant 3's single writer, but
//!    `Editor::input` ends every keystroke with the fork's own `focus()`, and
//!    `Editor::mouse` calls `scroll_up`/`scroll_down` directly. So while S1
//!    rides the vendored handler, the viewport moves from two places. `Action`
//!    needs a `Scroll(ScrollRequest)` variant — `ScrollRequest` is already
//!    shaped as its payload, `RevealRow { row, margin }` included — and `T026`
//!    has to stop calling `input`/`mouse` rather than wrap them.
//! 2. **A mode is a fact the statusline reads, not a flag input owns.** The
//!    chip here is hardcoded [`Mode::Normal`] because S1 has no modality at all
//!    and every keystroke inserts text. `T026`'s grammar owns the real one, and
//!    `soft_wrap::set_mode` already wants it (whitespace marks are INSERT-only).
//! 3. **Dirty is per buffer and comes from the edit stream**, not from a save
//!    path — see [`dirty_flag`].
//! 4. **A float is opened by whoever asked for it and closed by `esc`.** The
//!    one-float rule lives in [`FloatSlot`]; what `Action` owes it is
//!    `OpenFloat(kind)` / `CloseFloat`, and `T021`'s broken-`init.scm` float is
//!    the first real caller.
//!
//! # The two flags that are not in a mockup
//!
//! `--theme <slug>` is assumed by eight tapes and is the product's own CLI. The
//! other two are scaffolding for `CP-1`'s manual half, and are marked as such
//! in `--help`:
//!
//! * `--float <mood>` opens `T084`'s fixture body in one of the two moods, so
//!   the float contract can be looked at on a real terminal. No *feature* opens
//!   a float at S1 — the first one that does is `T021`.
//! * `--soft-wrap` turns `T081` on. The mockups say soft wrap is **off by
//!   default** ("Text details: soft-wrap off by default, ↪ continuation when
//!   on"), so the default here is off and this flag is the only way `CP-1` can
//!   see the on state before `T026` has a key for it.

use std::cell::Cell;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use clap::{Parser, ValueEnum};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent};
use phosphor_term::{Frame, Term};
use phosphor_ui::buffer_view::{self, BufferView, editor_area};
use phosphor_ui::float::{Float, FloatFooter, FloatHeader, FloatSlot, FooterHint, TextBody};
use phosphor_ui::soft_wrap;
use phosphor_ui::status_line::{CursorVm, FileVm, Mode, SessionState, StatusLine, StatusLineVm};
use phosphor_ui::theme::{BUILTIN_SLUGS, Theme, builtin};
use ratatui::layout::Rect;
// The same type `phosphor_ui::buffer_view` re-exports. Named through the fork
// here on purpose: this file is the one place that talks to the vendored
// handler, and `Editor::input` / `Editor::mouse` are the fork's API, not the
// widget layer's.
use ratatui_code_editor::editor::Editor;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// The S1 host's argument surface.
#[derive(Debug, Parser)]
#[command(
    name = "phosphor",
    version,
    about = "phosphor — an agent-native terminal editor",
    long_about = "Opens one file and draws it: BufferView + StatusLine, every frame inside a \
                  synchronized-output block.\n\nThis is the S1 host (T090) — scaffolding for \
                  CP-1. There is no Steel runtime, no keymap and no agent session yet; keys go \
                  to the vendored editor core, `q` or `esc` quits."
)]
struct Cli {
    /// File to open.
    #[arg(value_name = "FILE")]
    path: PathBuf,

    /// Theme slug: one of the six shipped themes.
    #[arg(long, value_name = "SLUG", default_value = DEFAULT_THEME)]
    theme: String,

    /// SCAFFOLDING (T090): open T084's fixture float, so CP-1 can see the
    /// float contract on a real terminal. `esc` closes it.
    #[arg(long, value_name = "MOOD", value_enum)]
    float: Option<FloatMood>,

    /// SCAFFOLDING (T090): turn T081's soft wrap on. Off by default, as the
    /// mockups specify; there is no Action to toggle it until T026.
    #[arg(long)]
    soft_wrap: bool,
}

/// Which mood [`Cli::float`] opens. Design Language §4's two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FloatMood {
    /// Pickers, help, diffs — anything you asked for.
    Informational,
    /// Questions and permission asks: warmer border, darker body.
    NeedsYou,
}

/// §10: "Phosphor (dark + light) ships as the v1 default", dark first.
const DEFAULT_THEME: &str = "phosphor-dark";

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&*error);
            ExitCode::FAILURE
        }
    }
}

/// The one legitimate write to stderr in this binary.
///
/// Every fallible step below either happens before [`Term`] exists or returns
/// through it, and `Term` restores on drop — so by the time this prints, the
/// alternate screen is gone and the message lands where it can be read.
#[expect(
    clippy::print_stderr,
    reason = "the CLI door reports failures on stderr; the TUI is already restored here"
)]
fn report(error: &dyn Error) {
    eprintln!("phosphor: {error}");
    let mut source = error.source();
    while let Some(cause) = source {
        eprintln!("  caused by: {cause}");
        source = cause.source();
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let theme = builtin(&cli.theme).ok_or_else(|| unknown_theme(&cli.theme))?;

    let text = std::fs::read_to_string(&cli.path)
        .map_err(|err| format!("{}: {err}", cli.path.display()))?;
    let language = language_of(&cli.path);

    let mut editor = Editor::new(language, &text, Vec::new())?;
    // Order matters: `soft_wrap::configure` puts folds back on (without the
    // gutter column `8e` does not draw), so it goes second.
    buffer_view::configure(&mut editor, &theme);
    soft_wrap::configure(&mut editor, &theme);
    let dirty = dirty_flag(&mut editor);

    // The path as the user typed it. Repo-relative is what the mockups draw,
    // but the repo root is `phosphor-vcs`'s answer (`T071`) and inventing one
    // here would be a value nobody asked for.
    let path = cli.path.display().to_string();

    let mut floats = FloatSlot::empty();
    if let Some(mood) = cli.float {
        floats.open(fixture_float(mood));
    }

    let mut term = Term::new()?;
    loop {
        // The size the *next* frame will be laid out at. `draw` re-splits
        // `frame.area()` itself, so this is only for the two things that need
        // `&mut editor` and therefore cannot happen inside the closure: the
        // wrap width, and the area the vendored input handler measures against.
        let size = term.size()?;
        let (body, _status) = split(Rect::new(0, 0, size.width, size.height));
        if cli.soft_wrap {
            // Free when the width has not changed, and it moves no viewport.
            soft_wrap::wrap_to(&mut editor, body);
        }

        let vm = StatusLineVm {
            mode: Mode::Normal,
            file: Some(FileVm {
                path: &path,
                dirty: dirty.get(),
            }),
            // Truthful, and the truth at S1 is that there is no session, no
            // store to count unseen regions in, and no VCS adapter. `T050`,
            // `T041` and `T071` fill these in; a fixture here would be a lie
            // on a real terminal.
            session: SessionState::None,
            ask_pending: false,
            unseen: 0,
            vcs: None,
            cursor: Some(cursor_of(&editor)),
        };

        term.draw(|frame| draw(frame, &editor, &theme, &vm, &floats))?;

        match event::read()? {
            Event::Key(key) => match key_step(key, &mut floats) {
                Step::Quit => break,
                Step::Handled => {}
                Step::ToEditor => {
                    editor.input(key, &editor_area(body))?;
                }
            },
            Event::Mouse(mouse) => to_editor_mouse(&mut editor, mouse, body)?,
            // A resize redraws from the new size on the next turn of the loop;
            // so does everything else this arm swallows (focus, paste).
            _ => {}
        }
    }

    term.restore()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The frame
// ---------------------------------------------------------------------------

/// Buffer area above, statusline on the last row — the layout every `CP-1`
/// mockup has, and the same split `T018`'s golden frames use.
fn split(area: Rect) -> (Rect, Rect) {
    // The statusline's row comes out of the buffer's height, so a terminal too
    // short for both gives the row to the statusline and a terminal of no
    // height gives neither of them a rect that is not inside `area`.
    let status_height = area.height.min(1);
    let body = Rect {
        height: area.height - status_height,
        ..area
    };
    let status = Rect {
        y: area.y + body.height,
        height: status_height,
        ..area
    };
    (body, status)
}

/// One frame: buffer, then the float over it, then the statusline.
///
/// The order is `8d`'s — [`FloatSlot::render`] dims what is behind it, so it
/// runs after the buffer and over the buffer's area only. The statusline never
/// dims: §9's dim means "behind", and chrome is not behind anything.
fn draw(
    frame: &mut Frame<'_>,
    editor: &Editor,
    theme: &Theme,
    vm: &StatusLineVm<'_>,
    floats: &FloatSlot<'_>,
) {
    let (body, status) = split(frame.area());

    // The state column is empty on purpose: §3's marks are a store query
    // (`T041`, S5) and there is no store. The column is still reserved, which
    // is the half of the 3-column contract S1 can be held to.
    frame.render_widget(BufferView::new(editor, theme), body);
    floats.render(body, frame.buffer_mut(), theme);
    StatusLine::new(vm, theme).render(status, frame.buffer_mut());

    if let Some((x, y)) = editor.get_visible_cursor(&editor_area(body)) {
        frame.set_cursor_position((x, y));
    }
}

/// The `12:1` counter, 1-based, as `1a` and `8e` draw it.
fn cursor_of(editor: &Editor) -> CursorVm {
    let (row, col) = editor.code_ref().point(editor.get_cursor());
    CursorVm {
        line: u32::try_from(row.saturating_add(1)).unwrap_or(u32::MAX),
        col: u32::try_from(col.saturating_add(1)).unwrap_or(u32::MAX),
    }
}

/// §5's `[+]`, wired to the edit stream.
///
/// The flag is set by the vendored core's change callback — which fires once
/// per committed edit batch — rather than by diffing the buffer against the
/// file each frame. It is one-way for now: there is no save path at S1 (no
/// `Action`, `T019`), so nothing can clear it, and a host that cannot write to
/// disk cannot lose anything by saying so.
fn dirty_flag(editor: &mut Editor) -> Rc<Cell<bool>> {
    let dirty = Rc::new(Cell::new(false));
    let flag = Rc::clone(&dirty);
    editor.set_change_callback(Box::new(move |_| flag.set(true)));
    dirty
}

// ---------------------------------------------------------------------------
// Input — the temporary path
// ---------------------------------------------------------------------------

/// What the loop does with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Leave the loop and restore the terminal.
    Quit,
    /// The host consumed it; the buffer never sees it.
    Handled,
    /// Hand it to the vendored `editor_crossterm` handler.
    ToEditor,
}

/// The three keys the host keeps for itself, and why each one is not an
/// invented keymap.
///
/// * `q` and `esc` are `T090`'s own acceptance criterion — *"`q`/`esc`
///   restores the terminal"*. They cost the buffer a printable `q`, which is
///   the price of having no modes until `T026`.
/// * `esc` closes an open float first: Design Language §9, *"esc closes
///   top-down"*, and there is only ever one level.
/// * `ctrl-c` is the safety valve. Raw mode means the terminal will not deliver
///   SIGINT, and a host that ignored it would be a host you cannot get out of.
///   The vendored handler maps it to `Copy`, which nothing at S1 can paste.
///
/// Everything else — arrows, clicks, text — goes to the fork.
fn key_step(key: KeyEvent, floats: &mut FloatSlot<'_>) -> Step {
    // Under the kitty protocol every press is also reported as a release
    // (`T014` negotiates `REPORT_EVENT_TYPES`), and the vendored handler does
    // not look at `kind` — so without this every keystroke would apply twice.
    if key.kind == KeyEventKind::Release {
        return Step::Handled;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Step::Quit,
        KeyCode::Char('q') if key.modifiers.is_empty() => Step::Quit,
        KeyCode::Esc if floats.close().is_some() => Step::Handled,
        KeyCode::Esc => Step::Quit,
        _ => Step::ToEditor,
    }
}

/// Clicks and wheel, straight to the fork.
///
/// **The area is [`editor_area`], not the widget's own rect.** The vendored
/// core measures click-to-offset from `area.left()`, and the two cells in front
/// of the line numbers belong to `BufferView`; passing the outer rect would put
/// the cursor two columns left of the character under the pointer.
fn to_editor_mouse(
    editor: &mut Editor,
    mouse: MouseEvent,
    body: Rect,
) -> Result<(), Box<dyn Error>> {
    editor.mouse(mouse, &editor_area(body))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Fixtures and lookups
// ---------------------------------------------------------------------------

/// `T084`'s fixture body, in the informational mood.
const INFORMATIONAL_LINES: &[&str] = &[
    "the one chrome primitive: header, body and footer",
    "inside a mood border (Design Language §4).",
    "",
    "this is T084's fixture body. pickers, diffs and",
    "asks plug their own in from S5 on.",
];

/// The same, in the needs-you mood.
const NEEDS_YOU_LINES: &[&str] = &[
    "the needs-you mood: warmer border, darker body,",
    "for questions and permission asks (§4).",
    "",
    "a real ask queues rather than replacing whatever",
    "float is open (Q9). this fixture does neither.",
];

static INFORMATIONAL_BODY: TextBody<'static> = TextBody::new(INFORMATIONAL_LINES);
static NEEDS_YOU_BODY: TextBody<'static> = TextBody::new(NEEDS_YOU_LINES);

/// Every float carries its keys in the footer (§4).
const FIXTURE_HINTS: &[FooterHint<'static>] = &[
    FooterHint::new("esc", "close"),
    FooterHint::new("q", "quit"),
];

/// A float in the asked-for mood, built from `T084`'s fixture body.
fn fixture_float(mood: FloatMood) -> Float<'static> {
    let footer = FloatFooter::new(FIXTURE_HINTS);
    match mood {
        FloatMood::Informational => Float::informational(
            FloatHeader::new("float fixture").meta("informational"),
            &INFORMATIONAL_BODY,
            footer,
        ),
        FloatMood::NeedsYou => Float::needs_you(
            FloatHeader::new("float fixture").meta("needs-you"),
            &NEEDS_YOU_BODY,
            footer,
        ),
    }
}

/// Extension → the vendored core's language name.
///
/// **Temporary, and small on purpose.** Language selection belongs to
/// `define-language` (`T036`+, S4), which owns grammars, LSP servers and
/// comment syntax together; this is the file-extension half of it, covering
/// exactly the ten grammars the fork is built with (`grammars-phosphor`).
/// An unknown extension is not an error — the core skips parser setup and the
/// buffer renders unhighlighted.
fn language_of(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "py" => "python",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "css" => "css",
        "html" | "htm" => "html",
        // Not a grammar the fork knows, which is the point: `Code::new` skips
        // parser setup for an unrecognised name and the buffer renders in
        // `syntax.text`. No new failure mode, no guessed grammar.
        _ => "text",
    }
}

/// A slug we do not ship, reported with the six we do.
///
/// `T011` loads themes from disk and this message is where that will surface —
/// today "unknown" and "not a builtin" are the same thing.
fn unknown_theme(slug: &str) -> String {
    let known = BUILTIN_SLUGS.join(", ");
    format!("unknown theme `{slug}` — shipped themes are: {known}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Only the decisions, not the loop: everything else about this file is
// perceptual and belongs to `CP-1`'s Tier-2 and Tier-3 halves. What is covered
// here is the routing table — which keys the host keeps and which reach the
// vendored handler — because a mistake there is silent on a screenshot and
// loud on a real terminal.

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use phosphor_ui::float::FloatSlot;
    use ratatui::layout::Rect;

    use super::{FloatMood, Step, fixture_float, key_step, language_of, split};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn q_and_esc_are_the_way_out() {
        let mut floats = FloatSlot::empty();
        assert_eq!(key_step(press(KeyCode::Char('q')), &mut floats), Step::Quit);
        assert_eq!(key_step(press(KeyCode::Esc), &mut floats), Step::Quit);
        assert_eq!(
            key_step(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &mut floats
            ),
            Step::Quit,
            "raw mode swallows SIGINT, so ctrl-c has to be handled or there is no way out"
        );
    }

    #[test]
    fn esc_closes_the_float_before_it_quits() {
        let mut floats = FloatSlot::empty();
        floats.open(fixture_float(FloatMood::Informational));

        assert_eq!(key_step(press(KeyCode::Esc), &mut floats), Step::Handled);
        assert!(
            !floats.is_open(),
            "esc must close the float, not the editor"
        );
        assert_eq!(
            key_step(press(KeyCode::Esc), &mut floats),
            Step::Quit,
            "with nothing left to close, esc is the quit key again"
        );
    }

    #[test]
    fn a_release_never_reaches_the_buffer() {
        // The kitty protocol reports press *and* release (`T014` asks for
        // REPORT_EVENT_TYPES) and the vendored handler ignores `kind`, so
        // without this filter every keystroke would be applied twice.
        let release = KeyEvent::new_with_kind_and_state(
            KeyCode::Char('a'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
            KeyEventState::NONE,
        );
        assert_eq!(key_step(release, &mut FloatSlot::empty()), Step::Handled);
    }

    #[test]
    fn everything_else_rides_the_vendored_handler() {
        let mut floats = FloatSlot::empty();
        for code in [
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Char('a'),
            KeyCode::Enter,
            KeyCode::Backspace,
        ] {
            assert_eq!(
                key_step(press(code), &mut floats),
                Step::ToEditor,
                "{code:?}"
            );
        }
    }

    #[test]
    fn a_grammar_the_fork_was_not_built_with_is_not_a_failure() {
        assert_eq!(language_of("src/main.rs".as_ref()), "rust");
        assert_eq!(language_of("Cargo.toml".as_ref()), "toml");
        assert_eq!(language_of("runtime/init.SCM".as_ref()), "text");
        assert_eq!(language_of("README".as_ref()), "text");
    }

    #[test]
    fn the_statusline_gets_the_last_row_and_never_a_second_one() {
        let (body, status) = split(Rect::new(0, 0, 80, 24));
        assert_eq!(body.height, 23);
        assert_eq!(status, Rect::new(0, 23, 80, 1));

        // A one-row terminal is all statusline, and a zero-row one draws
        // nothing rather than underflowing.
        let (body, status) = split(Rect::new(0, 0, 80, 1));
        assert_eq!(body.height, 0);
        assert_eq!(status, Rect::new(0, 0, 80, 1));
        let (body, status) = split(Rect::new(0, 0, 80, 0));
        assert!(body.is_empty() && status.is_empty());
    }
}
