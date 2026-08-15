//! `F5` — the LSP wire: framing, `Content-Length`, and what a body decodes to.
//!
//! # Why this target exists
//!
//! Everything `T036`'s client learns arrives as bytes from a subprocess we did
//! not write. A theme file is a user's; a journal is our own writer's; **a
//! server's stdout is a program with its own bugs, its own version and its own
//! opinion about the specification**, and the client has no way to refuse the
//! conversation. The review of `T036` asked what happens on a truncated header,
//! a lying `Content-Length` and a server that closes mid-message, and the
//! answer then was partly `unwrap`. `Bounded`/`FrameScan` closed the two that
//! abort the process; this target is the standing search for the rest, because
//! *a language server crashing must never take the editor with it* is a claim
//! about inputs nobody has thought of.
//!
//! # Input — the wire itself, and nothing wrapped around it
//!
//! A byte string, read as a server's stdout: header lines, a blank line,
//! `Content-Length` bytes of body, repeat. No length prefix, no `arbitrary`
//! encoding, for `fuzz/README.md`'s reason — the corpus is real traffic
//! (`seeds/lsp_wire/`, written by serializing real `lsp_types` values through
//! the same `serde` impls `async-lsp` writes with) and real traffic only seeds
//! a target whose input *is* the format.
//!
//! # Two halves, and the seam between them is upstream
//!
//! The shipping path is `pipe → Bounded → async-lsp's Message::read → serde →
//! Router → ingest → the converters`. The middle of that is `async-lsp`'s and
//! private — `Message::read` is not a public door — so this target reaches the
//! two ends, which are ours, and reimplements only the seam:
//!
//! * **Framing** is [`Bounded`] and [`FrameScan`] themselves. Both are the
//!   shipping types; neither is copied here, because a target over a copy
//!   proves things about the copy.
//! * **Decode** is `serde_json` into `lsp_types` — the step `Message::read`
//!   does — then the client's own converters: `diagnostic_from_lsp` (the whole
//!   of what `ingest` does with a `publishDiagnostics`), and the five response
//!   shapes `look_up` and `answer` ask for (`T038`, `T039`).
//! * **[`framed`]** is the reimplementation, one function pinned to `async-lsp`
//!   0.2.4's `Message::read`, and it exists only to state law 2. If `async-lsp`
//!   moves, it is the thing to re-read; that is a cost this target accepts to
//!   have a law about desync at all.
//!
//! # The laws
//!
//! 1. **The scanner is total, and chunk boundaries do not exist.** `Bounded`
//!    over the same bytes at four read sizes — one byte at a time included —
//!    gives one verdict, and never panics. A pipe chooses the chunking, so a
//!    scanner whose answer depends on it has a bug the pipe decides when to
//!    show you.
//! 2. **No false refusal, and no desync.** For any input `async-lsp`'s own
//!    rules frame completely, under both caps: `FrameScan` accepts every byte,
//!    and sits at a frame boundary (`mid_frame()` false) exactly where the
//!    framer does. This is the claim `FrameScan`'s header makes — *"the count
//!    comes from the same header field async-lsp will `read_exact` with, which
//!    keeps the two in step by construction"* — asserted rather than taken. A
//!    desync means the scanner is reading a body as headers, and the first
//!    diagnostic quoting a large `Content-Length` kills a working server.
//!
//!    *"Under both caps"* is over **every** `Content-Length` line, not just the
//!    effective one, and the difference is a real divergence found by reading
//!    rather than by running: both sides take the last of a duplicated header,
//!    but `FrameScan` refuses the moment any declaration passes
//!    `MAX_FRAME_BYTES`, so `Content-Length: 999999999999999` followed by
//!    `Content-Length: 5` ends the connection where `async-lsp` would have read
//!    five bytes. No server sends that, refusing early is the safe direction,
//!    and it is written down here rather than asserted away.
//! 3. **Decode is total.** Whatever a body says, the converters answer. A
//!    `character` no line is that long, a range that ends before it starts, a
//!    parameter label naming offsets outside its signature, a `MarkedString`
//!    that is a number — every one of them is a value on the wire, and the
//!    thread it lands on is the one holding the server.
//!
//! # What this target found
//!
//! **An arithmetic overflow in `column_from_utf16`, reachable from one
//! `publishDiagnostics` frame.** `Position::character` is a `u32`, so
//! `4294967295` deserialises, and the *past the end* case carried the excess
//! through as `column + (character - units)` — `1 + u32::MAX` against any
//! all-BMP line, `""` included. `attempt to add with overflow`, on the LSP
//! task, in every build with overflow checks on: the editor keeps running and
//! silently stops receiving diagnostics from that server. Fixed by saturating
//! (`crates/phosphor-buffer/src/lsp.rs::column_from_utf16`), pinned by
//! `lsp::tests::a_wire_position_at_the_u32_ceiling_does_not_overflow`, and
//! `seeds/lsp_wire/diagnostics-ceiling` keeps the reproducer in the corpus.
//!
//! It came out of the **seed corpus**, not out of a mutation, and that is worth
//! saying: `4294967295` is one exact 32-bit value, and coverage feedback has no
//! gradient towards a decimal literal it has never seen. Reverting the fix
//! kills a run in under a second; nothing in 5.28M runs (601 seconds) after it
//! tripped any of the three laws.
//!
//! # What it cannot reach, stated so nobody counts it as covered
//!
//! * **`MAX_FRAME_BYTES` honestly.** libFuzzer's `max_len` is 4096, so a
//!   64 MiB body cannot exist in an input; only the *declaration* is reachable,
//!   which is the half that matters and the half that is checked. The
//!   allocation itself is `tests/lsp.rs::an_absurd_content_length_is_a_crash_and_not_an_abort`,
//!   against a real subprocess.
//! * **`async-lsp`'s reader.** Nothing here calls it, so a panic inside
//!   `Message::read` is invisible to this target. That is upstream code and the
//!   reason `Bounded` sits underneath it.
//! * **Anything needing a child process** — spawn failures, the ready timeout,
//!   `ServerState`'s transitions. `tests/lsp.rs` owns those; they are not wire
//!   decode.
//! * **A body over ~4 KiB with an honest length**, and therefore the
//!   multi-megabyte responses a real rename produces.

#![no_main]

use std::io::Cursor;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use libfuzzer_sys::fuzz_target;
use phosphor_buffer::lsp::{
    Bounded, Completion, FrameScan, MAX_FRAME_BYTES, MAX_HEADER_BYTES, completions_from_lsp,
    diagnostic_from_lsp, file_edits_from_lsp, hover_prose, locations_from_lsp, lsp_types, narrow,
    signature_from_lsp, sync_kind,
};
use serde_json::Value;
use tokio::io::{AsyncRead, ReadBuf};

/// The read sizes law 1 compares. `1` is the one that matters — a pipe under
/// load hands over a byte at a time and that is when a per-chunk bug appears —
/// and `4096` is a whole input in one call, which is when it does not.
const READ_SIZES: [usize; 4] = [1, 3, 64, 4096];

/// Bodies the lenient scan will pull out of one input before giving up.
///
/// Only a bound on work: an input of nothing but empty frames would otherwise
/// spend the exec in `serde_json`, and the failures live between the first few
/// messages rather than the four-thousandth.
const MAX_BODIES: usize = 32;

/// The text every position is converted against.
///
/// Deliberately awkward: a CRLF line, a line whose content genuinely ends in
/// `\r` (the ambiguity `line_at`'s header records), an astral character so
/// UTF-16 units and characters disagree, and an empty last line. A server's
/// column is measured against *this*, and against `""` as well, because a file
/// the client has no text for is the common case for a go-to-definition target.
const TEXT: &str = "fn main() {\r\n    let 🦀 = \"a\r\";\n}\n";

// ---------------------------------------------------------------------------
// Law 1 — the scanner is total, and chunk boundaries do not exist
// ---------------------------------------------------------------------------

/// Runs `data` through the shipping [`Bounded`] adapter, `read_size` bytes at a
/// time, and reports only whether it refused.
///
/// Driven with a no-op waker rather than a runtime: a `Cursor` never pends, so
/// there is nothing to wake, and starting a `tokio` runtime per exec would cost
/// more than the read. This is the real `poll_read` — including the
/// `buffer.filled()[before..]` slice that decides which bytes the scanner is
/// shown, which is the part of `Bounded` a copy would not have.
fn through_bounded(data: &[u8], read_size: usize) -> bool {
    let mut reader = Bounded::new(Cursor::new(data));
    let mut context = Context::from_waker(Waker::noop());
    let mut store = vec![0_u8; read_size];
    loop {
        let mut buffer = ReadBuf::new(&mut store);
        match Pin::new(&mut reader).poll_read(&mut context, &mut buffer) {
            Poll::Ready(Ok(())) if buffer.filled().is_empty() => return false,
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(_)) => return true,
            Poll::Pending => unreachable!("a Cursor never pends"),
        }
    }
}

// ---------------------------------------------------------------------------
// Law 2 — no false refusal, and no desync
// ---------------------------------------------------------------------------

/// The input as `async-lsp` 0.2.4's `Message::read` would frame it, or [`None`]
/// if that reader would have stopped anywhere before the last byte.
///
/// Pinned to that version and to that function, whose rules are stricter than
/// the scanner's and are what makes this a *precondition* rather than a second
/// opinion: `\r\n` terminators, a literal `": "` between name and value, a
/// value that parses, the **last** `Content-Length` winning, and the body read
/// by count. Anything it refuses is traffic no server should send, so the
/// scanner may answer however it likes and this returns [`None`].
///
/// [`None`] as well for either cap, because past those the scanner is *supposed*
/// to refuse and law 2 is about the traffic it must not refuse.
///
/// Each frame is returned as the offset it ends at and the body it carried.
fn framed(data: &[u8]) -> Option<Vec<(usize, &[u8])>> {
    let mut frames = Vec::new();
    let mut at = 0;
    loop {
        if at == data.len() {
            return Some(frames);
        }
        let mut declared: Option<u64> = None;
        loop {
            let rest = &data[at..];
            // `read_line` stops after a `\n`; without one it hits EOF mid-header
            // and the connection ends, which is not a stream to hold the
            // scanner to.
            let end = rest.iter().position(|byte| *byte == b'\n')? + 1;
            let line = &rest[..end];
            if line.len() > MAX_HEADER_BYTES {
                return None;
            }
            at += end;
            // Into a `String`, so a header that is not UTF-8 is an io error.
            let line = std::str::from_utf8(line).ok()?;
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.strip_suffix("\r\n")?.split_once(": ")?;
            if name.eq_ignore_ascii_case("Content-Length") {
                let value = value.parse::<u64>().ok()?;
                if value > MAX_FRAME_BYTES {
                    return None;
                }
                declared = Some(value);
            }
        }
        let want = usize::try_from(declared?).ok()?;
        let end = at.checked_add(want)?;
        if end > data.len() {
            return None;
        }
        frames.push((end, &data[at..end]));
        at = end;
    }
}

// ---------------------------------------------------------------------------
// Law 3 — decode is total
// ---------------------------------------------------------------------------

/// Every body the input plausibly contains — **for reach, not for a law**.
///
/// Deliberately lenient where [`framed`] is strict, and the two must not be
/// confused: this one recovers after a header block it did not like and takes
/// what is left when a length promises more than there is, so that a mutation
/// which breaks the framing still delivers its body to the converters. A strict
/// framer would make law 3 apply to almost nothing, because the first mutated
/// digit of a `Content-Length` ends the stream.
///
/// An input with no header block at all is one body: that is what a mutation of
/// a JSON payload looks like, and refusing it would waste the corpus.
fn bodies(data: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut at = 0;
    while out.len() < MAX_BODIES {
        let Some(gap) = find(&data[at..], b"\r\n\r\n") else {
            break;
        };
        let head = &data[at..at + gap];
        let start = at + gap + 4;
        let want = content_length(head).unwrap_or(usize::MAX);
        let end = start.saturating_add(want).min(data.len());
        out.push(&data[start..end]);
        // `start` is past `at` by at least four, so this always advances.
        at = end;
    }
    if out.is_empty() {
        out.push(data);
    }
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The last `Content-Length` in a header block, read leniently.
fn content_length(head: &[u8]) -> Option<usize> {
    head.split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let colon = line.iter().position(|byte| *byte == b':')?;
            line[..colon]
                .eq_ignore_ascii_case(b"content-length")
                .then(|| std::str::from_utf8(&line[colon + 1..]).ok())?
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .next_back()
}

/// A file's text, as the client supplies it: the buffers it has open answer,
/// and everything else is `None`.
///
/// Both arms are exercised on purpose — `None` is `""`, which is the case
/// `file_span_from_lsp`'s header calls out as approximate rather than wrong.
fn text_of(path: &Path) -> Option<String> {
    path.extension()
        .is_some_and(|extension| extension == "rs")
        .then(|| TEXT.to_owned())
}

/// One body, decoded the way the client decodes it, and converted the way the
/// client converts it.
fn decode(body: &[u8]) {
    let Ok(message) = serde_json::from_slice::<Value>(body) else {
        return;
    };

    // The notification half. `ingest` does exactly this with the params, once
    // per diagnostic, against whatever text the client last saw. Any `params`
    // that deserialises is taken, without checking `method` — the shape is the
    // gate the `Router` would apply anyway, and a mutation that corrupts the
    // method name should not cost the exec its diagnostics.
    if let Some(params) = message.get("params")
        && let Ok(params) =
            serde_json::from_value::<lsp_types::PublishDiagnosticsParams>(params.clone())
    {
        for text in [TEXT, ""] {
            for diagnostic in &params.diagnostics {
                drop(diagnostic_from_lsp(text, diagnostic));
            }
        }
    }

    // The response half: the shapes `look_up` and `answer` ask for. A `result`
    // is tried as each of them because nothing in the payload says which
    // request it answers — an id nothing asked for is exactly this case, and
    // the converters have to be total against a shape meant for another one.
    let Some(result) = message.get("result") else {
        return;
    };

    if let Ok(response) = serde_json::from_value::<lsp_types::CompletionResponse>(result.clone()) {
        let items = completions_from_lsp(&response);
        let prefix: String = items
            .first()
            .map(|item: &Completion| item.label.chars().take(2).collect())
            .unwrap_or_default();
        drop(narrow(items.clone(), &prefix));
        drop(narrow(items, ""));
    }
    if let Ok(help) = serde_json::from_value::<lsp_types::SignatureHelp>(result.clone()) {
        drop(signature_from_lsp(&help));
    }
    if let Ok(hover) = serde_json::from_value::<lsp_types::Hover>(result.clone()) {
        drop(hover_prose(&hover.contents));
    }
    if let Ok(places) = serde_json::from_value::<lsp_types::GotoDefinitionResponse>(result.clone())
    {
        drop(locations_from_lsp(&places, &text_of));
    }
    if let Ok(edit) = serde_json::from_value::<lsp_types::WorkspaceEdit>(result.clone()) {
        drop(file_edits_from_lsp(&edit, &text_of));
    }
    if let Ok(ready) = serde_json::from_value::<lsp_types::InitializeResult>(result.clone()) {
        let _ = sync_kind(ready.capabilities.text_document_sync.as_ref());
    }
}

fuzz_target!(|data: &[u8]| {
    // Law 1 — one verdict, whatever the pipe chose.
    let verdicts: Vec<bool> = READ_SIZES
        .iter()
        .map(|size| through_bounded(data, *size))
        .collect();
    assert!(
        verdicts.iter().all(|refused| *refused == verdicts[0]),
        "the scanner's verdict changed with the read size: {READ_SIZES:?} gave {verdicts:?}"
    );

    // Law 2 — no false refusal, and no desync.
    if let Some(frames) = framed(data) {
        let mut scan = FrameScan::new();
        let mut at = 0;
        for (end, _) in &frames {
            scan.inspect(&data[at..*end])
                .expect("a frame async-lsp itself would read, under both caps, was refused");
            assert!(
                !scan.mid_frame(),
                "the scanner is still inside a body at offset {end}, where async-lsp is \
                 between frames — from here it reads a payload as headers"
            );
            at = *end;
        }
        assert_eq!(at, data.len(), "framed() must consume the whole input");
        assert!(
            !verdicts[0],
            "the whole input framed cleanly and the scanner still refused it"
        );
    }

    // Law 3 — decode is total.
    for body in bodies(data) {
        decode(body);
    }
});
