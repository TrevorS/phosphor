# `runtime/` — the editor layer

Not a crate. This is the Steel source tree: the editor itself, as opposed to the core
that hosts it (invariant 1 — Rust is the C, `runtime/*.scm` is the lisp, redefinable
from a live REPL).

What lands here, and when:

| Path | Contents | Task |
|---|---|---|
| `init.scm` | boot: load order, defaults | T033 |
| `keymaps.scm` | every binding, with counts and named registers | T033 |
| `leader.scm` | the leader tree behind which-key | T033 |
| `pickers/` | picker sources and columns | `store` |
| `permissions.scm`, `inbox.scm`, `watch.scm` | the directing surfaces | `agent` |

Placement test: *would two reasonable users want this to differ?* → here. *Can it
corrupt a buffer or drop a frame?* → Rust.

Steel composes primitives; it never defines them, and it never calls ratatui. It
returns a declarative view tree that `phosphor-ui` interprets (Q12).

Nothing is written yet — the files above are Window C and later.
