//! The preview: the report as a Limen view, with the controls that make the
//! document.

use crate::*;

/// How much of a table the pop-up shows before it stops being a preview.
///
/// A preview is for answering "is this the right data", and four hundred rows
/// in a pop-up answers it no better than twelve while making the window
/// unusable. The document itself gets every row.
pub(crate) const PREVIEW_ROWS: usize = 12;

/// What the PDF will be, and the two controls that make it.
///
/// `identities`: the ways the user can sign what is saved — their keys, as
/// each of their user ids.
pub(crate) fn export_row(
    spec: &Value,
    lang: &str,
    identities: &[limen_sdk_rust::GpgIdentity],
) -> Vec<limen_sdk_rust::ui::Widget> {
    let t = |k: &str| catalog().tr(lang, k);
    let Some(font) = pdf::font() else {
        return Vec::new();
    };
    let pages = pdf::layout::render(&canonical(spec, lang, None), font, &today()).pages;
    let sheets = if pages == 1 {
        t("preview.page_one")
    } else {
        t("preview.pages").replace("{n}", &pages.to_string())
    };

    // The whole spec travels with the button, so the export does not depend on
    // this module having remembered anything: the view is a value, and so is
    // what it will turn into.
    //
    // The ground is *not* among the args. It comes from the dropdown beside the
    // button, as one of the view's own inputs — and the host merges a button's
    // args over those, so a theme set here would quietly win over whatever the
    // user chose.
    let mut args = spec.clone();
    if let Some(o) = args.as_object_mut() {
        // There is one format, so the button names it rather than asking.
        o.insert("format".into(), Value::String("pdf".into()));
        o.remove("theme");
        o.remove("sign");
    }

    vec![
        label(t("preview.meta").replace("{sheets}", &sheets)).weak(),
        row(vec![
            // One control, because there is one thing left to decide. A report
            // is a document; a page, a table of cells and a file of Markdown
            // are the data behind one, and a module with data to hand over can
            // hand it over itself.
            select("theme", vec![t("preview.white"), t("preview.black")])
                .label(t("preview.ground"))
                .default(theme_label(spec, lang)),
            // Whether to sign what is saved, and as whom: none, or one of the
            // user's keys as one of its user ids. Like the ground, it is the
            // reader's to choose here, so it is not among the button's args.
            select(
                "sign",
                std::iter::once(t("preview.sign_no")).chain(identities.iter().map(identity_label)).collect(),
            )
            .label(t("preview.sign"))
            .default(t("preview.sign_no")),
            button(t("preview.save"), "report.build", "build")
                .primary()
                .args(args),
            button(t("preview.close"), "report.build", "build").dismiss(),
        ]),
        separator(),
    ]
}

/// Which way round the dropdown starts: what the caller asked for, if it asked.
pub(crate) fn theme_label(spec: &Value, lang: &str) -> String {
    catalog().tr(
        lang,
        if ground_of(spec) == "dark" {
            "preview.black"
        } else {
            "preview.white"
        },
    )
}

/// Render the spec as a Limen view — and as a **preview of the document**.
///
/// The same content, in the same order, under the same headings the PDF uses,
/// with the line the page is footed by and the number of sheets it comes to.
/// It is not a picture of the file: a module cannot draw one, and one drawn in
/// widgets would be a second layout to keep in step with the first. What it is
/// instead is an answer to the two questions somebody asks before pressing
/// print — *is this the right data*, and *how long is it* — with the buttons
/// that produce the file sitting on the answer.
#[cfg(test)]
pub(crate) fn render_view(spec: &Value, lang: &str) -> Value {
    render_view_with(spec, lang, &[])
}

/// [`render_view`], offering `identities` as the ways to sign what is saved.
pub(crate) fn render_view_with(spec: &Value, lang: &str, identities: &[limen_sdk_rust::GpgIdentity]) -> Value {
    let t = |k: &str| catalog().tr(lang, k);
    let title = {
        let t = str_at(spec, "title");
        if t.is_empty() {
            catalog().tr(lang, "ui.title")
        } else {
            t.to_string()
        }
    };
    let mut w = vec![label(title.clone()).heading()];
    let subtitle = str_at(spec, "subtitle");
    if !subtitle.is_empty() {
        w.push(label(subtitle).weak());
    }
    w.extend(export_row(spec, lang, identities));
    for line in arr_at(spec, "summary") {
        if let Some(s) = line.as_str() {
            w.push(label(s));
        }
    }
    if !arr_at(spec, "charts").is_empty() || !arr_at(spec, "sections").is_empty() {
        w.push(separator());
    }
    for c in arr_at(spec, "charts") {
        let data = chart_data(c);
        if !data.is_empty() {
            w.push(chart(str_at(c, "title"), data));
            w.push(separator());
        }
    }
    for section in arr_at(spec, "sections") {
        let heading = str_at(section, "heading");
        if !heading.is_empty() {
            w.push(label(heading).strong());
        }
        let all = rows(section);
        let shown: Vec<Vec<String>> = all.iter().take(PREVIEW_ROWS).cloned().collect();
        let hidden = all.len().saturating_sub(shown.len());
        w.push(table(columns(section), shown));
        if hidden > 0 {
            // Said out loud, because a preview that quietly shows twelve of
            // four hundred rows is a preview of a different document.
            w.push(label(t("preview.more_rows").replace("{n}", &hidden.to_string())).weak());
        }
        w.push(separator());
    }
    // What every page of the document is signed with, shown once here.
    w.push(
        label(t("doc.footer").replace("{generated}", &today())).weak(),
    );
    // A pop-up over the screen that asked for it, rather than a page that
    // replaces it: a report is something you look at and then decide about,
    // and the module underneath is what you decide to go back to.
    window_modal_sized(title, "report.preview", 900.0, w)
}
