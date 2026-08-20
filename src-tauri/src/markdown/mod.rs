//! Markdown → Typst conversion pipeline (§6 of SPEC.md).
//!
//! Pure Rust — no `tauri` imports, so this is unit-testable with plain
//! `cargo test`.

pub mod convert;
pub mod escape;
pub mod frontmatter;
pub mod suggest;

use comrak::Options;

use crate::options::FeatureFlags;

/// comrak parser options for an effective extension set (§6.1).
///
/// Two things are always on regardless of flavor: the front-matter
/// delimiter (front matter is parsed in every flavor so `title`/`author`
/// and the document settings work everywhere) and `header_attributes`
/// (heading IDs are core app behavior — anchor links, TOC — not a flavor
/// extension; labels are invisible metadata). Autolink is always OFF: the
/// app prefers explicit bare links (`<https://…>`, SPEC §4).
pub fn comrak_options(flags: &FeatureFlags) -> Options<'static> {
    Options {
        extension: comrak::options::Extension {
            strikethrough: flags.strikethrough,
            table: flags.table,
            autolink: false,
            tasklist: flags.task_list,
            footnotes: flags.footnotes,
            math_dollars: flags.math,
            highlight: flags.highlight,
            superscript: flags.superscript,
            subscript: flags.subscript,
            description_lists: flags.definition_list,
            front_matter_delimiter: Some("---".to_owned()),
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
        let commonmark = comrak_options(&Flavor::Commonmark.preset()).extension;
        assert!(!commonmark.strikethrough);
        assert!(!commonmark.table);
        assert!(!commonmark.autolink);
        assert!(!commonmark.tasklist);
        assert!(!commonmark.footnotes);
        assert!(!commonmark.math_dollars);
        assert!(!commonmark.highlight);
        assert_eq!(commonmark.front_matter_delimiter.as_deref(), Some("---"));
        assert!(commonmark.header_attributes, "header_attributes always on");

        let github = comrak_options(&Flavor::Github.preset()).extension;
        assert!(github.strikethrough);
        assert!(github.table);
        assert!(!github.autolink, "autolink stays off in every flavor");
        assert!(github.tasklist);
        assert!(!github.footnotes);
        assert!(!github.math_dollars);
        assert!(!github.highlight);
        assert_eq!(github.front_matter_delimiter.as_deref(), Some("---"));
        assert!(github.header_attributes, "header_attributes always on");

        let custom = comrak_options(&Flavor::Custom.preset()).extension;
        assert!(custom.strikethrough);
        assert!(custom.table);
        assert!(!custom.autolink, "autolink stays off in every flavor");
        assert!(custom.tasklist);
        assert!(custom.footnotes);
        assert!(custom.math_dollars);
        assert!(custom.highlight);
        assert!(custom.superscript);
        assert!(custom.subscript);
        assert!(custom.description_lists);
        assert_eq!(custom.front_matter_delimiter.as_deref(), Some("---"));
        assert!(custom.header_attributes, "header_attributes always on");
    }

    #[test]
    fn smart_quotes_never_enabled() {
        // §6.1: parse.smart would re-type the `"` the converter leaves alone.
        for flavor in [Flavor::Commonmark, Flavor::Github, Flavor::Custom] {
            let flags = flavor.preset();
            assert!(!comrak_options(&flags).parse.smart, "flavor: {flavor:?}");
        }
    }
}
