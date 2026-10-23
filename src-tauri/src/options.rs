//! User-facing render options (§6.1 of SPEC.md).
//!
//! Pure data types shared by the command layer (serde) and the plain modules.
//! `Paper::typst_name()` maps to Typst's `page` paper names.

use serde::{Deserialize, Serialize};

/// Markdown flavor: selects a preset of extension flags (§6.1). A flavor is
/// a *named preset* — the effective behavior is a `FeatureFlags` set, which
/// a document can further tweak via `ext_*` front-matter keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Flavor {
    Commonmark,
    Github,
    Custom,
}

/// A toggleable Markdown extension (§6.1). The `ext_*` front-matter keys map
/// 1:1 to these. Autolink is deliberately absent: it stays off in every
/// flavor (SPEC §4), so it is neither a preset bit nor a user toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Strikethrough,
    Table,
    TaskList,
    Footnotes,
    Math,
    Highlight,
    Superscript,
    Subscript,
    DefinitionList,
    PageBreak,
    Toc,
}

impl Feature {
    /// Every feature in stable order — the order `encode`/`decode` walk.
    pub const ALL: [Feature; 11] = [
        Feature::Strikethrough,
        Feature::Table,
        Feature::TaskList,
        Feature::Footnotes,
        Feature::Math,
        Feature::Highlight,
        Feature::Superscript,
        Feature::Subscript,
        Feature::DefinitionList,
        Feature::PageBreak,
        Feature::Toc,
    ];

    /// The front-matter key for this feature (`ext_…`).
    pub fn ext_key(self) -> &'static str {
        match self {
            Feature::Strikethrough => "ext_strikethrough",
            Feature::Table => "ext_table",
            Feature::TaskList => "ext_task_list",
            Feature::Footnotes => "ext_footnotes",
            Feature::Math => "ext_math",
            Feature::Highlight => "ext_highlight",
            Feature::Superscript => "ext_superscript",
            Feature::Subscript => "ext_subscript",
            Feature::DefinitionList => "ext_definition_list",
            Feature::PageBreak => "ext_page_break",
            Feature::Toc => "ext_page_toc",
        }
    }

    /// Parse an `ext_…` front-matter key back to its feature, if it is one.
    pub fn from_ext_key(key: &str) -> Option<Feature> {
        Feature::ALL.iter().copied().find(|f| f.ext_key() == key)
    }
}

/// The effective set of enabled extensions. Built from a `Flavor` preset,
/// then individual `ext_*` overrides applied on top (§6.1). Serialized to the
/// frontend so the settings UI can show the effective state, and deserialized
/// back from the settings popover's patch. Field names stay snake_case to
/// match the `ext_*` front-matter keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub strikethrough: bool,
    pub table: bool,
    pub task_list: bool,
    pub footnotes: bool,
    pub math: bool,
    pub highlight: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub definition_list: bool,
    pub page_break: bool,
    pub page_toc: bool,
}

impl Flavor {
    /// The extension preset for this flavor (SPEC §4): CommonMark enables
    /// nothing, GitHub the GFM set, Custom everything.
    pub fn preset(self) -> FeatureFlags {
        match self {
            Flavor::Commonmark => FeatureFlags::default(),
            Flavor::Github => FeatureFlags {
                strikethrough: true,
                table: true,
                task_list: true,
                ..FeatureFlags::default()
            },
            Flavor::Custom => FeatureFlags {
                strikethrough: true,
                table: true,
                task_list: true,
                footnotes: true,
                math: true,
                highlight: true,
                superscript: true,
                subscript: true,
                definition_list: true,
                page_break: true,
                page_toc: true,
            },
        }
    }

    /// The front-matter name of the flavor (`commonmark`/`github`/`custom`),
    /// matching the serde snake_case form.
    pub fn as_str(self) -> &'static str {
        match self {
            Flavor::Commonmark => "commonmark",
            Flavor::Github => "github",
            Flavor::Custom => "custom",
        }
    }

    /// Parse a front-matter `flavor:` value (case-insensitive).
    pub fn from_str_loose(value: &str) -> Option<Flavor> {
        match value.to_ascii_lowercase().as_str() {
            "commonmark" => Some(Flavor::Commonmark),
            "github" => Some(Flavor::Github),
            "custom" => Some(Flavor::Custom),
            _ => None,
        }
    }

    /// The flavors in tie-break order for `FeatureFlags::encode`.
    const ENCODE_ORDER: [Flavor; 3] = [Flavor::Commonmark, Flavor::Github, Flavor::Custom];
}

impl FeatureFlags {
    pub fn get(&self, feature: Feature) -> bool {
        match feature {
            Feature::Strikethrough => self.strikethrough,
            Feature::Table => self.table,
            Feature::TaskList => self.task_list,
            Feature::Footnotes => self.footnotes,
            Feature::Math => self.math,
            Feature::Highlight => self.highlight,
            Feature::Superscript => self.superscript,
            Feature::Subscript => self.subscript,
            Feature::DefinitionList => self.definition_list,
            Feature::PageBreak => self.page_break,
            Feature::Toc => self.page_toc,
        }
    }

    pub fn set(&mut self, feature: Feature, on: bool) {
        let slot = match feature {
            Feature::Strikethrough => &mut self.strikethrough,
            Feature::Table => &mut self.table,
            Feature::TaskList => &mut self.task_list,
            Feature::Footnotes => &mut self.footnotes,
            Feature::Math => &mut self.math,
            Feature::Highlight => &mut self.highlight,
            Feature::Superscript => &mut self.superscript,
            Feature::Subscript => &mut self.subscript,
            Feature::DefinitionList => &mut self.definition_list,
            Feature::PageBreak => &mut self.page_break,
            Feature::Toc => &mut self.page_toc,
        };
        *slot = on;
    }

    /// A flavor preset with `ext_*` overrides applied on top.
    pub fn from_preset(flavor: Flavor, overrides: &[(Feature, bool)]) -> FeatureFlags {
        let mut flags = flavor.preset();
        for &(feature, on) in overrides {
            flags.set(feature, on);
        }
        flags
    }

    /// The number of features that differ from a flavor's preset.
    fn flips_from(&self, flavor: Flavor) -> usize {
        let preset = flavor.preset();
        Feature::ALL
            .iter()
            .filter(|&&f| self.get(f) != preset.get(f))
            .count()
    }

    /// XOR-minimal encoding (SPEC §6.1): the flavor preset closest to this
    /// flag set — fewest flips, ties broken CommonMark → GitHub → Custom —
    /// plus each feature that still differs as a `(feature, desired)`
    /// override. Round-trips with `from_preset`.
    pub fn encode(&self) -> (Flavor, Vec<(Feature, bool)>) {
        let flavor = Flavor::ENCODE_ORDER
            .iter()
            .copied()
            .min_by_key(|&f| self.flips_from(f))
            .unwrap_or(Flavor::Commonmark);
        let preset = flavor.preset();
        let overrides = Feature::ALL
            .iter()
            .copied()
            .filter(|&f| self.get(f) != preset.get(f))
            .map(|f| (f, self.get(f)))
            .collect();
        (flavor, overrides)
    }
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
    /// Bundled template id (a subdirectory of the bundled `templates/`
    /// resource dir, e.g. `light`/`dark`/`obsidian`). `None` = the default
    /// `light` template. Ignored when `template_path` is set.
    #[serde(default)]
    pub template_name: Option<String>,
    /// Absolute path to a user template; None = use `template_name` (or the
    /// embedded default).
    pub template_path: Option<String>,
    /// Heading ID generation settings (§6.2). Defaults to enabling all 6 levels.
    #[serde(default)]
    pub heading_ids: HeadingIds,
}

impl RenderOptions {
    /// The fresh-install defaults, kept in sync with `DEFAULT_SETTINGS.options`
    /// in `src/lib/settings.js`. Stored as the frontend's own JSON so the
    /// serde camelCase contract stays in one place; parsed once at startup by
    /// the engine prewarm (§7.5) — a user who persisted different settings
    /// simply misses the prewarmed engine and rebuilds it (cheaply) on the
    /// first render, exactly as before the prewarm existed.
    pub fn frontend_defaults() -> Self {
        // If this literal and settings.js drift, the app still boots — the
        // prewarm is best-effort; `frontend_defaults_shape_is_the_fresh_install_state`
        // guards the values themselves.
        serde_json::from_str(
            r#"{"flavor":"custom","paper":"a4","margins":{"topMm":25,"bottomMm":25,"leftMm":20,"rightMm":20},"textAlign":"left","templateName":null,"templatePath":null}"#,
        )
        .expect("frontend default options must parse as RenderOptions")
    }
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
    use super::{
        Feature, FeatureFlags, Flavor, HeadingIds, Margins, Paper, RenderOptions, TextAlign,
    };

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
        assert_eq!(
            serde_json::to_string(&Flavor::Github).unwrap(),
            "\"github\""
        );
        assert_eq!(
            serde_json::from_str::<Flavor>("\"custom\"").unwrap(),
            Flavor::Custom
        );
        assert_eq!(
            serde_json::to_string(&TextAlign::Justified).unwrap(),
            "\"justified\""
        );
    }

    /// The prewarm's defaults must mirror `DEFAULT_SETTINGS.options` in
    /// `src/lib/settings.js`: custom flavor (every extension on), A4,
    /// 25/25/20/20 mm margins ("normal" preset), left-aligned, embedded
    /// template, all six auto-heading levels.
    #[test]
    fn frontend_defaults_shape_is_the_fresh_install_state() {
        let o = RenderOptions::frontend_defaults();
        assert_eq!(o.flavor, Flavor::Custom);
        assert_eq!(o.paper, Paper::A4);
        assert_eq!(
            o.margins,
            super::Margins {
                top_mm: 25.0,
                bottom_mm: 25.0,
                left_mm: 20.0,
                right_mm: 20.0
            }
        );
        assert_eq!(o.text_align, TextAlign::Left);
        assert_eq!(o.template_name, None);
        assert_eq!(o.template_path, None);
        assert_eq!(o.heading_ids, HeadingIds::default());
    }

    #[test]
    fn render_options_round_trips_with_camel_case() {
        let options = RenderOptions {
            flavor: Flavor::Github,
            paper: Paper::Letter,
            margins: super::Margins {
                top_mm: 20.0,
                bottom_mm: 20.0,
                left_mm: 15.0,
                right_mm: 15.0,
            },
            text_align: TextAlign::Justified,
            template_name: Some("dark".into()),
            template_path: Some("/tmp/t.typ".into()),
            heading_ids: HeadingIds::default(),
        };
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"textAlign\":\"justified\""), "{json}");
        assert!(json.contains("\"paper\":\"letter\""), "{json}");
        assert!(json.contains("\"templateName\":\"dark\""), "{json}");
        let back: RenderOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back.paper, Paper::Letter);
        assert_eq!(back.template_name.as_deref(), Some("dark"));
        assert_eq!(back.template_path.as_deref(), Some("/tmp/t.typ"));
    }

    #[test]
    fn flavor_presets_match_the_spec_table() {
        let cm = Flavor::Commonmark.preset();
        assert!(!cm.strikethrough && !cm.table && !cm.task_list && !cm.footnotes);

        let gh = Flavor::Github.preset();
        assert!(gh.strikethrough && gh.table && gh.task_list);
        assert!(!gh.footnotes && !gh.math && !gh.highlight);

        let custom = Flavor::Custom.preset();
        assert!(
            Feature::ALL.iter().all(|&f| custom.get(f)),
            "Custom enables every feature"
        );
    }

    #[test]
    fn from_preset_applies_ext_overrides() {
        // GitHub preset with footnotes forced on and table forced off.
        let flags = FeatureFlags::from_preset(
            Flavor::Github,
            &[(Feature::Footnotes, true), (Feature::Table, false)],
        );
        assert!(flags.footnotes);
        assert!(!flags.table);
        assert!(flags.strikethrough, "untouched preset bits stay");
    }

    #[test]
    fn encode_picks_the_flavor_with_fewest_flips() {
        // All-on is Custom with zero overrides.
        let (flavor, overrides) = FeatureFlags::from_preset(Flavor::Custom, &[]).encode();
        assert_eq!(flavor, Flavor::Custom);
        assert!(overrides.is_empty());

        // All-on except definition lists → Custom with one flip.
        let mut almost_all = Flavor::Custom.preset();
        almost_all.set(Feature::DefinitionList, false);
        let (flavor, overrides) = almost_all.encode();
        assert_eq!(flavor, Flavor::Custom);
        assert_eq!(overrides, vec![(Feature::DefinitionList, false)]);

        // The GFM trio alone is exactly the GitHub preset.
        let mut gh = FeatureFlags::default();
        gh.set(Feature::Strikethrough, true);
        gh.set(Feature::Table, true);
        gh.set(Feature::TaskList, true);
        let (flavor, overrides) = gh.encode();
        assert_eq!(flavor, Flavor::Github);
        assert!(overrides.is_empty());

        // Nothing on is CommonMark.
        let (flavor, overrides) = FeatureFlags::default().encode();
        assert_eq!(flavor, Flavor::Commonmark);
        assert!(overrides.is_empty());
    }

    #[test]
    fn encode_decode_round_trips() {
        // An arbitrary flag set survives encode → from_preset unchanged.
        let mut flags = Flavor::Github.preset();
        flags.set(Feature::Math, true);
        flags.set(Feature::Strikethrough, false);
        let (flavor, overrides) = flags.encode();
        assert_eq!(FeatureFlags::from_preset(flavor, &overrides), flags);
    }

    #[test]
    fn ext_keys_round_trip() {
        for feature in Feature::ALL {
            let key = feature.ext_key();
            assert!(key.starts_with("ext_"), "{key}");
            assert_eq!(Feature::from_ext_key(key), Some(feature), "{key}");
        }
        assert_eq!(Feature::from_ext_key("ext_nope"), None);
    }
}
