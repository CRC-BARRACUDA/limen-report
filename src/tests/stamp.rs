//! Stamping a report with a hash: what is hashed, what the pages say, and
//! what is written beside the file.

use crate::*;
use limen_sdk_rust::json;

fn spec(rows: usize) -> Value {
    json!({
        "title": "Звіт про пристрої",
        "subtitle": "WS-ALPHA · 2026-09-29",
        "summary": ["Пристроїв: 78", "Під'єднано: 66"],
        "charts": [{ "title": "За категорією", "data": [{"label": "usb", "value": 41.0}, {"label": "pci", "value": 22.0}]}],
        "sections": [{
            "heading": "Під'єднано зараз",
            "columns": ["Категорія", "Серійний номер"],
            "rows": (0..rows).map(|i| json!(["usb", format!("SN{i:08}")])).collect::<Vec<_>>(),
        }],
    })
}

fn sha256() -> HashOption {
    HashOption { key: "sha256".into(), label: "SHA-256".into() }
}

const HASH: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// The algorithms are the hash module's, as it lists them.
#[test]
fn the_algorithms_offered_are_the_hash_module_s() {
    let answer = json!({ "algorithms": [
        { "name": "sha256", "label": "SHA-256", "hex_digits": 64 },
        { "name": "kupyna256", "label": "Kupyna-256", "hex_digits": 64 },
        { "name": "", "label": "nameless" },
        { "label": "no name at all" },
    ]});
    assert_eq!(
        options_from(&answer),
        [sha256(), HashOption { key: "kupyna256".into(), label: "Kupyna-256".into() }]
    );
    assert!(options_from(&json!({})).is_empty());
    assert!(options_from(&Value::Null).is_empty());
}

/// The dropdown answers with the label it showed; "None" is none in either
/// language, and something no longer on offer is said to be gone.
#[test]
fn the_hash_is_picked_from_the_list_or_not_at_all() {
    let options = [sha256(), HashOption { key: "kupyna256".into(), label: "Kupyna-256".into() }];
    let chosen = |said: &str| hashing_choice(&json!({ "hash": said }), &options);
    assert_eq!(chosen(""), Hashing::No);
    assert_eq!(chosen("None"), Hashing::No);
    assert_eq!(chosen("Немає"), Hashing::No);
    assert_eq!(hashing_choice(&json!({}), &options), Hashing::No);
    assert_eq!(chosen("SHA-256"), Hashing::With(sha256()));
    assert_eq!(chosen(" Kupyna-256 "), Hashing::With(options[1].clone()));
    assert_eq!(chosen("Whirlpool"), Hashing::Gone("Whirlpool".into()));
    // The hash module removed since the list was shown.
    assert_eq!(hashing_choice(&json!({ "hash": "SHA-256" }), &[]), Hashing::Gone("SHA-256".into()));
}

/// What is hashed is what the report says — and nothing of how it is printed,
/// what it is called, or who signed it.
#[test]
fn the_content_is_what_the_report_says_and_not_how_it_is_printed() {
    let plain = content_text(&spec(3));
    let mut dressed = spec(3);
    dressed["theme"] = json!("dark");
    dressed["format"] = json!("pdf");
    dressed["file_name"] = json!("WS-ALPHA_devices");
    dressed["sign"] = json!("Olena Koval");
    dressed["hash"] = json!("SHA-256");
    dressed["lang"] = json!("uk");
    dressed["content_hash"] = json!({ "algorithm": "SHA-256", "hash": HASH });
    assert_eq!(content_text(&dressed), plain);

    // Anything it says, changed, is other content.
    for (what, change) in [
        ("the title", json!({ "title": "Another" })),
        ("a summary line", json!({ "summary": ["Пристроїв: 79", "Під'єднано: 66"] })),
        ("a figure", json!({ "charts": [{ "title": "За категорією", "data": [{"label": "usb", "value": 42.0}]}] })),
    ] {
        let mut other = spec(3);
        for (k, v) in change.as_object().unwrap() {
            other[k] = v.clone();
        }
        assert_ne!(content_text(&other), plain, "{what}");
    }
    let mut other = spec(3);
    other["sections"][0]["rows"][1][1] = json!("SN00000009");
    assert_ne!(content_text(&other), plain, "one cell");
}

/// The same content is the same text whoever writes it down: the keys of every
/// object in order, no spaces, the alphabet as it is.
#[test]
fn the_content_is_written_one_way() {
    let a = json!({ "title": "T", "summary": ["x"], "sections": [{ "rows": [["1"]], "heading": "H", "columns": ["c"] }] });
    let b = json!({ "sections": [{ "columns": ["c"], "heading": "H", "rows": [["1"]] }], "summary": ["x"], "title": "T" });
    assert_eq!(content_text(&a), content_text(&b));
    assert_eq!(
        content_text(&a),
        r#"{"sections":[{"columns":["c"],"heading":"H","rows":[["1"]]}],"summary":["x"],"title":"T"}"#
    );
    let text = content_text(&spec(1));
    // Cyrillic as it is, not as escapes: the text is UTF-8, and is hashed as that.
    assert!(text.contains("Звіт про пристрої"), "{text}");
    assert!(text.starts_with(r#"{"charts":[{"data":[{"label":"usb","value":41.0}"#), "{text}");
    // A quote or a line break inside a cell stays inside it.
    let tricky = json!({ "title": "a \"quoted\"\nline" });
    assert_eq!(content_text(&tricky), r#"{"title":"a \"quoted\"\nline"}"#);
}

/// A stamped report carries the hash of its content at the foot of every
/// page, whole; one that is not stamped says nothing, and a caller cannot
/// make it say so.
#[test]
fn a_stamped_report_carries_its_content_hash_on_every_page() {
    let font = pdf::font().unwrap();
    let stamp = Stamp { option: sha256(), hash: HASH.into() };
    let canon = stamped(canonical(&spec(400), "en", None), Some(&stamp));
    assert_eq!(pdf::layout::hash_line(&canon, "en").unwrap(), format!("SHA-256 of this report's content · {HASH}"));
    assert!(pdf::layout::hash_line(&stamped(canonical(&spec(3), "uk", None), Some(&stamp)), "uk").unwrap().starts_with("SHA-256 вмісту цього звіту"));

    let long = pdf::layout::render(&canon, font, "2026-09-29 12:00 UTC");
    assert!(long.pages > 2);
    let text = String::from_utf8_lossy(&long.bytes).into_owned();
    assert_eq!(text.matches(&font.encode(HASH)).count(), long.pages, "once a page, and all of it");

    // Not stamped: nothing of it. And a hash the caller wrote is dropped.
    let plain = pdf::layout::render(&canonical(&spec(3), "en", None), font, "2026-09-29 12:00 UTC");
    assert!(!String::from_utf8_lossy(&plain.bytes).contains(&font.encode(HASH)));
    let mut forged = spec(3);
    forged["content_hash"] = json!({ "algorithm": "SHA-256", "hash": HASH });
    assert!(canonical(&forged, "en", None).get("content_hash").is_none());
    let doc = pdf::layout::render(&stamped(canonical(&forged, "en", None), None), font, "2026-09-29 12:00 UTC");
    assert!(!String::from_utf8_lossy(&doc.bytes).contains(&font.encode(HASH)), "a caller cannot stamp for the module");
}

/// A long hash is set smaller, never cut short: a hash with its end missing
/// is not a hash.
#[test]
fn a_long_hash_is_set_smaller_and_never_cut() {
    let font = pdf::font().unwrap();
    let sha512 = "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f";
    let stamp = Stamp { option: HashOption { key: "sha512".into(), label: "SHA-512".into() }, hash: sha512.into() };
    let canon = stamped(canonical(&spec(3), "en", None), Some(&stamp));
    let line = pdf::layout::hash_line(&canon, "en").unwrap();
    assert!(line.ends_with(sha512));

    let width = 595.0 - 2.0 * 42.0;
    let size = pdf::layout::size_to_fit(font, &line, 6.5, width);
    assert!(size < 6.5, "it does not fit at the footer's size, so it is set smaller: {size}");
    assert!(size > 4.5, "and not so small as to be unreadable: {size}");
    assert!(font.width("0", size) * line.chars().count() as f32 <= width + 0.01);
    // What fits is left alone.
    assert_eq!(pdf::layout::size_to_fit(font, "short", 6.5, width), 6.5);

    let doc = pdf::layout::render(&canon, font, "2026-09-29 12:00 UTC");
    assert!(String::from_utf8_lossy(&doc.bytes).contains(&font.encode(sha512)), "the whole hash is on the page");
}

/// Signed and stamped, a page says both — and the second line has room of its
/// own rather than being printed off the foot of the sheet.
#[test]
fn a_report_can_be_signed_and_stamped() {
    let font = pdf::font().unwrap();
    let me = limen_sdk_rust::GpgIdentity {
        user_id: "Olena Koval <olena@example.invalid>".into(),
        fingerprint: "8D151FD231907502BBFA528E140ABA21244518B8".into(),
        default: false,
    };
    let stamp = Stamp { option: sha256(), hash: HASH.into() };
    let both = pdf::layout::render(&stamped(canonical(&spec(400), "en", Some(&me)), Some(&stamp)), font, "2026-09-29 12:00 UTC");
    let text = String::from_utf8_lossy(&both.bytes).into_owned();
    assert_eq!(text.matches(&font.encode(HASH)).count(), both.pages);
    assert_eq!(text.matches(&font.encode("8D15 1FD2 3190 7502 BBFA 528E 140A BA21 2445 18B8")).count(), both.pages);

    // The room is taken from the body, so a long report is no shorter in pages
    // for carrying one more line — and one line alone costs nothing.
    let signed_only = pdf::layout::render(&canonical(&spec(400), "en", Some(&me)), font, "2026-09-29 12:00 UTC");
    let stamped_only = pdf::layout::render(&stamped(canonical(&spec(400), "en", None), Some(&stamp)), font, "2026-09-29 12:00 UTC");
    let neither = pdf::layout::render(&canonical(&spec(400), "en", None), font, "2026-09-29 12:00 UTC");
    assert_eq!(signed_only.pages, neither.pages);
    assert_eq!(stamped_only.pages, neither.pages);
    assert!(both.pages >= neither.pages);
}

/// The file's own hash is written beside it, under its whole name, in the
/// line `sha256sum -c` reads.
#[test]
fn the_file_s_hash_is_written_beside_it_as_a_checksum_line() {
    let doc = std::path::Path::new("/reports/WS-ALPHA_devices_2026-09-29.pdf");
    assert_eq!(sidecar_path(doc, &sha256()), std::path::Path::new("/reports/WS-ALPHA_devices_2026-09-29.pdf.sha256"));
    let kupyna = HashOption { key: "kupyna256".into(), label: "Kupyna-256".into() };
    assert_eq!(sidecar_path(doc, &kupyna), std::path::Path::new("/reports/WS-ALPHA_devices_2026-09-29.pdf.kupyna256"));
    assert_eq!(sidecar_text(HASH, doc), format!("{HASH}  WS-ALPHA_devices_2026-09-29.pdf\n"));
}

/// The choice is offered only when there is a module to do the arithmetic,
/// and is the reader's — so it does not travel with the Save button.
#[test]
fn the_preview_offers_a_hash_only_when_one_can_be_made() {
    let find = |v: &Value, id: &str| -> Option<Value> {
        v["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|w| w["kind"] == "row")
            .flat_map(|w| w["children"].as_array().cloned().unwrap_or_default())
            .find(|c| c["id"] == id)
    };
    let without = render_view_full(&spec(3), "en", &[], &[]);
    assert!(find(&without, "hash").is_none(), "no hash module, no dropdown");

    let options = [sha256(), HashOption { key: "kupyna256".into(), label: "Kupyna-256".into() }];
    for (lang, none) in [("en", "None"), ("uk", "Немає")] {
        let with = render_view_full(&spec(3), lang, &[], &options);
        let hash = find(&with, "hash").expect("the dropdown");
        assert_eq!(hash["options"], json!([none, "SHA-256", "Kupyna-256"]), "{lang}");
        assert_eq!(hash["default"], none, "{lang}: not stamped unless asked");
    }
    // A hash the caller's spec asks for does not ride on the button: the
    // dropdown is what says.
    let mut asked = spec(3);
    asked["hash"] = json!("SHA-256");
    let view = render_view_full(&asked, "en", &[], &options).to_string();
    assert!(!view.contains(r#""hash":"SHA-256""#), "{view}");
}

/// The controls stand on short rows rather than one long one. Side by side
/// they were wider than the pop-up as soon as a key's user id was a name and
/// an address, and a row wider than its window pushed the whole preview out
/// past both edges — title, figures and the Close button with it.
#[test]
fn the_preview_s_controls_are_not_all_on_one_row() {
    let me = limen_sdk_rust::GpgIdentity {
        user_id: "Olena Koval (work laptop, second token) <olena.koval@example.invalid>".into(),
        fingerprint: "8D151FD231907502BBFA528E140ABA21244518B8".into(),
        default: false,
    };
    let view = render_view_full(&spec(3), "uk", &[me], &[sha256()]);
    let rows: Vec<Vec<String>> = view["widgets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["kind"] == "row")
        .map(|w| {
            w["children"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["id"].as_str().or(c["text"].as_str()).unwrap_or("").to_string())
                .collect()
        })
        .collect();
    // The signature is as wide as the longest user id, so it has a row to
    // itself; the two short choices share one; the buttons come last.
    assert_eq!(rows[0], ["theme", "hash"]);
    assert_eq!(rows[1], ["sign"]);
    assert_eq!(rows[2].len(), 2, "Save and Close: {:?}", rows[2]);
    // Nothing is lost for being rearranged.
    assert_eq!(super::controls(&view).len(), 5);
}
