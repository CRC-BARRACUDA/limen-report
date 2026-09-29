//! The document itself: what a Limen report looks like on paper.
//!
//! One design, drawn for every module that asks. A report from `devices` and a
//! report from `ms-licenses` are the same document with different rows in it —
//! that is what makes this module an engine rather than a formatter.
//!
//! Laid out in points, from the top of the page down, because that is how a
//! page reads; [`Page`] turns the coordinates over on the way out.

use limen_sdk_rust::Value;

use super::brand;
use super::font::Font;
use super::writer::{Page, Pdf, Rgb, PAGE_H, PAGE_W};

const MARGIN: f32 = 42.0;
const FOOT_H: f32 = 34.0;

/// The two grounds a report can be read on.
///
/// They differ in five things and nothing else: the ground, the ink, the rules,
/// the row tints, and which way the fish is painted. The amber is the same in
/// both — it is the brand, and a mark that changes colour with the theme is two
/// marks.
struct Palette {
    /// `None` means the paper itself. Painting white over white costs a
    /// cartridge on some drivers and gains nothing.
    ground: Option<Rgb>,
    ink: Rgb,
    dim: Rgb,
    rule: Rgb,
    head_bg: Rgb,
    row_bg: Rgb,
    /// The Barracuda logo: black on paper, white on the dark ground — it is one
    /// artwork, filled to suit what is behind it.
    fish: Rgb,
}

/// For printing: the page is the ground, and the ink is nearly black.
const LIGHT: Palette = Palette {
    ground: None,
    ink: Rgb::hex(0x11, 0x11, 0x11),
    dim: Rgb::hex(0x6b, 0x66, 0x5e),
    rule: Rgb::hex(0xcd, 0xc6, 0xba),
    head_bg: Rgb::hex(0xf2, 0xee, 0xe6),
    row_bg: Rgb::hex(0xfa, 0xf8, 0xf4),
    fish: Rgb::hex(0x1a, 0x1a, 0x1a),
};

/// For reading on a screen: the app's own warm near-black, so a report opened
/// beside the window it came from is the same object.
const DARK: Palette = Palette {
    ground: Some(Rgb::hex(0x08, 0x06, 0x04)),
    ink: Rgb::hex(0xe8, 0xe2, 0xd6),
    dim: Rgb::hex(0x96, 0x8c, 0x7a),
    rule: Rgb::hex(0x46, 0x30, 0x1a),
    head_bg: Rgb::hex(0x1a, 0x12, 0x0a),
    row_bg: Rgb::hex(0x0e, 0x0a, 0x06),
    fish: Rgb::hex(0xff, 0xff, 0xff),
};

const AMBER: Rgb = Rgb::hex(0xf9, 0x73, 0x16);
const AMBER_LIGHT: Rgb = Rgb::hex(0xf4, 0xc0, 0x78);

/// Everything a page needs to draw its furniture.
struct Chrome<'a> {
    title: &'a str,
    subtitle: &'a str,
    p: &'static Palette,
    /// The line along the bottom of every page: what made this, under what
    /// licence, and with which font.
    footer: String,
}

/// Draw the masthead and return the y the body starts at.
///
/// On **every** page, not only the first. Printed sheets get separated and
/// passed on one at a time, and a loose page of serial numbers has to say what
/// it belongs to and who made it.
fn masthead(page: &mut Page, font: &Font, c: &Chrome) -> f32 {
    let mark = 26.0;
    let top = MARGIN - 8.0;
    // The ground first, under everything: a page is white until something is
    // painted on it, and on the dark theme that white would show through every
    // margin.
    if let Some(ground) = c.p.ground {
        page.rect(0.0, 0.0, PAGE_W, PAGE_H, ground);
    }

    // The two marks, side by side: the company, then the tool.
    draw_barracuda(page, MARGIN, top, mark, c.p.fish);
    draw_limen(page, MARGIN + mark + 12.0, top, mark);

    page.text(font, 7.5, MARGIN + 2.0 * mark + 26.0, top + 11.0, c.p.dim, "BARRACUDA TEAM");
    page.text(font, 7.5, MARGIN + 2.0 * mark + 26.0, top + 22.0, c.p.dim, "LIMEN");

    let y = top + mark + 26.0;
    page.text(font, 15.0, MARGIN, y, c.p.ink, c.title);
    if !c.subtitle.is_empty() {
        page.text(font, 8.5, MARGIN, y + 13.0, c.p.dim, c.subtitle);
    }
    let rule_y = y + 20.0;
    page.line(MARGIN, rule_y, PAGE_W - MARGIN, 0.6, AMBER);
    rule_y + 18.0
}

/// The Barracuda logo, fitted into a square.
fn draw_barracuda(page: &mut Page, x: f32, y: f32, size: f32, colour: Rgb) {
    let facets = brand::barracuda();
    let (x0, y0, x1, y1) = brand::bounds(&facets);
    let (w, h) = (x1 - x0, y1 - y0);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let scale = (size / w).min(size / h);
    // Centred in its square, so the two marks sit on one line however the
    // artwork's own bounds happen to fall.
    let (ox, oy) = (x + (size - w * scale) / 2.0, y + (size - h * scale) / 2.0);
    for facet in &facets {
        let pts: Vec<(f32, f32)> = facet
            .iter()
            .map(|(px, py)| (ox + (px - x0) * scale, oy + (py - y0) * scale))
            .collect();
        page.polygon(&pts, colour);
    }
}

/// The Limen mark: two brackets and the lit gap between them.
fn draw_limen(page: &mut Page, x: f32, y: f32, size: f32) {
    let s = size / 256.0;
    let at = |gx: f32, gy: f32| (x + gx * s, y + gy * s);
    for near in [true, false] {
        for [x0, y0, x1, y1] in brand::bracket_rects(near) {
            let (px, py) = at(x0, y0);
            let (qx, qy) = at(x1, y1);
            page.rect(px, py, qx - px, qy - py, AMBER);
        }
    }
    // The gap marks stay brighter than the brackets, as they do on screen:
    // the threshold is the mark, the brackets are what make it visible.
    for (i, [x0, y0, x1, y1]) in brand::gap_marks().into_iter().enumerate() {
        let (px, py) = at(x0, y0);
        let (qx, qy) = at(x1, y1);
        let colour = if i == 1 { AMBER } else { AMBER_LIGHT };
        page.rect(px, py, qx - px, qy - py, colour);
    }
}

/// The line along the bottom of every page, and which page it is.
fn footer(page: &mut Page, font: &Font, c: &Chrome, n: usize, of: usize) {
    let y = PAGE_H - FOOT_H + 10.0;
    page.line(MARGIN, y - 10.0, PAGE_W - MARGIN, 0.4, c.p.rule);
    page.text(font, 6.5, MARGIN, y, c.p.dim, &c.footer);
    page.text_right(font, 6.5, PAGE_W - MARGIN, y, c.p.dim, &format!("{n} / {of}"));
}

/// Where a page can draw down to before it runs into the footer.
fn body_bottom() -> f32 {
    PAGE_H - FOOT_H - 8.0
}

/// How many characters fit in `width` points. Exact, because the face is
/// monospaced — no measuring loop, and no cell that overflows by one glyph.
fn room(font: &Font, size: f32, width: f32) -> usize {
    let per = font.width("0", size).max(0.01);
    // The hundredth is not slack, it is arithmetic: a column given exactly
    // thirteen characters computes 12.999… and wraps the thirteenth onto a line
    // of its own.
    (width / per + 0.01).floor().max(1.0) as usize
}

/// A cell's text as the lines it takes, wrapped to `width`.
///
/// Wrapped, not cut. A serial number shortened to fit a column cannot be
/// matched against the drive it came from — which is the only thing it is for —
/// and a document that quietly drops half of one is evidence of nothing. Broken
/// at a space where there is one, and through the middle of a word where there
/// is not: the long values here are identifiers, and they have no spaces at all.
fn wrap(font: &Font, text: &str, size: f32, width: f32) -> Vec<String> {
    let room = room(font, size, width);
    let mut lines: Vec<String> = Vec::new();
    let mut rest: Vec<char> = text.chars().collect();
    while rest.len() > room {
        // A break in the last third of the line reads as a wrap; earlier than
        // that and the column looks ragged for no reason.
        let window = &rest[..room];
        let at = window
            .iter()
            .rposition(|c| *c == ' ' || *c == ',')
            .filter(|i| *i * 3 >= room * 2)
            .map(|i| i + 1)
            .unwrap_or(room);
        lines.push(rest[..at].iter().collect::<String>().trim_end().to_string());
        rest.drain(..at);
        // A run of ten lines is a cell nobody reads and a page nobody can use.
        if lines.len() >= 10 {
            break;
        }
    }
    lines.push(rest.iter().collect());
    lines
}

/// A cell's text, cut to fit `width` points with an ellipsis.
///
/// The face is monospaced, so a column's capacity is exactly a number of
/// characters — no measuring loop, and no cell that overflows by one glyph.
fn fit(font: &Font, text: &str, size: f32, width: f32) -> String {
    let per = font.width("0", size).max(0.01);
    let room = (width / per).floor() as usize;
    if text.chars().count() <= room {
        return text.to_string();
    }
    if room <= 1 {
        return "…".to_string();
    }
    text.chars().take(room - 1).collect::<String>() + "…"
}

/// A finished document, and how many sheets it came to — which the preview
/// says out loud, because "this is three pages" changes whether somebody
/// presses print.
pub struct Rendered {
    pub bytes: Vec<u8>,
    pub pages: usize,
}

/// Render a spec as a PDF.
pub fn render(spec: &Value, font: &'static Font, generated: &str) -> Rendered {
    let title = str_at(spec, "title");
    let title = if title.is_empty() { "Report" } else { title };
    // Light unless asked otherwise: a report is printed more often than it is
    // read on a screen, and a dark page sent to a printer costs a cartridge.
    // Both the value a module sets and the one the pop-up's dropdown sends:
    // the choice is made in two places and means the same thing in both.
    let palette = match str_at(spec, "theme") {
        "dark" | "Dark" | "Black" => &DARK,
        _ => &LIGHT,
    };
    let chrome = Chrome {
        title,
        subtitle: str_at(spec, "subtitle"),
        p: palette,
        footer: format!(
            "LIMEN · Barracuda Team · GPL-3.0-or-later · {generated} · set in JetBrains Mono (SIL OFL 1.1)"
        ),
    };

    let mut doc = Pdf::new(font, title, format!("Generated by Limen · {generated}"));
    // Every string the document will draw, so the font carries the glyphs for
    // it and `ToUnicode` can map them back. Collected before anything is laid
    // out, because a glyph missing from the map is a character that cannot be
    // copied out of the finished file.
    note_everything(&mut doc, spec, &chrome);

    let mut page = Page::default();
    let mut y = masthead(&mut page, font, &chrome);

    // ---- what the module said about the whole thing ----------------------- //
    let summary: Vec<String> = arr_at(spec, "summary")
        .iter()
        .filter_map(|v| v.as_str())
        .map(str::to_string)
        .collect();
    if !summary.is_empty() {
        // A summary line is one of two things, and they do not belong in the
        // same shape: a figure — "Devices: 78", read by scanning — or a
        // sentence, like the note a module leaves about what it could not
        // read. A sentence squeezed into a quarter-width column comes out cut;
        // a figure given the whole page wastes it.
        let (figures, lines): (Vec<&String>, Vec<&String>) = summary
            .iter()
            .partition(|l| l.contains(':') && l.chars().count() <= 90);

        let col = (PAGE_W - 2.0 * MARGIN) / 4.0;
        // Wrapped first: a row is as tall as the tallest figure in it, and a
        // processor's name does not fit a quarter of a page on one line.
        let wrapped: Vec<(String, Vec<String>)> = figures
            .iter()
            .map(|line| {
                let (label, value) = match line.split_once(':') {
                    Some((l, v)) => (l.trim(), v.trim()),
                    None => ("", line.as_str()),
                };
                let parts = wrap(font, value, 9.5, col - 6.0).into_iter().take(3).collect();
                (label.to_uppercase(), parts)
            })
            .collect();

        for (r, chunk) in wrapped.chunks(4).enumerate() {
            let tall = chunk.iter().map(|(_, v)| v.len()).max().unwrap_or(1);
            for (i, (label, value)) in chunk.iter().enumerate() {
                let x = MARGIN + i as f32 * col;
                page.text(font, 6.5, x, y, chrome.p.dim, &fit(font, label, 6.5, col - 6.0));
                for (k, part) in value.iter().enumerate() {
                    page.text(font, 9.5, x, y + 13.0 + k as f32 * 11.0, chrome.p.ink, part);
                }
            }
            let _ = r;
            y += 13.0 + tall as f32 * 11.0 + 8.0;
        }

        // Whatever was a sentence, across the page, where it can be read whole.
        for line in lines {
            for part in wrap(font, line, 8.0, PAGE_W - 2.0 * MARGIN) {
                page.text(font, 8.0, MARGIN, y + 8.0, chrome.p.dim, &part);
                y += 11.0;
            }
            y += 4.0;
        }
        y += 6.0;
    }

    // ---- charts, drawn as bars -------------------------------------------- //
    for c in arr_at(spec, "charts") {
        let data = chart_data(c);
        if data.is_empty() {
            continue;
        }
        y = chart(&mut page, font, &chrome, y, str_at(c, "title"), &data);
    }

    // ---- the tables -------------------------------------------------------- //
    let mut pages: Vec<Page> = Vec::new();
    for section in arr_at(spec, "sections") {
        let cols = columns(section);
        let rows = rows(section);
        if cols.is_empty() && rows.is_empty() {
            continue;
        }
        let heading = str_at(section, "heading");
        // A heading with no room under it for a row belongs on the next page,
        // not stranded at the bottom of this one.
        if y + 40.0 > body_bottom() {
            pages.push(std::mem::take(&mut page));
            y = masthead(&mut page, font, &chrome);
        }
        if !heading.is_empty() {
            page.text(font, 9.5, MARGIN, y + 8.0, chrome.p.ink, heading);
            y += 18.0;
        }
        y = table(&mut page, &mut pages, font, &chrome, y, &cols, &rows);
        y += 12.0;
    }
    pages.push(page);

    let total = pages.len();
    for (i, mut p) in pages.into_iter().enumerate() {
        footer(&mut p, font, &chrome, i + 1, total);
        doc.add(p);
    }
    Rendered {
        bytes: doc.finish(),
        pages: total,
    }
}

/// A horizontal bar chart. Bars rather than a ring: the data a module hands
/// over is a list of named counts of any length, and a ring of fourteen slices
/// is a colour-matching puzzle.
fn chart(page: &mut Page, font: &Font, c: &Chrome, y: f32, title: &str, data: &[(String, f64)]) -> f32 {
    let mut y = y;
    if !title.is_empty() {
        page.text(font, 9.0, MARGIN, y + 8.0, c.p.ink, title);
        y += 16.0;
    }
    let max = data.iter().map(|(_, v)| *v).fold(0.0f64, f64::max).max(1.0);
    let label_w = 120.0;
    let bar_w = PAGE_W - 2.0 * MARGIN - label_w - 60.0;
    for (label, value) in data.iter().take(14) {
        let w = (value / max) as f32 * bar_w;
        page.text(font, 7.5, MARGIN, y + 7.0, c.p.ink, &fit(font, label, 7.5, label_w - 6.0));
        page.rect(MARGIN + label_w, y, w.max(0.6), 8.0, AMBER);
        page.text(font, 7.5, MARGIN + label_w + w + 5.0, y + 7.0, c.p.dim, &fmt_num(*value));
        y += 12.0;
    }
    y + 8.0
}

/// A table, broken across pages as it runs out of room.
#[allow(clippy::too_many_arguments)]
fn table(
    page: &mut Page,
    pages: &mut Vec<Page>,
    font: &Font,
    chrome: &Chrome,
    start_y: f32,
    cols: &[String],
    rows: &[Vec<String>],
) -> f32 {
    let inner = PAGE_W - 2.0 * MARGIN;
    // Set to fit. A six-column table of paths does not go on an A4 page at
    // seven points, and the alternative to smaller type is wrapping every cell
    // in it — which turns a row of six short values into a row four lines
    // deep. Down to five and a half points, which is small and still read.
    let size = [7.0f32, 6.5, 6.0, 5.5]
        .into_iter()
        .find(|s| demand(font, cols, rows, *s) <= inner)
        .unwrap_or(5.5);
    let row_h = size + 6.0;
    let widths = column_widths(font, cols, rows, inner, size);

    let header = |page: &mut Page, y: f32| {
        page.rect(MARGIN, y, inner, row_h, chrome.p.head_bg);
        let mut x = MARGIN;
        for (i, c) in cols.iter().enumerate() {
            let w = widths[i];
            // The same size as the body, because the columns were measured at
            // it: set larger, a seven-character heading no longer fits the
            // seven characters of room it was given, and `ENABLED` prints as
            // `ENAB…` over a column of `yes`.
            page.text(
                font,
                size,
                x + 4.0,
                y + 9.0,
                chrome.p.dim,
                &fit(font, &c.to_uppercase(), size, w - 8.0),
            );
            x += w;
        }
        page.line(MARGIN, y + row_h, PAGE_W - MARGIN, 0.5, chrome.p.rule);
        y + row_h
    };

    let mut y = header(page, start_y);
    for (n, row) in rows.iter().enumerate() {
        // Wrapped first, because how tall this row is depends on it — and so
        // does whether it fits on what is left of the page.
        let cells: Vec<Vec<String>> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let w = widths.get(i).copied().unwrap_or(60.0);
                wrap(font, cell, size, w - 8.0)
            })
            .collect();
        let lines = cells.iter().map(Vec::len).max().unwrap_or(1);
        let height = row_h + (lines - 1) as f32 * (size + 2.0);

        if y + height > body_bottom() {
            pages.push(std::mem::take(page));
            let top = masthead(page, font, chrome);
            y = header(page, top);
        }
        // Every other row on a tint: a report is read across, and a row of
        // twelve short cells is easy to lose your place in.
        if n % 2 == 1 {
            page.rect(MARGIN, y, inner, height, chrome.p.row_bg);
        }
        let mut x = MARGIN;
        for (i, cell) in cells.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(60.0);
            for (k, line) in cell.iter().enumerate() {
                page.text(
                    font,
                    size,
                    x + 4.0,
                    y + 9.0 + k as f32 * (size + 2.0),
                    chrome.p.ink,
                    line,
                );
            }
            x += w;
        }
        page.line(MARGIN, y + height, PAGE_W - MARGIN, 0.25, chrome.p.rule);
        y += height;
    }
    y
}

/// The padding inside a cell: the text is drawn 4 points in from each edge, so
/// a column needs that much more than the text in it.
const CELL_PAD: f32 = 8.0;

/// What each column would like, in points — heading or widest value, whichever
/// is longer, capped so one column of paths cannot ask for the page twice over.
fn wants(font: &Font, cols: &[String], rows: &[Vec<String>], size: f32) -> Vec<f32> {
    let per = font.width("0", size).max(0.01);
    let n = cols.len().max(rows.first().map_or(0, Vec::len)).max(1);
    (0..n)
        .map(|i| {
            let head = cols.get(i).map_or(0, |c| c.chars().count());
            let widest = rows
                .iter()
                .filter_map(|r| r.get(i))
                .map(|c| c.chars().count())
                .max()
                .unwrap_or(0);
            head.max(widest.min(44)).max(4) as f32 * per + CELL_PAD
        })
        .collect()
}

/// What the table would take if every column had what it asked for.
fn demand(font: &Font, cols: &[String], rows: &[Vec<String>], size: f32) -> f32 {
    wants(font, cols, rows, size).iter().sum()
}

/// Share `inner` points between the columns.
///
/// A column gets its **heading** first, whatever else happens — a table whose
/// headings read `S…` and `EN…` is one nobody can say what they are looking at
/// in — and a column of *short* values gets those in full as well. What is left
/// over is shared in proportion to what each column's values asked for, so a
/// column of paths grows and a column of `yes`/`no` does not.
///
/// Sharing the whole width in proportion — which this did — gave a six-character
/// column four characters of room, and `system` came out as three lines of two
/// letters.
fn column_widths(
    font: &Font,
    cols: &[String],
    rows: &[Vec<String>],
    inner: f32,
    size: f32,
) -> Vec<f32> {
    let per = font.width("0", size).max(0.01);
    let want = wants(font, cols, rows, size);
    let n = want.len();

    // `system` broken across three lines to save four characters is the whole
    // of what was wrong here: a column of paths can wrap, a column of `system`
    // cannot usefully.
    const SHORT: f32 = 16.0;
    let floor: Vec<f32> = (0..n)
        .map(|i| {
            let head = cols.get(i).map_or(0, |c| c.chars().count()) as f32 * per + CELL_PAD;
            let short = want[i].min(SHORT * per + CELL_PAD);
            head.max(short)
        })
        .collect();

    let needed: f32 = floor.iter().sum();
    if needed >= inner {
        // More heading than page. Nothing can be guaranteed, so share what
        // there is and let the wrapping do what it must.
        let total: f32 = want.iter().sum::<f32>().max(1.0);
        return want.iter().map(|w| w / total * inner).collect();
    }

    let extra: Vec<f32> = (0..n).map(|i| (want[i] - floor[i]).max(0.0)).collect();
    let asked: f32 = extra.iter().sum();
    let slack = inner - needed;
    if asked <= 0.0 {
        // Every column has all it asked for and there is room to spare: spread
        // it evenly rather than leaving the table short of the margin.
        return floor.iter().map(|f| f + slack / n as f32).collect();
    }
    let share = (slack / asked).min(1.0);
    let mut out: Vec<f32> = (0..n).map(|i| floor[i] + extra[i] * share).collect();
    // Anything still unspent goes to the column that asked for most, which is
    // the one that will have wrapped if anything did.
    let spent: f32 = out.iter().sum();
    if inner - spent > 1.0 {
        if let Some(widest) = (0..n).max_by(|a, b| extra[*a].total_cmp(&extra[*b])) {
            out[widest] += inner - spent;
        }
    }
    out
}

/// Tell the document every character it is going to draw.
fn note_everything(doc: &mut Pdf, spec: &Value, chrome: &Chrome) {
    doc.note_text(chrome.title);
    doc.note_text(chrome.subtitle);
    doc.note_text(&chrome.footer);
    doc.note_text("BARRACUDA TEAM LIMEN 0123456789 / …");
    for line in arr_at(spec, "summary") {
        if let Some(s) = line.as_str() {
            doc.note_text(&s.to_uppercase());
            doc.note_text(s);
        }
    }
    for c in arr_at(spec, "charts") {
        doc.note_text(str_at(c, "title"));
        for (label, value) in chart_data(c) {
            doc.note_text(&label);
            doc.note_text(&fmt_num(value));
        }
    }
    for section in arr_at(spec, "sections") {
        doc.note_text(str_at(section, "heading"));
        for c in columns(section) {
            doc.note_text(&c.to_uppercase());
        }
        for row in rows(section) {
            for cell in row {
                doc.note_text(&cell);
            }
        }
    }
}

// The spec readers, shared with the other renderers.
use crate::{arr_at, chart_data, columns, fmt_num, rows, str_at};
