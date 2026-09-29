//! The two marks the report is signed with, as geometry.
//!
//! Both are drawn rather than placed as images: they are flat-faceted shapes
//! with no gradients to lose, an image would have to be embedded at some
//! resolution and chosen wrong for one of print or screen, and a vector mark
//! stays sharp at whatever size a reader zooms to.
//!
//! The Limen mark's numbers are the icon's own 256-unit grid, the same ones
//! `scripts/make-icon.py` and the GUI draw from — a mark that drifts between
//! the app and its reports is two marks.

/// `[x0, y0, x1, y1]` rectangles on the 256 grid.
pub const THICK: f32 = 27.0;
pub const OUT_NEAR: f32 = 32.0;
pub const OUT_FAR: f32 = 224.0;
pub const ARM_NEAR: f32 = 105.0;
pub const ARM_FAR: f32 = 151.0;

/// One bracket, as three rectangles that share edges and never overlap.
pub fn bracket_rects(near: bool) -> [[f32; 4]; 3] {
    if near {
        let spine = OUT_NEAR + THICK;
        [
            [OUT_NEAR, OUT_NEAR, spine, OUT_FAR],
            [spine, OUT_NEAR, ARM_NEAR, OUT_NEAR + THICK],
            [spine, OUT_FAR - THICK, ARM_NEAR, OUT_FAR],
        ]
    } else {
        let spine = OUT_FAR - THICK;
        [
            [spine, OUT_NEAR, OUT_FAR, OUT_FAR],
            [ARM_FAR, OUT_NEAR, spine, OUT_NEAR + THICK],
            [ARM_FAR, OUT_FAR - THICK, spine, OUT_FAR],
        ]
    }
}

/// What stands in the gap: a tick level with each pair of arms, and the
/// crossing between them.
pub fn gap_marks() -> [[f32; 4]; 3] {
    [
        [120.0, 46.0, 136.0, 54.0],
        [124.0, 111.0, 132.0, 145.0],
        [120.0, 202.0, 136.0, 210.0],
    ]
}

/// The Barracuda logo's facets, parsed out of its own SVG.
///
/// Flattened to polygons: the file is `M`/`l`/`z` almost throughout, and the
/// three paths that carry a cubic are flattened to short segments. At the size
/// a masthead draws it, the difference is well under a printer's dot.
pub fn barracuda() -> Vec<Vec<(f32, f32)>> {
    const SVG: &str = include_str!("../resources/barracuda-white.svg");
    let mut out = Vec::new();
    let mut rest = SVG;
    while let Some(at) = rest.find("d=\"") {
        rest = &rest[at + 3..];
        let Some(end) = rest.find('"') else { break };
        let d = &rest[..end];
        rest = &rest[end + 1..];
        // `d="document"` and `d="stage"` are ids on the group elements, not
        // paths; a path always starts with a move.
        if !d.starts_with('M') && !d.starts_with('m') {
            continue;
        }
        let pts = parse_path(d);
        if pts.len() >= 3 {
            out.push(pts);
        }
    }
    out
}

/// One SVG `d` into a polyline: `M`/`m`, `L`/`l`, `H`/`h`, `V`/`v`, `C`/`c`
/// (flattened) and `Z`/`z`, absolute and relative, with the implicit repeats
/// SVG allows after the first operand set.
fn parse_path(d: &str) -> Vec<(f32, f32)> {
    let b = d.as_bytes();
    let mut i = 0;
    let mut cur = (0.0f32, 0.0f32);
    let mut pts: Vec<(f32, f32)> = Vec::new();
    let mut cmd = 0u8;

    let number = |b: &[u8], i: &mut usize| -> Option<f32> {
        while *i < b.len() && matches!(b[*i], b',' | b' ' | b'\n' | b'\t' | b'\r') {
            *i += 1;
        }
        let start = *i;
        if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') {
            *i += 1;
        }
        while *i < b.len() && (b[*i].is_ascii_digit() || b[*i] == b'.') {
            *i += 1;
        }
        // An exponent, which these coordinates do not use but a generator may.
        if *i < b.len() && (b[*i] == b'e' || b[*i] == b'E') {
            *i += 1;
            if *i < b.len() && (b[*i] == b'-' || b[*i] == b'+') {
                *i += 1;
            }
            while *i < b.len() && b[*i].is_ascii_digit() {
                *i += 1;
            }
        }
        (*i > start).then(|| std::str::from_utf8(&b[start..*i]).ok()?.parse().ok())?
    };

    while i < b.len() {
        match b[i] {
            b' ' | b',' | b'\n' | b'\t' | b'\r' => {
                i += 1;
                continue;
            }
            c if c.is_ascii_alphabetic() => {
                cmd = c;
                i += 1;
            }
            _ => {} // a number where a command could be: the previous one repeats
        }
        let rel = cmd.is_ascii_lowercase();
        match cmd.to_ascii_uppercase() {
            b'M' | b'L' => {
                let (Some(x), Some(y)) = (number(b, &mut i), number(b, &mut i)) else { break };
                cur = if rel { (cur.0 + x, cur.1 + y) } else { (x, y) };
                pts.push(cur);
                // A second pair after an `M` is a line, per the spec.
                if cmd == b'M' {
                    cmd = b'L';
                } else if cmd == b'm' {
                    cmd = b'l';
                }
            }
            b'H' => {
                let Some(x) = number(b, &mut i) else { break };
                cur = if rel { (cur.0 + x, cur.1) } else { (x, cur.1) };
                pts.push(cur);
            }
            b'V' => {
                let Some(y) = number(b, &mut i) else { break };
                cur = if rel { (cur.0, cur.1 + y) } else { (cur.0, y) };
                pts.push(cur);
            }
            b'C' => {
                let mut c = [0.0f32; 6];
                for slot in c.iter_mut() {
                    let Some(v) = number(b, &mut i) else { return pts };
                    *slot = v;
                }
                let p = |dx: f32, dy: f32| if rel { (cur.0 + dx, cur.1 + dy) } else { (dx, dy) };
                let (c1, c2, end) = (p(c[0], c[1]), p(c[2], c[3]), p(c[4], c[5]));
                // Eight segments: at masthead size a facet's curve spans a few
                // millimetres, where eight is already past what a printer can
                // resolve.
                for step in 1..=8 {
                    let t = step as f32 / 8.0;
                    let u = 1.0 - t;
                    pts.push((
                        u * u * u * cur.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * end.0,
                        u * u * u * cur.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * end.1,
                    ));
                }
                cur = end;
            }
            // A closed path: the polygon is closed by the renderer, and
            // nothing in this artwork continues after one.
            b'Z' => break,
            _ => break,
        }
    }
    pts
}

/// The bounding box of a set of facets, for fitting them into a rectangle.
pub fn bounds(polys: &[Vec<(f32, f32)>]) -> (f32, f32, f32, f32) {
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for poly in polys {
        for (x, y) in poly {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
    }
    (x0, y0, x1, y1)
}
