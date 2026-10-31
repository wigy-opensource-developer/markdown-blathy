//! MarkdownBláthy command-line interface.
//!
//! The CLI is a thin adapter over `markdown-blathy-core`. The GUI binary is a
//! separate executable; `gui` (and the default action) locates and launches it.

mod commands;
mod error;
mod gui;
mod options;

use std::ffi::OsString;

use clap::{Parser, Subcommand};

use commands::{ExportArgs, GetArgs, SetArgs, TemplateListArgs, ValidateTemplateArgs};

#[derive(Debug, Parser)]
#[command(
    name = "markdown-blathy",
    version,
    about = "MarkdownBláthy command-line interface"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Open the desktop application, optionally with a Markdown file.
    Gui {
        /// Markdown file to open in the GUI.
        file: Option<std::path::PathBuf>,
    },
    /// Export Markdown to PDF.
    Export(ExportArgs),
    /// Read or update document front matter.
    Frontmatter {
        #[command(subcommand)]
        command: FrontmatterCommands,
    },
    /// Compile-probe a custom Typst template.
    ValidateTemplate(ValidateTemplateArgs),
    /// Inspect the bundled templates.
    Templates {
        #[command(subcommand)]
        command: TemplateCommands,
    },
}

#[derive(Debug, Subcommand)]
enum FrontmatterCommands {
    /// Print the effective document metadata.
    Get(GetArgs),
    /// Update front matter in place (or to --output / stdout).
    Set(SetArgs),
}

#[derive(Debug, Subcommand)]
enum TemplateCommands {
    /// List bundled template ids.
    List(TemplateListArgs),
}

fn main() {
    let mut raw = std::env::args_os();
    let binary = raw
        .next()
        .unwrap_or_else(|| OsString::from("markdown-blathy"));
    let args = normalize_args(raw.collect());
    let cli = Cli::parse_from(std::iter::once(binary).chain(args));

    if let Err(error) = run(cli) {
        eprintln!("error: {error}");
        std::process::exit(error.code);
    }
}

fn run(cli: Cli) -> Result<(), error::CliError> {
    match cli.command {
        Commands::Gui { file } => gui::launch_gui(file),
        Commands::Export(args) => commands::export(args),
        Commands::Frontmatter { command } => match command {
            FrontmatterCommands::Get(args) => commands::frontmatter_get(args),
            FrontmatterCommands::Set(args) => commands::frontmatter_set(args),
        },
        Commands::ValidateTemplate(args) => commands::validate_template(args),
        Commands::Templates { command } => match command {
            TemplateCommands::List(args) => commands::templates_list(args),
        },
    }
}

/// The first non-option argument selects a subcommand. Anything else is the
/// default GUI action, so `markdown-blathy notes.md` means `gui notes.md`
/// (§6.1). `--` forces the following word to be treated as a file path.
fn normalize_args(args: Vec<OsString>) -> Vec<OsString> {
    if args.is_empty() {
        return vec![OsString::from("gui")];
    }
    if matches!(args[0].to_str(), Some("-h" | "--help" | "-V" | "--version")) {
        return args;
    }

    for arg in &args {
        let Some(text) = arg.to_str() else {
            break;
        };
        if text == "--" {
            break;
        }
        if text.starts_with('-') {
            continue;
        }
        if is_subcommand(text) {
            return args;
        }
        break;
    }

    let mut normalized = Vec::with_capacity(args.len() + 1);
    normalized.push(OsString::from("gui"));
    normalized.extend(args);
    normalized
}

fn is_subcommand(text: &str) -> bool {
    matches!(
        text,
        "gui" | "export" | "frontmatter" | "validate-template" | "templates" | "help"
    )
}

#[cfg(test)]
mod tests {
    use super::normalize_args;
    use std::ffi::OsString;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn empty_arguments_start_the_gui() {
        assert_eq!(normalize_args(Vec::new()), args(&["gui"]));
    }

    #[test]
    fn a_bare_file_becomes_the_gui_file_argument() {
        assert_eq!(
            normalize_args(args(&["notes.md"])),
            args(&["gui", "notes.md"])
        );
    }

    #[test]
    fn explicit_subcommands_are_left_alone() {
        for command in [
            "gui",
            "export",
            "frontmatter",
            "validate-template",
            "templates",
            "help",
        ] {
            let raw = args(&[command, "notes.md"]);
            assert_eq!(normalize_args(raw.clone()), raw, "{command}");
        }
    }

    #[test]
    fn double_dash_forces_a_file_named_like_a_subcommand() {
        assert_eq!(
            normalize_args(args(&["--", "export"])),
            args(&["gui", "--", "export"])
        );
    }

    #[test]
    fn top_level_help_and_version_are_not_rewritten() {
        assert_eq!(normalize_args(args(&["--help"])), args(&["--help"]));
        assert_eq!(normalize_args(args(&["--version"])), args(&["--version"]));
    }
}
