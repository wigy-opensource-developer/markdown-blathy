//! Locating and launching the Tauri GUI binary.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::CliError;

const PRODUCT_BINARY: &str = "MarkdownBláthy";
const TAURI_BINARY: &str = "markdown-blathy-tauri";

/// Launch the GUI with an optional Markdown document.
pub fn launch_gui(file: Option<PathBuf>) -> Result<(), CliError> {
    let document = file.map(|path| validate_document(&path)).transpose()?;
    let gui = locate_gui()?;

    let mut command = Command::new(&gui);
    if let Some(document) = &document {
        command.arg(document);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // `exec` replaces the CLI process; the GUI receives the file path as
        // its own argv entry, independent of the CLI's working directory.
        let error = command.exec();
        Err(CliError::failure(format!(
            "failed to launch {}: {error}",
            gui.display()
        )))
    }

    #[cfg(not(unix))]
    {
        command.spawn().map_err(|error| {
            CliError::failure(format!("failed to launch {}: {error}", gui.display()))
        })?;
        Ok(())
    }
}

fn validate_document(path: &Path) -> Result<PathBuf, CliError> {
    let absolute = std::fs::canonicalize(path)
        .map_err(|error| CliError::usage(format!("cannot open {}: {error}", path.display())))?;
    let metadata = std::fs::metadata(&absolute)
        .map_err(|error| CliError::usage(format!("cannot open {}: {error}", absolute.display())))?;
    if !metadata.is_file() {
        return Err(CliError::usage(format!(
            "cannot open {}: not a regular file",
            absolute.display()
        )));
    }
    std::fs::File::open(&absolute)
        .map_err(|error| CliError::usage(format!("cannot read {}: {error}", absolute.display())))?;
    Ok(absolute)
}

fn locate_gui() -> Result<PathBuf, CliError> {
    if let Some(value) = std::env::var_os("MARKDOWN_BLATHY_GUI") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Ok(path);
        }
        return Err(CliError::failure(format!(
            "MARKDOWN_BLATHY_GUI points to {}, which is not a file",
            path.display()
        )));
    }

    if let Some(sibling) = sibling_gui() {
        return Ok(sibling);
    }
    for name in gui_names() {
        if let Some(found) = find_in_path(&name) {
            return Ok(found);
        }
    }

    #[cfg(target_os = "macos")]
    if let Some(found) = macos_bundle() {
        return Ok(found);
    }

    #[cfg(windows)]
    if let Some(found) = windows_install() {
        return Ok(found);
    }

    Err(CliError::failure(format!(
        "could not find the MarkdownBláthy GUI ({TAURI_BINARY} or {PRODUCT_BINARY}); \
         install the desktop app or set MARKDOWN_BLATHY_GUI to its executable"
    )))
}

fn sibling_gui() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    for name in gui_names() {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn gui_names() -> Vec<String> {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut names = vec![TAURI_BINARY.to_string(), PRODUCT_BINARY.to_string()];
    #[cfg(windows)]
    for name in &mut names {
        name.push_str(".exe");
    }
    names
}

#[cfg(target_os = "macos")]
fn macos_bundle() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(
            PathBuf::from(home)
                .join("Applications/MarkdownBláthy.app/Contents/MacOS/MarkdownBláthy"),
        );
    }
    candidates.push(PathBuf::from(
        "/Applications/MarkdownBláthy.app/Contents/MacOS/MarkdownBláthy",
    ));
    candidates.into_iter().find(|path| path.is_file())
}

#[cfg(windows)]
fn windows_install() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Programs").join(PRODUCT_BINARY));
        roots.push(PathBuf::from(local).join(PRODUCT_BINARY));
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(program_files).join(PRODUCT_BINARY));
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(program_files_x86).join(PRODUCT_BINARY));
    }
    for root in roots {
        for name in gui_names() {
            let candidate = root.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_names_include_the_tauri_and_product_binary() {
        let names = gui_names();
        assert!(names.contains(&TAURI_BINARY.to_string()));
        assert!(names.contains(&PRODUCT_BINARY.to_string()));
    }
}
