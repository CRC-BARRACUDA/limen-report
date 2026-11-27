//! Stamping a report with a hash: of what it says, on every page, and of the
//! file it became, beside it.
//!
//! Two hashes, because one cannot do both jobs. A file cannot carry its own
//! hash — writing it in changes the file — so what is printed at the foot of
//! every page is the hash of the report's **content**: its title, its summary
//! and its figures, worked out before there is a document at all. The hash of
//! the **file** is worked out once the file exists and written beside it, in
//! the form `sha256sum -c` reads.
//!
//! The arithmetic is not this module's. The hashes come from whatever provides
//! `hash.calc` — the hash-checker module — which is why the choice is offered
//! only when that is installed, and why its list of algorithms is the list.

use crate::*;
use limen_sdk_rust::json;

/// One algorithm the user can stamp a report with.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HashOption {
    /// The name the hash provider knows it by: `sha256`.
    pub key: String,
    /// The name as it is written for people, and shown in the list: `SHA-256`.
    pub label: String,
}

/// The hash of a report's content, as its pages carry it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stamp {
    pub option: HashOption,
    pub hash: String,
}

/// The algorithms in a hash provider's answer to `algorithms`.
pub(crate) fn options_from(answer: &Value) -> Vec<HashOption> {
    arr_at(answer, "algorithms")
        .iter()
        .filter_map(|a| {
            let (key, label) = (str_at(a, "name"), str_at(a, "label"));
            (!key.is_empty() && !label.is_empty()).then(|| HashOption { key: key.to_string(), label: label.to_string() })
        })
        .collect()
}

/// The ways there are to stamp a report now: none when no hash provider is
/// installed, or when it would not say.
pub(crate) fn hash_options(host: &Host) -> Vec<HashOption> {
    if !host.has_capability("hash.calc") {
        return Vec::new();
    }
    host.call("hash.calc", "algorithms", json!({})).map(|a| options_from(&a)).unwrap_or_default()
}

/// What the reader chose under "Hash".
#[derive(Debug, PartialEq)]
pub(crate) enum Hashing {
    /// Not to: the first choice, or no answer at all.
    No,
    With(HashOption),
    /// Something that is not among the algorithms there are now — the hash
    /// module removed since the list was shown, say: what was picked.
    Gone(String),
}

/// Resolve the "hash" dropdown's answer — the label that was shown — against
/// the algorithms there are. "None" is none in whichever language.
pub(crate) fn hashing_choice(spec: &Value, options: &[HashOption]) -> Hashing {
    let said = str_at(spec, "hash").trim();
    if said.is_empty() || chose(said, "preview.hash_no") {
        return Hashing::No;
    }
    match options.iter().find(|o| o.label == said) {
        Some(option) => Hashing::With(option.clone()),
        None => Hashing::Gone(said.to_string()),
    }
}

/// JSON with nothing left to chance: an object's keys in order, no spaces.
fn written(v: &Value, out: &mut String) {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String(key.clone()).to_string());
                out.push(':');
                written(&o[key], out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, item) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                written(item, out);
            }
            out.push(']');
        }
        // Strings, numbers, true, false and null write themselves one way.
        other => out.push_str(&other.to_string()),
    }
}

/// What a report says, as the text its content hash is the hash of.
///
/// Its title, subtitle, summary, charts and tables — and nothing of how it is
/// printed: the same report on a white page and on a black one, in a file of
/// any name, signed or not, has the same content and so the same hash. Written
/// as JSON with the keys of every object in order and no spaces, so that the
/// same content is the same text whoever writes it down.
pub(crate) fn content_text(spec: &Value) -> String {
    let mut content = json!({});
    for part in ["title", "subtitle", "summary", "charts", "sections"] {
        if let Some(v) = spec.get(part) {
            content[part] = v.clone();
        }
    }
    let mut out = String::new();
    written(&content, &mut out);
    out
}

/// One hash out of a provider's answer.
fn hash_in(answer: &Value, key: &str) -> Result<String, String> {
    answer
        .get("hashes")
        .and_then(|h| h.get(key))
        .and_then(Value::as_str)
        .filter(|h| !h.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("the hash provider gave no {key}"))
}

/// The hash of `text` by `option`, from the hash provider.
pub(crate) fn hash_of_text(host: &Host, option: &HashOption, text: &str) -> Result<String, String> {
    let answer = host
        .call("hash.calc", "hash_text", json!({ "text": text, "algorithms": [option.key] }))
        .map_err(|e| e.message)?;
    hash_in(&answer, &option.key)
}

/// The hash of the file at `path` by `option`, from the hash provider.
pub(crate) fn hash_of_file(host: &Host, option: &HashOption, path: &std::path::Path) -> Result<String, String> {
    let answer = host
        .call("hash.calc", "hash_file", json!({ "path": path.to_string_lossy(), "algorithms": [option.key] }))
        .map_err(|e| e.message)?;
    hash_in(&answer, &option.key)
}

/// Where a document's own hash is written: beside it, under its whole name
/// with the algorithm's after it — `report.pdf.sha256`.
pub(crate) fn sidecar_path(document: &std::path::Path, option: &HashOption) -> std::path::PathBuf {
    let mut name = document.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(&option.key);
    document.with_file_name(name)
}

/// What is written there: the line `sha256sum` prints and `sha256sum -c`
/// reads — the hash, two spaces, the file's name.
pub(crate) fn sidecar_text(hash: &str, document: &std::path::Path) -> String {
    format!("{hash}  {}\n", document.file_name().unwrap_or_default().to_string_lossy())
}

/// The spec with the content hash its pages are to carry — or with none.
///
/// The only way one gets there, as with a signer: a `content_hash` in the
/// caller's spec is dropped by [`canonical`], because a hash printed on a
/// document is a claim about it, and it is this module that checks the claim.
pub(crate) fn stamped(mut canonical: Value, stamp: Option<&Stamp>) -> Value {
    if let (Some(o), Some(stamp)) = (canonical.as_object_mut(), stamp) {
        o.insert("content_hash".into(), json!({ "algorithm": stamp.option.label, "hash": stamp.hash }));
    }
    canonical
}
