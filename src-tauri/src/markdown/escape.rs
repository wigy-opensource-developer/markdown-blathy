//! Typst markup escaping (§6.2 of SPEC.md).
//!
//! `escape_text` is applied to comrak `Text` node payloads so user content
//! cannot break out of a content block or inject Typst markup. It escapes
//! Typst's reserved markup characters and, at the start of each line, the
//! block markers (`=`, `-`, `+`) that Typst would otherwise treat as
//! headings/list items.

/// Escape content for a Typst string literal (`"…"`): only `\` and `"` are
/// special there. Used by `#raw("…")`, `#link("…")` and front-matter values.
pub(crate) fn escape_typst_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            c => out.push(c),
        }
    }
    out
}

/// Escape a plain-text node for use inside a Typst content block.
pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_line_start = true;
    for c in text.chars() {
        match c {
            // Block markers: only special when they begin a line (heading /
            // list item in Typst), so escape only in that position.
            '=' | '-' | '+' if at_line_start => {
                out.push('\\');
                out.push(c);
            }
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
    out
}

#[cfg(test)]
mod tests {
    use super::escape_text;

    #[test]
    fn escapes_special_characters_in_text() {
        let cases = [
            ("a # b", "a \\# b"),
            ("x [y] z", "x \\[y\\] z"),
            ("*bold* not", "\\*bold\\* not"),
            ("_em_", "\\_em\\_"),
            ("cost: $5", "cost: \\$5"),
            ("back\\slash", "back\\\\slash"),
            ("quotes \"stay\"", "quotes \"stay\""),
        ];
        for (input, expected) in cases {
            assert_eq!(escape_text(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn escapes_block_markers_at_line_start_only() {
        let cases = [
            ("= heading-like text", "\\= heading-like text"),
            ("- list-like", "\\- list-like"),
            ("+ list-like", "\\+ list-like"),
            ("a = b not escaped", "a = b not escaped"),
            ("a - b not escaped", "a - b not escaped"),
            ("line one\n= line two", "line one\n\\= line two"),
        ];
        for (input, expected) in cases {
            assert_eq!(escape_text(input), expected, "input: {input:?}");
        }
    }
}
