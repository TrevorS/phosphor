//! `T021`'s acceptance criterion, as a test.
//!
//! > *"A **broken `init.scm` boots the editor anyway**, with the error in a
//! > float. … **Done when:** a syntax error in `init.scm` yields a working
//! > editor with a legible error float."* — `docs/TASKS.md`, `T021`
//!
//! Three claims, one per test, and each is checked against the **shipped**
//! `runtime/init.scm` with a mistake planted in it rather than against a
//! fixture written to pass:
//!
//! 1. The editor still constructs, and the good forms still ran.
//! 2. The error is *reachable* — file, line and Steel's own message, in the
//!    float, without opening anything else.
//! 3. A clean boot of the same tree opens no float at all, so the float means
//!    something when it appears.
//!
//! `CP-2`'s manual half — *"break `init.scm` on purpose. Does the editor still
//! boot, and is the error float actually readable?"* — is this, with eyes on it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use phosphor_core::view::{Float, Node};
use phosphor_steel::host::{Detached, Host};
use phosphor_steel::runtime::Runtime;

/// The `runtime/` tree this repo ships.
fn shipped() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime")
}

/// A throwaway copy of the shipped tree, so a test never edits the real one.
struct Planted(PathBuf);

impl Planted {
    /// Copies `runtime/` aside and appends `mistake` to its `init.scm`.
    fn new(name: &str, mistake: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "phosphor-t021-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("a temp runtime tree");

        for entry in fs::read_dir(shipped()).expect("the shipped runtime tree exists") {
            let entry = entry.expect("a readable directory entry");
            if entry.file_type().expect("a file type").is_file() {
                fs::copy(entry.path(), root.join(entry.file_name())).expect("a copied file");
            }
        }

        let init = root.join("init.scm");
        let mut source = fs::read_to_string(&init).expect("the shipped init.scm is readable");
        source.push_str(mistake);
        fs::write(&init, source).expect("a planted init.scm");

        Self(root)
    }

    fn boot(&self) -> Runtime {
        let host: Arc<dyn Host> = Arc::new(Detached);
        Runtime::boot(Some(&self.0), host)
    }
}

impl Drop for Planted {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Everything the float says, as a reader would see it.
fn readable(float: &Float) -> String {
    let mut text = String::new();
    if let Some(header) = &float.header {
        text.push_str(&header.left);
        if let Some(right) = &header.right {
            text.push(' ');
            text.push_str(right);
        }
        text.push('\n');
    }
    let Node::Spans { rows } = float.body.node() else {
        panic!(
            "the boot float's body is the spans hatch (T080), got {:?}",
            float.body.node().tag()
        );
    };
    for row in rows {
        for run in &row.runs {
            text.push_str(&run.text);
        }
        text.push('\n');
    }
    text
}

#[test]
fn a_syntax_error_in_init_scm_still_yields_an_editor() {
    // A stray closing paren: syntactically wrong, and — unlike a missing
    // opener — local enough that the scanner can carry on past it.
    let planted = Planted::new(
        "still-boots",
        "\n)\n(define phosphor/after-the-mistake 1)\n",
    );
    let mut runtime = planted.boot();

    assert!(
        !runtime.report().is_clean(),
        "the planted mistake should have been caught"
    );
    assert!(
        runtime.report().forms_ran() >= 2,
        "the shipped forms above the mistake still ran: {:?}",
        runtime.report()
    );

    // "A working editor": the VM is live, the vocabulary is installed, and the
    // form *after* the mistake ran too.
    assert!(
        runtime.global("phosphor/after-the-mistake").is_ok(),
        "the form below the mistake was discarded"
    );
    assert!(
        runtime.global("phosphor/boot-files").is_ok(),
        "the load order the shipped init.scm declares was discarded"
    );
    runtime
        .eval("(+ 1 2)")
        .expect("the VM still evaluates after a broken boot");
    runtime
        .eval("(close-float!)")
        .expect("the vocabulary is still installed after a broken boot");
}

#[test]
fn the_error_float_carries_the_file_the_line_and_the_message() {
    let planted = Planted::new("legible", "\n(define broken (+ 1 nonesuch))\n");
    let runtime = planted.boot();

    let float = runtime
        .boot_float()
        .expect("a fault opens the boot float — T021's whole point");
    let text = readable(&float);

    assert!(
        text.contains("init.scm:"),
        "no file:line in the float\n{text}"
    );

    let fault = runtime.report().faults.first().expect("one fault");
    let at = fault.at.expect("a fault inside a form knows where it is");
    assert!(
        text.contains(&format!("init.scm:{}:{}", at.line, at.column)),
        "the float does not name the position it recorded\n{text}"
    );
    assert!(
        text.contains(&fault.message),
        "steel's own message is not in the float\n{text}"
    );
    assert!(
        text.contains("(define broken (+ 1 nonesuch))"),
        "the offending source line is not in the float\n{text}"
    );
    assert!(
        text.contains("the editor is up"),
        "the float does not say the editor survived\n{text}"
    );
    assert!(
        float.footer.is_some(),
        "§4: every legal key, always visible — only the passive mood may skip the footer"
    );
}

#[test]
fn an_unclosed_form_is_reported_at_the_line_it_opened_on() {
    // The other shape of syntax error, and the one that costs the most: an
    // opener with no closer swallows the rest of the file, so the only useful
    // thing to say is where it started.
    let planted = Planted::new("unclosed", "\n(define oops\n");
    let runtime = planted.boot();

    let fault = runtime.report().faults.first().expect("one fault");
    assert_eq!(fault.label, "unterminated");
    let text = readable(&runtime.boot_float().expect("a fault opens the float"));
    assert!(text.contains("unterminated"), "{text}");
    assert!(text.contains("never closed"), "{text}");
}

#[test]
fn the_shipped_init_scm_opens_no_float_at_all() {
    // The float has to be silent when nothing is wrong, or it teaches nobody
    // anything when something is.
    let host: Arc<dyn Host> = Arc::new(Detached);
    let runtime = Runtime::boot(Some(&shipped()), host);

    assert!(
        runtime.report().is_clean(),
        "runtime/init.scm does not boot clean: {:?}",
        runtime.report().faults
    );
    assert!(runtime.boot_float().is_none());
    assert!(
        runtime.report().forms_ran() > 0,
        "runtime/init.scm ran no forms — the boot is not being exercised"
    );
}
