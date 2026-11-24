//! The name a report is saved under.

use crate::*;
use limen_sdk_rust::json;

/// The name the save dialog opens with: what it is of, then when.
///
/// A machine's report is filed under the machine — hostname, serial if
/// there is one, date — so the filename answers "which box, which one of
/// them, when" without opening it.
#[test]
fn the_suggested_name_is_the_callers_subject_and_the_date() {
    let spec = json!({ "title": "This machine", "file_name": "OkPrepareYourAss_PF3ZK9QW" });
    let name = suggested_name(&spec, "pdf");
    assert!(name.starts_with("OkPrepareYourAss_PF3ZK9QW_"), "{name}");
    assert!(name.ends_with(".pdf"));
    // `_YYYY-MM-DD.pdf`
    let date = &name[name.len() - 14..name.len() - 4];
    assert_eq!(date.len(), 10);
    assert_eq!(date.matches('-').count(), 2, "{date}");
    assert_eq!(&date[..2], "20", "{date}");
}

/// Case is kept. A hostname lower-cased is a hostname nobody recognises.
#[test]
fn a_hostname_keeps_its_shape() {
    assert_eq!(file_stem("OkPrepareYourAss"), "OkPrepareYourAss");
    assert_eq!(file_stem("host.example.org"), "host.example.org");
    assert_eq!(file_stem("PF3ZK9QW"), "PF3ZK9QW");
}

/// And anything a filesystem or a mail client would argue about does not
/// survive: a serial is whatever the vendor wrote, including slashes.
#[test]
fn a_filename_carries_nothing_a_path_would_argue_with() {
    assert_eq!(file_stem("a/b"), "a_b");
    assert_eq!(file_stem("Звіт про пристрої"), "Звіт_про_пристрої");
    assert_eq!(file_stem("  spaced  out  "), "spaced_out");
    assert_eq!(file_stem("SN: 12/34*56?"), "SN_12_34_56");
    for name in [file_stem(""), file_stem("///"), file_stem("···")] {
        assert_eq!(name, "report", "a nameless report still has a filename");
    }
    assert!(file_stem(&"x".repeat(200)).chars().count() <= 80);
}

/// A caller that says nothing is filed under its title, as before.
#[test]
fn without_a_name_the_title_is_the_name() {
    let name = suggested_name(&json!({ "title": "Device Report" }), "pdf");
    assert!(name.starts_with("device-report_"), "{name}");
    assert!(name.ends_with(".pdf"));
}
