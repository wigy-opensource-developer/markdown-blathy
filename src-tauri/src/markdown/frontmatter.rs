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

use crate::markdown::suggest;
use crate::options::{Feature, Flavor};

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
    let flavor = match value.to_ascii_lowercase().as_str() {
        "commonmark" => Some(Flavor::Commonmark),
        "github" => Some(Flavor::Github),
        "custom" => Some(Flavor::Custom),
        _ => None,
    };
    match flavor {
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
}
