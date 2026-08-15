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
use phosphor_ui::float::{
    ANCHORED_WIDTH_PCT, Anchor, CompletionItemVm, CompletionList, CompletionVm, Float, FloatBody,
    FloatSlot, SignatureBody, SignatureVm, anchored_max_cols, anchored_wrap_cols,
};
use phosphor_ui::theme::Theme;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;

/// The two widths every golden frame in this repo is taken at.
const WIDTHS: [u16; 2] = [120, 80];

fn item(label: &str, detail: Option<&str>) -> CompletionItemVm {
    CompletionItemVm {
        label: label.to_owned(),
        detail: detail.map(str::to_owned),
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
