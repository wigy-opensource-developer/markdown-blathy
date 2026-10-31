//! LaTeX math → Typst math translation (§6.2 of SPEC.md).
//!
//! Markdown math is authored in TeX/LaTeX notation; Typst is only the
//! rendering backend. This module owns the dialect boundary:
//!
//! - `tylax` parses the LaTeX and emits Typst math (including macros and
//!   common environments);
//! - its conversion warnings become user-facing diagnostics;
//! - the emitted Typst is syntax-checked here, so a translator regression is
//!   reported as a math diagnostic instead of surfacing as a cryptic compile
//!   failure later.
//!
//! The module is deliberately independent of comrak and Tauri: the converter
//! passes the literal inside `$…$`, and unit tests exercise it directly.

/// A non-fatal problem found while translating one math literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathDiagnostic {
    /// Human-readable message, including the LaTeX location when `tylax`
    /// provides one.
    pub message: String,
    /// A safe replacement for the whole math literal when the translator can
    /// repair the input (e.g. Typst's empty script base). Unsupported
    /// constructs have no automatic fix and keep this as `None`.
    pub fix: Option<String>,
}

/// The translated Typst math and any diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    /// Typst math source, without the surrounding `$…$`.
    pub typst: String,
    pub diagnostics: Vec<MathDiagnostic>,
}

/// Translate one LaTeX math literal (the text between `$…$`).
pub fn translate(latex: &str) -> Translation {
    let result = tylax::latex_math_to_typst_with_diagnostics(latex);
    let mut diagnostics: Vec<MathDiagnostic> = result
        .warnings
        .into_iter()
        .map(|warning| {
            // A parse warning describing a LaTeX/Typst difference is fixed by
            // the translated output itself (e.g. `$^+$` → `""^(+)`). Other
            // warnings (unsupported primitives/macros) have no safe fix.
            let fix = matches!(warning.kind, tylax::WarningKind::ParseError)
                .then(|| result.output.clone());
            MathDiagnostic {
                message: warning.to_string(),
                fix,
            }
        })
        .collect();

    // A translator regression (or a construct it emits invalidly) must be a
    // diagnostic here, not a later "Preview failed" with no source context.
    let parsed = typst::syntax::parse_math(&result.output);
    for error in parsed.errors_and_warnings().0 {
        diagnostics.push(MathDiagnostic {
            message: format!("translated math is not valid Typst: {}", error.message),
            fix: None,
        });
    }

    Translation {
        typst: result.output,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::translate;

    fn typst(latex: &str) -> String {
        translate(latex).typst
    }

    fn assert_clean(latex: &str) {
        let out = translate(latex);
        assert!(
            out.diagnostics.is_empty(),
            "{latex:?} produced diagnostics: {:?}",
            out.diagnostics
        );
    }

    #[test]
    fn translates_fractions_roots_and_scripts() {
        assert_eq!(typst(r"\frac{a}{b}"), "a/b");
        assert_eq!(typst(r"\sqrt{x}"), "sqrt(x)");
        assert_eq!(typst(r"\sqrt[3]{x}"), "root(3, x)");
        assert_eq!(typst("^+"), "\"\"^(+)");
        assert_eq!(typst(r"x_i^2"), "x_(i)^(2)");
    }

    #[test]
    fn translates_styles_text_and_scripts() {
        assert_eq!(typst(r"\mathbb{Z}_q"), "ZZ_(q)");
        assert_eq!(typst(r"\mathrm{PRF}_{msg}"), "upright(P R F)_(m s g)");
        assert_eq!(typst(r"\text{PK}"), "#text[PK]");
        assert_eq!(typst(r"\vec{x}"), "arrow(x)");
        assert_eq!(typst(r"\stackrel{?}{=}"), "limits(=)^(?)");
    }

    #[test]
    fn translates_multi_letter_variables_and_boundaries() {
        assert_eq!(typst("Ay + ct"), "A y + c t");
        assert_eq!(typst(r"O(n\log n)"), "O (n log n)");
        assert_eq!(typst(r"\pm1"), r"plus.minus 1");
    }

    #[test]
    fn translates_matrices_and_delimiters() {
        assert_eq!(
            typst(r"\begin{matrix}a&b\\c&d\end{matrix}"),
            "mat(delim: #none, a, b ;c, d)"
        );
        assert_eq!(typst(r"\left( x \right)"), "(x)");
    }

    #[test]
    fn reports_translator_warnings() {
        let out = translate(r"\catcode`\@=11");
        assert!(!out.diagnostics.is_empty(), "expected a warning");
        assert!(
            out.diagnostics[0].message.contains("catcode"),
            "{:?}",
            out.diagnostics
        );
        assert!(out.diagnostics[0].fix.is_none());
    }

    #[test]
    fn empty_script_base_is_reported_with_a_fix() {
        let out = translate("^+");
        assert_eq!(out.typst, "\"\"^(+)");
        assert_eq!(out.diagnostics.len(), 1, "{:?}", out.diagnostics);
        assert_eq!(out.diagnostics[0].fix.as_deref(), Some("\"\"^(+)"));
    }

    #[test]
    fn supported_latex_is_clean() {
        for latex in [
            r"\frac{a}{b}",
            r"\sqrt[3]{x}",
            r"\sum_{i=1}^n i",
            r"\int_0^1 x\,dx",
            r"\mathbb{Z}_q",
            r"\mathrm{HighBits}",
            r"Ay + ct",
            r"O(n\log n)",
            r"\stackrel{?}{=}",
            r"\left( x \right)",
            r"\text{PK}",
            r"\vec{x}",
            r"\begin{matrix}a&b\\c&d\end{matrix}",
        ] {
            assert_clean(latex);
        }
    }
}
