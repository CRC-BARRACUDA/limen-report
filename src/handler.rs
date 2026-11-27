//! The module itself: what it answers, and to which method.

use crate::*;

#[derive(Default)]
pub(crate) struct Report;

impl Handler for Report {
    fn capabilities(&self) -> Vec<String> {
        vec!["report.build".into()]
    }

    fn invoke(
        &mut self,
        _capability: &str,
        method: &str,
        params: Value,
        host: &Host,
    ) -> Result<Value, RpcError> {
        let lang = host.locale();
        let lang = lang.as_str();
        match method {
            "ui" => Ok(landing(lang)),
            "build" => Ok(build(&params, host, lang)),
            // Open what was just written, in whatever the system opens a PDF
            // with. Its own method rather than part of `build`, because it is
            // a different thing to ask for.
            "open" => {
                let path = params.get("path").and_then(Value::as_str).unwrap_or("");
                if !path.is_empty() {
                    host.open("path", path);
                }
                Ok(window(catalog().tr(lang, "ui.title"), vec![label(path).mono()]))
            }
            other => Err(RpcError::new(
                rpc::METHOD_NOT_FOUND,
                format!("report has no method {other}"),
            )),
        }
    }
}

/// Opening `report` on its own just explains what it's for.
pub(crate) fn landing(lang: &str) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    window(
        t("ui.title"),
        vec![
            label(t("ui.landing_title")).strong(),
            label(t("ui.landing_body")).weak(),
        ],
    )
}

// --------------------------------------------------------------------------- //
// Build
// --------------------------------------------------------------------------- //

/// Every name the one format goes by: what a module writes in a spec, and what
/// the preview's Save button sends. Both arrive in the same field, so both are
/// answered here rather than in two places that can disagree.
pub(crate) fn format_of(spec: &Value) -> &'static str {
    match str_at(spec, "format").trim() {
        "pdf" | "PDF" => "pdf",
        _ => "view",
    }
}

pub(crate) fn build(spec: &Value, host: &Host, lang: &str) -> Value {
    match format_of(spec) {
        "pdf" => match pdf::font() {
            Some(font) => {
                // One moment for the whole document, signed or not.
                let now = today();
                // The hash of what it says is settled before there is a page to
                // print it on: of the content, which no choice below changes.
                let stamp = match hashing_choice(spec, &hash_options(host)) {
                    Hashing::No => None,
                    Hashing::With(option) => Some(
                        hash_of_text(host, &option, &content_text(spec)).map(|hash| Stamp { option, hash }),
                    ),
                    Hashing::Gone(said) => Some(Err(catalog().tr(lang, "save.hash_gone").replace("{what}", &said))),
                };
                let content_hash = stamp.as_ref().and_then(|s| s.as_ref().ok());
                let render = |signer: Option<&limen_sdk_rust::GpgIdentity>| {
                    pdf::layout::render(&stamped(canonical(spec, lang, signer), content_hash), font, &now).bytes
                };
                // As whom it will be signed is settled first: every page names
                // them, and the pages are what is signed.
                let signer = match host.gpg_identities() {
                    Ok(identities) => match signing_choice(spec, &identities) {
                        Signing::No => None,
                        Signing::As(id) => Some(Ok(id)),
                        Signing::Gone(said) => Some(Err(catalog().tr(lang, "save.signer_gone").replace("{who}", &said))),
                    },
                    // No GnuPG to ask: only a problem if signing was asked for.
                    Err(e) => match signing_choice(spec, &[]) {
                        Signing::No => None,
                        _ => Some(Err(e.message)),
                    },
                };
                export_bytes(spec, host, "pdf", &render, signer, stamp.clone(), lang)
            }
            // The face is embedded in this library, so this cannot happen in a
            // build that shipped — but a report that silently came out as text
            // when a PDF was asked for would be worse than one that says so.
            None => {
                host.log("report: the embedded font could not be read; no PDF was written");
                Value::Null
            }
        },
        // "" | "view" | anything else → an in-app view the caller opens in a tab.
        // The ways there are to sign are asked for now, to offer them. An
        // older Limen, or one with no GnuPG: none to offer.
        // And the algorithms to stamp it with, if a hash module is installed.
        _ => render_view_full(spec, lang, &host.gpg_identities().unwrap_or_default(), &hash_options(host)),
    }
}
