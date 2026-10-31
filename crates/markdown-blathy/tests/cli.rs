//! End-to-end tests for the `markdown-blathy` binary.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_markdown-blathy")
}

fn temp_dir(tag: &str) -> PathBuf {
    let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "markdown-blathy-cli-{}-{tag}-{id}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn core_templates() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../markdown-blathy-core/templates")
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("CLI must start")
}

fn run_with_stdin(dir: &Path, args: &[&str], input: &str) -> Output {
    let mut child = Command::new(binary())
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("CLI must start");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn frontmatter_set_rewrites_in_place_and_get_reads_it() {
    let dir = temp_dir("fm-inplace");
    std::fs::write(dir.join("notes.md"), "# Body\n").unwrap();

    let output = run(
        &dir,
        &[
            "frontmatter",
            "set",
            "notes.md",
            "--title",
            "Notes",
            "--author",
            "Ada",
            "--lang",
            "hu",
            "--flavor",
            "github",
        ],
    );
    assert_success(&output);

    let content = std::fs::read_to_string(dir.join("notes.md")).unwrap();
    assert!(content.contains("title: Notes"), "{content}");
    assert!(content.contains("author: Ada"), "{content}");
    assert!(content.contains("lang: hu"), "{content}");
    assert!(content.contains("flavor: github"), "{content}");
    assert!(content.contains("# Body"), "{content}");

    let get = run(
        &dir,
        &["frontmatter", "get", "notes.md", "--field", "title"],
    );
    assert_success(&get);
    assert_eq!(String::from_utf8_lossy(&get.stdout).trim(), "Notes");

    let json = run(&dir, &["frontmatter", "get", "notes.md", "--json"]);
    assert_success(&json);
    let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(value["title"], "Notes");
    assert_eq!(value["flavor"], "github");
}

#[test]
fn frontmatter_set_output_dash_writes_only_markdown_to_stdout() {
    let dir = temp_dir("fm-stdout");
    std::fs::write(dir.join("notes.md"), "# Body\n").unwrap();

    let output = run(
        &dir,
        &[
            "frontmatter",
            "set",
            "notes.md",
            "--title",
            "Stdout",
            "--output",
            "-",
        ],
    );
    assert_success(&output);
    assert_eq!(
        std::fs::read_to_string(dir.join("notes.md")).unwrap(),
        "# Body\n",
        "input must stay untouched"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("title: Stdout"), "{stdout}");
    assert!(stdout.contains("# Body"), "{stdout}");
    assert!(!stdout.contains("wrote"), "logs must not reach stdout");
}

#[test]
fn frontmatter_set_from_stdin_defaults_to_stdout() {
    let dir = temp_dir("fm-stdin");
    let output = run_with_stdin(
        &dir,
        &["frontmatter", "set", "-", "--title", "Stdin"],
        "# Body\n",
    );
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("title: Stdin"), "{stdout}");
    assert!(stdout.contains("# Body"), "{stdout}");
}

#[test]
fn frontmatter_set_ext_preserves_the_document_flavor() {
    let dir = temp_dir("fm-ext");
    std::fs::write(dir.join("notes.md"), "---\nflavor: github\n---\n# Body\n").unwrap();

    let output = run(
        &dir,
        &["frontmatter", "set", "notes.md", "--ext", "footnotes=true"],
    );
    assert_success(&output);

    let flavor = run(
        &dir,
        &["frontmatter", "get", "notes.md", "--field", "flavor"],
    );
    assert_eq!(String::from_utf8_lossy(&flavor.stdout).trim(), "github");
    let ext = run(
        &dir,
        &["frontmatter", "get", "notes.md", "--field", "ext_footnotes"],
    );
    assert_eq!(String::from_utf8_lossy(&ext.stdout).trim(), "true");
}

#[test]
fn export_writes_the_sibling_pdf() {
    let dir = temp_dir("export-sibling");
    std::fs::write(dir.join("notes.md"), "# PDF\n").unwrap();

    let output = run(&dir, &["export", "notes.md"]);
    assert_success(&output);

    let pdf = std::fs::read(dir.join("notes.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF"), "missing PDF magic");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wrote notes.pdf"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn export_output_dash_streams_only_pdf_bytes() {
    let dir = temp_dir("export-stdout");
    std::fs::write(dir.join("notes.md"), "# PDF\n").unwrap();

    let output = run(&dir, &["export", "notes.md", "--output", "-"]);
    assert_success(&output);
    assert!(output.stdout.starts_with(b"%PDF"), "missing PDF magic");
    assert!(
        !output.stdout.windows(4).any(|window| window == b"wrote"),
        "diagnostics must not reach stdout"
    );
}

#[test]
fn export_no_clobber_refuses_an_existing_target() {
    let dir = temp_dir("export-noclobber");
    std::fs::write(dir.join("notes.md"), "# PDF\n").unwrap();
    std::fs::write(dir.join("notes.pdf"), b"OLD").unwrap();

    let output = run(&dir, &["export", "notes.md", "--no-clobber"]);
    assert!(!output.status.success());
    assert_eq!(std::fs::read(dir.join("notes.pdf")).unwrap(), b"OLD");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("refusing to overwrite"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn templates_list_uses_an_explicit_directory() {
    let dir = temp_dir("templates-explicit");
    let templates = core_templates();
    let output = run(
        &dir,
        &[
            "templates",
            "list",
            "--templates-dir",
            templates.to_str().unwrap(),
        ],
    );
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let names: Vec<&str> = stdout.lines().collect();
    assert_eq!(names.first().copied(), Some("light"));
    assert!(names.contains(&"dark"), "{stdout}");
    assert!(names.contains(&"obsidian"), "{stdout}");
}

#[test]
fn templates_list_json_and_environment_variable() {
    let dir = temp_dir("templates-env");
    let templates = core_templates();
    let output = Command::new(binary())
        .args(["templates", "list", "--json"])
        .current_dir(&dir)
        .env("MARKDOWN_BLATHY_TEMPLATES", &templates)
        .output()
        .unwrap();
    assert_success(&output);
    let names: Vec<String> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(names.first().map(String::as_str), Some("light"));
    assert!(names.contains(&"dark".to_string()));
}

#[test]
fn invalid_templates_directory_is_a_usage_error() {
    let dir = temp_dir("templates-invalid");
    let output = run(
        &dir,
        &[
            "templates",
            "list",
            "--templates-dir",
            "/nonexistent/templates",
        ],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("does not contain light/main.typ"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn validate_template_accepts_the_bundled_light_template() {
    let dir = temp_dir("validate");
    let templates = core_templates();
    let light = templates.join("light").join("main.typ");
    let output = run(
        &dir,
        &[
            "validate-template",
            light.to_str().unwrap(),
            "--templates-dir",
            templates.to_str().unwrap(),
        ],
    );
    assert_success(&output);
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("template valid"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[cfg(unix)]
#[test]
fn default_action_and_gui_subcommand_launch_the_gui_with_an_absolute_path() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("gui-launch");
    let notes = dir.join("notes.md");
    std::fs::write(&notes, "# Notes\n").unwrap();
    let script = dir.join("fake-gui.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$GUI_ARGS_FILE\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();

    let expected = std::fs::canonicalize(&notes).unwrap();
    for (index, args) in [vec!["notes.md"], vec!["gui", "notes.md"]]
        .into_iter()
        .enumerate()
    {
        let args_file = dir.join(format!("args-{index}.txt"));
        let output = Command::new(binary())
            .args(&args)
            .current_dir(&dir)
            .env("MARKDOWN_BLATHY_GUI", &script)
            .env("GUI_ARGS_FILE", &args_file)
            .output()
            .unwrap();
        assert_success(&output);
        let received = std::fs::read_to_string(&args_file).unwrap();
        assert_eq!(received.trim(), expected.to_string_lossy());
    }
}
