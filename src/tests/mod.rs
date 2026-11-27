//! What the module is expected to do, in the language of what it is for.
//!
//! A cdylib has no rlib to link an integration test against, so these live in
//! the crate: one file per part.

mod figures;
mod i18n;
mod pdf;
mod preview;
mod render_probe;
mod save;
mod stamp;
mod view;

/// Every control of the preview, in order, whichever row it stands on: the
/// rows are how they fit the pop-up, not what they are.
pub(crate) fn controls(view: &limen_sdk_rust::Value) -> Vec<limen_sdk_rust::Value> {
    view["widgets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["kind"] == "row")
        .flat_map(|w| w["children"].as_array().cloned().unwrap_or_default())
        .collect()
}
