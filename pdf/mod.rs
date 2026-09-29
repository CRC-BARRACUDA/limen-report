//! Reports as PDF: the engine's printable output.
//!
//! Written here rather than with a PDF crate — the smallest one that could do
//! this brought a hundred transitive packages into a module the host loads into
//! its own process, to answer questions that are a few hundred lines of
//! byte-writing. See [`writer`] for the format and [`font`] for the four
//! TrueType tables a document needs.

pub mod brand;
pub mod font;
pub mod layout;
pub mod writer;

use std::sync::OnceLock;

use font::Font;

/// The face every report is set in — the app's own, so a printed report and the
/// screen it came from read as one product.
pub fn font() -> Option<&'static Font> {
    static F: OnceLock<Option<Font>> = OnceLock::new();
    F.get_or_init(|| Font::parse(include_bytes!("../resources/JetBrainsMono-Regular.ttf")))
        .as_ref()
}
