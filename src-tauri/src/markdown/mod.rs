//! Markdown → Typst conversion pipeline (§6 of SPEC.md).
//!
//! Pure Rust — no `tauri` imports, so this is unit-testable with plain
//! `cargo test`.

pub mod convert;
pub mod escape;

use comrak::Options;

use crate::options::Flavor;

/// comrak parser options per flavor (§6.1).
pub fn comrak_options(flavor: Flavor) -> Options<'static> {
    let gfm = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    Options {
        extension: comrak::options::Extension {
            strikethrough: gfm,
            table: gfm,
            autolink: gfm,
            tasklist: gfm,
            footnotes: flavor == Flavor::Custom,
            math_dollars: flavor == Flavor::Custom,
            front_matter_delimiter: (flavor == Flavor::Custom).then(|| "---".to_owned()),
            // Heading IDs (auto slug + {#custom-id} override) are core app
            // behavior (anchor links, TOC), not a flavor extension. Labels are
            // invisible metadata — they don't affect rendering. Enabling
            // header_attributes in all flavors keeps the feature consistent.
            header_attributes: true,
            ..Default::default()
        },
        ..Options::default()
    }
}

#[cfg(test)]
mod tests {
    use super::comrak_options;
    use crate::options::Flavor;

    #[test]
    fn extension_matrix_matches_flavor_table() {
        let commonmark = comrak_options(Flavor::Commonmark).extension;
        assert!(!commonmark.strikethrough);
        assert!(!commonmark.table);
        assert!(!commonmark.autolink);
        assert!(!commonmark.tasklist);
        assert!(!commonmark.footnotes);
        assert!(!commonmark.math_dollars);
        assert_eq!(commonmark.front_matter_delimiter, None);
        assert!(commonmark.header_attributes, "header_attributes always on");

        let gfm = comrak_options(Flavor::Gfm).extension;
        assert!(gfm.strikethrough);
        assert!(gfm.table);
        assert!(gfm.autolink);
        assert!(gfm.tasklist);
        assert!(!gfm.footnotes);
        assert!(!gfm.math_dollars);
        assert_eq!(gfm.front_matter_delimiter, None);
        assert!(gfm.header_attributes, "header_attributes always on");

        let custom = comrak_options(Flavor::Custom).extension;
        assert!(custom.strikethrough);
        assert!(custom.table);
        assert!(custom.autolink);
        assert!(custom.tasklist);
        assert!(custom.footnotes);
        assert!(custom.math_dollars);
        assert_eq!(custom.front_matter_delimiter.as_deref(), Some("---"));
        assert!(custom.header_attributes, "header_attributes always on");
    }

    #[test]
    fn smart_quotes_never_enabled() {
        // §6.1: parse.smart would re-type the `"` the converter leaves alone.
        for flavor in [Flavor::Commonmark, Flavor::Gfm, Flavor::Custom] {
            assert!(!comrak_options(flavor).parse.smart, "flavor: {flavor:?}");
        }
    }
}
