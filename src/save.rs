//! Writing the document where the user chose — signed, if they asked — and the
//! name and date it goes under.

use crate::*;

/// Ask the user where the document goes, write it there, and say what happened.
///
/// A dialog rather than a temp folder: a report is a thing somebody keeps —
/// attached to a ticket, filed with a case — and a file written to
/// `/tmp/limen-reports` with a unix timestamp in its name is one they have to
/// go and find, on a path that is cleared on the next boot.
///
/// Answers with a **view**, never with null: this is what a button press
/// renders, and a module that answers a click with nothing leaves the tab it
/// was pressed in showing an error about a view it did not get.
/// `render` draws the document, naming the signer it is given on every page.
/// `signer`: None when no signature was asked for; else whose key will sign —
/// or why there is none to sign with.
/// `stamp`: None when no hash was asked for; else the hash of the content the
/// pages carry — or why there is none.
pub(crate) fn export_bytes(
    spec: &Value,
    host: &Host,
    ext: &str,
    render: &dyn Fn(Option<&limen_sdk_rust::GpgIdentity>) -> Vec<u8>,
    signer: Option<Result<limen_sdk_rust::GpgIdentity, String>>,
    stamp: Option<Result<Stamp, String>>,
    lang: &str,
) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    // A document says it is signed only when there is a key to sign it with.
    let identity = signer.as_ref().and_then(|s| s.as_ref().ok());
    let content = render(identity);
    let suggested = suggested_name(spec, ext);
    let Some(path) = host.save_file(&suggested) else {
        // Cancelled. Not an error, and not silence either — the pop-up is gone
        // by now, and a click that appears to have done nothing is a click
        // somebody presses again.
        return window(t("ui.title"), vec![label(t("save.cancelled")).weak()]);
    };
    let path = std::path::PathBuf::from(&path);
    // The dialog is where the name is chosen, but a name typed without one is
    // a file the system will not know how to open.
    let path = if path.extension().is_some() {
        path
    } else {
        path.with_extension(ext)
    };
    match std::fs::write(&path, &content) {
        Ok(()) => {
            host.log(&format!("report: wrote {}", path.display()));
            let mut said = vec![
                label(t("save.saved")).strong(),
                label(path.to_string_lossy().into_owned()).mono(),
            ];
            // Signed after it is written, and only if asked for: the document
            // is saved either way, and the view says which it was.
            match &signer {
                None => {}
                Some(Err(why)) => {
                    host.log(&format!("report: not signed: {why}"));
                    said.push(label(t("save.not_signed")).strong());
                    said.push(label(why.clone()).weak());
                }
                Some(Ok(who)) => match host.gpg_sign_file(&path.to_string_lossy(), Some(who)) {
                    Ok(signed) => {
                        host.log(&format!("report: signed as {}", signed.user_id));
                        said.push(label(t("save.signed").replace("{who}", &signed.user_id)).strong());
                        // Beside the document, in the folder it was saved to.
                        said.push(label(signed.signature).mono());
                    }
                    Err(e) => {
                        // Its pages say it is signed, and it is not: written
                        // again without that line, so it claims nothing.
                        let plain = std::fs::write(&path, render(None));
                        host.log(&format!(
                            "report: not signed: {}{}",
                            e.message,
                            if plain.is_ok() { "; saved again without the signer's line" } else { "" }
                        ));
                        said.push(label(t("save.not_signed")).strong());
                        said.push(label(e.message).weak());
                        if plain.is_err() {
                            said.push(label(t("save.claims_signature")).weak());
                        }
                    }
                },
            }
            // Stamped last, once the file is what it is going to be: a signing
            // that failed writes it again, and its hash is of what is there.
            match &stamp {
                None => {}
                Some(Err(why)) => {
                    host.log(&format!("report: not hashed: {why}"));
                    said.push(label(t("save.not_hashed")).strong());
                    said.push(label(why.clone()).weak());
                }
                Some(Ok(stamp)) => {
                    let what = stamp.option.label.as_str();
                    said.push(label(t("save.content_hash").replace("{what}", what)).strong());
                    said.push(label(stamp.hash.clone()).mono());
                    // The file's own, beside it — what a recipient checks the
                    // file they were sent against.
                    let beside = sidecar_path(&path, &stamp.option);
                    let kept = hash_of_file(host, &stamp.option, &path)
                        .and_then(|hash| std::fs::write(&beside, sidecar_text(&hash, &path)).map_err(|e| e.to_string()));
                    match kept {
                        Ok(()) => {
                            host.log(&format!("report: wrote {}", beside.display()));
                            said.push(label(t("save.file_hash").replace("{what}", what)).strong());
                            said.push(label(beside.to_string_lossy().into_owned()).mono());
                        }
                        Err(why) => {
                            host.log(&format!("report: the file's hash was not written: {why}"));
                            said.push(label(t("save.no_file_hash")).strong());
                            said.push(label(why).weak());
                        }
                    }
                }
            }
            said.push(row(vec![
                button("Open", "report.build", "open")
                    .primary()
                    .args(limen_sdk_rust::json!({ "path": path.to_string_lossy() })),
            ]));
            window(t("ui.title"), said)
        }
        Err(e) => {
            host.log(&format!("report: failed to write {}: {e}", path.display()));
            window(
                t("ui.title"),
                vec![
                    label(t("save.failed")).strong(),
                    label(format!("{}: {e}", path.display())).weak(),
                ],
            )
        }
    }
}

/// What the save dialog opens with: `<what it is about>_<date>.<ext>`.
///
/// The stem is the calling module's — `file_name` in the spec — because only it
/// knows what the report is *of*. A machine's report is filed under the machine:
/// `OkPrepareYourAss_PF3ZK9QW_2026-09-29.pdf` is a name that sorts with its
/// siblings and answers "which box, which one of them, when" from the filename
/// alone. A caller that says nothing gets its title instead.
///
/// The date is added here rather than by each caller, so every report this
/// engine produces is dated the same way whoever asked for it.
pub(crate) fn suggested_name(spec: &Value, ext: &str) -> String {
    let stem = match str_at(spec, "file_name") {
        "" => slug(str_at(spec, "title")),
        given => file_stem(given),
    };
    format!("{stem}_{}.{ext}", today_date())
}

/// A caller's stem, made safe for a filesystem without being flattened.
///
/// Case is kept — a hostname is `OkPrepareYourAss` and a serial is `PF3ZK9QW`,
/// and lower-casing them makes a name nobody recognises. Everything a path or a
/// mail client argues about becomes an underscore.
pub(crate) fn file_stem(s: &str) -> String {
    let mut out = String::new();
    let mut prev = false;
    for ch in s.chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '.' {
            out.push(ch);
            prev = false;
        } else if !prev {
            out.push('_');
            prev = true;
        }
    }
    let trimmed = out.trim_matches(['_', '.', '-']).to_string();
    // 80 characters is past any hostname and serial together, and short of
    // what a filesystem or a mail client starts truncating.
    let trimmed: String = trimmed.chars().take(80).collect();
    if trimmed.is_empty() {
        "report".to_string()
    } else {
        trimmed
    }
}

/// Today, as `YYYY-MM-DD` — the date part of [`today`].
pub(crate) fn today_date() -> String {
    today().chars().take(10).collect()
}

// --------------------------------------------------------------------------- //
// Renderers
// --------------------------------------------------------------------------- //

// --------------------------------------------------------------------------- //
// Small helpers
// --------------------------------------------------------------------------- //

/// A filename-safe slug (lowercase alphanumerics, dashes between).
pub(crate) fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() { "report".to_string() } else { trimmed }
}

/// Today, as `YYYY-MM-DD` — what a report is filed under.
///
/// Derived from the clock rather than read from anywhere: this module has no
/// permission to run `date`, and a report with no date on it is a report nobody
/// can put in order later.
pub(crate) fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let (h, m) = ((secs.rem_euclid(86_400)) / 3600, (secs.rem_euclid(3600)) / 60);
    // Civil-from-days: exact, and shorter than depending on a date library.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02} UTC")
}
