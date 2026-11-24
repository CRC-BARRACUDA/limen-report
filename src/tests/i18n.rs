//! The catalogue, and that both languages actually say everything.

use super::view::sample;
use crate::*;

/// Every `a.b` key a locale file defines, read from the file rather than
/// through the catalogue: `tr` falls back to English for a key Ukrainian is
/// missing, so asking it would hide exactly what this is looking for.
fn keys(src: &str) -> Vec<String> {
    let mut table = String::new();
    let mut out = Vec::new();
    for line in src.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            table = name.to_string();
        } else if let Some((key, _)) = line.split_once(" = ") {
            out.push(format!("{table}.{key}"));
        }
    }
    out
}

/// Ukrainian says everything English says. A key only one of them has is a
/// screen that falls back to English mid-sentence.
#[test]
fn both_languages_say_the_same_things() {
    let en = keys(include_str!("../../locales/en.toml"));
    let uk = keys(include_str!("../../locales/uk.toml"));
    assert!(en.len() > 15, "the catalogue is suspiciously small");
    for key in &en {
        assert!(uk.contains(key), "uk.toml is missing {key}");
    }
    for key in uk.iter().filter(|k| !k.starts_with("module.")) {
        assert!(en.contains(key), "en.toml is missing {key}");
    }
}

/// The pop-up is the module's own screen, and it is translated — the
/// controls, not only whatever the caller titled the report.
#[test]
fn the_preview_is_translated() {
    let view = render_view(&sample(), "uk").to_string();
    for word in ["Тло", "Біле", "Зберегти", "Закрити"] {
        assert!(view.contains(word), "{word} is missing: {view}");
    }
    for english in ["Ground", "\"Save\"", "\"Close\""] {
        assert!(!view.contains(english), "English survived: {english}");
    }
    assert!(view.contains("сторінка") || view.contains("сторінок"), "{view}");
}

/// The ground is chosen by a word in the reader's language, and the
/// document only ever hears the one word it knows.
#[test]
fn a_ground_chosen_in_any_language_is_the_same_ground() {
    for said in ["Чорне", "Black", "dark", "Dark"] {
        let mut spec = sample();
        spec["theme"] = Value::String(said.into());
        assert_eq!(ground_of(&spec), "dark", "{said} was not understood");
        assert_eq!(canonical(&spec, "uk", None)["theme"], "dark");
    }
    for said in ["Біле", "White", "light", ""] {
        let mut spec = sample();
        spec["theme"] = Value::String(said.into());
        assert_eq!(ground_of(&spec), "light", "{said} was not understood");
    }
    // And the dropdown opens on the ground that was asked for, in the
    // language it is being read in.
    let mut dark = sample();
    dark["theme"] = Value::String("dark".into());
    assert_eq!(theme_label(&dark, "uk"), "Чорне");
    assert_eq!(theme_label(&sample(), "uk"), "Біле");
}

/// The document signs every page in the language it was made in, and the
/// glyphs for it are actually embedded — a footer the font cannot draw is a
/// row of blanks on paper.
#[test]
fn the_document_is_signed_in_the_readers_language() {
    let font = pdf::font().expect("the embedded font parses");
    let doc = pdf::layout::render(&canonical(&sample(), "uk", None), font, "2026-09-29 12:00 UTC");
    assert!(doc.pages >= 1);
    // Inside the document the words are glyph numbers in a content stream,
    // so the line itself is checked where it is built — by the same
    // function the page is signed with.
    let uk = pdf::layout::footer_line("uk", "2026-09-29 12:00 UTC");
    let en = pdf::layout::footer_line("en", "2026-09-29 12:00 UTC");
    assert!(uk.contains("шрифт"), "the footer is still English: {uk}");
    assert!(en.contains("set in"), "{en}");
    assert_ne!(uk, en, "both languages sign the page the same way");
    // And it is the date it was given, not one of its own.
    assert!(uk.contains("2026-09-29 12:00 UTC"));
}
