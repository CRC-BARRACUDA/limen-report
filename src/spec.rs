//! Reading a report spec: what a caller sent, as the rest of the module wants it.

use crate::*;

/// Whether a dropdown's answer is this choice, in whichever language it was
/// shown in. An option is its own value, so a person reading Ukrainian sends
/// Ukrainian back — and a module's spec may say "dark" in neither.
pub(crate) fn chose(answer: &str, key: &str) -> bool {
    ["en", "uk"].iter().any(|lang| catalog().tr(lang, key) == answer)
}

/// The ground, named the one way the renderer knows it.
///
/// The dropdown sends a word in the reader's language and a module's spec says
/// `light`/`dark`; both mean the same thing, and the document should not have to
/// know about either.
pub(crate) fn ground_of(spec: &Value) -> &'static str {
    let said = str_at(spec, "theme");
    if said == "dark" || said == "Dark" || chose(said, "preview.black") {
        "dark"
    } else {
        "light"
    }
}

/// The spec as the renderer wants it: the ground named canonically, the
/// language carried along so the document's own lines can be written in it —
/// and, when the document is to be signed, whose key signs it, for the foot of
/// every page.
///
/// `signer` is the only way one gets there. A `signer` in the caller's spec is
/// dropped: who signed a document is for Limen to say, from the user's own
/// key, not for the module that asked for the report.
pub(crate) fn canonical(spec: &Value, lang: &str, signer: Option<&limen_sdk_rust::GpgIdentity>) -> Value {
    let mut out = spec.clone();
    if let Some(o) = out.as_object_mut() {
        o.insert("theme".into(), Value::String(ground_of(spec).into()));
        o.insert("lang".into(), Value::String(lang.to_string()));
        o.remove("signer");
        // Likewise a content hash: this module works it out, or there is none.
        o.remove("content_hash");
        if let Some(id) = signer {
            o.insert(
                "signer".into(),
                limen_sdk_rust::json!({ "user_id": id.user_id, "fingerprint": id.fingerprint }),
            );
        }
    }
    out
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
pub(crate) fn cell_str(v: &Value) -> String {
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

/// Integers without a decimal, otherwise two places.
pub(crate) fn fmt_num(v: f64) -> String {
    if v.fract().abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}
