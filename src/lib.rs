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
//!
//! ## Signing
//! The preview offers to sign what is saved with one of the user's own OpenPGP
//! keys: a list of them, each as each of its user ids, chosen there and then.
//! The keys are theirs and stay with their GnuPG: this module asks Limen
//! (`host.gpg_identities`, `host.gpg_sign_file` — the `gpg` permission), Limen
//! asks GnuPG, and GnuPG asks the user. The signature is a file beside the document —
//! `report.pdf.asc` — so the PDF itself is as it would have been, and anyone
//! with the user's public key can check it: `gpg --verify report.pdf.asc
//! report.pdf`. A report that could not be signed is still saved, and says so.

mod handler;
mod pdf;
mod save;
mod sign;
mod spec;
mod stamp;
mod view;

#[cfg(test)]
mod tests;

pub(crate) use handler::*;
pub(crate) use save::*;
pub(crate) use sign::*;
pub(crate) use spec::*;
pub(crate) use stamp::*;
pub(crate) use view::*;

pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use limen_sdk_rust::ui::{
    button, chart, label, row, select, separator, table, window, window_modal_sized,
};
pub(crate) use limen_sdk_rust::{rpc, Catalog, Handler, Host, RpcError, Value};

use limen_sdk_rust::export_module;

/// Every word this module shows, in each language it has.
///
/// English lives in a file beside the Ukrainian rather than in the code: a
/// string written into the source is a string nobody can translate.
pub(crate) fn catalog() -> &'static Catalog {
    static C: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        Catalog::new(&[
            ("en", include_str!("../locales/en.toml")),
            ("uk", include_str!("../locales/uk.toml")),
        ])
    })
}

export_module!(Report);
