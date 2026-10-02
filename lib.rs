//! `report` — turn a report **spec** into an in-app view or an exported document.
//!
//! Provides `report.build`. It's an optional companion for data modules (e.g.
//! `devices`): they hand it a spec; it renders tables + charts in a new tab, or
//! writes a branded PDF and saves it where you choose. Because it's *optional*, a
//! data module only offers "Make Report" when this module is loaded (discovered
//! via `host.capabilities()`).
//!
//! ## Spec shape (the `build` params)
//! ```json
//! {
//!   "title": "Device Report",
//!   "subtitle": "2026-07-27",
//!   "format": "view" | "pdf",
//!   "theme":  "light" | "dark",          // light is the default
//!   "summary": ["77 devices", "66 connected"],
//!   "charts":  [ { "title": "By category", "data": [ {"label":"usb","value":41} ] } ],
//!   "sections":[ { "heading": "Connected", "columns": ["A","B"], "rows": [["1","2"]] } ]
//! }
//! ```
//! `format` defaults to `view` — a view the caller opens in a tab. `pdf` writes
//! the document and answers with a view saying where it went.

use std::time::{SystemTime, UNIX_EPOCH};

mod pdf;

use limen_sdk_rust::ui::{
    button, chart, label, row, select, separator, table, window, window_modal_sized,
};
use limen_sdk_rust::{export_module, rpc, Catalog, Handler, Host, RpcError, Value};

/// Every word this module shows, in each language it has.
///
/// English lives in a file beside the Ukrainian rather than in the code: a
/// string written into the source is a string nobody can translate.
pub(crate) fn catalog() -> &'static Catalog {
    static C: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        Catalog::new(&[
            ("en", include_str!("locales/en.toml")),
            ("uk", include_str!("locales/uk.toml")),
        ])
    })
}

/// Whether a dropdown's answer is this choice, in whichever language it was
/// shown in. An option is its own value, so a person reading Ukrainian sends
/// Ukrainian back — and a module's spec may say "dark" in neither.
fn chose(answer: &str, key: &str) -> bool {
    ["en", "uk"].iter().any(|lang| catalog().tr(lang, key) == answer)
}

/// The ground, named the one way the renderer knows it.
///
/// The dropdown sends a word in the reader's language and a module's spec says
/// `light`/`dark`; both mean the same thing, and the document should not have to
/// know about either.
fn ground_of(spec: &Value) -> &'static str {
    let said = str_at(spec, "theme");
    if said == "dark" || said == "Dark" || chose(said, "preview.black") {
        "dark"
    } else {
        "light"
    }
}

/// The spec as the renderer wants it: the ground named canonically, and the
/// language carried along so the document's own lines can be written in it.
fn canonical(spec: &Value, lang: &str) -> Value {
    let mut out = spec.clone();
    if let Some(o) = out.as_object_mut() {
        o.insert("theme".into(), Value::String(ground_of(spec).into()));
        o.insert("lang".into(), Value::String(lang.to_string()));
    }
    out
}

#[derive(Default)]
struct Report;

impl Handler for Report {
    fn capabilities(&self) -> Vec<String> {
        vec!["report.build".into()]
    }

    fn invoke(
        &mut self,
        _capability: &str,
        method: &str,
        params: Value,
        host: &Host,
    ) -> Result<Value, RpcError> {
        let lang = host.locale();
        let lang = lang.as_str();
        match method {
            "ui" => Ok(landing(lang)),
            "build" => Ok(build(&params, host, lang)),
            // Open what was just written, in whatever the system opens a PDF
            // with. Its own method rather than part of `build`, because it is
            // a different thing to ask for.
            "open" => {
                let path = params.get("path").and_then(Value::as_str).unwrap_or("");
                if !path.is_empty() {
                    host.open("path", path);
                }
                Ok(window(catalog().tr(lang, "ui.title"), vec![label(path).mono()]))
            }
            other => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("report has no method {other}"),
            )),
        }
    }
}

/// How much of a table the pop-up shows before it stops being a preview.
///
/// A preview is for answering "is this the right data", and four hundred rows
/// in a pop-up answers it no better than twelve while making the window
/// unusable. The document itself gets every row.
const PREVIEW_ROWS: usize = 12;

/// What the PDF will be, and the two controls that make it.
fn export_row(spec: &Value, lang: &str) -> Vec<limen_sdk_rust::ui::Widget> {
    let t = |k: &str| catalog().tr(lang, k);
    let Some(font) = pdf::font() else {
        return Vec::new();
    };
    let pages = pdf::layout::render(&canonical(spec, lang), font, &today()).pages;
    let sheets = if pages == 1 {
        t("preview.page_one")
    } else {
        t("preview.pages").replace("{n}", &pages.to_string())
    };

    // The whole spec travels with the button, so the export does not depend on
    // this module having remembered anything: the view is a value, and so is
    // what it will turn into.
    //
    // The ground is *not* among the args. It comes from the dropdown beside the
    // button, as one of the view's own inputs — and the host merges a button's
    // args over those, so a theme set here would quietly win over whatever the
    // user chose.
    let mut args = spec.clone();
    if let Some(o) = args.as_object_mut() {
        // There is one format, so the button names it rather than asking.
        o.insert("format".into(), Value::String("pdf".into()));
        o.remove("theme");
    }

    vec![
        label(t("preview.meta").replace("{sheets}", &sheets)).weak(),
        row(vec![
            // One control, because there is one thing left to decide. A report
            // is a document; a page, a table of cells and a file of Markdown
            // are the data behind one, and a module with data to hand over can
            // hand it over itself.
            select("theme", vec![t("preview.white"), t("preview.black")])
                .label(t("preview.ground"))
                .default(theme_label(spec, lang)),
            button(t("preview.save"), "report.build", "build")
                .primary()
                .args(args),
            button(t("preview.close"), "report.build", "build").dismiss(),
        ]),
        separator(),
    ]
}

/// Which way round the dropdown starts: what the caller asked for, if it asked.
fn theme_label(spec: &Value, lang: &str) -> String {
    catalog().tr(
        lang,
        if ground_of(spec) == "dark" {
            "preview.black"
        } else {
            "preview.white"
        },
    )
}

/// Opening `report` on its own just explains what it's for.
fn landing(lang: &str) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    window(
        t("ui.title"),
        vec![
            label(t("ui.landing_title")).strong(),
            label(t("ui.landing_body")).weak(),
        ],
    )
}

// --------------------------------------------------------------------------- //
// Spec accessors
// --------------------------------------------------------------------------- //

pub(crate) fn str_at<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

pub(crate) fn arr_at<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// `columns` of a section.
pub(crate) fn columns(section: &Value) -> Vec<String> {
    arr_at(section, "columns")
        .iter()
        .filter_map(|c| c.as_str().map(str::to_string))
        .collect()
}

/// `rows` of a section (each a vector of string cells).
pub(crate) fn rows(section: &Value) -> Vec<Vec<String>> {
    arr_at(section, "rows")
        .iter()
        .map(|r| {
            r.as_array()
                .map(|cells| cells.iter().map(cell_str).collect())
                .unwrap_or_default()
        })
        .collect()
}

/// A cell as a string (numbers/bools rendered, not quoted).
fn cell_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// `data` of a chart as `(label, value)` pairs.
pub(crate) fn chart_data(c: &Value) -> Vec<(String, f64)> {
    arr_at(c, "data")
        .iter()
        .filter_map(|b| {
            Some((
                b.get("label")?.as_str()?.to_string(),
                b.get("value").and_then(Value::as_f64).unwrap_or(0.0),
            ))
        })
        .collect()
}

// --------------------------------------------------------------------------- //
// Build
// --------------------------------------------------------------------------- //

/// Every name the one format goes by: what a module writes in a spec, and what
/// the preview's Save button sends. Both arrive in the same field, so both are
/// answered here rather than in two places that can disagree.
fn format_of(spec: &Value) -> &'static str {
    match str_at(spec, "format").trim() {
        "pdf" | "PDF" => "pdf",
        _ => "view",
    }
}

fn build(spec: &Value, host: &Host, lang: &str) -> Value {
    match format_of(spec) {
        "pdf" => match pdf::font() {
            Some(font) => export_bytes(
                spec,
                host,
                "pdf",
                pdf::layout::render(&canonical(spec, lang), font, &today()).bytes,
                lang,
            ),
            // The face is embedded in this library, so this cannot happen in a
            // build that shipped — but a report that silently came out as text
            // when a PDF was asked for would be worse than one that says so.
            None => {
                host.log("report: the embedded font could not be read; no PDF was written");
                Value::Null
            }
        },
        // "" | "view" | anything else → an in-app view the caller opens in a tab.
        _ => render_view(spec, lang),
    }
}

/// Render the spec as a Limen view — and as a **preview of the document**.
///
/// The same content, in the same order, under the same headings the PDF uses,
/// with the line the page is footed by and the number of sheets it comes to.
/// It is not a picture of the file: a module cannot draw one, and one drawn in
/// widgets would be a second layout to keep in step with the first. What it is
/// instead is an answer to the two questions somebody asks before pressing
/// print — *is this the right data*, and *how long is it* — with the buttons
/// that produce the file sitting on the answer.
fn render_view(spec: &Value, lang: &str) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    let title = {
        let t = str_at(spec, "title");
        if t.is_empty() {
            catalog().tr(lang, "ui.title")
        } else {
            t.to_string()
        }
    };
    let mut w = vec![label(title.clone()).heading()];
    let subtitle = str_at(spec, "subtitle");
    if !subtitle.is_empty() {
        w.push(label(subtitle).weak());
    }
    w.extend(export_row(spec, lang));
    for line in arr_at(spec, "summary") {
        if let Some(s) = line.as_str() {
            w.push(label(s));
        }
    }
    if !arr_at(spec, "charts").is_empty() || !arr_at(spec, "sections").is_empty() {
        w.push(separator());
    }
    for c in arr_at(spec, "charts") {
        let data = chart_data(c);
        if !data.is_empty() {
            w.push(chart(str_at(c, "title"), data));
            w.push(separator());
        }
    }
    for section in arr_at(spec, "sections") {
        let heading = str_at(section, "heading");
        if !heading.is_empty() {
            w.push(label(heading).strong());
        }
        let all = rows(section);
        let shown: Vec<Vec<String>> = all.iter().take(PREVIEW_ROWS).cloned().collect();
        let hidden = all.len().saturating_sub(shown.len());
        w.push(table(columns(section), shown));
        if hidden > 0 {
            // Said out loud, because a preview that quietly shows twelve of
            // four hundred rows is a preview of a different document.
            w.push(label(t("preview.more_rows").replace("{n}", &hidden.to_string())).weak());
        }
        w.push(separator());
    }
    // What every page of the document is signed with, shown once here.
    w.push(
        label(t("doc.footer").replace("{generated}", &today())).weak(),
    );
    // A pop-up over the screen that asked for it, rather than a page that
    // replaces it: a report is something you look at and then decide about,
    // and the module underneath is what you decide to go back to.
    window_modal_sized(title, "report.preview", 900.0, w)
}

/// Ask the user where the document goes, write it there, and say what happened.
///
/// A dialog rather than a temp folder: a report is a thing somebody keeps —
/// attached to a ticket, filed with a case — and a file written to
/// `/tmp/limen-reports` with a unix timestamp in its name is one they have to
/// go and find, on a path that is cleared on the next boot.
///
/// Answers with a **view**, never with null: this is what a button press
/// renders, and a module that answers a click with nothing leaves the tab it
/// was pressed in showing an error about a view it did not get.
fn export_bytes(spec: &Value, host: &Host, ext: &str, content: Vec<u8>, lang: &str) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    let suggested = suggested_name(spec, ext);
    let Some(path) = host.save_file(&suggested) else {
        // Cancelled. Not an error, and not silence either — the pop-up is gone
        // by now, and a click that appears to have done nothing is a click
        // somebody presses again.
        return window(t("ui.title"), vec![label(t("save.cancelled")).weak()]);
    };
    let path = std::path::PathBuf::from(&path);
    // The dialog is where the name is chosen, but a name typed without one is
    // a file the system will not know how to open.
    let path = if path.extension().is_some() {
        path
    } else {
        path.with_extension(ext)
    };
    match std::fs::write(&path, &content) {
        Ok(()) => {
            host.log(&format!("report: wrote {}", path.display()));
            window(
                t("ui.title"),
                vec![
                    label(t("save.saved")).strong(),
                    label(path.to_string_lossy().into_owned()).mono(),
                    row(vec![
                        button("Open", "report.build", "open")
                            .primary()
                            .args(limen_sdk_rust::json!({ "path": path.to_string_lossy() })),
                    ]),
                ],
            )
        }
        Err(e) => {
            host.log(&format!("report: failed to write {}: {e}", path.display()));
            window(
                t("ui.title"),
                vec![
                    label(t("save.failed")).strong(),
                    label(format!("{}: {e}", path.display())).weak(),
                ],
            )
        }
    }
}

/// What the save dialog opens with: `<what it is about>_<date>.<ext>`.
///
/// The stem is the calling module's — `file_name` in the spec — because only it
/// knows what the report is *of*. A machine's report is filed under the machine:
/// `OkPrepareYourAss_PF3ZK9QW_2026-09-29.pdf` is a name that sorts with its
/// siblings and answers "which box, which one of them, when" from the filename
/// alone. A caller that says nothing gets its title instead.
///
/// The date is added here rather than by each caller, so every report this
/// engine produces is dated the same way whoever asked for it.
fn suggested_name(spec: &Value, ext: &str) -> String {
    let stem = match str_at(spec, "file_name") {
        "" => slug(str_at(spec, "title")),
        given => file_stem(given),
    };
    format!("{stem}_{}.{ext}", today_date())
}

/// A caller's stem, made safe for a filesystem without being flattened.
///
/// Case is kept — a hostname is `OkPrepareYourAss` and a serial is `PF3ZK9QW`,
/// and lower-casing them makes a name nobody recognises. Everything a path or a
/// mail client argues about becomes an underscore.
fn file_stem(s: &str) -> String {
    let mut out = String::new();
    let mut prev = false;
    for ch in s.chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '.' {
            out.push(ch);
            prev = false;
        } else if !prev {
            out.push('_');
            prev = true;
        }
    }
    let trimmed = out.trim_matches(['_', '.', '-']).to_string();
    // 80 characters is past any hostname and serial together, and short of
    // what a filesystem or a mail client starts truncating.
    let trimmed: String = trimmed.chars().take(80).collect();
    if trimmed.is_empty() {
        "report".to_string()
    } else {
        trimmed
    }
}

/// Today, as `YYYY-MM-DD` — the date part of [`today`].
fn today_date() -> String {
    today().chars().take(10).collect()
}

// --------------------------------------------------------------------------- //
// Renderers
// --------------------------------------------------------------------------- //

// --------------------------------------------------------------------------- //
// Small helpers
// --------------------------------------------------------------------------- //

/// A filename-safe slug (lowercase alphanumerics, dashes between).
fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() { "report".to_string() } else { trimmed }
}

/// Integers without a decimal, otherwise two places.
pub(crate) fn fmt_num(v: f64) -> String {
    if v.fract().abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

export_module!(Report);

#[cfg(test)]
mod tests {
    use super::*;
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

}

/// Today, as `YYYY-MM-DD` — what a report is filed under.
///
/// Derived from the clock rather than read from anywhere: this module has no
/// permission to run `date`, and a report with no date on it is a report nobody
/// can put in order later.
pub(crate) fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let (h, m) = ((secs.rem_euclid(86_400)) / 3600, (secs.rem_euclid(3600)) / 60);
    // Civil-from-days: exact, and shorter than depending on a date library.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02} UTC")
}

#[cfg(test)]
mod pdf_tests {
    use super::*;
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
            let doc = pdf::layout::render(&s, font, "2026-09-29 12:00 UTC");
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
}

#[cfg(test)]
mod preview_tests {
    use super::*;
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
        let row = view["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["kind"] == "row")
            .expect("the controls");
        let children = row["children"].as_array().unwrap();

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
        let row = view["widgets"].as_array().unwrap()
            .iter().find(|w| w["kind"] == "row").unwrap();
        let close = row["children"].as_array().unwrap().last().unwrap();
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
        let widgets = view["widgets"].as_array().unwrap();
        let row = widgets.iter().find(|w| w["kind"] == "row").expect("the export row");
        let save = row["children"].as_array().unwrap()
            .iter().find(|c| c["text"] == "Save").expect("the save button");
        assert_eq!(save["action"]["capability"], "report.build");
        assert_eq!(save["action"]["method"], "build");
        // Every row travels with it — the ninety the preview trimmed to twelve
        // included. The export does not depend on this module having
        // remembered the scan.
        assert_eq!(save["args"]["sections"][0]["rows"].as_array().unwrap().len(), 90);
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;
    use limen_sdk_rust::json;

    /// The name the save dialog opens with: what it is of, then when.
    ///
    /// A machine's report is filed under the machine — hostname, serial if
    /// there is one, date — so the filename answers "which box, which one of
    /// them, when" without opening it.
    #[test]
    fn the_suggested_name_is_the_callers_subject_and_the_date() {
        let spec = json!({ "title": "This machine", "file_name": "OkPrepareYourAss_PF3ZK9QW" });
        let name = suggested_name(&spec, "pdf");
        assert!(name.starts_with("OkPrepareYourAss_PF3ZK9QW_"), "{name}");
        assert!(name.ends_with(".pdf"));
        // `_YYYY-MM-DD.pdf`
        let date = &name[name.len() - 14..name.len() - 4];
        assert_eq!(date.len(), 10);
        assert_eq!(date.matches('-').count(), 2, "{date}");
        assert_eq!(&date[..2], "20", "{date}");
    }

    /// Case is kept. A hostname lower-cased is a hostname nobody recognises.
    #[test]
    fn a_hostname_keeps_its_shape() {
        assert_eq!(file_stem("OkPrepareYourAss"), "OkPrepareYourAss");
        assert_eq!(file_stem("host.example.org"), "host.example.org");
        assert_eq!(file_stem("PF3ZK9QW"), "PF3ZK9QW");
    }

    /// And anything a filesystem or a mail client would argue about does not
    /// survive: a serial is whatever the vendor wrote, including slashes.
    #[test]
    fn a_filename_carries_nothing_a_path_would_argue_with() {
        assert_eq!(file_stem("a/b"), "a_b");
        assert_eq!(file_stem("Звіт про пристрої"), "Звіт_про_пристрої");
        assert_eq!(file_stem("  spaced  out  "), "spaced_out");
        assert_eq!(file_stem("SN: 12/34*56?"), "SN_12_34_56");
        for name in [file_stem(""), file_stem("///"), file_stem("···")] {
            assert_eq!(name, "report", "a nameless report still has a filename");
        }
        assert!(file_stem(&"x".repeat(200)).chars().count() <= 80);
    }

    /// A caller that says nothing is filed under its title, as before.
    #[test]
    fn without_a_name_the_title_is_the_name() {
        let name = suggested_name(&json!({ "title": "Device Report" }), "pdf");
        assert!(name.starts_with("device-report_"), "{name}");
        assert!(name.ends_with(".pdf"));
    }
}

#[cfg(test)]
mod figure_tests {
    use super::*;
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
}

#[cfg(test)]
mod render_probe {
    use super::*;

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
}

/// The catalogue, and that both languages actually say everything.
#[cfg(test)]
mod i18n_tests {
    use super::tests::sample;
    use super::*;

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
        let en = keys(include_str!("locales/en.toml"));
        let uk = keys(include_str!("locales/uk.toml"));
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
            assert_eq!(canonical(&spec, "uk")["theme"], "dark");
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
        let doc = pdf::layout::render(&canonical(&sample(), "uk"), font, "2026-09-29 12:00 UTC");
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
}
