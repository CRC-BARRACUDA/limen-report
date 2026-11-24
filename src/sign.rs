//! Signing a report with one of the user's own OpenPGP keys: what there is to
//! choose from, and what was chosen.

use crate::*;

/// How a way of signing reads in the preview's list: the user id, and the end
/// of the key's fingerprint — two tokens carrying the same name and address
/// are told apart by it.
pub(crate) fn identity_label(id: &limen_sdk_rust::GpgIdentity) -> String {
    let f = id.fingerprint.as_str();
    let tail = &f[f.len().saturating_sub(16)..];
    let fours: Vec<&str> = tail.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap_or("")).collect();
    format!("{} · {}", id.user_id, fours.join(" "))
}

/// What the reader chose under "Signature".
#[derive(Debug, PartialEq)]
pub(crate) enum Signing {
    /// Not to sign: the first choice, or no answer at all.
    No,
    /// To sign as this one.
    As(limen_sdk_rust::GpgIdentity),
    /// Something that is not among the ways there are to sign now — a token
    /// pulled out since the list was shown, say: what was picked.
    Gone(String),
}

/// Resolve the "sign" dropdown's answer — the label that was shown — against
/// the ways there are to sign. "None" is none in whichever language.
pub(crate) fn signing_choice(spec: &Value, identities: &[limen_sdk_rust::GpgIdentity]) -> Signing {
    let said = str_at(spec, "sign").trim();
    if said.is_empty() || chose(said, "preview.sign_no") {
        return Signing::No;
    }
    match identities.iter().find(|id| identity_label(id) == said) {
        Some(id) => Signing::As(id.clone()),
        None => Signing::Gone(said.to_string()),
    }
}
