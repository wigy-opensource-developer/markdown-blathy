//! Typst markup escaping (§6.2 of SPEC.md).
//!
//! `escape_text` is applied to comrak `Text` node payloads so user content
//! cannot break out of a content block or inject Typst markup. It escapes
//! Typst's reserved markup characters (labels, references, raw, math, …)
//! and, at the start of each line, the block markers (`=`, `-`, `+`, `/`)
//! that Typst would otherwise treat as headings/list/term-list items.

/// Escape content for a Typst string literal (`"…"`): only `\` and `"` are
/// special there. Used by `#raw("…")`, `#link("…")` and front-matter values.
pub(crate) fn escape_typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_typst_string_into(text, &mut out);
    out
}

/// Accumulator form of `escape_typst_string`: appends to an existing buffer
/// so callers building larger markup avoid an intermediate string.
pub(crate) fn escape_typst_string_into(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c => out.push(c),
        }
    }
}

/// Escape a plain-text node for use inside a Typst content block.
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_text_into(text, &mut out);
    out
}

/// Accumulator form of `escape_text`: appends to an existing buffer so
/// callers building larger markup avoid an intermediate string.
pub fn escape_text_into(text: &str, out: &mut String) {
    let mut at_line_start = true;
    for c in text.chars() {
        match c {
            // Block markers: only special when they begin a line (heading /
            // list item / term list in Typst), so escape only in that
            // position.
            '=' | '-' | '+' | '/' if at_line_start => {
                out.push('\\');
                out.push(c);
            }
            // `<`/`>` delimit Typst labels: `(<` would start an unclosed
            // label and fail the compile, a balanced `<tag>` pair would
            // become an invisible label that drops the text.
            '<' => out.push_str("\\<"),
            '>' => out.push_str("\\>"),
            // `@` starts a reference; plain text such as an email address
            // must not turn into one.
            '@' => out.push_str("\\@"),
            // A literal backtick would start raw mode.
            '`' => out.push_str("\\`"),
            // `~` is a non-breaking space in Typst markup.
            '~' => out.push_str("\\~"),
            '#' => out.push_str("\\#"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '$' => out.push_str("\\$"),
            '\\' => out.push_str("\\\\"),
            '\n' => {
                out.push('\n');
                at_line_start = true;
                continue;
            }
            c => out.push(c),
        }
        at_line_start = false;
    }
}

#[cfg(test)]
mod tests {
    use super::escape_text;

    /// Assert each `(input, expected)` pair escapes to exactly `expected`.
    fn assert_escapes(cases: &[(&str, &str)]) {
        for &(input, expected) in cases {
            assert_eq!(escape_text(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn escapes_special_characters_in_text() {
        assert_escapes(&[
            ("a # b", "a \\# b"),
            ("x [y] z", "x \\[y\\] z"),
            ("*bold* not", "\\*bold\\* not"),
            ("_em_", "\\_em\\_"),
            ("cost: $5", "cost: \\$5"),
            ("back\\slash", "back\\\\slash"),
            ("quotes \"stay\"", "quotes \"stay\""),
        ]);
    }

    /// Typst parses `<…>` as a label. Unescaped text containing `<` (e.g.
    /// `(<5–15%)`) fails the compile with "unclosed label"; a balanced pair
    /// (e.g. HTML tags) turns into an invisible label and silently drops the
    /// text. Both must render literally.
    #[test]
    fn escapes_typst_label_delimiters() {
        assert_escapes(&[
            ("(<5–15%)", "(\\<5–15%)"),
            ("a > b", "a \\> b"),
            ("<div>x</div>", "\\<div\\>x\\</div\\>"),
        ]);
    }

    /// `@name` is a Typst reference. Plain text like an email address would
    /// fail the compile with "label `<example.com>` does not exist"; escaped
    /// it renders literally.
    #[test]
    fn escapes_typst_references() {
        assert_escapes(&[
            ("user@example.com", "user\\@example.com"),
            ("@figure", "\\@figure"),
        ]);
    }

    /// A literal backtick starts Typst raw mode; unpaired it fails the
    /// compile with "unclosed raw text".
    #[test]
    fn escapes_typst_raw_marker() {
        assert_escapes(&[("it`s fine", "it\\`s fine"), ("a ` b", "a \\` b")]);
    }

    /// A line starting with `/` is a Typst term list (`/ term: detail`);
    /// without a colon it fails the compile. Mid-line slashes stay literal.
    #[test]
    fn escapes_term_list_marker_at_line_start_only() {
        assert_escapes(&[
            ("/ foo bar", "\\/ foo bar"),
            ("a / b not escaped", "a / b not escaped"),
            ("line one\n/ line two", "line one\n\\/ line two"),
        ]);
    }

    /// `~` is Typst's non-breaking space; literal tildes must survive.
    #[test]
    fn escapes_typst_non_breaking_space() {
        assert_escapes(&[
            ("about ~5 percent", "about \\~5 percent"),
            ("H~2~O", "H\\~2\\~O"),
        ]);
    }

    #[test]
    fn escapes_block_markers_at_line_start_only() {
        assert_escapes(&[
            ("= heading-like text", "\\= heading-like text"),
            ("- list-like", "\\- list-like"),
            ("+ list-like", "\\+ list-like"),
            ("a = b not escaped", "a = b not escaped"),
            ("a - b not escaped", "a - b not escaped"),
            ("line one\n= line two", "line one\n\\= line two"),
        ]);
    }
}
