//! Just enough TrueType to place text in a PDF.
//!
//! A PDF that embeds a font has to know three things the font file keeps to
//! itself: which glyph a character is (`cmap`), how wide that glyph is
//! (`hmtx`), and the metrics a reader needs to lay the face out at all
//! (`head`, `hhea`). This reads those four tables and nothing else — the glyph
//! outlines are handed to the reader untouched, as the whole file.
//!
//! Four tables rather than a font crate: the alternative pulled a hundred
//! packages into a module that runs in the host's own process, to answer three
//! questions that are two hundred lines of byte-reading.

use std::collections::BTreeMap;

/// The parsed parts of a TrueType face.
pub struct Font {
    /// The file itself, embedded in the PDF as `FontFile2`.
    pub bytes: &'static [u8],
    /// The design grid. 1000 for JetBrains Mono, which is also PDF's text
    /// space — anything else has to be scaled on the way out.
    pub units_per_em: u16,
    pub ascent: i16,
    pub descent: i16,
    /// `[xMin, yMin, xMax, yMax]`, for the font descriptor.
    pub bbox: [i16; 4],
    /// Advance width per glyph, in font units.
    advances: Vec<u16>,
    /// Unicode → glyph id.
    cmap: BTreeMap<u32, u16>,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([*b.get(i).unwrap_or(&0), *b.get(i + 1).unwrap_or(&0)])
}

fn i16_at(b: &[u8], i: usize) -> i16 {
    u16_at(b, i) as i16
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([
        *b.get(i).unwrap_or(&0),
        *b.get(i + 1).unwrap_or(&0),
        *b.get(i + 2).unwrap_or(&0),
        *b.get(i + 3).unwrap_or(&0),
    ])
}

impl Font {
    /// Read a TrueType file. `None` if it is not one, or is missing a table
    /// text cannot be placed without.
    pub fn parse(bytes: &'static [u8]) -> Option<Font> {
        let count = u16_at(bytes, 4) as usize;
        let mut tables: BTreeMap<&[u8], (usize, usize)> = BTreeMap::new();
        for i in 0..count {
            let rec = 12 + 16 * i;
            if rec + 16 > bytes.len() {
                return None;
            }
            let tag = &bytes[rec..rec + 4];
            let off = u32_at(bytes, rec + 8) as usize;
            let len = u32_at(bytes, rec + 12) as usize;
            if off + len > bytes.len() {
                return None;
            }
            tables.insert(tag, (off, len));
        }
        let table = |tag: &[u8]| tables.get(tag).copied();

        let (head, _) = table(b"head")?;
        let units_per_em = u16_at(bytes, head + 18);
        let bbox = [
            i16_at(bytes, head + 36),
            i16_at(bytes, head + 38),
            i16_at(bytes, head + 40),
            i16_at(bytes, head + 42),
        ];

        let (hhea, _) = table(b"hhea")?;
        let ascent = i16_at(bytes, hhea + 4);
        let descent = i16_at(bytes, hhea + 6);
        let long_metrics = u16_at(bytes, hhea + 34) as usize;

        let (hmtx, _) = table(b"hmtx")?;
        let advances: Vec<u16> = (0..long_metrics)
            .map(|i| u16_at(bytes, hmtx + i * 4))
            .collect();

        let (cmap_off, _) = table(b"cmap")?;
        let cmap = read_cmap(bytes, cmap_off)?;

        Some(Font {
            bytes,
            units_per_em,
            ascent,
            descent,
            bbox,
            advances,
            cmap,
        })
    }

    /// The glyph for `ch`, or glyph 0 — the face's own "missing character" box,
    /// which is the honest thing to draw for something it cannot set.
    pub fn glyph(&self, ch: char) -> u16 {
        self.cmap.get(&(ch as u32)).copied().unwrap_or(0)
    }

    /// A glyph's advance, in thousandths of an em — PDF's own unit, whatever
    /// grid the font was drawn on.
    pub fn advance(&self, glyph: u16) -> u16 {
        let raw = match self.advances.get(glyph as usize) {
            Some(w) => *w,
            // Past `numberOfHMetrics` every glyph shares the last advance —
            // how a monospaced face stores one width for thousands of glyphs.
            None => *self.advances.last().unwrap_or(&0),
        };
        if self.units_per_em == 1000 || self.units_per_em == 0 {
            raw
        } else {
            (raw as u32 * 1000 / self.units_per_em as u32) as u16
        }
    }

    /// How wide `text` is at `size` points.
    pub fn width(&self, text: &str, size: f32) -> f32 {
        let units: u32 = text.chars().map(|c| self.advance(self.glyph(c)) as u32).sum();
        units as f32 * size / 1000.0
    }

    /// `text` as the glyph ids a PDF string holds under Identity-H encoding,
    /// hex, two bytes each.
    pub fn encode(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len() * 4);
        for ch in text.chars() {
            out.push_str(&format!("{:04X}", self.glyph(ch)));
        }
        out
    }

    /// Every glyph this document uses, with the character it stands for — what
    /// the `ToUnicode` map is built from.
    ///
    /// Without that map a reader can draw the page perfectly and still hand
    /// back nothing when somebody selects a line of it: the text is stored as
    /// glyph numbers, and only this says which characters those were. A report
    /// nobody can copy a serial number out of is a picture of a report.
    pub fn used(&self, text: &str, into: &mut BTreeMap<u16, char>) {
        for ch in text.chars() {
            into.insert(self.glyph(ch), ch);
        }
    }
}

/// The best Unicode subtable there is: a Windows BMP `format 4`, or the same
/// thing under the Unicode platform id.
fn read_cmap(b: &[u8], cmap: usize) -> Option<BTreeMap<u32, u16>> {
    let tables = u16_at(b, cmap + 2) as usize;
    let mut best: Option<usize> = None;
    for i in 0..tables {
        let rec = cmap + 4 + 8 * i;
        let platform = u16_at(b, rec);
        let encoding = u16_at(b, rec + 2);
        let off = cmap + u32_at(b, rec + 4) as usize;
        let format = u16_at(b, off);
        let wanted = matches!(
            (platform, encoding),
            (3, 1) | (0, 3) | (0, 4) | (0, 6) | (3, 10)
        );
        if wanted && format == 4 {
            best = Some(off);
            // A (3,1) table is the one every tool agrees on; take it and stop.
            if platform == 3 && encoding == 1 {
                break;
            }
        }
    }
    let off = best?;
    Some(read_format4(b, off))
}

/// `format 4`: the BMP as ranges, each with either a delta or an index into a
/// shared array of glyph ids.
fn read_format4(b: &[u8], off: usize) -> BTreeMap<u32, u16> {
    let mut map = BTreeMap::new();
    let seg_x2 = u16_at(b, off + 6) as usize;
    let segs = seg_x2 / 2;
    let ends = off + 14;
    let starts = ends + seg_x2 + 2; // +2 for the reserved pad
    let deltas = starts + seg_x2;
    let ranges = deltas + seg_x2;

    for s in 0..segs {
        let end = u16_at(b, ends + s * 2);
        let start = u16_at(b, starts + s * 2);
        if start > end {
            continue;
        }
        let delta = u16_at(b, deltas + s * 2);
        let range_off = u16_at(b, ranges + s * 2);
        for c in start..=end {
            // 0xFFFF terminates the table rather than naming a character.
            if c == 0xFFFF {
                continue;
            }
            let glyph = if range_off == 0 {
                c.wrapping_add(delta)
            } else {
                // The offset is from the *slot it was read out of*, which is
                // what makes this table compact and this line look wrong.
                let at = ranges + s * 2 + range_off as usize + 2 * (c - start) as usize;
                let g = u16_at(b, at);
                if g == 0 {
                    continue;
                }
                g.wrapping_add(delta)
            };
            if glyph != 0 {
                map.insert(c as u32, glyph);
            }
        }
    }
    map
}
