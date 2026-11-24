//! The in-app view of a report.

use crate::*;
use limen_sdk_rust::json;

pub(super) fn sample() -> Value {
    json!({
        "title": "Device Report",
        "subtitle": "today",
        "summary": ["77 devices", "66 connected"],
        "charts": [ { "title": "By category", "data": [
            {"label": "usb", "value": 41}, {"label": "pci", "value": 18}
        ] } ],
        "sections": [ { "heading": "Connected", "columns": ["A", "B"], "rows": [["x", "y"]] } ]
    })
}

#[test]
fn view_has_title_chart_and_table() {
    let v = render_view(&sample(), "en");
    assert_eq!(v["title"], "Device Report");
    let kinds: Vec<&str> = v["widgets"].as_array().unwrap().iter()
        .map(|w| w["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"chart"));
    assert!(kinds.contains(&"table"));
}
