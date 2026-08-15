//! The tree interpreter (`T079`) — a view tree walked into ratatui calls.
//!
//! The Rust half of Q12. `phosphor-steel` produces a
//! [`Tree`]; this walks it and drives the primitives in
//! this crate. *Does it produce pixels? Rust. Does it decide which pixels?
//! Steel.*
//!
//! # A pure function of `(tree, area, clock, resources)`
//!
//! Nothing here is stateful and nothing here mutates. That is what makes
//! [`crate::frame::FrameCache`] sound: the same tree drawn twice into the same
//! area produces the same cells, so a frame can be redrawn from cache without
//! asking anyone's permission. The two deliberate exceptions are the clock — a
//! [`Node::Spinner`] and a [`Node::Elapsed`] render the difference between
//! `now` and their [`Millis`] mark, so an animation costs zero recompositions —
//! and the buffers, which arrive through [`Resources`] as `&Editor`, never
//! `&mut`.
//!
//! # What this crate is allowed to know
//!
//! `phosphor_core::vm`, `::view`, `::request` and `::query::Revision`. Not
//! `::store` (`scripts/lint-no-store-mutation.sh`) and not `::action`
//! (`scripts/lint-no-action-in-ui.sh`), which is why a
//! [`Node::Buffer`] carries a [`BufferId`] rather than
//! anything you could edit through: the host resolves ids to editors and hands
//! them over by shared reference.
//!
//! # Which primitives exist, and which defer
//!
//! A node kind is a *protocol* commitment, made in one edit at `T078`; the
//! widget behind it lands with its own phase. Rather than half-draw them, an
//! arm with no widget draws **nothing** and records its tag in
//! [`Report::deferred`], so an unbuilt surface is visible to the host instead of
//! silently blank.
//!
//! **This list is checked by a test, because the last one was not.** It used to
//! carry all twelve kinds with a *still deferring* sentence under it, and it
//! kept saying that about `gutter`, `virtual-text` and `key-hints` for a whole
//! window after their arms started drawing. What holds it to the tree now is
//! `tests::the_deferred_set_is_exactly_the_kinds_named_here`, which draws one
//! node of every row below and asserts the tags that come back, and draws the
//! kinds that are *not* below and asserts none do. A widget that starts drawing
//! reddens it, and the fix is to delete a row — one edit, next to the code.
//!
//! | still deferred | widget | task |
//! |---|---|---|
//! | `tab-bar` | `TabBar` | `T089` |
//! | `picker` | `Picker` | `T045` |
//! | `diff` | `DiffBody` | `T063` |
//! | `question` | `QuestionBody` | `T059` |
//! | `transcript` | `TranscriptPane` | `T054` |
//! | `prompt` | `PromptLine` | `T058` |
//! | `watch` | `WatchOverlay` | `T076` |
//!
//! Every other kind draws. The two that arrived most recently are `completion`
//! ([`crate::float::CompletionList`], `T038`) and `signature`
//! ([`crate::float::SignatureBody`], `T039`); before them, `gutter`
//! ([`crate::gutter`], `T031`), `virtual-text` ([`crate::virtual_text`],
//! `T032`) and `key-hints` ([`crate::key_hints`], `T034` / `T086`) — each in an
//! arm of its own, which is what the split below bought: five widget tasks
//! landing in five places instead of colliding in one shared arm. The rest stay
//! grouped in a single arm and split the same way when their phase arrives.
//!
//! `completion` and `signature` got no new module, exactly as this table used to
//! say they would: the completion list *is* a float in the passive mood, which
//! `T038` added to [`crate::float`], and `T039` renders signature help through
//! the same chrome. Both read their session off [`Resources`] rather than off
//! the node, because `view.rs` gives neither kind any props — *"there is one
//! active completion session and the store holds it"*.
//!
//! [`Density::Footer`] *inside a float* is not this table's business either
//! way — it renders through [`crate::float::FloatFooter`] and the float's own
//! chrome, and always did.
//!
//! # Known gap, flagged not folded in
//!
//! **A `Node::Line` cannot say what ground it is painted on.** The statusline's
//! field (`#1a201a`, Design Language §5) is painted by
//! [`crate::status_line::StatusLine`] today; a statusline *composed as a tree*
//! (`T025`) has no way to ask for it — [`Tone`] names foregrounds and
//! [`phosphor_core::view::Tint`] is a row tint on
//! the `spans` hatch alone. This interpreter therefore draws a line
//! transparently, over whatever the caller painted. Raised as a contract
//! question rather than patched here, because the view tree is `spine`'s single
//! writer and a prop is a protocol change.
//!
//! Owned by `spine`.

use core::cell::RefCell;
use core::time::Duration;

use phosphor_core::request::BufferId;
use phosphor_core::view::{
    Axis, Child, Constraint, Density, Emphasis, Float as ViewFloat, Glyph, Millis,
    Mood as ViewMood, Node, SessionState as ViewSessionState, SpanRow, Tint, Tone, Tree,
};
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Constraint as UiConstraint, Direction, Layout, Rect};
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::text::Span;
use ratatui_core::widgets::Widget;

use crate::buffer_view::{BufferView, Editor, StateMark};
use crate::float::{
    Anchor, CompletionList, CompletionVm, Float as UiFloat, FloatBody, FloatFooter, FloatHeader,
    FloatSlot, FooterHint, Mood as UiMood, SignatureBody, SignatureVm,
};
use crate::status_line::{SessionState as UiSessionState, Spinner, format_elapsed};
use crate::theme::Theme;

/// §8: the spinner advances one frame every 80ms.
const SPINNER_PERIOD_MS: u64 = 80;

/// What the host lends the interpreter for the duration of one frame.
///
/// The seam that keeps `phosphor-ui` unable to mutate while still being able to
/// draw a buffer: the tree names a [`BufferId`], the host resolves it, and what
/// comes back is `&Editor`. There is no `&mut` anywhere in this trait and there
/// must never be one — *"holds `&Editor`, never `&mut` — which is how
/// 'rendering cannot scroll' stops being a promise and becomes a compile
/// error"* (`buffer_view.rs`).
///
/// # Soft wrap is reconciled at state change, not at frame time
///
/// [`Node::Buffer`]'s `soft_wrap` prop cannot be honoured from here:
/// `crate::soft_wrap::wrap_to` needs `&mut Editor`, and a frame that could
/// re-wrap could also move the cursor — invariant 3. The host applies it when
/// the tree changes (a [`crate::frame::FrameCache`] miss), which is also the
/// only time it can change. The interpreter reports the request in
/// [`Report::soft_wrap_requested`] so a host that forgot has something to see.
pub trait Resources: core::fmt::Debug {
    /// The editor behind a buffer id, or `None` if the tree names one this host
    /// does not have. An absent buffer draws nothing; a stale composition must
    /// never be able to break a frame (`query.rs`: *"an absent thing answers
    /// empty"*).
    fn editor(&self, buffer: BufferId) -> Option<&Editor>;

    /// The state column for a buffer, indexed by **visual row** — the same
    /// coordinate space [`crate::buffer_view::BufferView::state_column`] takes.
    /// Empty by default, which draws the ground.
    fn state_marks(&self, buffer: BufferId) -> &[StateMark] {
        let _ = buffer;
        &[]
    }

    /// The live completion session, or `None` when there is not one (`T038`).
    ///
    /// **This is the door [`Node::Completion`] reads through, and it has to be
    /// a door rather than a prop**: `view.rs` gives that kind no props at all —
    /// *"there is one active completion session and the store holds it —
    /// composition decides only where it goes"* — so the tree says *whether*
    /// the list is on screen and this says what is in it. Same seam
    /// [`Resources::editor`] already is, and the same reason: a widget crate
    /// cannot read the store.
    ///
    /// A tree that composes the node while this answers `None` draws nothing,
    /// which is `query.rs`'s *"an absent thing answers empty"* — a stale
    /// composition must never be able to break a frame.
    fn completion(&self) -> Option<&CompletionVm> {
        None
    }

    /// The live signature-help or hover answer (`T039`). Same contract as
    /// [`Resources::completion`], one node kind over.
    fn signature(&self) -> Option<&SignatureVm> {
        None
    }
}

/// A host with no buffers — every id resolves to nothing.
///
/// For surfaces that are pure chrome, for the benchmark, and for tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoResources;

impl Resources for NoResources {
    fn editor(&self, _buffer: BufferId) -> Option<&Editor> {
        None
    }
}

/// What one frame's walk found. Diagnostic only — nothing draws from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Nodes visited, dropped ones included. A rough size for the tree the
    /// cache is holding.
    pub nodes: u32,
    /// Wire tags of node kinds whose widget has not landed yet, deduplicated,
    /// in the order first met. See the table in the module docs.
    pub deferred: Vec<&'static str>,
    /// Buffers whose node asked for soft wrap. The host reconciles these when
    /// the tree changes; see [`Resources`].
    pub soft_wrap_requested: Vec<BufferId>,
}

impl Report {
    fn defer(&mut self, tag: &'static str) {
        if !self.deferred.contains(&tag) {
            self.deferred.push(tag);
        }
    }
}

/// Walks a [`Tree`] into a [`Buffer`].
///
/// Built per frame — it borrows the theme and the host's resources and owns
/// nothing. Cheap: three references and a `u64`.
#[derive(Debug, Clone, Copy)]
pub struct Interpreter<'a> {
    theme: &'a Theme,
    resources: &'a dyn Resources,
    now: Millis,
}

impl<'a> Interpreter<'a> {
    /// An interpreter over `theme`, resolving buffers through `resources`.
    ///
    /// The clock starts at zero; a host that draws a spinner or an elapsed
    /// counter sets it with [`at`](Interpreter::at) every frame.
    #[must_use]
    pub const fn new(theme: &'a Theme, resources: &'a dyn Resources) -> Self {
        Self {
            theme,
            resources,
            now: Millis(0),
        }
    }

    /// This frame's reading of the host's monotonic clock.
    ///
    /// The whole animation budget: [`Node::Spinner`] and [`Node::Elapsed`]
    /// render `now - since`, so 12 spinner frames a second cost twelve *frames*
    /// and zero recompositions.
    #[must_use]
    pub const fn at(mut self, now: Millis) -> Self {
        self.now = now;
        self
    }

    /// Draw `tree` into `area`.
    ///
    /// Everything is clipped to `area` and to the buffer; a tree that asks for
    /// more room than exists draws a prefix of itself rather than panicking.
    pub fn render(&self, tree: &Tree, area: Rect, buf: &mut Buffer) -> Report {
        let ctx = Ctx {
            interp: *self,
            report: RefCell::new(Report::default()),
        };
        let area = area.intersection(buf.area);
        if !area.is_empty() {
            ctx.node(&tree.root, area, buf);
            if let Some(float) = tree.float.as_ref() {
                ctx.float(float, area, buf);
            }
        }
        ctx.report.into_inner()
    }
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// One frame's walk: the interpreter plus the report it is filling in.
///
/// The [`RefCell`] exists because [`FloatBody::render`] takes `&self` — the
/// float chrome owns the body's area and calls back into the walk. Borrows are
/// taken at leaves only and never held across a recursive call.
#[derive(Debug)]
struct Ctx<'a> {
    interp: Interpreter<'a>,
    report: RefCell<Report>,
}

impl Ctx<'_> {
    fn theme(&self) -> &Theme {
        self.interp.theme
    }

    fn defer(&self, tag: &'static str) {
        self.report.borrow_mut().defer(tag);
    }

    /// Milliseconds since a mark, saturating — a mark in the future reads as
    /// "just now" rather than wrapping.
    fn since(&self, mark: Millis) -> u64 {
        self.interp.now.0.saturating_sub(mark.0)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one arm per node kind; the enum is exhaustive by design (view.rs: \
                  'a node kind that reaches it with no arm is a hole in the frame'), \
                  and splitting the match into groups would hide that from the compiler"
    )]
    fn node(&self, node: &Node, area: Rect, buf: &mut Buffer) {
        self.report.borrow_mut().nodes += 1;
        let area = area.intersection(buf.area);
        if area.is_empty() {
            return;
        }
        let theme = self.theme();

        match node {
            // -- containers -------------------------------------------------
            Node::Empty {} | Node::Spring {} | Node::Spacer { .. } => {}

            Node::Split { axis, slots } => {
                let direction = match axis {
                    Axis::Rows => Direction::Vertical,
                    Axis::Columns => Direction::Horizontal,
                };
                let constraints: Vec<UiConstraint> = slots
                    .iter()
                    .map(|slot| constraint(slot.constraint))
                    .collect();
                let areas = Layout::new(direction, constraints).split(area);
                for (slot, sub) in slots.iter().zip(areas.iter()) {
                    self.node(slot.child.node(), *sub, buf);
                }
            }

            Node::Line { children } => self.line(children, area, buf),

            // A shed wrapper outside a line has nothing to shed against, so it
            // draws its child at full size. `line` unwraps it before this.
            Node::Shed { child, .. } => self.node(child.node(), area, buf),

            // §9: panes never dim each other, and focus is drawn by the tab bar
            // rather than by the pane. So a pane is its contents.
            Node::Pane { child, .. } => self.node(child.node(), area, buf),

            // -- chrome ------------------------------------------------------
            Node::ModeChip { label, tone } => {
                // §5: the only inverted text on screen. Ground on an actor field.
                let style = Style::new()
                    .fg(theme.chrome.mode_chip_fg)
                    .bg(self.colour(*tone));
                write(buf, area, area.x, &format!(" {label} "), style);
            }

            Node::FileLabel { path, dirty } => {
                let text = Style::new().fg(theme.neutrals.text);
                let mut x = write(buf, area, area.x, &path.to_string_lossy(), text);
                if *dirty {
                    x = write(buf, area, x, " ", text);
                    write(buf, area, x, "[+]", Style::new().fg(theme.actors.attention));
                }
            }

            Node::Session {
                state,
                since,
                prose,
            } => {
                let session = self.session(*state, *since);
                let Some(glyph) = session.glyph() else { return };
                let style = Style::new().fg(session.colour(theme));
                let text = match (prose, session.prose()) {
                    (true, Some(words)) => format!("{glyph} {words}"),
                    _ => glyph.to_owned(),
                };
                write(buf, area, area.x, &text, style);
            }

            Node::Counter {
                glyph,
                count,
                label,
                tone,
            } => {
                if *count == 0 {
                    return;
                }
                let style = Style::new().fg(self.colour(*tone));
                let text = match label {
                    // `6 unseen` at width …
                    Some(word) => format!("{count} {word}"),
                    // … `●6` once the counters have shed their words.
                    None => format!("{}{count}", glyph_str(*glyph)),
                };
                write(buf, area, area.x, &text, style);
            }

            // §5, as amended at CP-1: structure, not decoration.
            Node::Divider {} => {
                write(buf, area, area.x, "│", Style::new().fg(theme.neutrals.meta));
            }

            // -- text --------------------------------------------------------
            Node::Label {
                text,
                tone,
                emphasis,
            } => {
                write(buf, area, area.x, text, self.style(*tone, *emphasis));
            }

            Node::Glyph { glyph, tone } => {
                write(
                    buf,
                    area,
                    area.x,
                    glyph_str(*glyph),
                    Style::new().fg(self.colour(*tone)),
                );
            }

            Node::Spans { rows } => self.spans(rows, area, buf),

            // -- time-derived --------------------------------------------------
            //
            // Both of these are why the clock is a parameter: they re-render
            // every frame from a cached tree and bump no revision.
            Node::Spinner { since } => {
                let frame = self.since(*since) / SPINNER_PERIOD_MS;
                let index = u8::try_from(frame % Spinner::FRAMES.len() as u64).unwrap_or(0);
                write(
                    buf,
                    area,
                    area.x,
                    Spinner(index).glyph(),
                    Style::new().fg(theme.actors.transient),
                );
            }

            Node::Elapsed { since } => {
                let text = format_elapsed(Duration::from_millis(self.since(*since)));
                write(
                    buf,
                    area,
                    area.x,
                    &text,
                    Style::new().fg(theme.neutrals.meta),
                );
            }

            // -- buffer surfaces ----------------------------------------------
            Node::Buffer { buffer, soft_wrap } => {
                if *soft_wrap {
                    let mut report = self.report.borrow_mut();
                    if !report.soft_wrap_requested.contains(buffer) {
                        report.soft_wrap_requested.push(*buffer);
                    }
                }
                let Some(editor) = self.interp.resources.editor(*buffer) else {
                    return;
                };
                BufferView::new(editor, theme)
                    .state_column(self.interp.resources.state_marks(*buffer))
                    .render(area, buf);
            }

            // -- the five Window D kinds, one arm each -------------------------
            //
            // The split bought five separate places for five widget tasks to
            // land instead of one shared arm they would collide in. Three have
            // landed and draw; `completion` and `signature` still defer, and
            // whoever builds one replaces its own line here and deletes its row
            // from the module docs' table — which is a test, so it will say so.

            // `T031`, drawn by `crate::gutter`. The marks arrive already
            // resolved, through the same `Resources` door `Node::Buffer` uses;
            // the editor is asked for nothing but its viewport, so a host that
            // has no editor behind the id still gets a column, from the top.
            // §8's degraded form is not reachable from here — the tree carries
            // no terminal capability, and adding a prop is `spine`'s call.
            Node::Gutter { buffer } => {
                let top_row = self
                    .interp
                    .resources
                    .editor(*buffer)
                    .map_or(0, |editor| crate::buffer_view::viewport_of(editor).top_row);
                crate::gutter::GutterBar::new(self.interp.resources.state_marks(*buffer), theme)
                    .top_row(top_row)
                    .render(area, buf);
            }

            // `T032`, drawn by `crate::virtual_text`. The node carries an
            // owner and its content and **no anchor**, so this is the
            // standalone half of the primitive: a `┊` row in whatever area
            // composition gave it. The half that hangs a row off a line lives
            // in the buffer's own row stream (`virtual_text::install`), which
            // is where `T081` legislated it has to be — a `Node::Buffer` draws
            // those, and it draws the same rail.
            Node::VirtualText { content, .. } => {
                // Local to this arm on purpose: `interpret.rs` is `spine`'s
                // file and `T032` owns exactly this arm of it. Flattens the
                // content the way `footer_hints` flattens a footer — the tree
                // says its colours in tones, and this is where they resolve.
                fn runs(ctx: &Ctx<'_>, node: &Node, out: &mut Vec<crate::virtual_text::Run>) {
                    match node {
                        Node::Label {
                            text,
                            tone,
                            emphasis,
                        } => out.push(crate::virtual_text::Run::new(
                            text.clone(),
                            ctx.style(*tone, *emphasis),
                        )),
                        Node::Glyph { glyph, tone } => out.push(crate::virtual_text::Run::new(
                            glyph_str(*glyph),
                            Style::new().fg(ctx.colour(*tone)),
                        )),
                        Node::Spacer { cells } => out.push(crate::virtual_text::Run::new(
                            " ".repeat(cells_to_u16(*cells) as usize),
                            Style::new(),
                        )),
                        Node::Line { children } => {
                            for child in children {
                                runs(ctx, child.node(), out);
                            }
                        }
                        // A `spans` row is already runs; several of them
                        // flatten onto the one row a virtual row is.
                        Node::Spans { rows } => {
                            for row in rows {
                                for run in &row.runs {
                                    out.push(crate::virtual_text::Run::new(
                                        run.text.clone(),
                                        ctx.style(run.tone, run.emphasis),
                                    ));
                                }
                            }
                        }
                        Node::Empty {} | Node::Spring {} => {}
                        other => ctx.defer(other.tag()),
                    }
                }
                let mut out = Vec::new();
                runs(self, content.node(), &mut out);
                crate::virtual_text::VirtualText::new(&out, theme).render(area, buf);
            }

            // `T034` / `T086`, drawn by `crate::key_hints`: one widget, three
            // densities. `Density::Footer` *inside* a float still goes through
            // `footer_hints` and the float's own chrome; this arm is the same
            // hints anywhere else — a leader strip (`3c`), a `:help` body
            // (`6d`), or a footer row in a surface that is not a float.
            Node::KeyHints { density, hints } => {
                crate::key_hints::KeyHints::new(hints, *density, theme).render(area, buf);
            }

            // `T038`, drawn by `crate::float::CompletionList` — the passive
            // mood, not a module of its own. The node carries nothing, so the
            // session comes off `Resources`; an absent one draws nothing rather
            // than reserving a rectangle.
            //
            // The float chrome around it is `Ctx::float`'s, and the two meet
            // only through `Mood::Passive` — a completion list composed *not*
            // in a float still draws here, as a bare list, which is what a
            // `Node` promises anywhere it is put.
            Node::Completion {} => {
                let Some(vm) = self.interp.resources.completion() else {
                    return;
                };
                CompletionList::new(vm).render(area, buf, theme, UiMood::Passive);
            }

            // `T039`, drawn by `crate::float::SignatureBody` alongside the
            // completion list. Signature help and hover are one body; see it.
            Node::Signature {} => {
                let Some(vm) = self.interp.resources.signature() else {
                    return;
                };
                SignatureBody::new(vm).render(area, buf, theme, UiMood::Passive);
            }

            // Deferred past Window D. Grouped, and split one kind at a time the
            // way the five above were, as each phase arrives.
            Node::TabBar { .. }
            | Node::Picker { .. }
            | Node::Diff { .. }
            | Node::Question { .. }
            | Node::Transcript { .. }
            | Node::Prompt { .. }
            | Node::Watch { .. } => self.defer(node.tag()),
        }
    }

    /// One row of nodes at their natural widths, left to right (`Node::Line`).
    ///
    /// **Never wraps** — a second row is a bug (§5), so when the row does not
    /// fit, the ladder gives things up one rung at a time until it does, and
    /// whatever is left over is clipped at the right edge.
    ///
    /// # The ladder is rungs, not items
    ///
    /// Each [`Node::Shed`] wrapper is **one rung**: it either contracts its
    /// child to a narrower form or drops it, and `priority` is that rung's place
    /// in the order — `view.rs`'s *"ascending priority is the order of the
    /// ladder"*, read as written. A wrapper around a wrapper is therefore two
    /// rungs on one segment, which is how `8d`'s file steps are said: rung 6
    /// contracts `src/retry.rs` to `retry.rs`, rung 8 drops it altogether.
    ///
    /// The consequence worth stating: **a rung that contracts does not also
    /// drop.** That is what makes §11's last-standing set — `✻` / `●n` / `!` —
    /// expressible, since those segments have to contract (`6 unseen` → `●6`)
    /// and must never go. A composition that wants both wraps twice.
    fn line(&self, children: &[Child], area: Rect, buf: &mut Buffer) {
        let (mut items, mut rungs) = unwrap(children);

        // One rung at a time, lowest first, until it fits or the ladder runs
        // out. Fit-driven: nothing is given up while there is room for it (§11
        // as `CP-1` settled it).
        while self.natural_total(&items) > area.width {
            let Some(next) = rungs
                .iter_mut()
                .filter(|rung| !rung.taken && !items[rung.item].dropped)
                .min_by_key(|rung| rung.priority)
            else {
                break;
            };
            next.taken = true;
            match next.contracted {
                Some(form) => items[next.item].form = form,
                None => items[next.item].dropped = true,
            }
        }

        // Whatever is left over goes to the flexible children — springs, and
        // anything with no natural width of its own.
        let fixed = self.natural_total(&items);
        let flexible = items
            .iter()
            .filter(|item| !item.dropped && self.natural(item.node()).is_none())
            .count();
        let slack = area.width.saturating_sub(fixed);
        let (share, mut extra) = if flexible == 0 {
            (0, 0)
        } else {
            let flexible = u16::try_from(flexible).unwrap_or(u16::MAX);
            (slack / flexible, slack % flexible)
        };

        let mut x = area.x;
        for item in &items {
            if item.dropped {
                continue;
            }
            let node = item.node();
            let width = match self.natural(node) {
                Some(natural) => natural,
                None => {
                    // Spread the remainder one cell at a time, leftmost first,
                    // so two springs never differ by more than one column.
                    let bonus = u16::from(extra > 0);
                    extra = extra.saturating_sub(bonus);
                    share + bonus
                }
            };
            let width = width.min(area.right().saturating_sub(x));
            if width > 0 {
                self.node(node, Rect { x, width, ..area }, buf);
            }
            x = x.saturating_add(width);
            if x >= area.right() {
                break;
            }
        }
    }

    /// The `spans` escape hatch (`T080`): styled rows straight from
    /// composition.
    fn spans(&self, rows: &[SpanRow], area: Rect, buf: &mut Buffer) {
        for (dy, row) in rows.iter().enumerate() {
            let Ok(dy) = u16::try_from(dy) else { break };
            if dy >= area.height {
                break;
            }
            let y = area.y + dy;
            let ground = row.tint.map(|tint| self.tint(tint));
            if let Some(bg) = ground {
                for x in area.x..area.right() {
                    buf[(x, y)].set_symbol(" ").set_style(Style::new().bg(bg));
                }
            }
            let mut x = area.x;
            for run in &row.runs {
                let mut style = self.style(run.tone, run.emphasis);
                if let Some(bg) = ground {
                    style = style.bg(bg);
                }
                x = write(
                    buf,
                    Rect {
                        y,
                        height: 1,
                        ..area
                    },
                    x,
                    &run.text,
                    style,
                );
                if x >= area.right() {
                    break;
                }
            }
        }
    }

    /// The one float, over a dimmed ground (§9) — except the passive one, which
    /// dims nothing (`Mood::dims`).
    fn float(&self, float: &ViewFloat, area: Rect, buf: &mut Buffer) {
        self.report.borrow_mut().nodes += 1;
        let mood = mood(float.mood);

        let node = float.body.node();
        let body = NodeBody { ctx: self, node };
        // **Header and footer are built only for the moods that draw them.** A
        // passive float has neither (§4's exception, and `Mood::Passive` for
        // the header), so building them first walked the footer subtree once
        // per frame for something the next line dropped.
        let hints = if matches!(mood, UiMood::Passive) {
            Vec::new()
        } else {
            float
                .footer
                .as_ref()
                .map(|footer| self.footer_hints(footer.node()))
                .unwrap_or_default()
        };
        let footer = FloatFooter::new(&hints);
        let ui = match mood {
            UiMood::Informational => UiFloat::informational(self.header(float), &body, footer),
            UiMood::NeedsYou => UiFloat::needs_you(self.header(float), &body, footer),
            // **The anchor rides on the ViewModel, not on the tree** (`T038`).
            // `view::Float` has no anchor prop and the cell is not the view
            // tree's to know: it is where the *cursor* is, which the host
            // computes to draw the buffer and recomposes on every keystroke.
            // Putting it in the tree would recompose the frame once per typed
            // character for a number the host already has. Raised for `spine`
            // rather than assumed — a `Float::anchor` prop is a protocol change.
            //
            // A header or a footer composed onto a passive float is dropped
            // here and nowhere reports it. `Report::deferred` is a list of node
            // kinds with no widget and this is not one, so the channel does not
            // exist yet — flagged for `spine` rather than bent.
            // **And held to the widest the session has been** (`CP-4`). Same
            // dispatch as the anchor and for the same reason: the floor belongs
            // to the session the body is drawing, not to whichever
            // `Resources` door answers first.
            UiMood::Passive => UiFloat::passive(&body, self.passive_anchor(node, area))
                .with_width_floor(self.passive_width_floor(node)),
        };
        FloatSlot::with(ui).render(area, buf, self.theme());
    }

    /// A float's header, empty when the composition set none.
    fn header<'n>(&self, float: &'n ViewFloat) -> FloatHeader<'n> {
        float.header.as_ref().map_or_else(
            || FloatHeader::new(""),
            |header| FloatHeader {
                left: &header.left,
                right: header.right.as_deref(),
            },
        )
    }

    /// Where a passive float hangs: **the anchor belonging to the body being
    /// drawn**, or the area's own corner when there is no session behind it.
    ///
    /// Chosen by the node rather than by whichever [`Resources`] door answers
    /// first. §9 allows one *float*; it says nothing about sessions, and
    /// nothing in [`Resources`] stops a host holding a completion session while
    /// it composes a signature float — which used to put the signature at the
    /// completion's anchor, two surfaces apart from the word it belongs to.
    fn passive_anchor(&self, node: &Node, area: Rect) -> Anchor {
        let resources = self.interp.resources;
        let anchor = match node {
            Node::Completion {} => resources.completion().map(|vm| vm.anchor),
            Node::Signature {} => resources.signature().map(|vm| vm.anchor),
            _ => None,
        };
        anchor.unwrap_or(Anchor::new(area.x, area.y))
    }

    /// The body columns the session says it has already taken —
    /// [`UiFloat::with_width_floor`], the anti-thrash half of `CP-4`.
    ///
    /// Zero for a node with no session behind it, which is content-sizing: an
    /// absent thing answers empty, the same rule [`Self::passive_anchor`] takes.
    fn passive_width_floor(&self, node: &Node) -> u16 {
        let resources = self.interp.resources;
        match node {
            Node::Completion {} => resources.completion().map_or(0, |vm| vm.width_floor),
            Node::Signature {} => resources.signature().map_or(0, |vm| vm.width_floor),
            _ => 0,
        }
    }

    /// A float footer's hints.
    ///
    /// [`Node::KeyHints`] at [`Density::Footer`] is the shape §4 asks for and
    /// the shape [`FloatFooter`] takes, so it maps straight across — that arm
    /// is `T034`'s widget arriving early, because the float footer is already
    /// built. Anything else in the footer slot is flattened to bare keys, which
    /// is what a [`Node::Line`] of labels reads as.
    fn footer_hints<'n>(&self, node: &'n Node) -> Vec<FooterHint<'n>> {
        match node {
            Node::KeyHints {
                density: Density::Footer,
                hints,
            } => hints
                .iter()
                .map(|hint| FooterHint::new(&hint.key.0, &hint.verb))
                .collect(),
            Node::Line { children } => children
                .iter()
                .flat_map(|child| self.footer_hints(child.node()))
                .collect(),
            Node::Label { text, .. } => vec![FooterHint::bare(text)],
            Node::Glyph { glyph, .. } => vec![FooterHint::bare(glyph_str(*glyph))],
            Node::Empty {} => Vec::new(),
            other => {
                self.defer(other.tag());
                Vec::new()
            }
        }
    }

    // -- measurement --------------------------------------------------------

    /// A node's natural width in cells, or `None` if it is flexible.
    ///
    /// Flexible means "share whatever the line has left": a spring, a split, a
    /// buffer, and every primitive that has not landed yet. A line of nothing
    /// but flexible children divides evenly, which is the sane degenerate case.
    fn natural(&self, node: &Node) -> Option<u16> {
        let width = match node {
            Node::Empty {} => 0,
            Node::Spacer { cells } => cells_to_u16(*cells),
            Node::Divider {} => 1,
            Node::Glyph { glyph, .. } => cells(glyph_str(*glyph)),
            Node::Label { text, .. } => cells(text),
            Node::ModeChip { label, .. } => cells(label).saturating_add(2),
            Node::FileLabel { path, dirty } => {
                cells(&path.to_string_lossy()).saturating_add(if *dirty { 4 } else { 0 })
            }
            Node::Session {
                state,
                since,
                prose,
            } => {
                let session = self.session(*state, *since);
                match (session.glyph(), prose.then(|| session.prose()).flatten()) {
                    (None, _) => 0,
                    (Some(glyph), None) => cells(glyph),
                    (Some(glyph), Some(words)) => cells(glyph) + 1 + cells(&words),
                }
            }
            Node::Counter {
                glyph,
                count,
                label,
                ..
            } => match (count, label) {
                (0, _) => 0,
                (n, Some(word)) => cells(&format!("{n} {word}")),
                (n, None) => cells(&format!("{}{n}", glyph_str(*glyph))),
            },
            Node::Spinner { .. } => 1,
            Node::Elapsed { since } => {
                cells(&format_elapsed(Duration::from_millis(self.since(*since))))
            }
            Node::Spans { rows } => rows
                .iter()
                .map(|row| row.runs.iter().map(|run| cells(&run.text)).sum::<u16>())
                .max()
                .unwrap_or(0),
            Node::Line { children } => {
                let mut total = 0u16;
                for child in children {
                    total = total.saturating_add(self.natural(child.node())?);
                }
                total
            }
            Node::Shed { child, .. } => self.natural(child.node())?,
            Node::Pane { child, .. } => self.natural(child.node())?,
            _ => return None,
        };
        Some(width)
    }

    /// Rows a node wants — what a float asks its body (§8: *"no surface is ever
    /// taller than its content"*).
    ///
    /// A primitive that draws nothing wants nothing, so a float over an unbuilt
    /// body collapses to its chrome rather than reserving a blank rectangle.
    ///
    /// **Takes no width**, unlike [`crate::float::FloatBody::desired_height`],
    /// and that is a fact about the protocol rather than an omission: §11 is
    /// *"nothing ever wraps"*, so no node's row count depends on how wide it is.
    /// The first body that reflows takes the width back as a parameter, and it
    /// will be visible in the diff that it did.
    fn height(&self, node: &Node) -> u16 {
        match node {
            Node::Empty {} | Node::Spring {} | Node::Spacer { .. } => 0,
            Node::Spans { rows } => u16::try_from(rows.len()).unwrap_or(u16::MAX),
            Node::Split { axis, slots } => slots
                .iter()
                .map(|slot| self.height(slot.child.node()))
                .fold(0u16, |acc, h| match axis {
                    Axis::Rows => acc.saturating_add(h),
                    Axis::Columns => acc.max(h),
                }),
            Node::Shed { child, .. } | Node::Pane { child, .. } => self.height(child.node()),
            Node::Line { .. }
            | Node::ModeChip { .. }
            | Node::FileLabel { .. }
            | Node::Session { .. }
            | Node::Counter { .. }
            | Node::Divider {}
            | Node::Label { .. }
            | Node::Glyph { .. }
            | Node::Spinner { .. }
            | Node::Elapsed { .. } => 1,
            // A buffer in a float body is the whole area it is given; the float
            // clamps this to the screen.
            Node::Buffer { .. } => u16::MAX,
            // `T086`: the `:help` body is a keymap surface, so a float over one
            // has to be able to ask how tall it is or it collapses to chrome.
            // `natural_height` is the width-free answer this signature can take
            // — exact at `Density::Help` and `Density::Footer`, whose row counts
            // do not depend on width, and the widest packing at `Density::Grid`,
            // which is composed into a sized slot rather than a float body
            // (`key_hints.rs` records the seam).
            Node::KeyHints { density, hints } => {
                crate::key_hints::KeyHints::new(hints, *density, self.theme()).natural_height()
            }
            // `T038` / `T039`: the passive float is sized to its list, so a
            // body that could not answer would collapse the float to chrome —
            // the same failure `T086` fixed one kind up.
            Node::Completion {} => self
                .interp
                .resources
                .completion()
                .map_or(0, |vm| CompletionList::new(vm).desired_height(0)),
            Node::Signature {} => self
                .interp
                .resources
                .signature()
                .map_or(0, |vm| SignatureBody::new(vm).desired_height(0)),
            _ => 0,
        }
    }

    /// Columns a node wants, for the one layout that asks —
    /// [`crate::float::Layout::Anchored`] (`T038`).
    ///
    /// Reuses [`Ctx::natural`] wherever a node has a natural width, which is
    /// every leaf. `0` for the flexible ones — a split, a spring, a buffer —
    /// and an anchored float whose body wants no columns draws nothing at all
    /// ([`crate::float::Float::frame`]), which is the right answer for a
    /// composition that anchored a pane beside the cursor.
    fn width(&self, node: &Node) -> u16 {
        match node {
            Node::Completion {} => self
                .interp
                .resources
                .completion()
                .map_or(0, |vm| CompletionList::new(vm).desired_width()),
            Node::Signature {} => self
                .interp
                .resources
                .signature()
                .map_or(0, |vm| SignatureBody::new(vm).desired_width()),
            other => self.natural(other).unwrap_or(0),
        }
    }

    // -- style --------------------------------------------------------------

    /// A tone resolved against the theme. **The only route from the protocol to
    /// a colour** — there are no RGB values in the tree and there can never be
    /// one (`view/props.rs`, `scripts/lint-no-literal-colours.sh`).
    fn colour(&self, tone: Tone) -> Color {
        let theme = self.theme();
        match tone {
            Tone::Claude => theme.actors.claude,
            Tone::You => theme.actors.you,
            Tone::Attention => theme.actors.attention,
            Tone::Trouble => theme.actors.trouble,
            Tone::Transient => theme.actors.transient,
            Tone::Steel => theme.actors.steel,
            Tone::Text => theme.neutrals.text,
            Tone::Prose => theme.neutrals.prose,
            Tone::Meta => theme.neutrals.meta,
            Tone::LineNumber => theme.neutrals.line_numbers,
            Tone::Ground => theme.neutrals.ground,
            Tone::BrightText => theme.neutrals.bright_text,
            Tone::Dimmed => theme.neutrals.dimmed_under_float,
        }
    }

    /// §3's three row tints.
    fn tint(&self, tint: Tint) -> Color {
        let theme = self.theme();
        match tint {
            Tint::Anchor => theme.regions.anchor,
            Tint::Selection => theme.regions.selection,
            Tint::Failure => theme.regions.failure,
        }
    }

    /// A tone and an emphasis, as a style.
    ///
    /// [`Emphasis::Undercurl`] degrades to a plain underline here: real
    /// undercurl is a cell-style capability the vendored editor core does not
    /// have yet (`T085`), and `vendor/` is not this crate's to change.
    fn style(&self, tone: Tone, emphasis: Emphasis) -> Style {
        let base = Style::new().fg(self.colour(tone));
        match emphasis {
            Emphasis::Plain => base,
            Emphasis::Inverted => base.add_modifier(Modifier::REVERSED),
            Emphasis::Underline | Emphasis::Undercurl => base.add_modifier(Modifier::UNDERLINED),
        }
    }

    /// The protocol's session state, rendered through the widget layer's.
    ///
    /// Two enums, one meaning — the seam `view/props.rs` flags and does not
    /// fold in. This function is where they meet, and it is the only place that
    /// should ever know both names.
    fn session(&self, state: ViewSessionState, since: Option<Millis>) -> UiSessionState {
        match state {
            ViewSessionState::None => UiSessionState::None,
            ViewSessionState::Idle => UiSessionState::Idle,
            ViewSessionState::Working => {
                let elapsed = since.map(|mark| Duration::from_millis(self.since(mark)));
                let frame = since.map_or(0, |mark| self.since(mark) / SPINNER_PERIOD_MS);
                let index = u8::try_from(frame % Spinner::FRAMES.len() as u64).unwrap_or(0);
                UiSessionState::Working {
                    elapsed,
                    spinner: Spinner(index),
                }
            }
            ViewSessionState::Waiting => UiSessionState::Waiting,
            ViewSessionState::Paused => UiSessionState::Paused,
            ViewSessionState::Lost => UiSessionState::Lost,
        }
    }
}

// ---------------------------------------------------------------------------
// The float body
// ---------------------------------------------------------------------------

/// A view-tree node plugged into [`crate::float::Float`]'s body slot.
///
/// This is what makes `T084`'s chrome primitive reachable from composition
/// without the float knowing anything about the tree: the float owns the
/// geometry, the border and the padding; the body is one recursion back into
/// the walk.
struct NodeBody<'a, 'n> {
    ctx: &'a Ctx<'a>,
    node: &'n Node,
}

impl core::fmt::Debug for NodeBody<'_, '_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NodeBody")
            .field("node", &self.node.tag())
            .finish()
    }
}

impl FloatBody for NodeBody<'_, '_> {
    fn desired_height(&self, _width: u16) -> u16 {
        // Nothing in the tree reflows, so the width does not enter into it —
        // see `Ctx::height`.
        self.ctx.height(self.node)
    }

    fn desired_width(&self) -> u16 {
        self.ctx.width(self.node)
    }

    fn render(&self, area: Rect, buf: &mut Buffer, _theme: &Theme, _mood: UiMood) {
        // The theme and the mood arrive from the float; the walk already holds
        // the same theme, and the tree names its own tones.
        self.ctx.node(self.node, area, buf);
    }
}

// ---------------------------------------------------------------------------
// Line items
// ---------------------------------------------------------------------------

/// One child of a line, in whatever form the ladder has left it in.
#[derive(Debug)]
struct Item<'n> {
    /// What to draw right now — the innermost form, or whatever rung last
    /// contracted it.
    form: &'n Node,
    dropped: bool,
}

impl<'n> Item<'n> {
    /// The form this item is currently in.
    const fn node(&self) -> &'n Node {
        self.form
    }
}

/// One step of the ladder: give this item up, this way, at this priority.
#[derive(Debug)]
struct Rung<'n> {
    /// Lower goes first. `view.rs`: *"ascending priority is the order of the
    /// ladder"*.
    priority: u32,
    /// The narrower form to fall back to, or [`None`] to drop the item.
    contracted: Option<&'n Node>,
    /// Which item it governs, as an index into the line's items.
    item: usize,
    taken: bool,
}

/// A line's children as items, and every rung that can give one of them up.
///
/// [`Node::Shed`] is the only node that is *about* the line rather than *in*
/// it, so it is unwrapped here and never drawn: a chain of them is a chain of
/// rungs on one item, outermost first, and the innermost non-shed node is the
/// item's full form.
fn unwrap<'n>(children: &'n [Child]) -> (Vec<Item<'n>>, Vec<Rung<'n>>) {
    let mut items = Vec::with_capacity(children.len());
    let mut rungs = Vec::new();

    for child in children {
        let index = items.len();
        let mut node = child.node();
        while let Node::Shed {
            priority,
            contracted,
            child,
        } = node
        {
            rungs.push(Rung {
                priority: *priority,
                contracted: contracted.as_ref().map(Child::node),
                item: index,
                taken: false,
            });
            node = child.node();
        }
        items.push(Item {
            form: node,
            dropped: false,
        });
    }

    (items, rungs)
}

impl Ctx<'_> {
    /// Cells the fixed-width items need. Flexible ones contribute nothing —
    /// they live on the slack.
    fn natural_total(&self, items: &[Item<'_>]) -> u16 {
        items
            .iter()
            .filter(|item| !item.dropped)
            .filter_map(|item| self.natural(item.node()))
            .fold(0u16, u16::saturating_add)
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// The protocol's constraint, as ratatui's.
///
/// `view::Constraint` is deliberately the same five shapes named independently,
/// because `phosphor-core` may not depend on ratatui (Q12) — this is the seam
/// that mirror exists for, and the saturating narrowing `props.rs` promised
/// would happen here.
fn constraint(constraint: Constraint) -> UiConstraint {
    match constraint {
        Constraint::Cells { cells } => UiConstraint::Length(cells_to_u16(cells)),
        Constraint::Min { cells } => UiConstraint::Min(cells_to_u16(cells)),
        Constraint::Max { cells } => UiConstraint::Max(cells_to_u16(cells)),
        Constraint::Percent { percent } => UiConstraint::Percentage(cells_to_u16(percent)),
        Constraint::Fill { weight } => UiConstraint::Fill(cells_to_u16(weight)),
    }
}

/// The protocol's mood, as the chrome's. Total since `T038` built the third.
const fn mood(mood: ViewMood) -> UiMood {
    match mood {
        ViewMood::Informational => UiMood::Informational,
        ViewMood::NeedsYou => UiMood::NeedsYou,
        ViewMood::Passive => UiMood::Passive,
    }
}

/// Design Language §2's lexicon — *one cell, one concept*.
///
/// Transcribed from `view/props.rs`'s own doc comments, which are §2. The
/// degradations §8 names (`▎` for the state bar, a static `✻` for the spinner)
/// belong to the widgets that need them, not here.
const fn glyph_str(glyph: Glyph) -> &'static str {
    match glyph {
        Glyph::Claude => "✻",
        Glyph::Working => "⠸",
        Glyph::NeedsYou => "!",
        Glyph::Paused => "⏸",
        Glyph::ChangedOnDisk => "✱",
        Glyph::SessionLost => "✕",
        Glyph::Diagnostic => "■",
        Glyph::Unseen => "●",
        Glyph::Anchor => "⚓",
        Glyph::VirtualRail => "┊",
        Glyph::Watch => "◉",
        Glyph::ValueStream => "⇒",
        Glyph::FoldClosed => "▸",
        Glyph::FoldOpen => "▾",
        Glyph::Elided => "⋯",
        Glyph::SteelPrompt => "λ",
        Glyph::SteelSurface => "◆",
        Glyph::Prompt => "❯",
        Glyph::Check => "✓",
        Glyph::WrapContinuation => "↪",
    }
}

/// Display width in cells — grapheme- and East-Asian-aware, the same
/// measurement [`Buffer::set_stringn`] writes with.
///
/// **Crate-wide, because it was written three times.** `float` and `key_hints`
/// each carried a byte-identical private copy; two lines is not a seam, and
/// three copies of two lines is not a seam either — it is three places for the
/// measurement a widget lays out with to drift from the one it draws with.
pub(crate) fn cells(text: &str) -> u16 {
    u16::try_from(Span::raw(text).width()).unwrap_or(u16::MAX)
}

/// The protocol counts cells in `u32`; a terminal coordinate is `u16`.
/// Saturating, once, at the seam (`view/props.rs`).
fn cells_to_u16(cells: u32) -> u16 {
    u16::try_from(cells).unwrap_or(u16::MAX)
}

/// Write `text` at `x` on `area`'s first row, clipped to the area and to the
/// buffer. Returns the column after the last cell written.
fn write(buf: &mut Buffer, area: Rect, x: u16, text: &str, style: Style) -> u16 {
    if area.is_empty() || x >= area.right() {
        return x;
    }
    let room = area.right() - x;
    let (next, _) = buf.set_stringn(x, area.y, text, room as usize, style);
    next.min(area.right())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{Interpreter, NoResources, Report};
    use crate::float::{Anchor, CompletionVm, SignatureVm};
    use crate::frame::FrameCache;
    use crate::theme::Theme;
    use phosphor_core::query::Revision;
    use phosphor_core::request::{
        AskId, BufferId, DiffMode, Grouping, PaneId, PaneKind, PromptKind, SourceId, WatchId,
    };
    use phosphor_core::view::{
        Axis, Child, Constraint, Density, DiffSource, Emphasis, Float, FloatHeader, Glyph, Millis,
        Mood, Node, Run, SessionState, Slot, SpanRow, Tone, Tree,
    };
    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;

    const AREA: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };

    fn draw_at(tree: &Tree, now: Millis) -> (Buffer, Report) {
        let theme = Theme::phosphor_dark();
        let mut buf = Buffer::empty(AREA);
        let report = Interpreter::new(&theme, &NoResources)
            .at(now)
            .render(tree, AREA, &mut buf);
        (buf, report)
    }

    fn draw(tree: &Tree) -> (Buffer, Report) {
        draw_at(tree, Millis(0))
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (AREA.x..AREA.right())
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim_end()
            .to_owned()
    }

    fn label(text: &str, tone: Tone) -> Node {
        Node::Label {
            text: text.to_owned(),
            tone,
            emphasis: Emphasis::Plain,
        }
    }

    /// A host with an LSP session and no buffers — what `T038` and `T039` need
    /// from [`Resources`], and nothing else.
    #[derive(Debug)]
    struct Session {
        completion: Option<CompletionVm>,
        signature: Option<SignatureVm>,
    }

    impl super::Resources for Session {
        fn editor(&self, _buffer: BufferId) -> Option<&crate::buffer_view::Editor> {
            None
        }

        fn completion(&self) -> Option<&CompletionVm> {
            self.completion.as_ref()
        }

        fn signature(&self) -> Option<&SignatureVm> {
            self.signature.as_ref()
        }
    }

    fn item(label: &str, detail: Option<&str>) -> crate::float::CompletionItemVm {
        crate::float::CompletionItemVm {
            label: label.to_owned(),
            detail: detail.map(str::to_owned),
            ..crate::float::CompletionItemVm::default()
        }
    }

    // -- the cache contract, end to end -------------------------------------

    /// **The half of `T079`'s acceptance the frame cache cannot show on its
    /// own:** the pixels change every frame while the VM stays out of the frame
    /// path. A spinner animates from one cached tree.
    #[test]
    fn an_animation_redraws_from_the_cached_tree() {
        let tree = Tree::new(Node::Spinner { since: Millis(0) });
        let mut cache = FrameCache::new();
        let mut compositions = 0;
        let mut glyphs = Vec::new();

        // Twelve frames across one second of animation; the store never moves.
        for frame in 0..12u64 {
            cache.update(Revision::INITIAL, || {
                compositions += 1;
                tree.clone()
            });
            let (buf, _) = draw_at(cache.tree(), Millis(frame * 100));
            glyphs.push(buf[(0, 0)].symbol().to_owned());
        }

        assert_eq!(compositions, 1, "an animation is not a state change");
        assert_eq!(cache.stats().frames(), 12);
        assert!(
            glyphs
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1,
            "the spinner has to actually move: {glyphs:?}"
        );
    }

    /// The property the cache rests on: same tree, same area, same clock, same
    /// cells. If this ever fails, redrawing from cache is not sound.
    #[test]
    fn the_interpreter_is_a_pure_function_of_the_tree() {
        let tree = Tree::new(Node::split(
            Axis::Rows,
            [
                Slot::new(Constraint::Fill { weight: 1 }, label("body", Tone::Text)),
                Slot::new(
                    Constraint::Cells { cells: 1 },
                    Node::line([
                        Node::ModeChip {
                            label: "NORMAL".to_owned(),
                            tone: Tone::Claude,
                        },
                        Node::Spring {},
                        Node::Counter {
                            glyph: Glyph::Unseen,
                            count: 6,
                            label: Some("unseen".to_owned()),
                            tone: Tone::Meta,
                        },
                    ]),
                ),
            ],
        ));
        let (first, r1) = draw(&tree);
        let (second, r2) = draw(&tree);
        assert_eq!(first, second);
        assert_eq!(r1, r2);
    }

    // -- containers ----------------------------------------------------------

    #[test]
    fn a_split_divides_its_area_by_the_constraints() {
        let tree = Tree::new(Node::split(
            Axis::Rows,
            [
                Slot::new(Constraint::Cells { cells: 1 }, label("top", Tone::Text)),
                Slot::new(Constraint::Fill { weight: 1 }, label("middle", Tone::Text)),
                Slot::new(Constraint::Cells { cells: 1 }, label("bottom", Tone::Text)),
            ],
        ));
        let (buf, _) = draw(&tree);
        assert_eq!(row(&buf, 0), "top");
        assert_eq!(row(&buf, 1), "middle");
        assert_eq!(row(&buf, AREA.height - 1), "bottom");
    }

    #[test]
    fn a_line_places_children_at_their_natural_widths() {
        let tree = Tree::new(Node::line([
            label("ab", Tone::Text),
            Node::Spacer { cells: 3 },
            label("cd", Tone::Text),
        ]));
        let (buf, _) = draw(&tree);
        assert_eq!(row(&buf, 0), "ab   cd");
    }

    #[test]
    fn a_spring_pushes_the_rest_to_the_right() {
        let tree = Tree::new(Node::line([
            label("left", Tone::Text),
            Node::Spring {},
            label("right", Tone::Text),
        ]));
        let (buf, _) = draw(&tree);
        let drawn = row(&buf, 0);
        assert!(drawn.starts_with("left"), "{drawn:?}");
        assert!(drawn.ends_with("right"), "{drawn:?}");
        assert_eq!(drawn.chars().count(), AREA.width as usize);
    }

    #[test]
    fn a_line_never_produces_a_second_row() {
        // Far more content than fits, with nothing marked sheddable.
        let tree = Tree::new(Node::line(
            (0..40).map(|i| label(&format!("segment-{i}"), Tone::Text)),
        ));
        let (buf, _) = draw(&tree);
        for y in 1..AREA.height {
            assert_eq!(row(&buf, y), "", "row {y} was written to");
        }
    }

    // -- shedding ------------------------------------------------------------

    #[test]
    fn a_rung_that_contracts_does_not_also_drop() {
        // A segment's own leading gap rides *inside* its shed wrapper — drop
        // the segment and the gap goes with it. A composition that leaves the
        // separator outside gets a double space when the thing between them
        // sheds, which is a composition bug and not the interpreter's to
        // second-guess.
        let gapped = |node: Node| Node::line([Node::Spacer { cells: 1 }, node]);
        let counters = |width: u16| {
            let tree = Tree::new(Node::line([
                label("NORMAL", Tone::Text),
                Node::Shed {
                    priority: 0,
                    contracted: Some(Child::new(gapped(label("●6", Tone::Meta)))),
                    child: Child::new(gapped(label("6 unseen", Tone::Meta))),
                },
                Node::Shed {
                    priority: 1,
                    contracted: None,
                    child: Child::new(gapped(label("jj ✓", Tone::Meta))),
                },
            ]));
            let theme = Theme::phosphor_dark();
            let area = Rect {
                width,
                height: 1,
                ..AREA
            };
            let mut buf = Buffer::empty(area);
            Interpreter::new(&theme, &NoResources).render(&tree, area, &mut buf);
            (area.x..area.right())
                .map(|x| buf[(x, 0)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        };

        // Everything fits: nothing sheds. Shedding is fit-driven (§11 as CP-1
        // settled it), not width-labelled.
        assert_eq!(counters(40), "NORMAL 6 unseen jj ✓");
        // Rung 0 contracts the counter …
        assert_eq!(counters(18), "NORMAL ●6 jj ✓");
        // … and rung 1 is the *next* step, not a second one on the counter.
        // §11: `●n` is in the last-standing set and `jj ✓` is not, which is
        // exactly the difference between a rung that contracts and one that
        // drops.
        assert_eq!(counters(12), "NORMAL ●6");
        // The ladder is out of rungs; what is left is clipped at the right
        // edge, still on one row.
        assert_eq!(counters(8), "NORMAL ●");
    }

    #[test]
    fn a_wrapper_around_a_wrapper_is_two_rungs_on_one_segment() {
        // `8d`'s file steps: the path contracts to its basename at one rung and
        // the whole file goes at a later one. One segment, two rungs, which is
        // the only way a protocol with one `contracted` slot per wrapper can
        // say it.
        let file = |width: u16| {
            let full = Node::FileLabel {
                path: std::path::PathBuf::from("src/retry.rs"),
                dirty: false,
            };
            let basename = Node::FileLabel {
                path: std::path::PathBuf::from("retry.rs"),
                dirty: false,
            };
            let tree = Tree::new(Node::line([
                label("N", Tone::Text),
                Node::Spacer { cells: 1 },
                Node::Shed {
                    priority: 8,
                    contracted: None,
                    child: Child::new(Node::Shed {
                        priority: 6,
                        contracted: Some(Child::new(basename)),
                        child: Child::new(full),
                    }),
                },
            ]));
            let theme = Theme::phosphor_dark();
            let area = Rect {
                width,
                height: 1,
                ..AREA
            };
            let mut buf = Buffer::empty(area);
            Interpreter::new(&theme, &NoResources).render(&tree, area, &mut buf);
            (area.x..area.right())
                .map(|x| buf[(x, 0)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        };

        assert_eq!(file(20), "N src/retry.rs");
        assert_eq!(file(12), "N retry.rs");
        assert_eq!(file(6), "N");
    }

    #[test]
    fn a_child_with_no_shed_wrapper_never_drops() {
        // §11's last-standing set is exactly "the things nobody wrapped".
        let tree = Tree::new(Node::line([
            Node::Glyph {
                glyph: Glyph::Claude,
                tone: Tone::Claude,
            },
            Node::Shed {
                priority: 0,
                contracted: None,
                child: Child::new(label("claude idle", Tone::Claude)),
            },
        ]));
        let theme = Theme::phosphor_dark();
        let area = Rect {
            width: 3,
            height: 1,
            ..AREA
        };
        let mut buf = Buffer::empty(area);
        Interpreter::new(&theme, &NoResources).render(&tree, area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "✻");
    }

    // -- leaves --------------------------------------------------------------

    #[test]
    fn the_mode_chip_is_the_only_inverted_text() {
        let theme = Theme::phosphor_dark();
        let tree = Tree::new(Node::ModeChip {
            label: "NORMAL".to_owned(),
            tone: Tone::Claude,
        });
        let (buf, _) = draw(&tree);
        assert_eq!(row(&buf, 0), " NORMAL");
        assert_eq!(buf[(0, 0)].bg, theme.actors.claude);
        assert_eq!(buf[(0, 0)].fg, theme.chrome.mode_chip_fg);
    }

    #[test]
    fn a_counter_of_zero_draws_nothing_at_all() {
        let tree = Tree::new(Node::Counter {
            glyph: Glyph::Unseen,
            count: 0,
            label: Some("unseen".to_owned()),
            tone: Tone::Meta,
        });
        let (buf, _) = draw(&tree);
        assert_eq!(row(&buf, 0), "");
    }

    #[test]
    fn a_counter_without_its_word_is_glyph_and_number() {
        let tree = Tree::new(Node::Counter {
            glyph: Glyph::Unseen,
            count: 6,
            label: None,
            tone: Tone::Meta,
        });
        let (buf, _) = draw(&tree);
        assert_eq!(row(&buf, 0), "●6");
    }

    #[test]
    fn session_prose_contracts_to_its_glyph() {
        let with_prose = Tree::new(Node::Session {
            state: SessionState::Idle,
            since: None,
            prose: true,
        });
        let (buf, _) = draw(&with_prose);
        assert_eq!(row(&buf, 0), "✻ claude idle");

        let contracted = Tree::new(Node::Session {
            state: SessionState::Idle,
            since: None,
            prose: false,
        });
        let (buf, _) = draw(&contracted);
        assert_eq!(row(&buf, 0), "✻");
    }

    #[test]
    fn a_working_session_counts_up_without_a_recomposition() {
        let tree = Tree::new(Node::Session {
            state: SessionState::Working,
            since: Some(Millis(0)),
            prose: true,
        });
        // 42 000ms is spinner frame 525, which is `⠴` — the frame index is
        // derived, not stored, which is the whole point.
        let (buf, _) = draw_at(&tree, Millis(42_000));
        assert_eq!(row(&buf, 0), "⠴ claude working · 0:42");
    }

    #[test]
    fn an_elapsed_counter_renders_from_the_frame_clock() {
        let tree = Tree::new(Node::Elapsed { since: Millis(500) });
        let (buf, _) = draw_at(&tree, Millis(3_723_500));
        assert_eq!(row(&buf, 0), "1:02:03");
    }

    #[test]
    fn spans_carry_their_own_tint() {
        let theme = Theme::phosphor_dark();
        let tree = Tree::new(Node::Spans {
            rows: vec![
                SpanRow {
                    runs: vec![Run::new("store", Tone::Steel)],
                    tint: Some(phosphor_core::view::Tint::Selection),
                },
                SpanRow {
                    runs: vec![Run::new("query", Tone::Meta)],
                    tint: None,
                },
            ],
        });
        let (buf, _) = draw(&tree);
        assert_eq!(row(&buf, 0), "store");
        assert_eq!(row(&buf, 1), "query");
        assert_eq!(buf[(0, 0)].bg, theme.regions.selection);
        assert_eq!(buf[(0, 0)].fg, theme.actors.steel);
        assert_eq!(buf[(0, 1)].fg, theme.neutrals.meta);
    }

    // -- floats --------------------------------------------------------------

    #[test]
    fn a_float_draws_through_the_chrome_primitive() {
        let tree = Tree::new(label("code", Tone::Text)).with_float(Float {
            mood: Mood::Informational,
            header: Some(FloatHeader::new("❯ files")),
            body: Child::new(Node::Spans {
                rows: vec![SpanRow {
                    runs: vec![Run::new("src/retry.rs", Tone::Text)],
                    tint: None,
                }],
            }),
            footer: Some(Child::new(Node::KeyHints {
                density: Density::Footer,
                hints: vec![phosphor_core::view::KeyHint {
                    key: phosphor_core::request::KeySeq("↵".to_owned()),
                    verb: "open".to_owned(),
                }],
            })),
        });
        let (buf, report) = draw(&tree);
        let drawn = (0..AREA.height)
            .map(|y| row(&buf, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(drawn.contains("❯ files"), "{drawn}");
        assert!(drawn.contains("src/retry.rs"), "{drawn}");
        assert!(drawn.contains("↵ open"), "{drawn}");
        assert!(report.deferred.is_empty(), "{report:?}");
    }

    /// `T038`'s half of screen `7c`, composed the way the editor composes it:
    /// a passive float around a `completion` node, with the session behind
    /// `Resources` and nothing in the tree naming an item.
    #[test]
    fn a_passive_float_draws_the_completion_session_at_its_anchor() {
        let theme = Theme::phosphor_dark();
        let host = Session {
            completion: Some(CompletionVm {
                items: vec![
                    item("default()", Some("fn() -> RetryPolicy")),
                    item("default_delay", Some("Duration")),
                ],
                selected: 0,
                documentation: vec!["Returns the policy with 3 attempts.".to_owned()],
                anchor: Anchor::new(20, 4),
                width_floor: 0,
            }),
            signature: None,
        };
        let tree =
            Tree::new(Node::Empty {}).with_float(Float::new(Mood::Passive, Node::Completion {}));
        let mut buf = Buffer::empty(AREA);
        let report = Interpreter::new(&theme, &host).render(&tree, AREA, &mut buf);

        assert!(report.deferred.is_empty(), "{report:?}");
        // Anchored: below the anchor row, starting at the anchor column.
        assert_eq!(buf[(20, 5)].symbol(), "┌");
        assert_eq!(buf[(20, 5)].fg, theme.float.passive);
        let rows = (0..AREA.height).map(|y| row(&buf, y)).collect::<Vec<_>>();
        let drawn = rows.join("\n");
        assert!(drawn.contains("default()"), "{drawn}");
        assert!(drawn.contains("fn() -> RetryPolicy"), "{drawn}");
        assert!(drawn.contains("Returns the policy"), "{drawn}");
        // §9's dim is not taken: `7c` draws live code around the list.
        assert_ne!(buf[(0, 0)].fg, theme.neutrals.dimmed_under_float);
    }

    /// `T039`: the same chrome, the other body — and the active parameter is
    /// the one thing on the row in bright text.
    #[test]
    fn a_passive_float_draws_signature_help_through_the_same_chrome() {
        let theme = Theme::phosphor_dark();
        let label = "fn fetch_json(url: &str) -> Value";
        let start = label.find("url").expect("a parameter");
        let host = Session {
            completion: None,
            signature: Some(SignatureVm {
                label: Some(label.to_owned()),
                active: Some((start, start + "url: &str".len())),
                prose: vec!["fetches one url".to_owned()],
                anchor: Anchor::new(6, 2),
                width_floor: 0,
            }),
        };
        let tree =
            Tree::new(Node::Empty {}).with_float(Float::new(Mood::Passive, Node::Signature {}));
        let mut buf = Buffer::empty(AREA);
        let report = Interpreter::new(&theme, &host).render(&tree, AREA, &mut buf);

        assert!(report.deferred.is_empty(), "{report:?}");
        assert_eq!(buf[(6, 3)].symbol(), "┌");
        assert_eq!(buf[(6, 3)].fg, theme.float.passive);
        let drawn = (0..AREA.height).map(|y| row(&buf, y)).collect::<Vec<_>>();
        let text = drawn.join("\n");
        assert!(text.contains("fn fetch_json(url: &str) -> Value"), "{text}");
        assert!(text.contains("fetches one url"), "{text}");
        // The label row: border + 2 pad cols in, then the label. `url: &str`
        // starts `start` cells along it and is the only bright run.
        let label_x = 6 + 1 + 2;
        assert_eq!(buf[(label_x, 4)].fg, theme.neutrals.text);
        let active_x = label_x + u16::try_from(start).expect("a column");
        assert_eq!(buf[(active_x, 4)].fg, theme.neutrals.bright_text);
        assert_eq!(buf[(active_x, 4)].symbol(), "u");
    }

    /// An anchored float is sized to **any** leaf, not only to the two LSP
    /// bodies: `Ctx::width` answers off `Ctx::natural` for everything else, and
    /// a float that came out six columns wide with a label in it would be the
    /// artifact `Float::frame`'s zero-content rule exists to prevent.
    ///
    /// It hangs at the area's corner, because a label has no session and so no
    /// anchor — `passive_anchor`'s fallback, which nothing else reaches.
    #[test]
    fn a_passive_float_is_sized_to_whatever_leaf_it_holds() {
        let tree = Tree::new(Node::Empty {})
            .with_float(Float::new(Mood::Passive, label("mark seen", Tone::Text)));
        let (buf, report) = draw(&tree);
        assert!(report.deferred.is_empty(), "{report:?}");
        assert_eq!(row(&buf, 1), "┌─────────────┐");
        assert!(row(&buf, 2).contains("mark seen"), "{}", row(&buf, 2));
    }

    /// **The anchor belongs to the body, not to whichever door answers first.**
    /// A host that still holds a completion session while it composes a
    /// signature float used to get the signature drawn at the completion's
    /// anchor — §9 constrains floats, not sessions, and nothing in `Resources`
    /// stops both from answering.
    #[test]
    fn a_signature_float_hangs_off_the_signature_even_with_a_completion_live() {
        let theme = Theme::phosphor_dark();
        let host = Session {
            completion: Some(CompletionVm {
                items: vec![item("default()", None)],
                selected: 0,
                documentation: Vec::new(),
                anchor: Anchor::new(30, 8),
                width_floor: 0,
            }),
            signature: Some(SignatureVm {
                label: Some("fn get(url: &str)".to_owned()),
                active: None,
                prose: Vec::new(),
                anchor: Anchor::new(4, 1),
                width_floor: 0,
            }),
        };
        let tree =
            Tree::new(Node::Empty {}).with_float(Float::new(Mood::Passive, Node::Signature {}));
        let mut buf = Buffer::empty(AREA);
        let report = Interpreter::new(&theme, &host).render(&tree, AREA, &mut buf);

        assert!(report.deferred.is_empty(), "{report:?}");
        assert_eq!(buf[(4, 2)].symbol(), "┌", "the signature's own anchor");
        assert_eq!(buf[(30, 9)].symbol(), " ", "not the completion's");
    }

    /// **The anti-thrash floor reaches the float** (`CP-4`).
    ///
    /// `Float::with_width_floor` shipped with no non-test caller — the widget
    /// had the knob, the ViewModel had nowhere to put the number, and nothing
    /// between them turned one into the other, so every anchored float still
    /// recomputed its width per keystroke. This is the composition half, and it
    /// is stated over the *drawn* border rather than over a returned `Rect`
    /// because that is the thing the wiring can be missing from.
    ///
    /// Both doors, in one test, because they take different `Resources`
    /// methods and a single one would leave the other free to come loose.
    #[test]
    fn float_is_held_to_the_widest_the_session_has_been() {
        let theme = Theme::phosphor_dark();
        // Content of two columns; a floor of nine is what a wider answer
        // earlier in the same session left behind.
        let floor = 9;
        let host = Session {
            completion: Some(CompletionVm {
                items: vec![item("de", None)],
                selected: 0,
                documentation: Vec::new(),
                anchor: Anchor::new(1, 1),
                width_floor: floor,
            }),
            signature: Some(SignatureVm {
                label: Some("fn g()".to_owned()),
                active: None,
                prose: Vec::new(),
                anchor: Anchor::new(1, 1),
                width_floor: floor,
            }),
        };
        for node in [Node::Completion {}, Node::Signature {}] {
            let tag = node.tag();
            let tree = Tree::new(Node::Empty {}).with_float(Float::new(Mood::Passive, node));
            let mut buf = Buffer::empty(AREA);
            let report = Interpreter::new(&theme, &host).render(&tree, AREA, &mut buf);
            assert!(report.deferred.is_empty(), "{report:?}");
            // Border at the anchor column, and the right border `floor + 6`
            // columns along: the floor plus an anchored float's chrome.
            assert_eq!(buf[(1, 2)].symbol(), "┌", "{tag}");
            assert_eq!(
                buf[(1 + floor + 6 - 1, 2)].symbol(),
                "┐",
                "{tag}: the float is not held to the session's width\n{}",
                row(&buf, 2)
            );
        }
    }

    /// A composition that raises the passive float with nothing behind it draws
    /// **nothing at all** — not even the border. `query.rs`'s *"an absent thing
    /// answers empty"* meets `Float::frame`'s rule that an anchored float with
    /// no content is not a float, which is the same statement
    /// `float.rs::an_empty_completion_list_draws_nothing_at_all` makes one
    /// layer down. (The doc used to say *"chrome and no body"*; the assertion
    /// has always been that no `│` is drawn, which is the stronger claim.)
    #[test]
    fn a_passive_float_with_no_session_behind_it_draws_nothing_at_all() {
        let tree =
            Tree::new(Node::Empty {}).with_float(Float::new(Mood::Passive, Node::Completion {}));
        let (buf, report) = draw(&tree);
        assert!(report.deferred.is_empty(), "{report:?}");
        let drawn = (0..AREA.height)
            .map(|y| row(&buf, y))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!drawn.contains('│'), "{drawn}");
    }

    // -- deferred primitives -------------------------------------------------

    /// **The module docs' table, executed.**
    ///
    /// Two directions, because the stale version of that table was wrong in
    /// only one of them: every kind it lists defers *and* every kind it does
    /// not list draws. Land a widget and the second half fails; add a node kind
    /// with no arm and the first half fails. Either way the fix is one row.
    #[test]
    fn the_deferred_set_is_exactly_the_kinds_named_here() {
        // Payloads are the emptiest legal ones — this test is about which arm
        // runs, and every one of these arms ignores its props entirely.
        let deferred = [
            Node::TabBar { tabs: Vec::new() },
            Node::Picker {
                source: SourceId("files".to_owned()),
                filter: String::new(),
                columns: Vec::new(),
                preview: false,
            },
            Node::Diff {
                source: DiffSource::Disk {
                    buffer: BufferId(1),
                },
                mode: DiffMode::Unified,
                grouping: Grouping::Flat,
            },
            Node::Question { ask: AskId(8) },
            Node::Transcript {
                follow: false,
                folded: Vec::new(),
            },
            Node::Prompt {
                prompt: PromptKind::Ex,
                text: String::new(),
                anchor: None,
            },
            Node::Watch { watch: WatchId(5) },
        ];
        for node in deferred {
            let tag = node.tag();
            let (_, report) = draw(&Tree::new(node));
            assert_eq!(
                report.deferred,
                vec![tag],
                "`{tag}` is in the module docs' table, so it must defer"
            );
        }

        // The other half. `NoResources` hands back no editor, no marks and no
        // LSP session, so these draw an empty column, an empty rail, an empty
        // strip and an empty list — which is drawing, and is what `deferred`
        // distinguishes from it.
        let drawn = [
            Node::Gutter {
                buffer: BufferId(1),
            },
            Node::Completion {},
            Node::Signature {},
            Node::VirtualText {
                owner: None,
                content: Child::new(label("hint", Tone::Meta)),
            },
            Node::KeyHints {
                density: Density::Footer,
                hints: Vec::new(),
            },
        ];
        for node in drawn {
            let tag = node.tag();
            let (_, report) = draw(&Tree::new(node));
            assert!(
                report.deferred.is_empty(),
                "`{tag}` draws, so the module docs' table must not list it: {report:?}"
            );
        }
    }

    #[test]
    fn an_unbuilt_primitive_is_reported_not_silently_blank() {
        let tree = Tree::new(Node::split(
            Axis::Rows,
            [
                Slot::new(
                    Constraint::Fill { weight: 1 },
                    Node::Pane {
                        pane: PaneId(1),
                        holds: PaneKind::Transcript,
                        focused: true,
                        child: Child::new(Node::Transcript {
                            follow: true,
                            folded: Vec::new(),
                        }),
                    },
                ),
                Slot::new(
                    Constraint::Cells { cells: 1 },
                    Node::Picker {
                        source: SourceId("files".to_owned()),
                        filter: String::new(),
                        columns: Vec::new(),
                        preview: false,
                    },
                ),
            ],
        ));
        let (_, report) = draw(&tree);
        assert_eq!(report.deferred, vec!["transcript", "picker"]);
    }

    #[test]
    fn a_buffer_the_host_does_not_have_draws_nothing() {
        let tree = Tree::new(Node::Buffer {
            buffer: BufferId(7),
            soft_wrap: true,
        });
        let (buf, report) = draw(&tree);
        assert_eq!(row(&buf, 0), "");
        assert_eq!(report.soft_wrap_requested, vec![BufferId(7)]);
    }

    #[test]
    fn an_empty_area_draws_nothing_and_does_not_panic() {
        let theme = Theme::phosphor_dark();
        let tree = Tree::new(label("x", Tone::Text));
        let mut buf = Buffer::empty(AREA);
        let zero = Rect {
            width: 0,
            height: 0,
            ..AREA
        };
        let report = Interpreter::new(&theme, &NoResources).render(&tree, zero, &mut buf);
        assert_eq!(report, Report::default());
    }
}
