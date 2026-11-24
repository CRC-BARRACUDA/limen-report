# limen-report

A [Limen](https://github.com/CRC-BARRACUDA/Limen) module that turns a **report
spec** into an in-app view or an exported document. It provides the
`report.build` capability and is an *optional* companion for data modules: they
hand it a spec, it renders tables + charts in a new tab, or writes a document
and saves it where you choose: a **branded PDF**.

Because it's optional, a data module only shows its "Make Report" action while
this module is installed (discovered via `host.capabilities()`). Install both,
for example, `devices` and `report`, and Devices gains a **Make Report** button.

## The PDF

The output this module exists for. One design, drawn for every module that asks:
a report from `devices` and a report from `ms-licenses` are the same document
with different rows in it.

```
  ▸ masthead   the Barracuda logo and the Limen mark, both as vector paths,
               with the title and subtitle under them — on EVERY page, because
               printed sheets get separated and a loose page of serial numbers
               has to say what it belongs to
  ▸ figures    the summary lines as a four-across grid, read by scanning
  ▸ charts     horizontal bars, labelled and valued
  ▸ tables     one per section, header band, alternating row tint, broken
               across pages as they run out of room
  ▸ footer     LIMEN · Barracuda Team · GPL-3.0-or-later · when it was made ·
               set in JetBrains Mono (SIL OFL 1.1)            and  n / total
```

Two themes, `"theme": "light" | "dark"`. Light is the default — a report is
printed more often than it is read on a screen, and a dark page sent to a
printer costs a cartridge. Dark is the app's own warm near-black, so a report
opened beside the window it came from is the same object. The amber does not
change between them: it is the brand, and a mark that changes colour with the
theme is two marks.

**There is no watermark.** The document says who made it, under what licence,
and when — it does not trace who exported it.

### Written by hand, with no dependencies

The smallest PDF crate that could do this brought **102 transitive packages**
into a module the host loads into its own process. What it would have answered
is a few hundred lines of byte-writing:

| file | what it is |
|---|---|
| `src/pdf/writer.rs` | objects, streams, the page tree, the cross-reference table, a composite font, a `ToUnicode` map |
| `src/pdf/font.rs` | four TrueType tables — `head`, `hhea`, `hmtx`, `cmap` — which is all a PDF needs from a face it embeds whole |
| `src/pdf/brand.rs` | both marks as geometry: the Limen brackets on the icon's own 256 grid, the Barracuda facets parsed out of its SVG |
| `src/pdf/layout.rs` | the design above |

Nothing is compressed. A report is a few hundred kilobytes either way once the
font is in it, and an uncompressed content stream is one a person can read with
`less` — which, for a tool whose output is evidence, is worth more than the
bytes.

**The text stays text.** JetBrains Mono is embedded and the glyphs are mapped
back to characters, so `pdftotext` — and anyone selecting a line — gets the
serial numbers out, in Ukrainian as well as English. A report nobody can copy a
serial out of is a picture of a report.

## Signing a report

The preview has a **Signature** choice beside the ground: *None*, or one of
your OpenPGP keys — every key GnuPG has of yours that can sign, listed once for
each of its user IDs, with the end of its fingerprint so that two tokens with
the same name on them are told apart. You pick there and then, each time; the
key set on Limen's OpenPGP page is only what other things sign with by default.
With one picked, saving a report also:

- writes a detached signature **beside the PDF, in the same folder** —
  `report.pdf.asc` — which anyone with your public key can check:
  `gpg --verify report.pdf.asc report.pdf`;
- names you at the foot of **every page**, under the usual footer: the user ID
  you picked (name and email) and the key's fingerprint. That user ID is
  written into the signature as well.

The key is yours and stays with your GnuPG. This module never sees it: it asks
Limen (`host.gpg_identities`, `host.gpg_sign_file` — the `gpg` permission),
Limen asks GnuPG, and GnuPG asks you for the passphrase or PIN. Which `gpg`
program to use is set on Limen's **OpenPGP** page. Limen
signs only the file you just saved through this module's own save dialog.

If the signature cannot be made — the token pulled out, the passphrase turned
down — the report is still saved, the view says it is not signed and
why, and its pages do not name a signer: one written naming you is written
again without that line.

A `signer` in a caller's spec is ignored. Who signed a report is for Limen to
say, from your own key, not for the module that asked for the report.

Needs a Limen that has the `gpg` permission; on an older one the report is
saved unsigned, and says so.

## Capability

`report.build` — method **`build(spec)`**:

```json
{
  "title": "Device Report",
  "subtitle": "2026-07-27",
  "theme": "light",
  "format": "view" | "pdf",
  "summary": ["77 devices", "66 connected"],
  "charts":  [ { "title": "By category", "data": [ {"label":"usb","value":41} ] } ],
  "sections":[ { "heading": "Connected", "columns": ["A","B"], "rows": [["1","2"]] } ]
}
```

- `format` defaults to **`view`** — the preview: a pop-up over the screen that
  asked for it, holding the report's own content, the number of sheets it comes
  to, a ground to print it on, and Save.
- `theme` is the ground: `light` (default) or `dark`.

There is one output, and it is a document. A page, a table of cells and a file
of Markdown are the data behind a report rather than the report — and a module
with data to hand over can hand it over itself.

## Build

```sh
# Release asset for this platform (report-<os>-<arch>.<ext> + .sha256):
scripts/package.sh
```

Native Rust (`cdylib`) built with
[`limen-sdk-rust`](https://github.com/CRC-BARRACUDA/Limen). Licensed
GPL-3.0-or-later.
