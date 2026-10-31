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
        at_line_start = push_escaped(c, at_line_start, out);
    }
}

/// Push one escaped character, returning whether the next character starts a
/// line (only `\n` does). Shared by `escape_text_into` and
/// `escape_text_wrapping_long_words` so both keep the block-marker rule.
fn push_escaped(c: char, at_line_start: bool, out: &mut String) -> bool {
    match c {
        // Block markers: only special when they begin a line (heading /
        // list item / term list in Typst), so escape only in that position.
        '=' | '-' | '+' | '/' if at_line_start => {
            out.push('\\');
            out.push(c);
        }
        // `<`/`>` delimit Typst labels: `(<` would start an unclosed label and
        // fail the compile, a balanced `<tag>` pair would become an invisible
        // label that drops the text.
        '<' => out.push_str("\\<"),
        '>' => out.push_str("\\>"),
        // `@` starts a reference; plain text such as an email address must
        // not turn into one.
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
            return true;
        }
        c => out.push(c),
    }
    false
}

/// Tokens at least this many characters long are handed to the prelude's
/// `break-long` helper.
const LONG_WORD_CHARS: usize = 24;

/// `escape_text`, but wrapping pathologically long tokens in the prelude's
/// `break-long` helper.
///
/// Typst's line breaker follows Unicode UAX #14: it breaks at spaces and soft
/// punctuation, but a long run like a chemical formula
/// (`CH₃CH₂CH=CH…`) or a hash has no break opportunity and crosses the margin.
/// `break-long` inserts zero-width break opportunities, so the token wraps
/// where it must. Tokens shorter than `LONG_WORD_CHARS` are escaped exactly as
/// `escape_text` does — their length is not a wrapping problem.
pub fn escape_text_wrapping_long_words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_text_wrapping_long_words_into(text, &mut out);
    out
}

/// Accumulator form of `escape_text_wrapping_long_words`.
pub fn escape_text_wrapping_long_words_into(text: &str, out: &mut String) {
    let mut at_line_start = true;
    let mut token_start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(start) = token_start.take() {
                push_token(&text[start..i], at_line_start, out);
                at_line_start = false;
            }
            at_line_start = push_escaped(c, at_line_start, out);
        } else if token_start.is_none() {
            token_start = Some(i);
        }
    }
    if let Some(start) = token_start {
        push_token(&text[start..], at_line_start, out);
    }
}

/// Push one whitespace-free token: long ones become a `#break-long("…")` call
/// (a string literal, so only string escaping applies), short ones are
/// escaped as plain markup.
fn push_token(token: &str, at_line_start: bool, out: &mut String) {
    if token.chars().count() >= LONG_WORD_CHARS {
        out.push_str("#break-long(\"");
        escape_typst_string_into(token, out);
        out.push_str("\")");
    } else {
        let mut line_start = at_line_start;
        for c in token.chars() {
            line_start = push_escaped(c, line_start, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_text, escape_text_wrapping_long_words};

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

    /// Tokens at least `LONG_WORD_CHARS` long cannot rely on Typst's UAX #14
    /// line breaking: a chemical formula or hash has no break opportunity and
    /// would cross the margin. Such tokens are handed to the prelude's
    /// `break-long`, which inserts zero-width break opportunities.
    #[test]
    fn wraps_long_tokens_in_the_break_long_helper() {
        let formula = "CH₃CH₂CH=CHCH₂CH=CHCH₂CH=CH(CH₂)₇COOH";
        assert_eq!(
            escape_text_wrapping_long_words(formula),
            format!("#break-long(\"{formula}\")")
        );
    }

    #[test]
    fn leaves_short_tokens_escaped_as_plain_text() {
        assert_eq!(
            escape_text_wrapping_long_words("megkülönböztetés #x"),
            "megkülönböztetés \\#x"
        );
    }

    #[test]
    fn wraps_only_the_long_tokens_and_keeps_whitespace() {
        assert_eq!(
            escape_text_wrapping_long_words("a #b\nCH₃CH₂CH=CHCH₂CH=CHCH₂CH=CH(CH₂)₇COOH c"),
            "a \\#b\n#break-long(\"CH₃CH₂CH=CHCH₂CH=CHCH₂CH=CH(CH₂)₇COOH\") c"
        );
    }

    /// The token travels as a Typst string literal, so string escaping applies
    /// (only `\` and `"`), not markup escaping.
    #[test]
    fn string_escapes_inside_wrapped_tokens() {
        let token = "x\"y".repeat(9); // 27 chars, over the threshold
        assert_eq!(
            escape_text_wrapping_long_words(&token),
            format!("#break-long(\"{}\")", token.replace('"', "\\\""))
        );
    }

    /// A long token at the start of a line is wrapped, so it is no longer a
    /// block marker and needs no `\=`/`\-` escape.
    #[test]
    fn wraps_long_tokens_at_line_start_without_marker_escape() {
        let token = "=".to_string() + &"x".repeat(23);
        assert_eq!(
            escape_text_wrapping_long_words(&token),
            format!("#break-long(\"{token}\")")
        );
    }

    #[test]
    fn keeps_block_marker_escaping_for_unwrapped_tokens() {
        assert_eq!(
            escape_text_wrapping_long_words("line one\n= short"),
            "line one\n\\= short"
        );
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
