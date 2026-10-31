//! CLI command implementations over `markdown_blathy_core::Service`.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};
use markdown_blathy_core::markdown::convert::DocMeta;
use markdown_blathy_core::markdown::frontmatter::Patch;
use markdown_blathy_core::options::{Feature, FeatureFlags};
use markdown_blathy_core::service::{RenderRequest, Service};
use markdown_blathy_core::CoreError;

use crate::error::CliError;
use crate::options::{resolve_templates_dir, FlavorArg, RenderArgs};

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Markdown file to export, or `-` for stdin.
    pub input: PathBuf,
    /// Output PDF path, or `-` for stdout. Defaults to the sibling `.pdf`.
    #[arg(long, value_name = "PATH|-")]
    pub output: Option<PathBuf>,
    /// Fail instead of overwriting an existing PDF.
    #[arg(long)]
    pub no_clobber: bool,
    /// Open the exported PDF in the system viewer.
    #[arg(long)]
    pub open: bool,
    #[command(flatten)]
    pub render: RenderArgs,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    /// Markdown file to read, or `-` for stdin.
    pub input: PathBuf,
    /// Emit the metadata as JSON.
    #[arg(long)]
    pub json: bool,
    /// Print one metadata value.
    #[arg(long, value_name = "KEY")]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HyphenateArg {
    Auto,
    On,
    Off,
}

#[derive(Debug, Args)]
pub struct SetArgs {
    /// Markdown file to update, or `-` for stdin.
    pub input: PathBuf,
    #[arg(long, value_name = "TEXT")]
    pub title: Option<String>,
    #[arg(long)]
    pub clear_title: bool,
    #[arg(long, value_name = "TEXT")]
    pub author: Option<String>,
    #[arg(long)]
    pub clear_author: bool,
    #[arg(long, value_name = "LANG")]
    pub lang: Option<String>,
    #[arg(long)]
    pub clear_lang: bool,
    #[arg(long, value_enum)]
    pub hyphenate: Option<HyphenateArg>,
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(0..=6))]
    pub auto_heading_level: Option<u8>,
    #[arg(long, value_enum)]
    pub flavor: Option<FlavorArg>,
    /// `--ext key=bool` (the `ext_` prefix is optional), repeatable.
    #[arg(long = "ext", value_name = "KEY=BOOL")]
    pub ext: Vec<String>,
    /// Write the result here, or `-` for stdout. Defaults to in-place; stdin
    /// defaults to stdout.
    #[arg(long, value_name = "PATH|-")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ValidateTemplateArgs {
    /// Custom `.typ` template to validate.
    pub template: PathBuf,
    #[command(flatten)]
    pub render: RenderArgs,
}

#[derive(Debug, Args)]
pub struct TemplateListArgs {
    #[arg(long)]
    pub json: bool,
    #[arg(long, value_name = "DIR")]
    pub templates_dir: Option<PathBuf>,
}

pub fn export(args: ExportArgs) -> Result<(), CliError> {
    let service = Service::new(resolve_templates_dir(args.render.templates_dir.as_deref())?);
    let options = args.render.options()?;
    let markdown = read_input(&service, &args.input)?;

    let stdin = is_stdin(&args.input);
    let target = match args.output {
        Some(path) => path,
        None if stdin => {
            return Err(CliError::usage(
                "export from stdin requires --output <path> or --output -",
            ))
        }
        None => args.input.with_extension("pdf"),
    };
    if args.open && is_stdin(&target) {
        return Err(CliError::usage("--open cannot be used with --output -"));
    }
    if args.no_clobber && !is_stdin(&target) && target.exists() {
        return Err(CliError::failure(format!(
            "refusing to overwrite {} (--no-clobber)",
            target.display()
        )));
    }

    let document_path = if stdin {
        None
    } else {
        Some(canonical_input(&args.input)?)
    };
    let request = RenderRequest {
        markdown,
        options,
        document_path,
    };
    let pdf = service.render_pdf_detailed(&request)?;
    for warning in &pdf.warnings {
        eprintln!("warning: {warning}");
    }
    write_output(&service, &target, &pdf.bytes)?;

    if args.open && !is_stdin(&target) {
        if let Err(error) = open::that(&target) {
            eprintln!("warning: failed to open {}: {error}", target.display());
        }
    }
    Ok(())
}

pub fn frontmatter_get(args: GetArgs) -> Result<(), CliError> {
    if args.json && args.field.is_some() {
        return Err(CliError::usage("--json and --field cannot be combined"));
    }
    let service = Service::new(None);
    let markdown = read_input(&service, &args.input)?;
    let doc = service.read_front_matter(&markdown)?;

    if args.json {
        let json = serde_json::to_string_pretty(&doc)
            .map_err(|e| CliError::failure(format!("failed to serialize metadata: {e}")))?;
        println!("{json}");
    } else if let Some(field) = &args.field {
        println!("{}", field_value(&doc, field)?);
    } else {
        print_metadata(&doc);
    }
    Ok(())
}

pub fn frontmatter_set(args: SetArgs) -> Result<(), CliError> {
    let service = Service::new(None);
    let markdown = read_input(&service, &args.input)?;
    let current = service.read_front_matter(&markdown)?;
    let patch = build_patch(&args, &current)?;
    let updated = service.update_front_matter(&markdown, &patch)?;

    let output = args.output.unwrap_or_else(|| {
        if is_stdin(&args.input) {
            PathBuf::from("-")
        } else {
            args.input.clone()
        }
    });
    write_output(&service, &output, updated.as_bytes())
}

pub fn validate_template(args: ValidateTemplateArgs) -> Result<(), CliError> {
    let service = Service::new(resolve_templates_dir(args.render.templates_dir.as_deref())?);
    let options = args.render.options()?;
    service.validate_template(&args.template, &options)?;
    println!("template valid: {}", args.template.display());
    Ok(())
}

pub fn templates_list(args: TemplateListArgs) -> Result<(), CliError> {
    let dir = resolve_templates_dir(args.templates_dir.as_deref())?;
    let service = Service::new(dir);
    let names = match service.list_templates() {
        Ok(names) => names,
        // The embedded light template is always available (§5.3).
        Err(CoreError::MissingResource(_)) => vec!["light".to_string()],
        Err(error) => return Err(error.into()),
    };
    if args.json {
        let json = serde_json::to_string(&names)
            .map_err(|e| CliError::failure(format!("failed to serialize template list: {e}")))?;
        println!("{json}");
    } else {
        for name in names {
            println!("{name}");
        }
    }
    Ok(())
}

fn build_patch(args: &SetArgs, current: &DocMeta) -> Result<Patch, CliError> {
    if args.clear_title && args.title.is_some() {
        return Err(CliError::usage(
            "--title and --clear-title cannot be combined",
        ));
    }
    if args.clear_author && args.author.is_some() {
        return Err(CliError::usage(
            "--author and --clear-author cannot be combined",
        ));
    }
    if args.clear_lang && args.lang.is_some() {
        return Err(CliError::usage(
            "--lang and --clear-lang cannot be combined",
        ));
    }

    let title = if args.clear_title {
        Some(String::new())
    } else {
        args.title.clone()
    };
    let author = if args.clear_author {
        Some(String::new())
    } else {
        args.author.clone()
    };
    let lang = if args.clear_lang {
        Some(String::new())
    } else {
        args.lang.clone()
    };
    let hyphenate = args.hyphenate.map(|value| match value {
        HyphenateArg::Auto => None,
        HyphenateArg::On => Some(true),
        HyphenateArg::Off => Some(false),
    });

    let mut features: Option<FeatureFlags> = None;
    if args.flavor.is_some() || !args.ext.is_empty() {
        let mut flags = match args.flavor {
            Some(flavor) => markdown_blathy_core::options::Flavor::from(flavor).preset(),
            None => current.features,
        };
        for spec in &args.ext {
            let (key, on) = parse_ext_spec(spec)?;
            let feature = Feature::from_ext_key(&key).ok_or_else(|| {
                CliError::usage(format!(
                    "unknown extension '{key}' — use an ext_* front-matter key"
                ))
            })?;
            flags.set(feature, on);
        }
        features = Some(flags);
    }

    Ok(Patch {
        title,
        author,
        auto_heading_level: args.auto_heading_level,
        lang,
        hyphenate,
        features,
    })
}

fn parse_ext_spec(spec: &str) -> Result<(String, bool), CliError> {
    let (key, value) = spec
        .split_once('=')
        .ok_or_else(|| CliError::usage(format!("--ext expects key=bool, got '{spec}'")))?;
    let key = if key.starts_with("ext_") {
        key.to_string()
    } else {
        format!("ext_{key}")
    };
    let on = parse_bool(value)
        .ok_or_else(|| CliError::usage(format!("--ext expects a boolean value, got '{value}'")))?;
    Ok((key, on))
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Some(true),
        "false" | "off" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn field_value(doc: &DocMeta, field: &str) -> Result<String, CliError> {
    let value = match field {
        "title" => doc.title.clone().unwrap_or_default(),
        "author" => doc.author.clone().unwrap_or_default(),
        "lang" => doc.lang.clone().unwrap_or_default(),
        "hyphenate" => doc
            .hyphenate
            .map(|value| value.to_string())
            .unwrap_or_else(|| "auto".to_string()),
        "flavor" => doc.flavor.as_str().to_string(),
        "autoHeadingLevel" | "auto_heading_level" => doc.auto_heading_level.to_string(),
        key if key.starts_with("ext_") => {
            let feature = Feature::from_ext_key(key)
                .ok_or_else(|| CliError::usage(format!("unknown metadata field '{field}'")))?;
            doc.features.get(feature).to_string()
        }
        _ => return Err(CliError::usage(format!("unknown metadata field '{field}'"))),
    };
    Ok(value)
}

fn print_metadata(doc: &DocMeta) {
    println!("title: {}", doc.title.as_deref().unwrap_or(""));
    println!("author: {}", doc.author.as_deref().unwrap_or(""));
    println!("lang: {}", doc.lang.as_deref().unwrap_or(""));
    println!(
        "hyphenate: {}",
        doc.hyphenate
            .map(|value| value.to_string())
            .unwrap_or_else(|| "auto".to_string())
    );
    println!("flavor: {}", doc.flavor.as_str());
    println!("autoHeadingLevel: {}", doc.auto_heading_level);
    for feature in Feature::ALL {
        println!("{}: {}", feature.ext_key(), doc.features.get(feature));
    }
}

fn read_input(service: &Service, input: &Path) -> Result<String, CliError> {
    if is_stdin(input) {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .map_err(|e| CliError::failure(format!("failed to read stdin: {e}")))?;
        Ok(buffer)
    } else {
        service.read_document(input).map_err(Into::into)
    }
}

fn write_output(service: &Service, output: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if is_stdin(output) {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(bytes)
            .and_then(|()| stdout.flush())
            .map_err(|e| CliError::failure(format!("failed to write stdout: {e}")))?;
    } else {
        service.write_document(output, bytes)?;
        eprintln!("wrote {}", output.display());
    }
    Ok(())
}

fn canonical_input(input: &Path) -> Result<PathBuf, CliError> {
    std::fs::canonicalize(input)
        .map_err(|e| CliError::usage(format!("cannot open {}: {e}", input.display())))
}

fn is_stdin(path: &Path) -> bool {
    path.as_os_str() == "-"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_spec_accepts_optional_prefix_and_boolean_aliases() {
        assert_eq!(
            parse_ext_spec("table=on").unwrap(),
            ("ext_table".to_string(), true)
        );
        assert_eq!(
            parse_ext_spec("ext_footnotes=false").unwrap(),
            ("ext_footnotes".to_string(), false)
        );
        assert_eq!(
            parse_ext_spec("ext_math=1").unwrap(),
            ("ext_math".to_string(), true)
        );
    }

    #[test]
    fn ext_spec_rejects_malformed_values() {
        assert_eq!(parse_ext_spec("table").unwrap_err().code, 2);
        assert_eq!(parse_ext_spec("table=maybe").unwrap_err().code, 2);
    }
}
