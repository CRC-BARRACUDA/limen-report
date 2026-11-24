//! How a line of figures is laid out.

use crate::*;
use limen_sdk_rust::json;

/// A label that came with its own colon does not open the value with a
/// second one — `From:: someone` was what a module's own screen label
/// looked like once the layout split the line.
#[test]
fn a_label_keeps_its_own_colon_out_of_the_value() {
    let font = pdf::font().unwrap();
    let spec = json!({
        "title": "t",
        "summary": ["From:: jsegui@soluafrica.test", "Score: 100/100"],
        "sections": [],
    });
    let bytes = pdf::layout::render(&spec, font, "now").bytes;
    let text = String::from_utf8_lossy(&bytes);
    // Drawn in pieces, because a figure's value wraps — so the run is
    // matched by its start rather than whole.
    assert!(text.contains(&font.encode("jsegui")), "the address was not drawn");
    assert!(
        !text.contains(&font.encode(": jsegui")),
        "the value still opens with a colon"
    );
    // And the label above it is the label, without one.
    assert!(text.contains(&font.encode("FROM")), "the label");
}
