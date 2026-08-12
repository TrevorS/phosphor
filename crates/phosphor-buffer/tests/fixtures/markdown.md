# T083 fixture — Markdown

Phosphor's transcript renders markdown, so the fixture carries the constructs the
transcript actually meets: fenced code with an info string, tables, task lists,
nested quotes and reference links.

## Lists and emphasis

- **bold**, *italic*, `inline code`, and a [link](https://example.invalid).
- Nested:
  1. first
  2. second
     - deep
- [ ] unchecked
- [x] checked

## A fenced block

```rust
fn main() {
    println!("{}", "✻");
}
```

```
no info string
```

## Table

| Language | Crate | ABI |
|---|---:|:--:|
| Rust | `tree-sitter-rust` | 15 |
| Steel | `tree-sitter-scheme` | 14 |

## Quote and rule

> A quote
> > nested one level
>
> back to one

---

Setext heading
==============

Term with a footnote[^1] and a reference link [spec][ts].

[^1]: the footnote body.
[ts]: https://tree-sitter.github.io "tree-sitter"

<div align="center">raw html block</div>

Trailing hard break at line end  
and the continuation.
