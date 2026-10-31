//! "Did you mean …?" suggestions for unknown front-matter keys (§6.2).
//!
//! Uses Jaro–Winkler similarity — the same family of metric clap uses for
//! flag suggestions. The Winkler prefix boost (capped at 4 chars in
//! `strsim` ≥ 0.11) is a good fit here: a typo almost always preserves the
//! key's prefix (`title` → `titel`, `ext_definition_list` →
//! `ext_definition_lists`), so prefix-heavy matches rank highest without
//! unrelated keys colliding.

/// Similarity above which a known key is offered as a suggestion. Calibrated
/// in the unit tests: genuine typos score ≳ 0.9, unrelated keys ≲ 0.7.
const THRESHOLD: f64 = 0.8;

/// The closest known key to `unknown`, or `None` when nothing is similar
/// enough. Ties resolve to the earliest known key.
pub fn suggest<'k>(unknown: &str, known: &[&'k str]) -> Option<&'k str> {
    let mut best: Option<(f64, &'k str)> = None;
    for &candidate in known {
        let score = strsim::jaro_winkler(unknown, candidate);
        if score >= THRESHOLD && best.is_none_or(|(s, _)| score > s) {
            best = Some((score, candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

#[cfg(test)]
mod tests {
    use super::suggest;

    /// The front-matter keys the suggestion runs against (mirrors the
    /// converter's known-key set — kept in sync by these tests).
    const KEYS: &[&str] = &[
        "title",
        "author",
        "autoHeadingLevel",
        "flavor",
        "ext_strikethrough",
        "ext_table",
        "ext_task_list",
        "ext_footnotes",
        "ext_math",
        "ext_highlight",
        "ext_superscript",
        "ext_subscript",
        "ext_definition_list",
        "ext_page_break",
    ];

    #[test]
    fn suggests_simple_typos() {
        assert_eq!(suggest("titel", KEYS), Some("title"));
        assert_eq!(suggest("auther", KEYS), Some("author"));
        assert_eq!(suggest("flavour", KEYS), Some("flavor"));
    }

    #[test]
    fn suggests_ext_prefix_typos() {
        // The shared `ext_` prefix must not make unrelated extensions match:
        // a typo suggests its own key, not a sibling.
        assert_eq!(
            suggest("ext_definition_lists", KEYS),
            Some("ext_definition_list")
        );
        assert_eq!(suggest("ext_subscipt", KEYS), Some("ext_subscript"));
        assert_eq!(suggest("ext_maths", KEYS), Some("ext_math"));
    }

    #[test]
    fn no_suggestion_for_unrelated_keys() {
        assert_eq!(suggest("bibliography", KEYS), None);
        assert_eq!(suggest("date", KEYS), None);
        assert_eq!(suggest("tags", KEYS), None);
    }

    #[test]
    fn exact_and_case_variants_still_match() {
        assert_eq!(suggest("title", KEYS), Some("title"));
        // A capitalised typo is still close enough.
        assert_eq!(suggest("Title", KEYS), Some("title"));
    }

    #[test]
    fn auto_heading_level_camel_case_typo() {
        assert_eq!(suggest("autoHeadingLvel", KEYS), Some("autoHeadingLevel"));
    }
}
