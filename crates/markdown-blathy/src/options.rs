//! Shared render options and bundled-template discovery for the CLI.

use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use markdown_blathy_core::options::{Flavor, HeadingIds, Margins, Paper, RenderOptions, TextAlign};

use crate::error::CliError;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FlavorArg {
    Commonmark,
    Github,
    Custom,
}

impl From<FlavorArg> for Flavor {
    fn from(value: FlavorArg) -> Self {
        match value {
            FlavorArg::Commonmark => Flavor::Commonmark,
            FlavorArg::Github => Flavor::Github,
            FlavorArg::Custom => Flavor::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PaperArg {
    #[value(name = "a4")]
    A4,
    #[value(name = "a5")]
    A5,
    Letter,
    Legal,
}

impl From<PaperArg> for Paper {
    fn from(value: PaperArg) -> Self {
        match value {
            PaperArg::A4 => Paper::A4,
            PaperArg::A5 => Paper::A5,
            PaperArg::Letter => Paper::Letter,
            PaperArg::Legal => Paper::Legal,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AlignArg {
    Left,
    Center,
    Justified,
}

impl From<AlignArg> for TextAlign {
    fn from(value: AlignArg) -> Self {
        match value {
            AlignArg::Left => TextAlign::Left,
            AlignArg::Center => TextAlign::Center,
            AlignArg::Justified => TextAlign::Justified,
        }
    }
}

/// Shared render/export flags (§6.2). CLI flags override the frontend's
/// fresh-install defaults.
#[derive(Debug, Args)]
pub struct RenderArgs {
    #[arg(long, value_enum)]
    pub flavor: Option<FlavorArg>,
    #[arg(long, value_enum)]
    pub paper: Option<PaperArg>,
    #[arg(long, value_name = "N")]
    pub margin_top: Option<f32>,
    #[arg(long, value_name = "N")]
    pub margin_bottom: Option<f32>,
    #[arg(long, value_name = "N")]
    pub margin_left: Option<f32>,
    #[arg(long, value_name = "N")]
    pub margin_right: Option<f32>,
    #[arg(long, value_enum)]
    pub align: Option<AlignArg>,
    #[arg(long = "template", value_name = "PATH.TYP")]
    pub template_path: Option<PathBuf>,
    #[arg(long, value_name = "NAME")]
    pub template_name: Option<String>,
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(0..=6))]
    pub heading_level: Option<u8>,
    #[arg(long, value_name = "DIR")]
    pub templates_dir: Option<PathBuf>,
}

impl RenderArgs {
    /// Build `RenderOptions` from the defaults plus the supplied flags.
    pub fn options(&self) -> Result<RenderOptions, CliError> {
        let mut options = RenderOptions::frontend_defaults();
        if let Some(flavor) = self.flavor {
            options.flavor = flavor.into();
        }
        if let Some(paper) = self.paper {
            options.paper = paper.into();
        }
        if let Some(align) = self.align {
            options.text_align = align.into();
        }

        let defaults = options.margins;
        options.margins = Margins {
            top_mm: margin_value("--margin-top", self.margin_top, defaults.top_mm)?,
            bottom_mm: margin_value("--margin-bottom", self.margin_bottom, defaults.bottom_mm)?,
            left_mm: margin_value("--margin-left", self.margin_left, defaults.left_mm)?,
            right_mm: margin_value("--margin-right", self.margin_right, defaults.right_mm)?,
        };

        if let Some(template) = &self.template_path {
            options.template_path = Some(template.to_string_lossy().into_owned());
        }
        if let Some(name) = &self.template_name {
            options.template_name = Some(name.clone());
        }
        if let Some(level) = self.heading_level {
            options.heading_ids = HeadingIds { max_level: level };
        }
        Ok(options)
    }
}

fn margin_value(flag: &'static str, value: Option<f32>, default: f32) -> Result<f32, CliError> {
    match value {
        Some(v) if v.is_finite() && v >= 0.0 => Ok(v),
        Some(v) => Err(CliError::usage(format!(
            "{flag} must be a non-negative number, got {v}"
        ))),
        None => Ok(default),
    }
}

/// Resolve the bundled-template directory in the documented order (§5.3):
/// `--templates-dir`, `MARKDOWN_BLATHY_TEMPLATES`, installed share locations,
/// then the embedded default fallback (`None`).
pub fn resolve_templates_dir(explicit: Option<&Path>) -> Result<Option<PathBuf>, CliError> {
    let env_dir = std::env::var_os("MARKDOWN_BLATHY_TEMPLATES").map(PathBuf::from);
    let exe = std::env::current_exe().ok();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let candidates = installed_candidates(exe.as_deref(), home.as_deref());
    resolve_templates_dir_from(explicit.map(Path::to_path_buf), env_dir, candidates)
}

fn resolve_templates_dir_from(
    explicit: Option<PathBuf>,
    env_dir: Option<PathBuf>,
    candidates: impl IntoIterator<Item = PathBuf>,
) -> Result<Option<PathBuf>, CliError> {
    if let Some(dir) = explicit {
        if is_templates_dir(&dir) {
            return Ok(Some(dir));
        }
        return Err(CliError::usage(format!(
            "templates directory {} does not contain light/main.typ",
            dir.display()
        )));
    }
    if let Some(dir) = env_dir {
        if is_templates_dir(&dir) {
            return Ok(Some(dir));
        }
        return Err(CliError::failure(format!(
            "MARKDOWN_BLATHY_TEMPLATES points to {}, which does not contain light/main.typ",
            dir.display()
        )));
    }
    for candidate in candidates {
        if is_templates_dir(&candidate) {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn installed_candidates(exe: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(exe) = exe {
        if let Some(parent) = exe.parent() {
            // `/usr/bin/markdown-blathy` -> `/usr/share/markdown-blathy/templates`.
            candidates.push(parent.join("../share/markdown-blathy/templates"));
        }
    }
    candidates.push(PathBuf::from("/usr/share/markdown-blathy/templates"));
    candidates.push(PathBuf::from("/usr/local/share/markdown-blathy/templates"));
    candidates.push(PathBuf::from(
        "/opt/homebrew/share/markdown-blathy/templates",
    ));
    candidates.push(PathBuf::from(
        "/usr/local/opt/markdown-blathy/share/markdown-blathy/templates",
    ));
    if let Some(home) = home {
        candidates.push(home.join(".local/share/markdown-blathy/templates"));
    }
    candidates
}

fn is_templates_dir(dir: &Path) -> bool {
    dir.join("light").join("main.typ").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_templates(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "markdown-blathy-cli-templates-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("light")).unwrap();
        fs::write(dir.join("light").join("main.typ"), "#let article(..) = {}").unwrap();
        dir
    }

    #[test]
    fn explicit_templates_dir_wins() {
        let explicit = temp_templates("explicit");
        let env = temp_templates("env");
        let resolved =
            resolve_templates_dir_from(Some(explicit.clone()), Some(env), Vec::new()).unwrap();
        assert_eq!(resolved, Some(explicit));
    }

    #[test]
    fn environment_variable_is_used_next() {
        let env = temp_templates("env-only");
        let installed = temp_templates("installed");
        let resolved =
            resolve_templates_dir_from(None, Some(env.clone()), vec![installed]).unwrap();
        assert_eq!(resolved, Some(env));
    }

    #[test]
    fn installed_candidate_is_used_last() {
        let installed = temp_templates("installed-only");
        let resolved = resolve_templates_dir_from(None, None, vec![installed.clone()]).unwrap();
        assert_eq!(resolved, Some(installed));
    }

    #[test]
    fn no_candidate_falls_back_to_embedded() {
        let resolved =
            resolve_templates_dir_from(None, None, vec![PathBuf::from("/nonexistent")]).unwrap();
        assert_eq!(resolved, None);
    }

    #[test]
    fn invalid_explicit_dir_is_a_usage_error() {
        let err = resolve_templates_dir_from(Some(PathBuf::from("/nonexistent")), None, Vec::new())
            .unwrap_err();
        assert_eq!(err.code, 2);
    }

    #[test]
    fn invalid_environment_dir_is_an_operational_error() {
        let err = resolve_templates_dir_from(None, Some(PathBuf::from("/nonexistent")), Vec::new())
            .unwrap_err();
        assert_eq!(err.code, 1);
    }
}
