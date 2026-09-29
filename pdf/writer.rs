//! A PDF, written out by hand.
//!
//! Enough of the format to place text, lines and filled paths on A4 pages with
//! one embedded font: indirect objects, a page tree, content streams, a
//! composite font so Cyrillic sets, a `ToUnicode` map so what is drawn can be
//! selected and copied, and a cross-reference table.
//!
//! Nothing is compressed. A report is a few hundred kilobytes either way once
//! the font is in it, and an uncompressed content stream is one a person can
//! read with `less` — which, for a tool whose output is evidence, is worth more
//! than the bytes.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::font::Font;

/// A4 in PostScript points, which is the only unit PDF has.
pub const PAGE_W: f32 = 595.28;
pub const PAGE_H: f32 = 841.89;

/// A colour, 0..1 per channel — PDF's own range.
#[derive(Clone, Copy, PartialEq)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    /// From the 0..255 the rest of the world uses.
    pub const fn hex(r: u8, g: u8, b: u8) -> Rgb {
        Rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }
}

/// One page being drawn on.
#[derive(Default)]
pub struct Page {
    ops: String,
}

impl Page {
    /// Text at a baseline. `y` is measured from the *top* of the page, because
    /// every layout in this file counts downwards and PDF counts up.
    pub fn text(&mut self, font: &Font, size: f32, x: f32, y: f32, colour: Rgb, s: &str) {
        if s.is_empty() {
            return;
        }
        let _ = writeln!(
            self.ops,
            "BT /F1 {size} Tf {r} {g} {b} rg 1 0 0 1 {x} {y} Tm <{hex}> Tj ET",
            r = colour.0,
            g = colour.1,
            b = colour.2,
            y = PAGE_H - y,
            hex = font.encode(s),
        );
    }

    /// The same, ending at `x` instead of starting there.
    pub fn text_right(&mut self, font: &Font, size: f32, x: f32, y: f32, colour: Rgb, s: &str) {
        self.text(font, size, x - font.width(s, size), y, colour, s);
    }

    /// A filled rectangle, `y` from the top.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, colour: Rgb) {
        let _ = writeln!(
            self.ops,
            "{r} {g} {b} rg {x} {y} {w} {h} re f",
            r = colour.0,
            g = colour.1,
            b = colour.2,
            y = PAGE_H - y - h,
        );
    }

    /// A horizontal rule.
    pub fn line(&mut self, x0: f32, y: f32, x1: f32, width: f32, colour: Rgb) {
        let _ = writeln!(
            self.ops,
            "{r} {g} {b} RG {width} w {x0} {y} m {x1} {y} l S",
            r = colour.0,
            g = colour.1,
            b = colour.2,
            y = PAGE_H - y,
        );
    }

    /// A filled polygon, points `(x, y)` with `y` from the top.
    pub fn polygon(&mut self, points: &[(f32, f32)], colour: Rgb) {
        let Some(((x0, y0), rest)) = points.split_first() else {
            return;
        };
        let _ = write!(
            self.ops,
            "{r} {g} {b} rg {x0} {y0} m ",
            r = colour.0,
            g = colour.1,
            b = colour.2,
            y0 = PAGE_H - y0,
        );
        for (x, y) in rest {
            let _ = write!(self.ops, "{x} {y} l ", y = PAGE_H - y);
        }
        self.ops.push_str("h f\n");
    }

}

/// The document being built.
pub struct Pdf {
    font: &'static Font,
    pages: Vec<Page>,
    /// Every glyph used anywhere, so `ToUnicode` covers the whole document.
    used: BTreeMap<u16, char>,
    pub title: String,
    pub subject: String,
}

impl Pdf {
    pub fn new(font: &'static Font, title: impl Into<String>, subject: impl Into<String>) -> Pdf {
        Pdf {
            font,
            pages: Vec::new(),
            used: BTreeMap::new(),
            title: title.into(),
            subject: subject.into(),
        }
    }

    /// Take a page the layout has finished drawing.
    pub fn add(&mut self, page: Page) {
        self.pages.push(page);
    }

    /// Record the characters a page will draw. Called by the layout as it
    /// writes, because only it knows what it wrote.
    pub fn note_text(&mut self, s: &str) {
        let font = self.font;
        font.used(s, &mut self.used);
    }

    /// Serialise the whole document.
    pub fn finish(self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::with_capacity(400 * 1024);
        // Offsets of every object, for the cross-reference table. Index 0 is
        // the free object every PDF begins its table with.
        let mut offsets: Vec<usize> = vec![0];
        let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &str| -> usize {
            let id = offsets.len();
            offsets.push(out.len());
            out.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
            id
        };

        // A header with high bytes on the second line: it tells anything
        // sniffing the file that it is binary, which stops a transfer helpfully
        // rewriting the line endings inside the font.
        out.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");

        // ---- the font: a CID font over the file, Identity encoded --------- //
        let file_id = {
            let id = offsets.len();
            offsets.push(out.len());
            out.extend_from_slice(
                format!(
                    "{id} 0 obj\n<< /Length {} /Length1 {} >>\nstream\n",
                    self.font.bytes.len(),
                    self.font.bytes.len()
                )
                .as_bytes(),
            );
            out.extend_from_slice(self.font.bytes);
            out.extend_from_slice(b"\nendstream\nendobj\n");
            id
        };
        let descriptor = obj(
            &mut out,
            &mut offsets,
            &format!(
                "<< /Type /FontDescriptor /FontName /JetBrainsMono /Flags 5 \
                 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} \
                 /CapHeight {} /StemV 80 /FontFile2 {file_id} 0 R >>",
                self.font.bbox[0],
                self.font.bbox[1],
                self.font.bbox[2],
                self.font.bbox[3],
                self.font.ascent,
                self.font.descent,
                self.font.ascent,
            ),
        );
        // Widths, as `[gid [w]]` runs — one entry per glyph the document uses,
        // not per glyph in the face: a face of three thousand glyphs would put
        // three thousand numbers in a file that drew forty of them.
        let mut widths = String::from("[");
        for gid in self.used.keys() {
            let _ = write!(widths, "{gid} [{}] ", self.font.advance(*gid));
        }
        widths.push(']');
        let cid_font = obj(
            &mut out,
            &mut offsets,
            &format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /JetBrainsMono \
                 /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                 /FontDescriptor {descriptor} 0 R /DW {} /W {widths} \
                 /CIDToGIDMap /Identity >>",
                self.font.advance(0).max(500),
            ),
        );
        let to_unicode = {
            let map = self.to_unicode();
            let id = offsets.len();
            offsets.push(out.len());
            out.extend_from_slice(
                format!("{id} 0 obj\n<< /Length {} >>\nstream\n{map}\nendstream\nendobj\n", map.len())
                    .as_bytes(),
            );
            id
        };
        let font_id = obj(
            &mut out,
            &mut offsets,
            &format!(
                "<< /Type /Font /Subtype /Type0 /BaseFont /JetBrainsMono \
                 /Encoding /Identity-H /DescendantFonts [{cid_font} 0 R] \
                 /ToUnicode {to_unicode} 0 R >>"
            ),
        );

        // ---- the pages ---------------------------------------------------- //
        // The page tree's own id is needed by each page and is not known until
        // they are written, so it is reserved here and filled in below.
        let pages_id = offsets.len();
        offsets.push(0);

        let mut page_ids: Vec<usize> = Vec::new();
        for page in &self.pages {
            let content = {
                let id = offsets.len();
                offsets.push(out.len());
                out.extend_from_slice(
                    format!(
                        "{id} 0 obj\n<< /Length {} >>\nstream\n{}endstream\nendobj\n",
                        page.ops.len(),
                        page.ops
                    )
                    .as_bytes(),
                );
                id
            };
            page_ids.push(obj(
                &mut out,
                &mut offsets,
                &format!(
                    "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] \
                     /Resources << /Font << /F1 {font_id} 0 R >> >> /Contents {content} 0 R >>"
                ),
            ));
        }

        offsets[pages_id] = out.len();
        let kids: Vec<String> = page_ids.iter().map(|id| format!("{id} 0 R")).collect();
        out.extend_from_slice(
            format!(
                "{pages_id} 0 obj\n<< /Type /Pages /Count {} /Kids [{}] >>\nendobj\n",
                page_ids.len(),
                kids.join(" ")
            )
            .as_bytes(),
        );

        let info = obj(
            &mut out,
            &mut offsets,
            &format!(
                "<< /Title ({}) /Subject ({}) /Creator (Limen) /Producer (Limen report module) >>",
                pdf_string(&self.title),
                pdf_string(&self.subject),
            ),
        );
        let catalog = obj(
            &mut out,
            &mut offsets,
            &format!("<< /Type /Catalog /Pages {pages_id} 0 R >>"),
        );

        // ---- the cross-reference table ------------------------------------ //
        let xref_at = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets[1..] {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {catalog} 0 R /Info {info} 0 R >>\n\
                 startxref\n{xref_at}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        out
    }

    /// The `ToUnicode` CMap: glyph id back to the character it stands for.
    fn to_unicode(&self) -> String {
        let mut map = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
             /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
             /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
             1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );
        // A hundred at a time, which is the format's own limit.
        for chunk in self.used.iter().collect::<Vec<_>>().chunks(100) {
            let _ = writeln!(map, "{} beginbfchar", chunk.len());
            for (gid, ch) in chunk {
                let mut utf16 = String::new();
                for unit in ch.encode_utf16(&mut [0u16; 2]).iter() {
                    let _ = write!(utf16, "{unit:04X}");
                }
                let _ = writeln!(map, "<{gid:04X}> <{utf16}>");
            }
            map.push_str("endbfchar\n");
        }
        map.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        map
    }
}

/// A string for the document information dictionary.
///
/// UTF-16BE with a byte-order mark, which is PDF's way of saying "not
/// Latin-1". Written as a literal ASCII string instead, a Ukrainian title came
/// back from every reader as a row of question marks — the file said what it
/// was called and no one could read it.
fn pdf_string(s: &str) -> String {
    let mut out = String::from("\\376\\377"); // U+FEFF, octal-escaped
    for unit in s.encode_utf16() {
        for byte in unit.to_be_bytes() {
            // Octal for every byte: a literal string cannot carry a raw NUL,
            // a backslash or an unbalanced parenthesis, and half of UTF-16 is
            // NULs.
            let _ = std::fmt::Write::write_fmt(&mut out, format_args!("\\{byte:03o}"));
        }
    }
    out
}
