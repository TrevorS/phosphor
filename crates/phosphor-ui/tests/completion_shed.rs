//! `CP-4` — the shed order of the decorated completion row, as a law.
//!
//! The finding was *"do we have plans to decorate the auto complete with things
//! like src and meta info about each item"*, and the answer added two columns
//! to a row that already had two. Four columns inside a float capped at 60% of
//! the screen means most real rows cannot show all four, and Design Language
//! §11 says what to do about that: **drop, never squeeze**.
//!
//! §11 writes the statusline's shed order out by name — *"counters → jj →
//! cursor pos → session prose → mode word"*. A completion row needs one too,
//! and [`ListLayout`] states it: **source → detail → kind → and only then the
//! label elides**. This file is the part of that a golden frame cannot hold: a
//! frame is one item set at one width, and the claim is about every item set at
//! every width.
//!
//! # What the generator cannot produce, and why that is on purpose
//!
//! It cannot produce a **negative or fractional** width, because a terminal
//! column is a `u16`; it cannot produce a row whose columns disagree about how
//! many items there are, because the four fields are generated per item; and it
//! cannot produce a `kind` outside the twenty-five the protocol defines,
//! because [`CompletionKind`] is a closed enum and the client maps an unknown
//! number to `None` before this type ever sees it
//! (`phosphor_buffer::lsp::completion_kind`).
//!
//! It **can** produce every degenerate case the laws below have to survive: an
//! empty list, empty strings in any column, a `source` with no `detail` beside
//! it, labels far wider than any terminal, CJK and emoji, and a width of zero.
//!
//! # What it cannot produce that a real server can
//!
//! Listed because the `CP-4` review found the paragraph above accurate and
//! **incomplete**, and an unstated limit reads as coverage. Everything here is
//! a width question, and every width in this widget goes through
//! [`cells`] — `Span::width`, the same call `phosphor_ui`'s own `cells` makes —
//! which is where this repo has shipped the cells-versus-characters bug three
//! times.
//!
//! * **Covered**: ASCII, CJK (`名前`, two cells per character) and one emoji
//!   (`🙂`, two cells), in the **label** column. The label is the column that
//!   matters most, because it is the text that gets inserted.
//! * **Not covered**: combining marks, zero-width joiner sequences, RTL, and
//!   tabs or control characters, in any column. `Span::width` sums
//!   `unicode-width` per character, so a base plus a combining mark measures 2
//!   where a terminal draws 1 — a grapheme cluster is not a unit anything in
//!   this path knows about.
//! * **Not covered**: any wide character in a **meta** column. `detail` and
//!   `source` are generated from ASCII classes, so the only wide meta cells
//!   anywhere in this crate's tests are the hand-written `送信` row in
//!   `tests/float_width.rs`'s `decorated()` frame.
//! * **Bounded**: widths stop at 200, lists at 11 items, documentation at 2
//!   lines. A 4-cell terminal and a 300-item list are both outside it.

use phosphor_core::request::CompletionKind;
use phosphor_ui::float::{
    Anchor, CompletionItemVm, CompletionList, CompletionVm, Float, FloatBody as _, FloatSlot,
    MAX_ITEM_ROWS, PAD_COLS,
};
use phosphor_ui::theme::Theme;
use proptest::prelude::*;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::text::Span;

/// §2's lexicon: `⋯` is *"elided"*. Written here rather than reached for in
/// `phosphor_ui` because the constant there is crate-private, and because what
/// this test is entitled to know is the design language's glyph, not the
/// widget's spelling of it.
const ELISION: &str = "\u{22ef}";

/// Display width in cells, measured the way the widget measures it.
///
/// **Not `str::len` and not `chars().count()`**: `名前` is two characters and
/// four columns, and a test that counted either would agree with a widget that
/// had the same bug. This is the same `Span::width` `phosphor_ui`'s own `cells`
/// uses, reached through the public API rather than copied.
fn cells(text: &str) -> u16 {
    u16::try_from(Span::raw(text).width()).unwrap_or(u16::MAX)
}

fn widest_label(vm: &CompletionVm) -> u16 {
    vm.items
        .iter()
        .map(|item| cells(&item.label))
        .max()
        .unwrap_or(0)
}

prop_compose! {
    /// One decorated row, with every column independently present or absent.
    ///
    /// The strings run past any terminal on purpose, and the `source` is
    /// generated **independently of the `detail`** — a server that sends a
    /// description and no detail is legal and is the case that would break a
    /// layout which assumed the columns arrive in order.
    fn any_item()(
        label in "[a-z_]{0,40}|名前{1,10}|🙂x",
        detail in prop::option::of("[a-zA-Z<>() ,:-]{0,120}"),
        source in prop::option::of("[a-z:_]{0,60}"),
        kind in prop::option::of(0usize..25),
        deprecated in any::<bool>(),
    ) -> CompletionItemVm {
        CompletionItemVm {
            label,
            detail,
            source,
            kind: kind.map(|index| CompletionKind::ALL[index]),
            deprecated,
        }
    }
}

prop_compose! {
    fn any_session()(
        items in prop::collection::vec(any_item(), 0..12),
        selected in 0usize..14,
        docs in prop::collection::vec("[a-z .]{0,200}", 0..3),
    ) -> CompletionVm {
        CompletionVm {
            items,
            selected,
            documentation: docs,
            anchor: Anchor::new(2, 1),
            width_floor: 0,
        }
    }
}

proptest! {
    /// **The label is never the first thing to lose**, which is the whole point
    /// of writing the shed order down.
    ///
    /// A label is the text that gets inserted into the buffer; a detail is a
    /// type you already half know and a source is where it came from. So while
    /// *either* meta column is still on screen, every label must be drawable
    /// whole — `label_room` is what `text_elided` is handed, and it elides
    /// exactly when the text is wider than that.
    ///
    /// Written as an implication rather than as an equality on purpose: once
    /// both meta columns are gone the label is entitled to every column that is
    /// left, and to an `⋯` after that.
    ///
    /// # The half that could not fail, and what replaced it
    ///
    /// The `CP-4` review caught this test comparing a value to itself: in the
    /// branch its guard selects, `layout.label_room` **is** `widths().1`, and
    /// [`widest_label`] recomputes that same maximum with the same `cells`. The
    /// guard was `keep_detail || keep_source` spelled a second way and the
    /// assertion reduced to `label >= label`.
    ///
    /// The room claim is kept — it is what a reader wants to see, and it does
    /// bite on a `label_room` computed from anything other than the labels —
    /// but the load-bearing assertion is now the second one, which ties **two**
    /// fields of the layout together: the cells the label was promised are
    /// cells no meta column also claims. That is not restatable as one branch
    /// condition, and it is red under the shape this widget shipped with
    /// (`label_room = width - label_at`, which collides with the detail instead
    /// of quietly overrunning).
    #[test]
    fn a_label_is_never_elided_while_a_meta_column_survives(
        vm in any_session(),
        width in 0u16..200,
    ) {
        let list = CompletionList::new(&vm);
        let layout = list.layout(width);
        let Some(first_meta) = layout.detail_at.or(layout.source_at) else {
            return Ok(());
        };
        prop_assert!(
            layout.label_room >= widest_label(&vm),
            "meta survived at width {width} but the label has only {} of {} cells: {layout:?}",
            layout.label_room,
            widest_label(&vm),
        );
        prop_assert!(
            layout.label_at.saturating_add(layout.label_room) < first_meta,
            "the label's cells run into the meta column at width {width}: {layout:?}",
        );
    }

    /// **A surviving detail column is never one cell of pure elision.**
    ///
    /// `Canvas::text_elided` writes `room - 1` characters and then `⋯`, so a
    /// detail column with one cell draws the elision mark and nothing else —
    /// two gap cells and one content cell spent saying *"something was
    /// removed"*. That is the squeeze §11 forbids, one column over from where
    /// [`ListLayout`] argues the elision is allowed, and it was reachable on
    /// any 30–40 column split: against `rust-analyzer` at `cols=30` a row read
    /// `meth len   ⋯`.
    ///
    /// The floor is `min(widest detail, 2)` and not a flat `2` because a detail
    /// that *is* one cell wide is not elided at all — it fits.
    ///
    /// **This bites:** put `float::keep_detail` back to
    /// `label_end + DETAIL_GAP < width` and this fails with `detail_room: 1`
    /// against a detail dozens of cells wide.
    #[test]
    fn a_surviving_detail_column_can_say_something(
        vm in any_session(),
        width in 0u16..200,
    ) {
        let layout = CompletionList::new(&vm).layout(width);
        if layout.detail_at.is_none() {
            return Ok(());
        }
        let widest = vm
            .items
            .iter()
            .map(|item| cells(item.detail.as_deref().unwrap_or_default()))
            .max()
            .unwrap_or(0);
        prop_assert!(
            layout.detail_room >= widest.min(2),
            "a detail column of {} cells against a {widest}-cell detail at width {width}: \
             {layout:?}",
            layout.detail_room,
        );
    }

    /// The order is an order: **source goes before detail, always.**
    ///
    /// Without this the previous law is satisfiable by a layout that drops the
    /// detail and keeps the source, which is the wrong column to have kept —
    /// `7c` draws the detail and draws no source at all.
    #[test]
    fn the_source_column_is_shed_before_the_detail(
        vm in any_session(),
        width in 0u16..200,
    ) {
        let layout = CompletionList::new(&vm).layout(width);
        prop_assert!(
            layout.source_at.is_none() || layout.detail_at.is_some()
                || vm.items.iter().all(|item| {
                    item.detail.as_deref().unwrap_or_default().is_empty()
                }),
            "the source survived a shed the detail did not, at width {width}: {layout:?}",
        );
    }

    /// …and the kind goes before the label is touched.
    ///
    /// The third step, and the one with no meta column to hide behind: a list
    /// whose every detail and source is empty still has four cells to give back
    /// before an `⋯` is allowed near a label.
    ///
    /// **Stated against the drawn cells**, because the layout form of it could
    /// not fail: `if layout.kind_at.is_some() { label_at + widest <= width }` is
    /// `keep_kind`'s own definition rearranged, `label_at` being `kind_block`
    /// exactly when the kind is kept. The `CP-4` review found that by reading
    /// it. What is asserted here instead is the sentence in the title — *the
    /// kind column is on screen and no label carries an `⋯`* — which says
    /// nothing about how `keep_kind` is spelled.
    ///
    /// **This bites:** drop the `+ label` from `keep_kind` in `float::layout`
    /// and a list with a label wider than the float keeps its kind column while
    /// the labels elide under it.
    #[test]
    fn the_kind_column_is_shed_before_the_label_is_elided(
        vm in any_session(),
        width in 8u16..140,
    ) {
        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, width, 14);
        let mut buf = Buffer::empty(area);
        let list = CompletionList::new(&vm);
        let float = Float::passive(&list, vm.anchor);
        let frame = float.frame(area);
        FloatSlot::with(float).render(area, &mut buf, &theme);
        let chrome = 2 * (1 + PAD_COLS);
        let Some(body_width) = frame.width.checked_sub(chrome) else {
            return Ok(());
        };
        let layout = list.layout(body_width);
        if layout.kind_at.is_none() {
            return Ok(());
        }
        let left = frame.x + 1 + PAD_COLS;
        // **Item rows only.** The documentation block under the rule is
        // prose and is entitled to elide — scanning it too made this test fail
        // on a doc line, which is not a label at all.
        let item_rows = u16::try_from(vm.items.len())
            .unwrap_or(u16::MAX)
            .min(MAX_ITEM_ROWS);
        for dy in 1..=item_rows {
            for dx in layout.label_at..layout.label_at.saturating_add(layout.label_room) {
                let x = left + dx;
                if x >= area.right() {
                    break;
                }
                let y = frame.y + dy;
                prop_assert_ne!(
                    buf[(x, y)].symbol(),
                    ELISION,
                    "{}",
                    format!("the kind column kept its cells while a label elided at \
                             ({x}, {y}), width {width}: {layout:?}"),
                );
            }
        }
    }

    /// Every column the layout places starts and ends inside the body.
    ///
    /// A layout is a promise about where `render` will write, and a column
    /// placed past the right edge is a row that silently draws nothing where
    /// the shed order said it would draw something.
    #[test]
    fn no_column_is_placed_outside_the_body(
        vm in any_session(),
        width in 0u16..200,
    ) {
        let layout = CompletionList::new(&vm).layout(width);
        prop_assert!(layout.label_at.saturating_add(layout.label_room) <= width, "{layout:?}");
        for (at, room) in [
            (layout.detail_at, layout.detail_room),
            (layout.source_at, layout.source_room),
        ] {
            if let Some(at) = at {
                prop_assert!(at.saturating_add(room) <= width, "{layout:?} at width {width}");
            }
        }
    }

    /// **The law stated against the cells rather than against the layout.**
    ///
    /// The four above are about a pure function; this one renders and reads the
    /// buffer back, because a `ListLayout` that was right and a `render` that
    /// ignored it would pass every one of them. If a detail or a source is on
    /// screen, no `⋯` may appear in the label's own columns.
    #[test]
    fn a_rendered_row_carries_no_elision_in_the_label_while_meta_is_drawn(
        vm in any_session(),
        width in 8u16..140,
    ) {
        let theme = Theme::phosphor_dark();
        let area = Rect::new(0, 0, width, 14);
        let mut buf = Buffer::empty(area);
        let list = CompletionList::new(&vm);
        let float = Float::passive(&list, vm.anchor);
        let frame = float.frame(area);
        FloatSlot::with(float).render(area, &mut buf, &theme);
        // The body is the frame inset by the border and §8's two padding
        // columns, on both sides — the rectangle `FloatBody::render` is handed.
        let chrome = 2 * (1 + PAD_COLS);
        let Some(body_width) = frame.width.checked_sub(chrome) else {
            return Ok(());
        };
        let layout = list.layout(body_width);
        if layout.detail_at.is_none() && layout.source_at.is_none() {
            return Ok(());
        }
        let left = frame.x + 1 + PAD_COLS;
        // **Item rows only.** The documentation block under the rule is
        // prose and is entitled to elide — scanning it too made this test fail
        // on a doc line, which is not a label at all.
        let item_rows = u16::try_from(vm.items.len())
            .unwrap_or(u16::MAX)
            .min(MAX_ITEM_ROWS);
        for dy in 1..=item_rows {
            for dx in 0..layout.label_at.saturating_add(layout.label_room) {
                let x = left + dx;
                if x >= area.right() {
                    break;
                }
                let y = frame.y + dy;
                prop_assert_ne!(
                    buf[(x, y)].symbol(),
                    ELISION,
                    "{}",
                    format!("an elided label at ({x}, {y}) while meta is drawn: {layout:?}"),
                );
            }
        }
    }

    /// `desired_width` is what the layout needs to keep everything.
    ///
    /// The two are written separately — one sums the columns, the other places
    /// them — and this is what stops them drifting: at the width the body asks
    /// for, nothing is shed. Without it a `desired_width` that under-counted a
    /// gap would make the float one column too narrow and drop a whole column
    /// on every list, at every terminal size, silently.
    #[test]
    fn nothing_is_shed_at_the_width_the_list_asked_for(vm in any_session()) {
        let list = CompletionList::new(&vm);
        let wanted = list.desired_width();
        let layout = list.layout(wanted);
        let has_detail = vm.items.iter().any(|item| {
            !item.detail.as_deref().unwrap_or_default().is_empty()
        });
        let has_source = vm.items.iter().any(|item| {
            !item.source.as_deref().unwrap_or_default().is_empty()
        });
        let has_kind = vm.items.iter().any(|item| item.kind.is_some());
        prop_assert_eq!(layout.detail_at.is_some(), has_detail, "{:?}", layout);
        prop_assert_eq!(layout.source_at.is_some(), has_source, "{:?}", layout);
        prop_assert_eq!(layout.kind_at.is_some(), has_kind, "{:?}", layout);
        prop_assert!(layout.label_room >= widest_label(&vm), "{layout:?}");
    }
}
