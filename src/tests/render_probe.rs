//! Rendering a real spec from a file, for looking at.

use crate::*;

/// Render a real spec from a file, for looking at: drop a caller's spec
/// next to this and see what the document actually comes out like.
///
/// Both paths are in the platform's own temp directory rather than `/tmp`,
/// which does not exist on Windows — and `LIMEN_SPEC` overrides the input,
/// so a spec kept anywhere can be rendered without editing this.
#[test]
#[ignore = "reads spec.json from the temp dir (or $LIMEN_SPEC); run with --ignored"]
fn render_spec_file() {
    let input = std::env::var_os("LIMEN_SPEC")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("spec.json"));
    let text = std::fs::read_to_string(&input).unwrap_or_else(|e| {
        // Which file, and that it is the input rather than the module,
        // because this test is usually run by somebody who has just put a
        // spec somewhere and wants to see it.
        panic!("no spec to render at {}: {e}", input.display())
    });
    let spec: Value = text.parse().unwrap();
    let font = pdf::font().unwrap();
    let doc = pdf::layout::render(&spec, font, &today());
    let out = std::env::temp_dir().join("spec.pdf");
    std::fs::write(&out, &doc.bytes).unwrap();
    println!("wrote {} — {} pages", out.display(), doc.pages);
}
