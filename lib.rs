//! `report` — turn a report **spec** into an in-app view or an exported document.
//!
//! Provides `report.build`. It's an optional companion for data modules (e.g.
//! `devices`): they hand it a spec; it renders tables + charts in a new tab, or
//! writes a Markdown / HTML / CSV file and opens it. Because it's *optional*, a
//! data module only offers "Make Report" when this module is loaded (discovered
//! via `host.capabilities()`).
//!
//! ## Spec shape (the `build` params)
//! ```json
//! {
//!   "title": "Device Report",
//!   "subtitle": "2026-07-27",
//!   "format": "view" | "pdf" | "markdown" | "html" | "csv",
//!   "theme":  "light" | "dark",          // PDF only; light is the default
//!   "summary": ["77 devices", "66 connected"],
//!   "charts":  [ { "title": "By category", "data": [ {"label":"usb","value":41} ] } ],
//!   "sections":[ { "heading": "Connected", "columns": ["A","B"], "rows": [["1","2"]] } ]
//! }
//! ```
//! `format` defaults to `view` (return a view the caller opens in a tab); the
//! export formats write a file and return null.

use std::time::{SystemTime, UNIX_EPOCH};

mod pdf;

use limen_sdk_rust::ui::{chart, label, separator, table, window};
use limen_sdk_rust::{export_module, rpc, Handler, Host, RpcError, Value};

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
        match method {
            "ui" => Ok(landing()),
            "build" => Ok(build(&params, host)),
            other => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("report has no method {other}"),
            )),
        }
    }
}

/// Opening `report` on its own just explains what it's for.
fn landing() -> Value {
    window(
        "Report",
        vec![
            label("Report generator").strong(),
            label(
                "This module builds reports for other modules — a branded PDF, or Markdown, \
                 HTML and CSV. Open a data module (e.g. Devices) and use its “Make Report” \
                 action; the button only appears while this module is installed.",
            )
            .weak(),
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

fn build(spec: &Value, host: &Host) -> Value {
    match str_at(spec, "format") {
        "markdown" | "md" => export(spec, host, "md", render_markdown(spec)),
        "html" => export(spec, host, "html", render_html(spec)),
        "csv" => export(spec, host, "csv", render_csv(spec)),
        "pdf" => match pdf::font() {
            Some(font) => export_bytes(spec, host, "pdf", pdf::layout::render(spec, font, &today())),
            // The face is embedded in this library, so this cannot happen in a
            // build that shipped — but a report that silently came out as text
            // when a PDF was asked for would be worse than one that says so.
            None => {
                host.log("report: the embedded font could not be read; no PDF was written");
                Value::Null
            }
        },
        // "" | "view" | anything else → an in-app view the caller opens in a tab.
        _ => render_view(spec),
    }
}

/// Render the spec as a Limen view (title, summary, charts, tables).
fn render_view(spec: &Value) -> Value {
    let title = {
        let t = str_at(spec, "title");
        if t.is_empty() { "Report" } else { t }
    };
    let mut w = vec![label(title).heading()];
    let subtitle = str_at(spec, "subtitle");
    if !subtitle.is_empty() {
        w.push(label(subtitle).weak());
    }
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
        w.push(table(columns(section), rows(section)));
        w.push(separator());
    }
    window(title, w)
}

/// Write `content` to a temp file and ask the host to open it. Returns null
/// (fire-and-forget — the opened file is the feedback).
fn export(spec: &Value, host: &Host, ext: &str, content: String) -> Value {
    export_bytes(spec, host, ext, content.into_bytes())
}

/// The same, for a document that is not text.
fn export_bytes(spec: &Value, host: &Host, ext: &str, content: Vec<u8>) -> Value {
    let dir = std::env::temp_dir().join("limen-reports");
    let _ = std::fs::create_dir_all(&dir);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = format!("{}-{stamp}.{ext}", slug(str_at(spec, "title")));
    let path = dir.join(name);
    if std::fs::write(&path, &content).is_ok() {
        host.open("path", &path.to_string_lossy());
    } else {
        host.log(&format!("report: failed to write {}", path.display()));
    }
    Value::Null
}

// --------------------------------------------------------------------------- //
// Renderers
// --------------------------------------------------------------------------- //

fn render_markdown(spec: &Value) -> String {
    let mut out = String::new();
    let title = str_at(spec, "title");
    out.push_str(&format!("# {}\n\n", if title.is_empty() { "Report" } else { title }));
    let subtitle = str_at(spec, "subtitle");
    if !subtitle.is_empty() {
        out.push_str(&format!("_{subtitle}_\n\n"));
    }
    for line in arr_at(spec, "summary") {
        if let Some(s) = line.as_str() {
            out.push_str(&format!("- {s}\n"));
        }
    }
    out.push('\n');
    for c in arr_at(spec, "charts") {
        let ct = str_at(c, "title");
        if !ct.is_empty() {
            out.push_str(&format!("## {ct}\n\n"));
        }
        let data = chart_data(c);
        let max = data.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max).max(1.0);
        for (label, value) in &data {
            let bars = ((value / max) * 24.0).round() as usize;
            out.push_str(&format!("- {label}: {} `{}`\n", fmt_num(*value), "█".repeat(bars)));
        }
        out.push('\n');
    }
    for section in arr_at(spec, "sections") {
        let heading = str_at(section, "heading");
        if !heading.is_empty() {
            out.push_str(&format!("## {heading}\n\n"));
        }
        let cols = columns(section);
        if !cols.is_empty() {
            out.push_str(&format!("| {} |\n", cols.join(" | ")));
            out.push_str(&format!("| {} |\n", cols.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")));
            for row in rows(section) {
                let cells: Vec<String> = row.iter().map(|c| c.replace('|', "\\|")).collect();
                out.push_str(&format!("| {} |\n", cells.join(" | ")));
            }
        }
        out.push('\n');
    }
    out
}

fn render_html(spec: &Value) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let title = {
        let t = str_at(spec, "title");
        if t.is_empty() { "Report".to_string() } else { esc(t) }
    };
    let mut body = String::new();
    body.push_str(&format!("<h1>{title}</h1>\n"));
    let subtitle = str_at(spec, "subtitle");
    if !subtitle.is_empty() {
        body.push_str(&format!("<p class=sub>{}</p>\n", esc(subtitle)));
    }
    if !arr_at(spec, "summary").is_empty() {
        body.push_str("<ul>\n");
        for line in arr_at(spec, "summary") {
            if let Some(s) = line.as_str() {
                body.push_str(&format!("<li>{}</li>\n", esc(s)));
            }
        }
        body.push_str("</ul>\n");
    }
    for c in arr_at(spec, "charts") {
        let ct = str_at(c, "title");
        if !ct.is_empty() {
            body.push_str(&format!("<h2>{}</h2>\n", esc(ct)));
        }
        let data = chart_data(c);
        let max = data.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max).max(1.0);
        body.push_str("<div class=chart>\n");
        for (label, value) in &data {
            let pct = (value / max * 100.0).clamp(0.0, 100.0);
            body.push_str(&format!(
                "<div class=bar><span class=k>{}</span>\
                 <span class=track><span class=fill style=\"width:{pct:.1}%\"></span></span>\
                 <span class=v>{}</span></div>\n",
                esc(label),
                fmt_num(*value),
            ));
        }
        body.push_str("</div>\n");
    }
    for section in arr_at(spec, "sections") {
        let heading = str_at(section, "heading");
        if !heading.is_empty() {
            body.push_str(&format!("<h2>{}</h2>\n", esc(heading)));
        }
        let cols = columns(section);
        body.push_str("<table>\n<thead><tr>");
        for col in &cols {
            body.push_str(&format!("<th>{}</th>", esc(col)));
        }
        body.push_str("</tr></thead>\n<tbody>\n");
        for row in rows(section) {
            body.push_str("<tr>");
            for cellv in row {
                body.push_str(&format!("<td>{}</td>", esc(&cellv)));
            }
            body.push_str("</tr>\n");
        }
        body.push_str("</tbody></table>\n");
    }
    format!(
        "<!doctype html><html><head><meta charset=utf-8><title>{title}</title><style>\
         body{{font-family:system-ui,Segoe UI,Arial,sans-serif;max-width:900px;margin:2rem auto;padding:0 1rem;color:#1c1f24}}\
         h1{{margin-bottom:.2rem}} .sub{{color:#7a828e;margin-top:0}}\
         table{{border-collapse:collapse;width:100%;margin:.5rem 0 1.5rem}}\
         th,td{{border:1px solid #d7dae0;padding:.35rem .6rem;text-align:left;font-size:.92rem}}\
         th{{background:#f2f4f7}} tr:nth-child(even) td{{background:#fafbfc}}\
         .chart{{margin:.5rem 0 1.5rem}} .bar{{display:flex;align-items:center;gap:.6rem;margin:.2rem 0}}\
         .bar .k{{width:9rem;text-align:right;color:#3b414d;font-size:.9rem}}\
         .bar .track{{flex:1;background:#eef1f5;border-radius:4px;overflow:hidden;height:1rem}}\
         .bar .fill{{display:block;height:100%;background:#5c9cf5}} .bar .v{{width:3rem;color:#7a828e;font-size:.9rem}}\
         </style></head><body>\n{body}</body></html>\n"
    )
}

fn render_csv(spec: &Value) -> String {
    let mut out = String::new();
    for c in arr_at(spec, "charts") {
        let ct = str_at(c, "title");
        out.push_str(&format!("# {}\n", if ct.is_empty() { "chart" } else { ct }));
        out.push_str("label,value\n");
        for (label, value) in chart_data(c) {
            out.push_str(&format!("{},{}\n", csv_field(&label), fmt_num(value)));
        }
        out.push('\n');
    }
    for section in arr_at(spec, "sections") {
        let heading = str_at(section, "heading");
        if !heading.is_empty() {
            out.push_str(&format!("# {heading}\n"));
        }
        let cols = columns(section);
        out.push_str(&cols.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
        for row in rows(section) {
            out.push_str(&row.iter().map(|c| csv_field(c)).collect::<Vec<_>>().join(","));
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

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

/// Quote a CSV field if it contains a comma, quote, or newline (doubling quotes).
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
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

    fn sample() -> Value {
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
        let v = render_view(&sample());
        assert_eq!(v["title"], "Device Report");
        let kinds: Vec<&str> = v["widgets"].as_array().unwrap().iter()
            .map(|w| w["kind"].as_str().unwrap()).collect();
        assert!(kinds.contains(&"chart"));
        assert!(kinds.contains(&"table"));
    }

    #[test]
    fn markdown_and_html_and_csv_render() {
        let md = render_markdown(&sample());
        assert!(md.contains("# Device Report"));
        assert!(md.contains("| A | B |"));
        let html = render_html(&sample());
        assert!(html.contains("<h1>Device Report</h1>"));
        assert!(html.contains("class=fill"));
        let csv = render_csv(&sample());
        assert!(csv.contains("label,value"));
        assert!(csv.contains("A,B"));
    }

    #[test]
    fn csv_quotes_tricky_fields() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("she said \"hi\""), "\"she said \"\"hi\"\"\"");
        assert_eq!(csv_field("plain"), "plain");
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
        pdf::layout::render(&spec(rows), font, "2026-09-29 12:00 UTC")
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
        let bytes = pdf::layout::render(&spec(3), font, "2026-09-29 12:00 UTC");
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
        let bytes = pdf::layout::render(&json!({}), font, "2026-09-29 12:00 UTC");
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
        let dark = pdf::layout::render(&dark, font, "now");
        let light = pdf::layout::render(&spec(3), font, "now");

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
        let bytes = pdf::layout::render(&odd, font, "now");
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
            let bytes = pdf::layout::render(&s, font, "2026-09-29 12:00 UTC");
            let path = format!("/tmp/limen-report-{theme}.pdf");
            std::fs::write(&path, &bytes).unwrap();
            println!("wrote {path} — {} bytes", bytes.len());
        }
    }
}
