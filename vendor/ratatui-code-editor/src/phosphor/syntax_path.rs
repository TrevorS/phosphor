//! PHOSPHOR PATCH 12 — the named-node chain covering a byte, for `T042`'s
//! anchors. See `VENDOR.md`.
//!
//! # Why the fork and not the host
//!
//! [`Code`](crate::code::Code) owns the `Tree` and keeps it incrementally
//! up to date across every edit; the field is private and upstream exposes no
//! accessor. The host could parse a second time with its own `Parser`, and that
//! is the design this replaced: it means a second grammar table, a second parse
//! of every reanchored file, and — the part that actually decides it — a tree
//! that can disagree with the one the editor highlights from, because the
//! editor's is edited incrementally and a fresh parse is not.
//!
//! So the walk lives here, next to the tree it reads, and the seam is one
//! `pub fn` on `Code`. Nothing is mutated: this module is a pure read.
//!
//! # What a path is, and why it is this and not the child-index route
//!
//! The obvious fingerprint is the child-index path from the root — `[3, 1, 0]`.
//! It is exact, and it is worthless for the thing anchors exist for: inserting
//! one function above another shifts every index after it, so the anchor moves
//! to the wrong node on the most ordinary edit there is.
//!
//! What survives a rewrite is what a person would say out loud — *"`retry`, in
//! `impl Backoff`"*. So a path is the chain of **named** ancestors that carry a
//! `name` field, each as its kind plus that name:
//!
//! ```text
//!   impl Backoff {              ─┐
//!       fn retry(&self) {        ├─ [impl_item "Backoff", function_item "retry"]
//!           let x = 1;          ─┘
//!       }
//!   }
//! ```
//!
//! That is stable under reformatting, under reordering, under edits inside the
//! function, and under the whole construct moving anywhere in the file. It is
//! *not* stable under a rename, which is correct and deliberate: renaming
//! `retry` to `retry_with_backoff` makes it a different construct, and an
//! anchor that silently followed the rename would claim a person had seen code
//! they had not.
//!
//! # What is deliberately absent
//!
//! **Anonymous and unnamed nodes.** A path step for `block` or `{` carries no
//! information a rewrite preserves, and including them makes every path differ
//! from every other path for reasons nobody can act on.
//!
//! **Depth beyond the named chain.** Two anchors inside the same function share
//! a path. That is not a defect — it is the node tier being honest about its
//! resolution, and `T043`'s line-and-content tier is what separates them. A
//! fingerprint that pretended to more precision than the grammar gives would
//! resolve confidently to the wrong line.

use ropey::Rope;
use tree_sitter::{Node, Tree};

/// One step of a syntax path: a named node's kind, and the text of its `name`
/// field.
///
/// Both are owned `String`s because the path outlives the tree it was read
/// from — that is the entire point of it, since the tree is reparsed by the
/// rewrite the anchor has to survive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxStep {
    /// The grammar's node kind — `function_item`, `impl_item`, `class_definition`.
    pub kind: String,
    /// The text of the node's `name` field.
    pub name: String,
}

/// The named-node chain covering `byte`, outermost first.
///
/// Empty when the file has no grammar, when `byte` is past the end, or when
/// nothing covering it carries a name — a bare script with no functions is the
/// ordinary case, and it is exactly when `T043`'s tier takes over.
///
/// # Cost
///
/// One descent from the root, so depth-proportional and not size-proportional:
/// `Node::child_with_descendant` walks children at each level, and the tree is
/// already built. Callers build one path per anchored line, and reanchoring is
/// not on the frame path.
#[must_use]
pub fn path_at(tree: &Tree, source: &Rope, byte: usize) -> Vec<SyntaxStep> {
    if byte > source.len_bytes() {
        return Vec::new();
    }
    let mut steps = Vec::new();
    let mut node = tree.root_node();
    loop {
        if let Some(step) = step_of(node, source) {
            steps.push(step);
        }
        // Descend to the named child covering `byte`. `named_child` rather than
        // `child` so anonymous punctuation never ends the walk early.
        let mut next = None;
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.start_byte() <= byte && byte < child.end_byte() {
                next = Some(child);
                break;
            }
        }
        match next {
            Some(child) => node = child,
            None => break,
        }
    }
    steps
}

/// The fields a construct's identity is spelled in, in the order they are
/// joined.
///
/// **Three and not one, because `name` alone is a Rust-shaped assumption that
/// this module's own test caught.** `function_item`, `struct_item`,
/// `class_definition` and `function_definition` all carry `name`; `impl_item`
/// does not — its identity is the `type` field, with `trait` present only for
/// `impl Display for Backoff`. Taking all three in a fixed order keeps the walk
/// grammar-blind (it is a field-name list tried on every node, not a table of
/// node kinds) and it is what makes two impls of one type distinguishable:
/// `Display Backoff` and `Debug Backoff` are different paths, where `type`
/// alone would have collapsed them into one and moved an anchor between them.
const IDENTIFYING: [&str; 3] = ["name", "trait", "type"];

/// A step for `node`, or `None` when nothing in [`IDENTIFYING`] names it.
fn step_of(node: Node<'_>, source: &Rope) -> Option<SyntaxStep> {
    let mut parts: Vec<String> = Vec::new();
    for field in IDENTIFYING {
        let Some(child) = node.child_by_field_name(field) else {
            continue;
        };
        let (start, end) = (child.start_byte(), child.end_byte());
        if start > end || end > source.len_bytes() {
            continue;
        }
        let text = source.byte_slice(start..end).to_string();
        if !text.is_empty() {
            parts.push(text);
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(SyntaxStep {
        kind: node.kind().to_owned(),
        name: parts.join(" "),
    })
}
