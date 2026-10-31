# Command-line interface

The `markdown-blathy` CLI is a thin adapter over the same headless core as the
desktop app. The GUI remains the primary product.

## Where the CLI ships

| Package | GUI | CLI |
|---|---|---|
| Linux `.deb` / `.rpm` | yes (`/usr/bin/MarkdownBláthy`) | yes (`/usr/bin/markdown-blathy`) |
| Linux AppImage | yes | no |
| macOS `.dmg` / `.app` | yes | no |
| Windows NSIS / MSI | yes | no |

The deb/rpm packages also install the bundled templates under
`/usr/share/markdown-blathy/templates`, so the CLI and the GUI use the same
themes.

On macOS and Windows the desktop packages are GUI-only. Developers can still
build the CLI from source:

```bash
cargo build -p markdown-blathy --release
```

The resulting binary can launch an installed GUI with `markdown-blathy gui`.

## Default action: the GUI

```bash
markdown-blathy                 # start the GUI
markdown-blathy notes.md        # start the GUI and open notes.md
markdown-blathy gui notes.md    # the same, explicitly
```

The first non-option argument selects a subcommand. Otherwise the arguments
are treated as GUI arguments. A file whose name looks like a subcommand can be
forced with `--`:

```bash
markdown-blathy -- export
markdown-blathy gui -- export
```

The CLI locates the GUI binary in this order:

1. `MARKDOWN_BLATHY_GUI` (an explicit executable path);
2. a sibling executable next to the CLI (`markdown-blathy-tauri` or
   `MarkdownBláthy`);
3. `PATH` lookup for those names;
4. platform install locations (macOS `~/Applications` and `/Applications`
   bundles; Windows per-user and machine install directories).

A relative file path is canonicalized before launch, so the GUI receives an
absolute path. A missing/unreadable file is reported before the GUI starts.
v1 starts a new GUI instance for each invocation; single-instance forwarding is
not implemented yet.

## Export to PDF

```bash
markdown-blathy export notes.md
# notes.md -> notes.pdf (sibling, overwritten by default)

markdown-blathy export notes.md --output build/notes.pdf
markdown-blathy export notes.md --no-clobber
markdown-blathy export notes.md --output - > notes.pdf
markdown-blathy export notes.md --open
```

- `--output -` writes only PDF bytes to stdout; diagnostics go to stderr.
- `--no-clobber` refuses to overwrite an existing target.
- `--open` explicitly opens the written PDF in the system viewer.
- Exporting from stdin (`export -`) requires an explicit `--output`.

## Front matter

```bash
markdown-blathy frontmatter get notes.md
markdown-blathy frontmatter get notes.md --json
markdown-blathy frontmatter get notes.md --field title

markdown-blathy frontmatter set notes.md --title "Notes"
markdown-blathy frontmatter set notes.md --author Ada --lang hu
markdown-blathy frontmatter set notes.md --hyphenate on
markdown-blathy frontmatter set notes.md --flavor github
markdown-blathy frontmatter set notes.md --ext footnotes=true --ext table=off
markdown-blathy frontmatter set notes.md --clear-title --clear-author
markdown-blathy frontmatter set notes.md --title "Preview" --output -
```

- `set` rewrites the input atomically in place by default.
- `--output <path>` writes elsewhere; `--output -` writes the updated Markdown
  to stdout and leaves the input untouched.
- Input `-` reads stdin; with stdin the default output is stdout.
- Unmanaged front-matter keys and the document body are preserved.
- `--ext key=bool` accepts the `ext_` prefix optionally.
- `get --field` accepts `title`, `author`, `lang`, `hyphenate`, `flavor`,
  `autoHeadingLevel`, and any `ext_*` key.

## Templates

```bash
markdown-blathy validate-template theme.typ
markdown-blathy templates list
markdown-blathy templates list --json
```

`validate-template` runs the same probe compile as the GUI's Page settings. A
broken template exits non-zero and prints the diagnostics to stderr.

Bundled templates are resolved in this order:

1. `--templates-dir <dir>`;
2. `MARKDOWN_BLATHY_TEMPLATES`;
3. installed share locations (`/usr/share/markdown-blathy/templates`,
   `/usr/local/share/markdown-blathy/templates`, Homebrew, and
   `~/.local/share/markdown-blathy/templates`);
4. the embedded default `light` template.

## Shared render options

`export` and `validate-template` accept:

```text
--flavor commonmark|github|custom
--paper a4|a5|letter|legal
--margin-top N --margin-bottom N --margin-left N --margin-right N
--align left|center|justified
--template <path.typ>
--template-name light|dark|obsidian
--heading-level N
--templates-dir <dir>
```

CLI flags override the desktop app's fresh-install defaults. There is no shared
settings file in v1.

## Exit codes and output

- `0` — success.
- `1` — operational failure (I/O, render/compile, missing resources).
- `2` — invalid arguments/usage.

Errors are printed to stderr. Machine-readable output (`--json`, `--output -`)
is written only to stdout.
