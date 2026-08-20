//! Front-matter scanning and parsing (§6.2).
//!
//! Front matter is a leading `---`-delimited block of `key: value` lines,
//! parsed in **every** flavor. It carries the document's settings — `title`,
//! `author`, `autoHeadingLevel`, `flavor`, and `ext_*` extension overrides —
//! which override the toolbar options. Unknown keys warn (with a "did you
//! mean …?" suggestion); the block is otherwise left untouched.
//!
//! Only `key: value` syntax is supported (one pair per line, no YAML crate).
//! A leading `---` block that is *not* valid front matter (e.g. a thematic
//! break followed later by a horizontal rule) is detected and the leading
//! `---` is stripped from a transient copy before parsing, so the swallowed
//! content is recovered — the original document is never mutated.

use std::borrow::Cow;

use serde::Deserialize;

use crate::markdown::suggest;
use crate::options::{Feature, FeatureFlags, Flavor};

/// The document settings parsed from front matter, plus any warnings raised
/// while parsing (unknown keys, invalid values).
#[derive(Debug, Default)]
pub struct Settings {
    pub title: Option<String>,
    pub author: Option<String>,
    /// Document-declared flavor, when the front matter sets one.
    pub flavor: Option<Flavor>,
    /// Document-declared `ext_*` overrides, applied on top of the flavor
    /// preset (first occurrence of a key wins).
    pub ext_overrides: Vec<(Feature, bool)>,
    /// Document-declared auto heading level, when set and numeric.
    pub auto_heading_level: Option<u8>,
    pub warnings: Vec<String>,
}

/// The outcome of scanning the document's leading front matter.
pub struct Scan<'a> {
    /// The source to hand to comrak: the original, or a transient copy with
    /// the leading `---` removed when the block was not valid front matter.
    pub source: Cow<'a, str>,
    /// Parsed settings, or `None` when there is no (valid) front matter.
    pub settings: Option<Settings>,
    /// Scan-level warnings (e.g. the stripped-delimiter notice). Settings
    /// warnings live in `Settings::warnings`.
    pub warnings: Vec<String>,
}

/// Scan the document's leading front matter. See module docs.
pub fn scan(markdown: &str) -> Scan<'_> {
    let body = strip_bom(markdown);
    let Some(block) = leading_block(body) else {
        return Scan {
            source: Cow::Borrowed(markdown),
            settings: None,
            warnings: Vec::new(),
        };
    };

    if !is_valid_block(block) {
        // Not front matter (likely a thematic break mistaken for one). Drop
        // the leading `---` from a transient copy so comrak does not swallow
        // the content between it and the next `---`; warn the user.
        let patched = strip_leading_delimiter(markdown);
        return Scan {
            source: Cow::Owned(patched),
            settings: None,
            warnings: vec![
                "leading `---` is not valid front matter (expected `key: value` lines) — \
                 treated as content, not settings"
                    .to_string(),
            ],
        };
    }

    Scan {
        source: Cow::Borrowed(markdown),
        settings: Some(parse_block(block)),
        warnings: Vec::new(),
    }
}

/// The known top-level front-matter keys — the suggestion dictionary.
fn known_keys() -> Vec<&'static str> {
    ["title", "author", "autoHeadingLevel", "flavor"]
        .into_iter()
        .chain(Feature::ALL.iter().map(|f| f.ext_key()))
        .collect()
}

/// A settings patch from the UI (§6.2). `None` leaves the existing value
/// untouched; `Some` sets it — for `title`/`author` an empty string clears
/// the key. `features` is the *effective* extension set; it is written back
/// in the XOR-minimal encoding (`flavor:` + only the differing `ext_*`).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Patch {
    pub title: Option<String>,
    pub author: Option<String>,
    pub auto_heading_level: Option<u8>,
    pub features: Option<FeatureFlags>,
}

/// Rewrite the document's front matter with `patch` applied, preserving any
/// keys this app does not manage (and the body) verbatim. Managed keys are
/// canonicalized to the top of the block. Returns the full new document.
pub fn apply_patch(markdown: &str, patch: &Patch) -> String {
    let bom = if markdown.starts_with('\u{feff}') {
        "\u{feff}"
    } else {
        ""
    };
    let src = strip_bom(markdown);

    let (existing_block, body) = match split_front_matter(src) {
        Some((block, body)) => (Some(block), body),
        None => (None, src),
    };
    let existing = existing_block
        .map(parse_block_for_patch)
        .unwrap_or_default();

    // Merge the patch onto the existing values (None = keep existing).
    let title = patch.title.clone().or(existing.title);
    let author = patch.author.clone().or(existing.author);
    let auto = patch.auto_heading_level.or(existing.auto_heading_level);

    let mut lines: Vec<String> = Vec::new();
    if let Some(t) = title.as_deref().filter(|t| !t.is_empty()) {
        lines.push(format!("title: {}", yaml_value(t)));
    }
    if let Some(a) = author.as_deref().filter(|a| !a.is_empty()) {
        lines.push(format!("author: {}", yaml_value(a)));
    }
    if let Some(n) = auto {
        lines.push(format!("autoHeadingLevel: {n}"));
    }
    match patch.features {
        Some(flags) => {
            let (flavor, overrides) = flags.encode();
            lines.push(format!("flavor: {}", flavor.as_str()));
            for (feature, on) in overrides {
                lines.push(format!("{}: {}", feature.ext_key(), on));
            }
        }
        None => {
            // Keep the existing flavor / overrides representation as-is.
            if let Some(flavor) = existing.flavor {
                lines.push(format!("flavor: {}", flavor.as_str()));
            }
            for (feature, on) in &existing.ext_overrides {
                lines.push(format!("{}: {}", feature.ext_key(), on));
            }
        }
    }
    // Preserved (unmanaged) lines keep their original relative order.
    lines.extend(existing.preserved.iter().cloned());

    if lines.is_empty() {
        // Nothing managed and nothing preserved — drop the block entirely.
        return format!("{bom}{body}");
    }
    format!("{bom}---\n{}---\n{body}", lines.join("\n") + "\n")
}

/// The existing front matter split into managed settings and the raw lines
/// this app does not manage (unknown keys, comments) that must be preserved.
#[derive(Default)]
struct ExistingBlock {
    title: Option<String>,
    author: Option<String>,
    auto_heading_level: Option<u8>,
    flavor: Option<Flavor>,
    ext_overrides: Vec<(Feature, bool)>,
    preserved: Vec<String>,
}

/// Parse an existing block for `apply_patch`: extract the managed settings
/// (first occurrence wins) and collect every non-managed line verbatim.
fn parse_block_for_patch(block: &str) -> ExistingBlock {
    let mut ex = ExistingBlock::default();
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue; // blank lines are not fields; the block is re-emitted
        }
        let Some((key, raw_value)) = split_key_value(trimmed) else {
            ex.preserved.push(trimmed.to_string());
            continue;
        };
        let value = clean_value(raw_value);
        match key {
            "title" => {
                if ex.title.is_none() {
                    ex.title = Some(value);
                }
            }
            "author" => {
                if ex.author.is_none() {
                    ex.author = Some(value);
                }
            }
            "autoHeadingLevel" => {
                if ex.auto_heading_level.is_none() {
                    ex.auto_heading_level = value.parse().ok();
                }
            }
            "flavor" => {
                if ex.flavor.is_none() {
                    ex.flavor = Flavor::from_str_loose(&value);
                }
            }
            key if key.starts_with("ext_") => match Feature::from_ext_key(key) {
                Some(feature) => {
                    let already = ex.ext_overrides.iter().any(|(f, _)| *f == feature);
                    if !already {
                        if let Some(on) = parse_bool(&value) {
                            ex.ext_overrides.push((feature, on));
                        }
                    }
                }
                // An unknown ext_* key is an extra field — preserve it.
                None => ex.preserved.push(trimmed.to_string()),
            },
            // Unknown key — preserve it untouched.
            _ => ex.preserved.push(trimmed.to_string()),
        }
    }
    ex
}

/// Format a value for writing: quote it when an unquoted value would break
/// the `key: value` parse (contains `": "` or edge whitespace). Values that
/// themselves contain a quote are left unquoted (the parser does not unescape
/// inner quotes, so quoting those would not round-trip).
fn yaml_value(value: &str) -> String {
    let needs_quotes = value.contains(": ") || value.starts_with(' ') || value.ends_with(' ');
    if needs_quotes && !value.contains('"') {
        format!("\"{}\"", value)
    } else {
        value.to_string()
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Split the document into (front-matter block content, body) when it begins
/// with a `---`-delimited block; `None` when there is no such block (the
/// whole document is body). Matches comrak's capture rule.
fn split_front_matter(markdown: &str) -> Option<(&str, &str)> {
    let mut lines = markdown.split_inclusive('\n');
    let first = lines.next()?;
    if trim_line_ending(first) != "---" {
        return None;
    }
    let mut offset = first.len();
    for line in lines {
        if trim_line_ending(line) == "---" {
            let block = &markdown[first.len()..offset];
            let body = &markdown[offset + line.len()..];
            return Some((block, body));
        }
        offset += line.len();
    }
    None
}

/// Parse a validated front-matter block into `Settings`.
fn parse_block(block: &str) -> Settings {
    let mut settings = Settings::default();
    let known = known_keys();
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Validity was checked by `is_valid_block`, so this cannot fail —
        // guard anyway to keep parsing total.
        let Some((key, raw_value)) = split_key_value(trimmed) else {
            continue;
        };
        let value = clean_value(raw_value);
        match key {
            "title" => {
                settings.title.get_or_insert(value);
            }
            "author" => {
                settings.author.get_or_insert(value);
            }
            "autoHeadingLevel" => parse_auto_heading_level(&value, &mut settings),
            "flavor" => parse_flavor(&value, &mut settings),
            key if key.starts_with("ext_") => parse_ext(key, &value, &mut settings),
            _ => {
                let message = match suggest::suggest(key, &known) {
                    Some(suggestion) => format!(
                        "unknown front-matter key \"{key}\" — did you mean \"{suggestion}\"?"
                    ),
                    None => format!("unknown front-matter key \"{key}\" — ignored"),
                };
                settings.warnings.push(message);
            }
        }
    }
    settings
}

fn parse_auto_heading_level(value: &str, settings: &mut Settings) {
    if settings.auto_heading_level.is_some() {
        return; // first occurrence wins
    }
    match value.parse::<u8>() {
        Ok(level) => settings.auto_heading_level = Some(level),
        Err(_) => settings.warnings.push(format!(
            "invalid front-matter autoHeadingLevel \"{value}\" — keeping the toolbar setting"
        )),
    }
}

fn parse_flavor(value: &str, settings: &mut Settings) {
    if settings.flavor.is_some() {
        return; // first occurrence wins
    }
    match Flavor::from_str_loose(value) {
        Some(f) => settings.flavor = Some(f),
        None => settings.warnings.push(format!(
            "invalid front-matter flavor \"{value}\" — expected commonmark, github or custom; \
             keeping the toolbar setting"
        )),
    }
}

fn parse_ext(key: &str, value: &str, settings: &mut Settings) {
    let Some(feature) = Feature::from_ext_key(key) else {
        let message = match suggest::suggest(key, &known_keys()) {
            Some(suggestion) => {
                format!("unknown front-matter key \"{key}\" — did you mean \"{suggestion}\"?")
            }
            None => format!("unknown front-matter key \"{key}\" — ignored"),
        };
        settings.warnings.push(message);
        return;
    };
    if settings.ext_overrides.iter().any(|(f, _)| *f == feature) {
        return; // first occurrence wins
    }
    match value.to_ascii_lowercase().as_str() {
        "true" => settings.ext_overrides.push((feature, true)),
        "false" => settings.ext_overrides.push((feature, false)),
        _ => settings.warnings.push(format!(
            "invalid front-matter {key} \"{value}\" — expected true or false"
        )),
    }
}

/// A non-empty line is a valid front-matter line when it is a `key: value`
/// pair with a non-empty key. Blank lines are tolerated and checked
/// separately.
fn is_valid_block(block: &str) -> bool {
    block
        .lines()
        .filter(|line| !line.trim().is_empty())
        .all(|line| split_key_value(line.trim()).is_some())
}

/// Split one `key: value` line on the first `:`. The key must be non-empty.
fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    Some((key, value))
}

/// Trim whitespace and strip one pair of surrounding double quotes.
fn clean_value(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

/// The inner content of a leading `---` block (between the opening and
/// closing delimiter lines), matching comrak's capture rule: the first line
/// is exactly `---` and a later line is exactly `---`. `None` when there is
/// no such block.
fn leading_block(markdown: &str) -> Option<&str> {
    let mut lines = markdown.split_inclusive('\n');
    let first = lines.next()?;
    if trim_line_ending(first) != "---" {
        return None;
    }
    let block_start = first.len();
    let mut offset = block_start;
    for line in lines {
        if trim_line_ending(line) == "---" {
            return Some(&markdown[block_start..offset]);
        }
        offset += line.len();
    }
    None
}

/// Remove the document's first line (the stray `---`). Used to build the
/// transient patched copy when the block is not valid front matter.
fn strip_leading_delimiter(markdown: &str) -> String {
    match markdown.split_once('\n') {
        Some((_, rest)) => rest.to_string(),
        None => String::new(),
    }
}

fn trim_line_ending(line: &str) -> &str {
    let s = line.strip_suffix('\n').unwrap_or(line);
    s.strip_suffix('\r').unwrap_or(s)
}

fn strip_bom(markdown: &str) -> &str {
    markdown.strip_prefix('\u{feff}').unwrap_or(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title_author_and_settings() {
        let scan = scan("---\ntitle: \"My Doc\"\nauthor: Ada\nautoHeadingLevel: 2\n---\n\n# Body");
        let s = scan.settings.expect("front matter");
        assert_eq!(s.title.as_deref(), Some("My Doc"));
        assert_eq!(s.author.as_deref(), Some("Ada"));
        assert_eq!(s.auto_heading_level, Some(2));
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
        assert!(matches!(scan.source, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn parses_flavor_and_ext_overrides() {
        let scan = scan("---\nflavor: github\next_footnotes: true\next_math: false\n---\nx");
        let s = scan.settings.expect("front matter");
        assert_eq!(s.flavor, Some(Flavor::Github));
        assert_eq!(
            s.ext_overrides,
            vec![(Feature::Footnotes, true), (Feature::Math, false)]
        );
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    }

    #[test]
    fn no_front_matter_returns_none() {
        let scan = scan("# Just a heading\n\nbody");
        assert!(scan.settings.is_none());
        assert!(scan.warnings.is_empty());
        assert!(matches!(scan.source, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn unclosed_leading_delimiter_is_not_front_matter() {
        // A leading `---` with no closing `---` is a thematic break, not
        // front matter — comrak would not capture it either.
        let scan = scan("---\n\nsome text\n\nmore");
        assert!(scan.settings.is_none());
        assert!(scan.warnings.is_empty());
    }

    #[test]
    fn weird_block_is_stripped_and_warns() {
        // The classic foot-gun: a leading `---` (thematic break) with a later
        // `---` (horizontal rule) — the content between would be swallowed.
        let md = "---\nreal content here\nnot key value\n---\n\nafter";
        let scan = scan(md);
        assert!(scan.settings.is_none());
        assert_eq!(scan.warnings.len(), 1);
        assert!(scan.warnings[0].contains("not valid front matter"));
        // The patched source drops only the leading `---`.
        assert_eq!(
            scan.source,
            "real content here\nnot key value\n---\n\nafter"
        );
    }

    #[test]
    fn unknown_key_warns_with_suggestion() {
        let scan = scan("---\ntitel: Oops\n---\nx");
        let s = scan.settings.expect("front matter");
        assert_eq!(s.warnings.len(), 1);
        assert!(
            s.warnings[0].contains("did you mean \"title\""),
            "{:?}",
            s.warnings
        );
    }

    #[test]
    fn unknown_key_without_match_warns_plainly() {
        let scan = scan("---\nbibliography: refs.bib\n---\nx");
        let s = scan.settings.expect("front matter");
        assert_eq!(s.warnings.len(), 1);
        assert!(s.warnings[0].contains("ignored"), "{:?}", s.warnings);
    }

    #[test]
    fn invalid_values_warn_and_are_dropped() {
        let scan = scan("---\nautoHeadingLevel: abc\nflavor: markdown\next_math: maybe\n---\nx");
        let s = scan.settings.expect("front matter");
        assert!(s.auto_heading_level.is_none());
        assert!(s.flavor.is_none());
        assert!(s.ext_overrides.is_empty());
        assert_eq!(s.warnings.len(), 3, "{:?}", s.warnings);
    }

    #[test]
    fn first_occurrence_wins() {
        let scan = scan("---\ntitle: First\ntitle: Second\n---\nx");
        let s = scan.settings.expect("front matter");
        assert_eq!(s.title.as_deref(), Some("First"));
    }

    #[test]
    fn blank_lines_and_quotes_tolerated() {
        let scan = scan("---\n\ntitle: \"Quoted\"\n\nauthor: Plain\n\n---\nx");
        let s = scan.settings.expect("front matter");
        assert_eq!(s.title.as_deref(), Some("Quoted"));
        assert_eq!(s.author.as_deref(), Some("Plain"));
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    }

    // --- apply_patch --------------------------------------------------------

    fn patch_title(title: &str) -> Patch {
        Patch {
            title: Some(title.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn patch_adds_front_matter_to_a_bare_document() {
        let out = apply_patch("# Body", &patch_title("My Doc"));
        assert_eq!(out, "---\ntitle: My Doc\n---\n# Body");
    }

    #[test]
    fn patch_updates_existing_title_and_preserves_body() {
        let out = apply_patch("---\ntitle: Old\n---\n\n# Body", &patch_title("New"));
        assert_eq!(out, "---\ntitle: New\n---\n\n# Body");
    }

    #[test]
    fn patch_preserves_unknown_keys_and_comments() {
        let md = "---\ntitle: T\nbibliography: refs.bib\n# a comment\nauthor: A\n---\nBody";
        let out = apply_patch(md, &patch_title("T2"));
        // Managed keys canonicalize to the top; unknown key + comment kept.
        assert_eq!(
            out,
            "---\ntitle: T2\nauthor: A\nbibliography: refs.bib\n# a comment\n---\nBody"
        );
    }

    #[test]
    fn patch_empty_title_clears_the_key() {
        let out = apply_patch("---\ntitle: Old\nauthor: A\n---\nBody", &patch_title(""));
        assert_eq!(out, "---\nauthor: A\n---\nBody");
    }

    #[test]
    fn patch_none_fields_keep_existing_values() {
        let md = "---\ntitle: T\nautoHeadingLevel: 3\n---\nBody";
        let out = apply_patch(md, &Patch::default());
        assert_eq!(out, "---\ntitle: T\nautoHeadingLevel: 3\n---\nBody");
    }

    #[test]
    fn patch_features_writes_xor_minimal_encoding() {
        // Custom minus definition lists → flavor: custom + one override.
        let mut flags = Flavor::Custom.preset();
        flags.set(Feature::DefinitionList, false);
        let patch = Patch {
            features: Some(flags),
            ..Default::default()
        };
        let out = apply_patch("# Body", &patch);
        assert_eq!(
            out,
            "---\nflavor: custom\next_definition_list: false\n---\n# Body"
        );
    }

    #[test]
    fn patch_features_exact_preset_has_no_overrides() {
        let patch = Patch {
            features: Some(Flavor::Github.preset()),
            ..Default::default()
        };
        let out = apply_patch("Body", &patch);
        assert_eq!(out, "---\nflavor: github\n---\nBody");
    }

    #[test]
    fn patch_none_features_keeps_existing_flavor_and_overrides() {
        let md = "---\nflavor: github\next_footnotes: true\n---\nBody";
        let out = apply_patch(md, &patch_title("T"));
        assert_eq!(
            out,
            "---\ntitle: T\nflavor: github\next_footnotes: true\n---\nBody"
        );
    }

    #[test]
    fn patch_preserves_unknown_ext_keys() {
        let md = "---\next_future_thing: true\n---\nBody";
        let out = apply_patch(md, &patch_title("T"));
        assert_eq!(out, "---\ntitle: T\next_future_thing: true\n---\nBody");
    }

    #[test]
    fn patch_round_trips_through_scan() {
        let mut flags = Flavor::Github.preset();
        flags.set(Feature::Math, true);
        let patch = Patch {
            title: Some("RT".into()),
            auto_heading_level: Some(2),
            features: Some(flags),
            ..Default::default()
        };
        let out = apply_patch("Body", &patch);
        let s = scan(&out).settings.expect("front matter");
        assert_eq!(s.title.as_deref(), Some("RT"));
        assert_eq!(s.auto_heading_level, Some(2));
        assert_eq!(s.flavor, Some(Flavor::Github));
        assert_eq!(s.ext_overrides, vec![(Feature::Math, true)]);
        assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    }

    #[test]
    fn patch_quotes_values_that_contain_colon_space() {
        let out = apply_patch("Body", &patch_title("Note: important"));
        assert_eq!(out, "---\ntitle: \"Note: important\"\n---\nBody");
        // …and it round-trips.
        let s = scan(&out).settings.expect("front matter");
        assert_eq!(s.title.as_deref(), Some("Note: important"));
    }

    #[test]
    fn patch_everything_cleared_removes_the_block() {
        let out = apply_patch("---\ntitle: Old\n---\nBody", &patch_title(""));
        assert_eq!(out, "Body");
    }
}
