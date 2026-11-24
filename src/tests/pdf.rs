//! The document: that it is a well-formed PDF, and says what it should.

use crate::*;
use limen_sdk_rust::json;

/// A spec with everything in it, and Cyrillic throughout: the alphabet is
/// the reason the font is embedded at all.
fn spec(rows: usize) -> Value {
    json!({
        "title": "Звіт про пристрої",
        "subtitle": "OkPrepareYourAss · 2026-09-29",
        "summary": ["Пристроїв: 78", "Під'єднано: 66", "Раніше: 12", "Хостів: 1"],
        "charts": [{ "title": "За категорією", "data": [
            {"label": "usb", "value": 41.0}, {"label": "pci", "value": 22.0}]}],
        "sections": [{
            "heading": "Під'єднано зараз",
            "columns": ["Категорія", "Тип", "Серійний номер"],
            "rows": (0..rows).map(|i| json!([
                "usb", "storage", format!("SN{i:08}")
            ])).collect::<Vec<_>>(),
        }],
    })
}

fn render(rows: usize) -> Vec<u8> {
    let font = pdf::font().expect("the embedded font parses");
    pdf::layout::render(&spec(rows), font, "2026-09-29 12:00 UTC").bytes
}

/// The four tables a PDF cannot place text without.
#[test]
fn the_embedded_font_reads() {
    let font = pdf::font().expect("the font this module ships parses");
    assert_eq!(font.units_per_em, 1000, "the width maths assumes a 1000 grid");
    // Latin and Cyrillic both resolve to real glyphs, and to different ones.
    let (a, de) = (font.glyph('A'), font.glyph('Д'));
    assert!(a > 0 && de > 0, "A={a} Д={de}");
    assert_ne!(a, de);
    // Monospaced: every advance is the same, which is what the column
    // arithmetic in the layout relies on.
    assert_eq!(font.advance(a), font.advance(de));
    assert!(font.advance(a) > 0);
    // A character the face does not have is glyph 0 — the box a reader
    // draws for "missing" — rather than a silently dropped one.
    assert_eq!(font.glyph('\u{10FFFF}'), 0);
}

/// Text is measured, not guessed: the layout cuts cells to fit using this.
#[test]
fn text_is_measured_in_points() {
    let font = pdf::font().unwrap();
    let w = font.width("12345", 10.0);
    assert!((w - font.width("abcde", 10.0)).abs() < 0.01, "monospaced");
    assert!(w > 0.0 && w < 100.0, "five characters at 10pt is {w}");
    assert!((font.width("aa", 10.0) - 2.0 * font.width("a", 10.0)).abs() < 0.01);
}

/// The file is a PDF that says where its objects are.
#[test]
fn the_document_is_well_formed() {
    let bytes = render(10);
    let head = String::from_utf8_lossy(&bytes[..16]).to_string();
    assert!(head.starts_with("%PDF-1.7"), "{head:?}");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Type /Catalog"));
    assert!(text.contains("/Type /Pages"));
    assert!(text.ends_with("%%EOF\n"));
    // The cross-reference offset at the end must point at the table.
    let tail = text.rsplit("startxref").next().unwrap();
    let at: usize = tail.trim().lines().next().unwrap().trim().parse().unwrap();
    assert!(bytes[at..].starts_with(b"xref"), "startxref points at {at}");
}

/// The font travels with the document, and so does the map back to
/// characters — without it the text draws and cannot be copied.
#[test]
fn the_font_and_its_unicode_map_are_embedded() {
    let bytes = render(4);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/FontFile2"), "the face itself");
    assert!(text.contains("/Subtype /CIDFontType2"));
    assert!(text.contains("/Encoding /Identity-H"));
    assert!(text.contains("/ToUnicode"));
    assert!(text.contains("beginbfchar"), "the glyph → character map");
    // Cyrillic went in, so Cyrillic is in the map: U+0417 З, the first
    // letter of the title.
    assert!(text.contains("<0417>"), "З is not mapped back");
}

/// A long table runs on to as many pages as it needs, and every one of
/// them carries the masthead and a footer — sheets get separated.
#[test]
fn a_long_report_paginates() {
    let one = render(5);
    let many = render(300);
    // `/Count n` in the page tree is the document's own answer.
    let pages_of = |b: &[u8]| -> usize {
        let t = String::from_utf8_lossy(b).to_string();
        let at = t.find("/Type /Pages /Count ").unwrap() + "/Type /Pages /Count ".len();
        t[at..].split_whitespace().next().unwrap().parse().unwrap()
    };
    assert_eq!(pages_of(&one), 1, "five rows fit on one page");
    let n = pages_of(&many);
    assert!(n >= 5, "three hundred rows came out as {n} page(s)");
    // Every page draws the marks and the footer: the licence line appears
    // once per page.
    let text = String::from_utf8_lossy(&many);
    assert!(
        text.matches("BT /F1 6.5 Tf").count() >= n * 2,
        "the footer is missing from some of the {n} pages"
    );
}

/// The licence is named on the page, not only in a file somewhere.
#[test]
fn the_footer_names_the_licence_and_the_font() {
    // The footer is drawn as glyph ids, so it is checked before encoding:
    // this is the string the layout is given.
    let font = pdf::font().unwrap();
    let bytes = pdf::layout::render(&spec(3), font, "2026-09-29 12:00 UTC").bytes;
    // GPL-3.0-or-later, as glyphs: find it by encoding it the same way.
    let want = font.encode("GPL-3.0-or-later");
    assert!(
        String::from_utf8_lossy(&bytes).contains(&want),
        "the licence is not on the page"
    );
    let font_licence = font.encode("SIL OFL 1.1");
    assert!(
        String::from_utf8_lossy(&bytes).contains(&font_licence),
        "the font's own licence is not on the page"
    );
}

/// The preview lists every way there is to sign: each key as each of its
/// user ids. Two tokens, two user ids each: four, told apart by the user
/// id and the end of the fingerprint. What is picked comes back as the
/// label shown, and leads to that key and that user id.
#[test]
fn the_signer_is_picked_from_a_list_of_keys_and_user_ids() {
    let id = |f: &str, u: &str| limen_sdk_rust::GpgIdentity { fingerprint: f.into(), user_id: u.into(), default: false };
    let (a, b) = ("46AC14F35EEB6798C506E9CB04F5252E9BD3FC8D", "8D151FD231907502BBFA528E140ABA21244518B8");
    let ids = vec![
        id(a, "Olena Koval <olena@example.invalid>"),
        id(a, "Olena Koval <olena@work.invalid>"),
        id(b, "Olena Koval <olena@example.invalid>"),
        id(b, "Olena Koval <olena@work.invalid>"),
    ];
    let view = render_view_with(&spec(3), "uk", &ids);
    let row = view["widgets"].as_array().unwrap().iter().find(|w| w["kind"] == "row").unwrap();
    let sign = row["children"].as_array().unwrap().iter().find(|c| c["id"] == "sign").unwrap();
    let options: Vec<&str> = sign["options"].as_array().unwrap().iter().map(|o| o.as_str().unwrap()).collect();
    assert_eq!(
        options,
        vec![
            "Без підпису",
            "Olena Koval <olena@example.invalid> · 04F5 252E 9BD3 FC8D",
            "Olena Koval <olena@work.invalid> · 04F5 252E 9BD3 FC8D",
            "Olena Koval <olena@example.invalid> · 140A BA21 2445 18B8",
            "Olena Koval <olena@work.invalid> · 140A BA21 2445 18B8",
        ]
    );
    assert_eq!(sign["default"], "Без підпису", "signing is asked for, never assumed");

    // The same name and address on two tokens: the fingerprint decides.
    let picked = json!({ "sign": options[4] });
    assert_eq!(signing_choice(&picked, &ids), Signing::As(ids[3].clone()));
    assert_eq!(signing_choice(&json!({ "sign": options[1] }), &ids), Signing::As(ids[0].clone()));
    // None, in either language, or no answer.
    for none in [json!({}), json!({ "sign": "None" }), json!({ "sign": "Без підпису" }), json!({ "sign": "  " })] {
        assert_eq!(signing_choice(&none, &ids), Signing::No);
    }
    // A token pulled out since the list was shown.
    assert_eq!(signing_choice(&picked, &ids[..2]), Signing::Gone(options[4].to_string()));
}

/// A signed report names its signer — user id with email, and fingerprint —
/// at the foot of every page; one that is not signed says nothing, and a
/// caller cannot make it say so.
#[test]
fn a_signed_report_names_its_signer_on_every_page() {
    let font = pdf::font().unwrap();
    let me = limen_sdk_rust::GpgIdentity {
        user_id: "Olena Koval <olena@example.invalid>".into(),
        fingerprint: "8D151FD231907502BBFA528E140ABA21244518B8".into(),
        default: false,
    };
    let line = pdf::layout::signer_line(&canonical(&spec(3), "en", Some(&me)), "en").unwrap();
    assert_eq!(
        line,
        "Signed with OpenPGP · Olena Koval <olena@example.invalid> · 8D15 1FD2 3190 7502 BBFA 528E 140A BA21 2445 18B8"
    );
    assert!(pdf::layout::signer_line(&canonical(&spec(3), "uk", Some(&me)), "uk").unwrap().starts_with("Підписано OpenPGP"));

    // On every page of a long one.
    let long = pdf::layout::render(&canonical(&spec(400), "en", Some(&me)), font, "2026-09-29 12:00 UTC");
    assert!(long.pages > 2);
    let text = String::from_utf8_lossy(&long.bytes).into_owned();
    let fingerprint = font.encode("8D15 1FD2 3190 7502 BBFA 528E 140A BA21 2445 18B8");
    assert_eq!(text.matches(&fingerprint).count(), long.pages, "once a page");
    assert!(text.contains(&font.encode("olena@example.invalid")), "the email is on the page");

    // Not signed: nothing of it. And a `signer` the caller wrote is dropped.
    let plain = pdf::layout::render(&canonical(&spec(3), "en", None), font, "2026-09-29 12:00 UTC");
    assert!(!String::from_utf8_lossy(&plain.bytes).contains(&fingerprint));
    let mut forged = spec(3);
    forged["signer"] = json!({ "user_id": "Someone Else <x@example.invalid>", "fingerprint": "8D151FD231907502BBFA528E140ABA21244518B8" });
    assert!(canonical(&forged, "en", None).get("signer").is_none());
    let doc = pdf::layout::render(&canonical(&forged, "en", None), font, "2026-09-29 12:00 UTC");
    assert!(!String::from_utf8_lossy(&doc.bytes).contains(&fingerprint), "a caller cannot sign for the user");
    assert_eq!(canonical(&forged, "en", Some(&me))["signer"]["user_id"], "Olena Koval <olena@example.invalid>");
}

/// Both marks are drawn as vector paths rather than placed as pictures.
#[test]
fn both_marks_are_drawn() {
    let facets = pdf::brand::barracuda();
    assert!(facets.len() > 40, "the logo came out as {} facets", facets.len());
    let (x0, y0, x1, y1) = pdf::brand::bounds(&facets);
    assert!(x1 > x0 && y1 > y0, "the artwork has no extent");
    // The Limen mark: two brackets of three rectangles, and three marks in
    // the gap between them.
    assert_eq!(pdf::brand::bracket_rects(true).len(), 3);
    assert_eq!(pdf::brand::gap_marks().len(), 3);
}

/// An empty spec still produces a readable page rather than nothing at all.
#[test]
fn an_empty_report_is_still_a_report() {
    let font = pdf::font().unwrap();
    let bytes = pdf::layout::render(&json!({}), font, "2026-09-29 12:00 UTC").bytes;
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Type /Catalog"));
    assert!(text.contains(&font.encode("Report")), "the default title");
}

/// The dark theme paints the page; the light one leaves it as paper.
///
/// Both halves matter: a dark report with no ground is white with pale grey
/// text on it, and a light one that paints white costs a cartridge on the
/// printers that take it literally.
#[test]
fn a_theme_decides_whether_the_page_is_painted() {
    let font = pdf::font().unwrap();
    let mut dark = spec(3);
    dark["theme"] = json!("dark");
    let dark = pdf::layout::render(&dark, font, "now").bytes;
    let light = pdf::layout::render(&spec(3), font, "now").bytes;

    // A full-page rectangle is the ground being painted.
    let ground = format!("0 0 {} {} re f", pdf::writer::PAGE_W, pdf::writer::PAGE_H);
    assert!(String::from_utf8_lossy(&dark).contains(&ground), "the dark page has no ground");
    assert!(
        !String::from_utf8_lossy(&light).contains(&ground),
        "the light page painted itself white"
    );
    // And the ink swaps with it, or the text would be invisible on one of them.
    assert!(String::from_utf8_lossy(&dark).contains("0.9098039 0.8862745"), "pale ink");
}

/// An unknown theme is the printable one rather than an error: a module
/// asking for "Dark " or "paper" still gets a report.
#[test]
fn an_unknown_theme_prints() {
    let font = pdf::font().unwrap();
    let mut odd = spec(2);
    odd["theme"] = json!("chartreuse");
    let bytes = pdf::layout::render(&odd, font, "now").bytes;
    let ground = format!("0 0 {} {} re f", pdf::writer::PAGE_W, pdf::writer::PAGE_H);
    assert!(!String::from_utf8_lossy(&bytes).contains(&ground));
}

/// Write both out for a human to look at.
#[test]
#[ignore = "writes files; run with --ignored"]
fn write_sample_pdf() {
    let font = pdf::font().unwrap();
    for theme in ["light", "dark"] {
        let mut s = spec(80);
        s["theme"] = json!(theme);
        // Signed, so the line that says by whom can be looked at too.
        let me = limen_sdk_rust::GpgIdentity {
            user_id: "Olena Koval <olena@example.invalid>".into(),
            fingerprint: "8D151FD231907502BBFA528E140ABA21244518B8".into(),
            default: false,
        };
        let doc = pdf::layout::render(&canonical(&s, "en", Some(&me)), font, "2026-09-29 12:00 UTC");
        // The platform's own temp directory, not `/tmp`: this test exists so
        // a person can open the result and look at it, and on Windows that
        // path does not exist — the write failed with `NotFound`, which is a
        // confusing way to be told a test is Linux-only.
        let path = std::env::temp_dir().join(format!("limen-report-{theme}.pdf"));
        std::fs::write(&path, &doc.bytes).unwrap();
        println!(
            "wrote {} — {} bytes, {} pages",
            path.display(),
            doc.bytes.len(),
            doc.pages
        );
    }
}
