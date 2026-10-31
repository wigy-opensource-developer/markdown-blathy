// Windows release builds are GUI applications: without this attribute the
// binary opens a console window alongside the app. Debug builds keep the
// console so `println!`/panic output stays visible during development.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::OsString;
use std::path::PathBuf;

fn main() {
    markdown_blathy_lib::run(startup_file(std::env::args_os().skip(1)));
}

/// One optional positional document path, independent of Tauri/webview
/// arguments. `--` forces the next argument to be treated as a path.
fn startup_file(mut args: impl Iterator<Item = OsString>) -> Option<PathBuf> {
    let first = args.next()?;
    if first == "--" {
        return args.next().map(PathBuf::from);
    }
    if first.to_string_lossy().starts_with('-') {
        return None;
    }
    Some(PathBuf::from(first))
}

#[cfg(test)]
mod tests {
    use super::startup_file;
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn args(values: &[&str]) -> std::vec::IntoIter<OsString> {
        values
            .iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn no_arguments_means_no_startup_document() {
        assert_eq!(startup_file(args(&[])), None);
    }

    #[test]
    fn one_positional_path_is_the_startup_document() {
        assert_eq!(
            startup_file(args(&["/tmp/notes.md"])),
            Some(PathBuf::from("/tmp/notes.md"))
        );
    }

    #[test]
    fn double_dash_forces_a_dash_prefixed_path() {
        assert_eq!(
            startup_file(args(&["--", "-notes.md"])),
            Some(PathBuf::from("-notes.md"))
        );
    }

    #[test]
    fn webview_flags_are_not_document_paths() {
        assert_eq!(startup_file(args(&["--webview-flag"])), None);
    }
}
