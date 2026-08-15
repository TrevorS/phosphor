//! `CP-4` — the anchored float's width band, as golden frames at both widths.
//!
//! The finding, reported by hand against the running binary: *"we need a max
//! width for completion and hover and stuff — right now it's very dynamic and
//! will go from small to across the screen."*
//!
//! `tests/screen_7c.rs` commits the screen the design draws, where every row
//! fits and the float is sized to its content. **These are the frames it does
//! not draw**: a `detail` from a real server, a hover paragraph, and a list of
//! wide characters — each at 120 columns and at 80, because a cap expressed as
//! a percentage is exactly the kind of rule that holds at one width and rounds
//! wrong at the other.
//!
//! What each frame is asked to show:
//!
//! * The float stops at [`anchored_max_cols`] — 72 of 120, 48 of 80 — instead
//!   of running to the edge, so the code being typed into is still on screen.
//! * A row too long for that ends in §2's `⋯`. It does not stop mid-word and it
//!   does not disappear.
//! * A completion row spends its columns on the **label** and drops the meta
//!   `detail` — §11's *"drop, never squeeze"* inside one row — while hover
//!   prose, which has no such split, truncates on its own line.
//! * Wide characters are counted in cells: `名前` is two columns per character
//!   and the mark still lands in the last one, with no half-drawn glyph.
//!
//! Same serialiser and the same review loop as `tests/golden_frames.rs` and
//! `tests/screen_7c.rs` (`cargo insta review`, or `just review`).

mod frame_grid;

use frame_grid::Frame;
use phosphor_core::request::CompletionKind;
use phosphor_ui::float::{
    ANCHORED_WIDTH_PCT, Anchor, CompletionItemVm, CompletionList, CompletionVm, Float, FloatBody,
    FloatSlot, SignatureBody, SignatureVm, anchored_max_cols, anchored_wrap_cols,
};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::text::Span;

/// The two widths every golden frame in this repo is taken at.
const WIDTHS: [u16; 2] = [120, 80];

fn item(label: &str, detail: Option<&str>) -> CompletionItemVm {
    CompletionItemVm {
        label: label.to_owned(),
        detail: detail.map(str::to_owned),
        ..CompletionItemVm::default()
    }
}

/// Draw one passive float on an empty buffer and serialise the whole area.
///
/// **The area, not the frame.** What the finding is about is how much of the
/// screen the float takes, so the empty columns to its right are the evidence
/// and cropping to the frame would delete it.
fn frame_of(body: &dyn FloatBody, area: Rect) -> (Buffer, Rect) {
    let theme = Theme::phosphor_dark();
    let mut buf = Buffer::empty(area);
    let float = Float::passive(body, Anchor::new(2, 1));
    let frame = float.frame(area);
    FloatSlot::with(float).render(area, &mut buf, &theme);
    (buf, frame)
}

fn commit(name: &str, buf: &Buffer, notes: &[&str]) {
    let theme = Theme::phosphor_dark();
    let frame = Frame {
        screen: name,
        theme_label: "phosphor dark",
        theme: &theme,
        notes,
    };
    insta::assert_snapshot!(name.to_owned(), frame.to_text(buf));
    assert!(frame.unnamed(buf).is_empty(), "{:?}", frame.unnamed(buf));
}

// ---------------------------------------------------------------------------
// A completion detail longer than the terminal
// ---------------------------------------------------------------------------

/// `rust-analyzer`'s answer for a method on a generic type, which is what took
/// the screen at `CP-4`. The documentation line is a real one-line summary and
/// is also past 80 columns.
fn long_detail() -> CompletionVm {
    CompletionVm {
        items: vec![
            item(
                "retry_with_backoff",
                Some(
                    "fn(&mut Client, RetryPolicy, Duration, &[String]) \
                     -> Result<Vec<Result<Value, FetchError>>, FetchError>",
                ),
            ),
            item(
                "retry_once",
                Some("fn(&mut Client) -> Result<Value, FetchError>"),
            ),
            item("retries_left", Some("u32")),
        ],
        selected: 0,
        documentation: vec![
            "Retries the request with exponential backoff, honouring the policy's attempt \
             count and its ceiling."
                .to_owned(),
        ],
        anchor: Anchor::new(2, 1),
        width_floor: 0,
    }
}

#[test]
fn a_completion_detail_longer_than_the_terminal() {
    for width in WIDTHS {
        let vm = long_detail();
        let list = CompletionList::new(&vm);
        let area = Rect::new(0, 0, width, 9);
        let (buf, frame) = frame_of(&list, area);

        assert_eq!(frame.width, anchored_max_cols(width));
        commit(
            &format!("long-detail-{width}"),
            &buf,
            &[
                "CP-4: 'we need a max width for completion and hover and stuff — right",
                "  now its very dynamic and will go from small to across the screen'.",
                "The float stops at 60% of the area (ANCHORED_WIDTH_PCT) — §8's band",
                "  floor read as the passive float's ceiling — where before it was",
                "  content plus chrome capped only by the terminal.",
                "Every row here is longer than the float. The label keeps its columns",
                "  and the meta detail loses them (§11 'drop, never squeeze'), so the",
                "  text that would be inserted is never the half that goes.",
                "Rows that do not fit end in §2's ⋯ rather than stopping mid-word.",
                "7c at the width the design draws it is unchanged by any of this, and",
                "  the measurement that says so lives on ANCHORED_WIDTH_PCT rather than",
                "  here — one statement, because four copies of an arithmetic claim in",
                "  prose is four things no lint can reconcile.",
            ],
        );
    }
}

// ---------------------------------------------------------------------------
// A hover paragraph
// ---------------------------------------------------------------------------

/// `textDocument/hover` on `retry_with_backoff`, as one unwrapped paragraph —
/// the shape a host that has **not** run [`phosphor_ui::float::wrap_prose`]
/// sends. The shipping binary does run it (`Editing::wrapped`), so this frame is
/// the degraded case rather than the ordinary one.
fn long_hover() -> SignatureVm {
    SignatureVm {
        label: None,
        active: None,
        prose: vec![
            "Retries the request with exponential backoff. The delay doubles after each \
             failure and is capped by the policy, so a server that is down does not turn \
             into a busy loop."
                .to_owned(),
            "Returns the first successful response, or the last error if every attempt \
             failed."
                .to_owned(),
        ],
        anchor: Anchor::new(2, 1),
        width_floor: 0,
    }
}

#[test]
fn a_hover_paragraph_longer_than_the_terminal() {
    for width in WIDTHS {
        let vm = long_hover();
        let body = SignatureBody::new(&vm);
        let area = Rect::new(0, 0, width, 6);
        let (buf, frame) = frame_of(&body, area);

        assert_eq!(frame.width, anchored_max_cols(width));
        commit(
            &format!("long-hover-{width}"),
            &buf,
            &[
                "The same cap over hover, where the prose IS the answer and there is no",
                "  list above it competing for the columns — so it truncates on its own",
                "  line rather than giving its columns to something else.",
                "This paragraph arrives UNWRAPPED, which is the BACKSTOP and not the",
                "  path the binary takes: §11 is 'nothing ever wraps' and",
                "  SignatureVm::prose is one string per screen row, so wrapping is the",
                "  host's job. float::wrap_prose at anchored_wrap_cols(width) is that",
                "  job and Editing::wrapped is where the shipping binary does it, so a",
                "  real hover gets no ⋯ at all. The width itself is pinned in",
                "  the_cap_at_both_widths below.",
                "What must NOT happen is the answer vanishing. The mark says the line",
                "  goes on; the first cells of it are still readable.",
            ],
        );
    }
}

// ---------------------------------------------------------------------------
// Wide characters
// ---------------------------------------------------------------------------

/// A session of CJK and emoji labels. Width here is a **cell** count and not a
/// character count — the confusion this repo has shipped three bugs from.
fn wide_chars() -> CompletionVm {
    CompletionVm {
        items: vec![
            item(
                "送信する",
                Some("fn(あて: Addr, 本文: Body) -> Result<(), 送信エラー>"),
            ),
            item("送信済み確認", Some("bool")),
            item("🙂reaction", Some("fn(&str) -> Emoji")),
        ],
        selected: 1,
        documentation: vec![
            "指定した宛先にメッセージを送信します。失敗した場合は送信エラーを返します。".to_owned(),
        ],
        anchor: Anchor::new(2, 1),
        width_floor: 0,
    }
}

#[test]
fn a_list_of_wide_characters() {
    for width in WIDTHS {
        let vm = wide_chars();
        let list = CompletionList::new(&vm);
        let area = Rect::new(0, 0, width, 9);
        let (buf, frame) = frame_of(&list, area);

        assert!(frame.width <= anchored_max_cols(width));
        // The rows that overran carry the mark, and none of them left a
        // continuation cell with no lead character in front of it — which is
        // what truncating a `送` at the cap by counting characters would do.
        let rows: Vec<String> = (frame.y + 1..frame.bottom() - 1)
            .map(|y| {
                (frame.x..frame.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect()
            })
            .collect();
        assert!(rows.iter().any(|row| row.contains('\u{22ef}')), "{rows:?}");
        for row in &rows {
            assert_eq!(row.chars().count(), usize::from(frame.width), "{row:?}");
        }
        commit(
            &format!("wide-chars-{width}"),
            &buf,
            &[
                "Width is measured in CELLS, not characters. Each CJK character is two",
                "  columns wide, so the label column and the ⋯ both land where a",
                "  character count would put them two columns early.",
                "A truncated row never splits a wide grapheme: the last character that",
                "  did not fit goes whole, its cell is blanked, and the mark takes the",
                "  final column. Buffer::set_stringn does the walking, so the",
                "  measurement that lays out is the measurement that draws.",
                "The second item is selected here (#26332a + bright text), so the tint",
                "  is on a row whose label is wide — the tint covers the body's columns",
                "  and not the chrome's, as in 7c.",
            ],
        );
    }
}

// ---------------------------------------------------------------------------
// The decorated row (`CP-4`)
// ---------------------------------------------------------------------------

/// A row with all four columns, so the shed order is visible rather than
/// argued.
///
/// One item of each awkward shape, because a list where every row was the same
/// would pass a renderer that measured only the first: a long label, a long
/// detail, a long `source`, a **deprecated** row, and a CJK label whose width
/// in cells is twice its length in characters.
fn decorated() -> CompletionVm {
    let full = |label: &str, detail: &str, source: &str, deprecated: bool| CompletionItemVm {
        label: label.to_owned(),
        detail: Some(detail.to_owned()),
        source: Some(source.to_owned()),
        kind: Some(CompletionKind::Function),
        deprecated,
    };
    CompletionVm {
        items: vec![
            full(
                "retry_with_backoff",
                "fn(&mut Client) -> Value",
                "net::retry",
                false,
            ),
            CompletionItemVm {
                label: "retries_left".to_owned(),
                detail: Some("u32".to_owned()),
                source: None,
                kind: Some(CompletionKind::Constant),
                deprecated: false,
            },
            full("retry_forever", "fn(&mut Client) -> !", "net::legacy", true),
            CompletionItemVm {
                label: "送信する".to_owned(),
                detail: Some("fn(Addr) -> Result".to_owned()),
                source: Some("送信".to_owned()),
                kind: Some(CompletionKind::Method),
                deprecated: false,
            },
        ],
        selected: 0,
        documentation: vec!["Retries the request with exponential backoff.".to_owned()],
        anchor: Anchor::new(2, 1),
        width_floor: 0,
    }
}

#[test]
fn a_decorated_completion_list() {
    for width in WIDTHS {
        let vm = decorated();
        let list = CompletionList::new(&vm);
        let area = Rect::new(0, 0, width, 10);
        let (buf, frame) = frame_of(&list, area);

        assert!(frame.width <= anchored_max_cols(width));
        // The claim the frames are here to show, stated as arithmetic so a
        // reader does not have to count cells in a snapshot: at 120 there is
        // room for the source column and at 80 there is not, and the label
        // keeps every one of its cells at both.
        let body = frame.width - 6;
        let layout = list.layout(body);
        let widest = vm
            .items
            .iter()
            .map(|item| u16::try_from(Span::raw(&item.label).width()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0);
        assert_eq!(layout.label_room, widest, "the label is whole at {width}");
        assert!(layout.detail_at.is_some(), "the detail survives at {width}");
        assert_eq!(
            layout.source_at.is_some(),
            width == 120,
            "the source column is the first thing shed, at {width} columns"
        );

        commit(
            &format!("decorated-{width}"),
            &buf,
            &[
                "CP-4: 'do we have plans to decorate the auto complete with things like",
                "  src and meta info about each item like in common lsp implementations",
                "  in emacs and vim?'. Four columns now: kind, label, detail, source —",
                "  the shape nvim-cmp+lspkind, corfu+kind-icon, company-box and VS Code",
                "  all share, with the kind left and the meta dimmed on the right.",
                "The kind is a WORD (fn, cnst, meth) and not an icon. §2's glyph lexicon",
                "  is ten entries, Nerd-Font-free, and none of them is a completion",
                "  kind — inventing twenty-five would be inventing a second lexicon, and",
                "  four ASCII letters cannot have the cells-vs-chars bug this repo has",
                "  shipped three times.",
                "SHED ORDER (§11 'drop, never squeeze'): source → detail → kind → and",
                "  only then the label elides. At 120 all four fit; at 80 the source is",
                "  gone, the detail has taken what is left and elides into it, and the",
                "  label has lost nothing.",
                "Three of the four columns are whole or absent. The detail is the one",
                "  that elides, because it is the last column before the label and has",
                "  nothing further right to hand its room to — and because a real",
                "  rust-analyzer signature is 100 cells against a 72-cell float, so a",
                "  detail that had to fit whole would never be drawn in Rust at all.",
                "  A source is a NAME and net::ret⋯ names nothing, which is why that",
                "  one goes whole.",
                "The deprecated row (retry_forever) is struck through AND receded one",
                "  step down §1's neutral ramp, on T085's degradation principle: SGR 9",
                "  is not universal and phosphor-term cannot report on it, so the colour",
                "  carries the meaning where the escape is ignored.",
                "7c itself draws NEITHER of the two new columns. Adding them is a design",
                "  change, flagged rather than folded in.",
            ],
        );
    }
}

/// **The deprecated row, selected** — the branch of `float::label_style` the
/// frames above execute and assert nothing about.
///
/// `label_style` recedes one step down §1's neutral ramp, and *which* step
/// depends on where the row started: `bright_text → text` on the selected row,
/// `text → prose` everywhere else. `decorated()` selects row 0 and the
/// deprecated row is row 2, so the `decorated-80` frame pins only the second
/// arm — a mutation swapping the two neutrals leaves every committed frame
/// byte-identical. `CP-4`'s review found that by reading the frame.
///
/// The assertion is on the buffer rather than only on the snapshot, because
/// *"the struck cells are `neutrals.text`"* is the claim and a reader should
/// not have to decode a legend letter to check it. The frame is committed
/// beside it so the two treatments are visible together.
///
/// **This bites:** swap `theme.neutrals.text` and `theme.neutrals.prose` in
/// `label_style` and this fails naming both colours.
#[test]
fn a_deprecated_row_recedes_from_the_selected_colour_when_it_is_the_selected_row() {
    let theme = Theme::phosphor_dark();
    let mut vm = decorated();
    // `retry_forever`, the one row with `deprecated: true`.
    vm.selected = 2;
    assert!(
        vm.items[vm.selected].deprecated,
        "this test is about the selected row being the deprecated one"
    );

    let list = CompletionList::new(&vm);
    let area = Rect::new(0, 0, 120, 10);
    let (buf, _) = frame_of(&list, area);

    let struck: Vec<(u16, u16)> = area
        .rows()
        .flat_map(|row| row.columns())
        .filter(|position| {
            buf[*position]
                .modifier
                .contains(ratatui_core::style::Modifier::CROSSED_OUT)
        })
        .map(|position| (position.x, position.y))
        .collect();
    assert!(
        !struck.is_empty(),
        "the deprecated label is struck through; nothing on the frame is"
    );
    for (x, y) in &struck {
        assert_eq!(
            buf[(*x, *y)].fg,
            theme.neutrals.text,
            "the selected deprecated row recedes bright_text -> text, not text -> prose"
        );
    }
    assert_ne!(
        theme.neutrals.text, theme.neutrals.prose,
        "the two arms of label_style would be indistinguishable otherwise"
    );

    commit(
        "decorated-selected-deprecated",
        &buf,
        &[
            "The same list as decorated-120 with the DEPRECATED row selected.",
            "label_style recedes one step down §1's neutral ramp and which step",
            "  depends on where the row started: bright_text -> text when it is the",
            "  selected row, text -> prose otherwise. decorated-120 selects row 0 and",
            "  so only ever draws the second arm — swapping the two neutrals left",
            "  every committed frame byte-identical, which CP-4's review found by",
            "  reading the frame rather than by running anything.",
            "The strikethrough is unchanged: SGR 9 is not universal, so the colour",
            "  is what carries the meaning where the escape is ignored (T085).",
        ],
    );
}

// ---------------------------------------------------------------------------
// The numbers the frames above are drawn against
// ---------------------------------------------------------------------------

/// The cap and the published wrap width at both frame widths, stated once so a
/// change to [`ANCHORED_WIDTH_PCT`] is a diff here rather than six snapshot
/// diffs whose cause has to be inferred.
#[test]
fn the_cap_at_both_widths() {
    assert_eq!(ANCHORED_WIDTH_PCT, 60);
    assert_eq!(anchored_max_cols(120), 72);
    assert_eq!(anchored_max_cols(80), 48);
    // Six columns of chrome: a border and §8's two padding columns each side.
    assert_eq!(anchored_wrap_cols(120), 66);
    assert_eq!(anchored_wrap_cols(80), 42);
    // Below the chrome the float is not drawn at all, so the wrap width bottoms
    // out at zero rather than wrapping around.
    assert_eq!(anchored_wrap_cols(4), 0);
}
