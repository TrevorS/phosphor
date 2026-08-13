# Folds collapsing and expanding. CP-3's VHS list names this on its own
# line, separate from `3c`/`8e` (no single mockup id owns it — `8e` draws a
# fold as one of five buffer rows, but this tape is about the toggle itself,
# `za`/`zR`/`zM`, on real tree-sitter fold ranges).
#
# **Verified wired before capturing, not assumed:**
#   - `crates/phosphor-core/src/action.rs` declares `SetFold`/`FoldAll`/
#     `UnfoldAll` against `"T016"`; `crates/phosphor/src/main.rs:1964`,
#     `:1971`, `:1975` are the three match arms (current line numbers, read
#     this session — docs/TASKS.md's own citation, :1911/:1918/:1922, has
#     drifted off the tree since it was written, same class of drift
#     docs/TASKS.md's own "verification discipline" section warns about).
#   - `runtime/keymaps.scm:586-593` binds `za` (toggle), `zc` (close),
#     `zo` (open), `zM` (fold everything, level 0), `zR` (unfold everything)
#     — read directly this session.
#   - `crates/phosphor/tests/loop_pty.rs:566`
#     `driven::za_closes_the_fold_the_cursor_is_in` opens a real `.rs` fixture
#     on a pty, presses `za`, asserts the fold body left the frame, presses
#     `zR`, asserts it's back — green this session
#     (`cargo nextest run -p phosphor --test loop_pty driven::`) before this
#     tape was written.
#
# Fixture is planted inline (same pattern `broken-init.tape` and
# `insert-whitespace-marks.tape` use) rather than added under `fixtures/` —
# `V006`'s tree is out of this window's file lock (`tapes/**` only). A
# function body long enough that its fold header (`fn outer() {`) and its
# closing brace both stay on screen after collapse, so the "N lines" fold
# marker and its later disappearance on `zR` are both visible in one frame
# each without scrolling.
#
# Three screenshots off one session: before any fold action (everything
# open), after `za` (the cursor's fold closed, marker row visible, hidden
# lines gone from the frame), after `zR` (open again, byte-for-byte the same
# buffer as the first shot).
#
# Sentinel: the fold marker `▸⋯ N lines` is what distinguishes the closed
# frame from the open ones — more specific than the mode chip alone, which
# doesn't change across any of these three shots. Exact glyph run confirmed
# by reading `crates/phosphor-ui/src/soft_wrap.rs:546` and
# `crates/phosphor/tests/screen_8e.rs:551-561` this session — `▸` immediately
# followed by `⋯`, no space between them (docs/TASKS.md's own prose, "`▸ ⋯ n
# lines`", has a space that the rendered glyph does not — read the code, not
# the prose, per CLAUDE.md).

Output "artifacts/folds.gif"

Require phosphor

Source "_config.tape"

Set Width 1228
Set Height 700

Hide
Type "printf 'fn outer() {\n    let marker_inside_the_fold = 1;\n    let another = 2;\n    let third = 3;\n    let fourth = 4;\n}\n\nfn second() {\n    2\n}\n' > /tmp/phosphor-folds-fixture.rs && PHOSPHOR_RUNTIME=../runtime phosphor /tmp/phosphor-folds-fixture.rs"
Enter
Show
Wait+Screen@10s /(?m)^ (NORMAL|N)\b/
# Settle guard for the screenshot pipeline, not a sync primitive — see
# 1a.tape's header for the full finding and why this isn't V004's Sleep.
Sleep 500ms
Screenshot "artifacts/folds-open.png"

Type "za"
Wait+Screen@10s /▸⋯/
Sleep 500ms
Screenshot "artifacts/folds-closed.png"

Type "zR"
Wait+Screen@10s /marker_inside_the_fold/
Sleep 500ms
Screenshot "artifacts/folds-reopened.png"
