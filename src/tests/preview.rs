//! The preview pop-up, and the controls on it.

use crate::*;
use limen_sdk_rust::json;

fn spec() -> Value {
    json!({
        "title": "Device Report",
        "subtitle": "78 devices · 66 connected",
        "summary": ["Total: 78"],
        "sections": [{ "columns": ["A"], "rows": (0..90).map(|i| json!([i.to_string()])).collect::<Vec<_>>() }],
    })
}

/// The view a caller opens is the preview, and it is a pop-up over the
/// screen that asked for it — a report is something you look at and then
/// decide about, and the module underneath is what you decide to go back to.
#[test]
fn the_view_is_a_pop_up_that_previews_the_document() {
    let view = render_view(&spec(), "en");
    assert_eq!(view["modal"], "report.preview", "not a pop-up");
    assert!(view["modal_width"].as_f64().unwrap() > 400.0);

    let text = view.to_string();
    assert!(text.contains("Device Report"), "the title");
    assert!(text.contains("GPL-3.0-or-later"), "the line every page is footed with");
    assert!(text.contains("JetBrains Mono"), "the face it is set in");
    assert!(text.contains("A4"));
    assert!(text.contains("\"text\":\"Save\""));
}

/// The ground is chosen in the pop-up, not decided by the button.
///
/// The host merges a button's args **over** the view's inputs, so a theme
/// carried by the button would quietly win over whatever the dropdown says
/// — the choice would be on screen and have no effect.
#[test]
fn the_dropdown_owns_the_choice_of_ground() {
    let view = render_view(&spec(), "en");
    let children = super::controls(&view);

    let picker = children
        .iter()
        .find(|c| c["id"] == "theme")
        .expect("a ground picker");
    assert_eq!(picker["kind"], "select");
    let options: Vec<&str> = picker["options"].as_array().unwrap()
        .iter().map(|o| o.as_str().unwrap()).collect();
    assert_eq!(options, vec!["White", "Black"]);

    let save = children.iter().find(|c| c["text"] == "Save").expect("the save button");
    assert_eq!(save["action"]["capability"], "report.build");
    assert!(save["args"]["theme"].is_null(), "the button must not carry a ground");

    // Signing is the reader's to ask for, beside it: off unless they do.
    // With no key to sign with, "None" is all there is to pick.
    let sign = children.iter().find(|c| c["id"] == "sign").expect("a signature picker");
    assert_eq!(sign["kind"], "select");
    assert_eq!(sign["options"], json!(["None"]));
    assert_eq!(sign["default"], "None");
    assert!(save["args"]["sign"].is_null(), "nor an answer about signing");

    // And a caller that asked for one gets the dropdown pointing at it.
    let mut dark = spec();
    dark["theme"] = json!("dark");
    let view = render_view(&dark, "en");
    let text = view.to_string();
    assert!(text.contains("\"default\":\"Black\""), "{text}");
}

/// Both spellings of each ground mean the same thing: what a module sets
/// in a spec, and what the dropdown sends.
#[test]
fn the_dropdown_and_a_spec_agree_on_what_dark_means() {
    let font = pdf::font().unwrap();
    let ground = format!("0 0 {} {} re f", pdf::writer::PAGE_W, pdf::writer::PAGE_H);
    for word in ["dark", "Dark", "Black"] {
        let mut s = spec();
        s["theme"] = json!(word);
        let bytes = pdf::layout::render(&s, font, "now").bytes;
        assert!(
            String::from_utf8_lossy(&bytes).contains(&ground),
            "{word} did not paint the page"
        );
    }
    for word in ["light", "White", ""] {
        let mut s = spec();
        s["theme"] = json!(word);
        let bytes = pdf::layout::render(&s, font, "now").bytes;
        assert!(
            !String::from_utf8_lossy(&bytes).contains(&ground),
            "{word} painted the page"
        );
    }
}

/// The pop-up shows a sample of a long table and says so. A preview that
/// quietly shows twelve of four hundred rows is a preview of a different
/// document.
#[test]
fn a_long_table_is_sampled_and_says_so() {
    let view = render_view(&spec(), "en");
    let table = view["widgets"].as_array().unwrap()
        .iter().find(|w| w["kind"] == "table").unwrap();
    assert_eq!(table["rows"].as_array().unwrap().len(), PREVIEW_ROWS);
    assert!(view.to_string().contains("and 78 more row(s)"), "it did not say so");

    // A short table is shown whole, with nothing said about it.
    let small = json!({
        "title": "t",
        "sections": [{ "columns": ["A"], "rows": [["1"], ["2"]] }],
    });
    let text = render_view(&small, "en").to_string();
    assert!(!text.contains("more row(s)"), "{text}");
}

/// Closing the pop-up calls nobody: there is nothing to cancel remotely.
#[test]
fn close_just_closes() {
    let view = render_view(&spec(), "en");
    let controls = super::controls(&view);
    let close = controls.last().unwrap();
    assert_eq!(close["dismiss"], true, "{close}");
}

/// The number of sheets is the document's own, from laying it out — a
/// preview that says two pages and prints nine is worse than one that says
/// nothing at all.
#[test]
fn the_page_count_is_the_real_one() {
    let font = pdf::font().unwrap();
    let real = pdf::layout::render(&spec(), font, &today()).pages;
    assert!(real >= 2, "ninety rows came to {real} page(s)");
    let text = render_view(&spec(), "en").to_string();
    assert!(text.contains(&format!("{real} pages")), "said something else: {text}");

    // One page says "1 page", not "1 pages".
    let small = json!({ "title": "t", "sections": [{ "columns": ["A"], "rows": [["1"]] }] });
    assert!(render_view(&small, "en").to_string().contains("1 page ·"));
}

/// Pressing the button is the same call the caller could have made itself —
/// the preview adds nothing the export has to be told separately.
#[test]
fn the_button_carries_the_whole_spec() {
    let view = render_view(&spec(), "en");
    let controls = super::controls(&view);
    let save = controls.iter().find(|c| c["text"] == "Save").expect("the save button");
    assert_eq!(save["action"]["capability"], "report.build");
    assert_eq!(save["action"]["method"], "build");
    // Every row travels with it — the ninety the preview trimmed to twelve
    // included. The export does not depend on this module having
    // remembered the scan.
    assert_eq!(save["args"]["sections"][0]["rows"].as_array().unwrap().len(), 90);
}
