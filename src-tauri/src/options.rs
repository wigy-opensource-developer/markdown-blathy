//! User-facing render options (§6.1 of SPEC.md).
//!
//! Pure data types shared by the command layer (serde) and the plain modules.
//! `Paper::typst_name()` maps to Typst's `page` paper names.

use serde::{Deserialize, Serialize};

/// Markdown flavor: selects which comrak extensions are enabled (§6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    Commonmark,
    Gfm,
    Custom,
}

/// Paper size; `typst_name()` returns the Typst `page` paper string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Paper {
    A4,
    A5,
    Letter,
    Legal,
}

/// Paragraph alignment, mapped to Typst set rules in the template (§6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    Left,
    Center,
    Justified,
}

/// Page margins in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Margins {
    pub top_mm: f32,
    pub bottom_mm: f32,
    pub left_mm: f32,
    pub right_mm: f32,
}

/// Everything the user can tweak, serialized over IPC from the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    pub flavor: Flavor,
    pub paper: Paper,
    pub margins: Margins,
    pub text_align: TextAlign,
    /// Absolute path to a user template; None = embedded default.
    pub template_path: Option<String>,
}

impl Paper {
    /// Typst `page` paper names.
    pub fn typst_name(self) -> &'static str {
        match self {
            Paper::A4 => "a4",
            Paper::A5 => "a5",
            Paper::Letter => "us-letter",
            Paper::Legal => "us-legal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Flavor, Paper, RenderOptions, TextAlign};

    #[test]
    fn paper_maps_to_typst_names() {
        assert_eq!(Paper::A4.typst_name(), "a4");
        assert_eq!(Paper::A5.typst_name(), "a5");
        assert_eq!(Paper::Letter.typst_name(), "us-letter");
        assert_eq!(Paper::Legal.typst_name(), "us-legal");
    }

    #[test]
    fn serde_uses_snake_case_enum_names() {
        assert_eq!(serde_json::to_string(&Flavor::Gfm).unwrap(), "\"gfm\"");
        assert_eq!(
            serde_json::from_str::<Flavor>("\"custom\"").unwrap(),
            Flavor::Custom
        );
        assert_eq!(
            serde_json::to_string(&TextAlign::Justified).unwrap(),
            "\"justified\""
        );
    }

    #[test]
    fn render_options_round_trips_with_camel_case() {
        let options = RenderOptions {
            flavor: Flavor::Gfm,
            paper: Paper::Letter,
            margins: super::Margins {
                top_mm: 20.0,
                bottom_mm: 20.0,
                left_mm: 15.0,
                right_mm: 15.0,
            },
            text_align: TextAlign::Justified,
            template_path: Some("/tmp/t.typ".into()),
        };
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"textAlign\":\"justified\""), "{json}");
        assert!(json.contains("\"paper\":\"letter\""), "{json}");
        let back: RenderOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back.paper, Paper::Letter);
        assert_eq!(back.template_path.as_deref(), Some("/tmp/t.typ"));
    }
}
