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

/// Heading ID generation (§6.2): `max_level` limits AUTO-slug generation to
/// headings of level 1 through N (0 = no auto-slugs at all).
///
/// Manual `{#custom-id}` IDs are the author's explicit intent: they are
/// always honored — at every level, even when `max_level` is 0. All IDs
/// (manual and auto) share one uniqueness namespace; the first occurrence
/// wins and later duplicates are dropped with a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadingIds {
    /// Maximum heading level (1-based) that gets an AUTO-slug. 0 = disabled.
    pub max_level: u8,
}

impl Default for HeadingIds {
    fn default() -> Self {
        Self { max_level: 6 }
    }
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
    /// Heading ID generation settings (§6.2). Defaults to enabling all 6 levels.
    #[serde(default)]
    pub heading_ids: HeadingIds,
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

impl TextAlign {
    /// Typst `text-align` value ("left" | "center" | "justified", §6.3).
    pub fn typst_name(self) -> &'static str {
        match self {
            TextAlign::Left => "left",
            TextAlign::Center => "center",
            TextAlign::Justified => "justified",
        }
    }
}

impl Margins {
    /// Horizontal page margin in mm: the smaller of left/right (§7.4).
    pub fn x_mm(self) -> f32 {
        self.left_mm.min(self.right_mm)
    }

    /// Vertical page margin in mm: the smaller of top/bottom (§7.4).
    pub fn y_mm(self) -> f32 {
        self.top_mm.min(self.bottom_mm)
    }
}

#[cfg(test)]
mod tests {
    use super::{Flavor, HeadingIds, Margins, Paper, RenderOptions, TextAlign};

    #[test]
    fn paper_maps_to_typst_names() {
        assert_eq!(Paper::A4.typst_name(), "a4");
        assert_eq!(Paper::A5.typst_name(), "a5");
        assert_eq!(Paper::Letter.typst_name(), "us-letter");
        assert_eq!(Paper::Legal.typst_name(), "us-legal");
    }

    #[test]
    fn text_align_maps_to_typst_names() {
        assert_eq!(TextAlign::Left.typst_name(), "left");
        assert_eq!(TextAlign::Center.typst_name(), "center");
        assert_eq!(TextAlign::Justified.typst_name(), "justified");
    }

    #[test]
    fn margins_use_the_smaller_value_per_axis() {
        let margins = Margins {
            top_mm: 20.0,
            bottom_mm: 25.0,
            left_mm: 15.0,
            right_mm: 30.0,
        };
        assert_eq!(
            margins.x_mm(),
            15.0,
            "x is the smaller of left/right (§7.4)"
        );
        assert_eq!(
            margins.y_mm(),
            20.0,
            "y is the smaller of top/bottom (§7.4)"
        );
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
            heading_ids: HeadingIds::default(),
        };
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"textAlign\":\"justified\""), "{json}");
        assert!(json.contains("\"paper\":\"letter\""), "{json}");
        let back: RenderOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back.paper, Paper::Letter);
        assert_eq!(back.template_path.as_deref(), Some("/tmp/t.typ"));
    }
}
