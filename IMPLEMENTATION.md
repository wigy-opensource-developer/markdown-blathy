# Tauri MD Editor — Implementation Guide

## 1. Module structure

```
src-tauri/src/
├── main.rs              # calls tauri_md_editor_lib::run()
├── lib.rs               # init_logging(), tauri::Builder, command registration
├── commands.rs          # #[tauri::command] wrappers, TemplateEngineCache
├── options.rs           # Flavor, Paper, TextAlign, Margins, RenderOptions
├── markdown/
│   ├── mod.rs
│   ├── convert.rs       # comrak AST → Typst markup (the core)
│   └── escape.rs        # Typst markup escaping helpers
└── typst_engine/
    ├── mod.rs
    ├── template.rs      # prelude embedding, compose(), build_static_source, render_inputs
    ├── engine.rs        # TypstEngine build, compile → SVG pages / PDF
    └── templates/
        └── default.typ  # embedded prelude (default helpers + article)
```

**Design rule:** `markdown/` and `typst_engine/` must not import `tauri`. The `#[tauri::command]` functions are thin, serialization-only wrappers.

## 2. Dependencies

### Rust (`src-tauri/Cargo.toml`)

```toml
comrak = { version = "0.54", default-features = false, features = ["bon", "attributes"] }
typst = "0.15"
typst-layout = "0.15"       # PagedDocument type (re-exported as typst_layout::PagedDocument)
typst-pdf = "0.15"
typst-svg = "0.15"
typst-as-lib = "0.16"       # TypstWorld wrapper (pins typst ^0.15)
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

**Version pinning rule:** `typst`, `typst-layout`, `typst-pdf`, `typst-svg` must all share the same minor version (released in lockstep). Pin exact versions in `Cargo.lock`. **MSRV:** Typst 0.15 requires Rust ≥ 1.92.

### Frontend (`package.json`)

React 19 + Mantine 8, bundled by Vite. `@codemirror/lang-markdown` added explicitly (transitive via `@uiw/react-codemirror`). Fonts via `@fontsource` (self-hosted, no external requests).

## 3. API contracts

### `render_inputs` — per-render sys.inputs

```rust
pub fn render_inputs(markdown: &str, options: &RenderOptions) -> Result<(Dict, Vec<String>), String>
```

Returns `(inputs, converter_warnings)`. The `inputs` dict contains `title`, `author`, `body` keys for `sys.inputs`. Converter warnings (e.g. duplicate heading IDs) are merged with engine warnings in the command layer.

### `compose` — Typst source assembly

```rust
pub fn compose(template_source: &str, args: &TemplateArgs) -> String
```

Assembles `{template_source}\n\n#article(paper: "a4", margin-x: 20mm, margin-y: 25mm, text-align: "left", title: none, author: none)[\n{body}\n]`. The `{:?}` formatter on `paper`/`text_align` yields a valid Typst string literal for ASCII values. `title`/`author` are `none` or a debug-quoted string literal. The body is placed inside a content block — never interpolated into template source.

### `PreviewOutcome`

```rust
pub struct PreviewOutcome { pub pages: Vec<String>, pub warnings: Vec<String>, pub seq: u64 }
```

`pages` = per-page SVG strings. `warnings` = merged converter + engine warnings. `seq` = request sequence number for stale-response guard.

## 4. Markdown → Typst converter

### Node mapping

| Markdown node | Typst output |
|---|---|
| `Heading` | `"="` × level + space + inline + `#label("slug")` (see slug algorithm) |
| `Text` | escaped text (see table below) |
| `Strong` | `*…*` |
| `Emph` | `_…_` |
| `Strikethrough` | `#strike[…]` |
| `Highlight` | `#mark(body)` |
| `Subscript` | `#subscript(body)` |
| `Superscript` | `#superscript(body)` |
| `Code` | `#inline-code("…")` |
| `CodeBlock` | `#code-block("lang", "source")` |
| `Link` (url starts with `#`) | `#internal-link("slug")[label]` (prelude guards missing labels) |
| `Link` (external url) | `#link("url")[label]` |
| `Image` | `#figure-image("url", [alt])` |
| `List` | native Typst `- ` lists; ordered → `#ordered-list(start, …)` |
| `TaskItem` | `#task-list(task-done([…]), task-todo([…]), task-plain([…]))` |
| `BlockQuote` | `#blockquote[…children…]` |
| `Table` / `TableRow` / `TableCell` | `#markdown-table(columns: N, header: …, rows: …)` |
| `ThematicBreak` | `#thematic-break()` |
| `FootnoteReference` | inlined at call site as `#footnote[<definition>]` |
| `FootnoteDefinition` | dropped (collected for matching) |
| `Math` | passthrough: inline `$…$`, block `$ … $` |
| `FrontMatter` | parsed → `title`/`author` in `ConvertedDocument` + `headingIdsMaxLevel` override; nothing in body |
| `HtmlInline` / `HtmlBlock` | **lossy**: raw literal as plain text, escaped |
| `DescriptionList` | `#definition-list((term, [def]), …)` |

### Escaping rules

| Source char | Typst output |
|---|---|
| `#` | `\#` |
| `[` | `\[` |
| `]` | `\]` |
| `*` | `\*` |
| `_` | `\_` |
| `$` | `\$` |
| `\` | `\\` |
| `"` | leave as-is (template disables smart quotes) |

### Slug algorithm

IDs attach to headings as `#label("id")` — the function form (not the native
`<id>` literal) takes a string argument, so manual IDs with spaces, quotes or
unicode stay valid once escaped, and the call renders nothing.

1. **Manual ID wins**: if `ast.attrs.id` is present (from comrak's
   `header_attributes`, i.e. `# Heading {#custom-id}`), it is used as-is
   (trimmed) — at every level, regardless of `maxLevel`. A manual ID is the
   author's explicit intent; `maxLevel` limits auto-generation only.
2. **Auto-slug** otherwise: extract plain text from the heading's inline
   content only (Text, Code, Strong, Emph, etc. — children of the heading
   node, not sibling paragraphs below it); lowercase, replace
   non-alphanumeric runs with `-`, collapse consecutive `-`, trim ends.
   `{#…}` attribute syntax left in the text (comrak only parses it at end of
   line) is stripped before slugifying. Auto-slugs are generated only for
   levels 1..=`maxLevel` (`maxLevel: 0` disables them entirely).
3. **One uniqueness namespace**: manual and auto IDs share it. The first
   occurrence wins and gets the label; later duplicates (auto↔auto,
   manual↔manual, or mixed) are dropped with a warning to
   `ConvertedDocument.warnings` — Typst accepts duplicate labels at compile
   time but any reference to them fails.
4. **`maxLevel` sources**: the toolbar `RenderOptions.heading_ids`,
   overridable per document by the YAML front-matter key
   `headingIdsMaxLevel: N` (Custom flavor only, like all front matter). An
   invalid value warns and keeps the toolbar setting.

### Internal links

`[text](#fragment)` becomes `#internal-link("fragment")[text]`; the prelude
helper checks the label exists (`context { query(label(fragment)) }`) and
degrades a broken TOC entry to plain text instead of failing the compile —
the lossy philosophy the converter applies to HTML and dangling footnotes.
Links resolve in the PDF; the SVG preview follows once typst resolves
Location links there (see below).

### comrak options

```rust
pub fn comrak_options(flavor: Flavor) -> comrak::Options<'static> {
    let mut ext = comrak::Extension::default();
    ext.strikethrough = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    ext.table = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    ext.header_attributes = true;        // heading IDs: all flavors
    ext.tasklist = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    ext.footnotes = flavor == Flavor::Custom;
    ext.math_dollars = flavor == Flavor::Custom;
    ext.front_matter_delimiter = (flavor == Flavor::Custom).then(|| "---".to_owned());
    if flavor == Flavor::Custom {
        ext.superscript = true;
        ext.subscript = true;
        ext.highlight = true;
        ext.description_lists = true;
    }
    comrak::Options { extension: ext, ..comrak::Options::default() }
}
```

## 5. Template system

### Prelude contract

The embedded `default.typ` provides default implementations of every helper the converter emits. A user template is composed **after** the prelude (`{prelude}\n\n{user}`), so Typst shadowing applies — any function the user redefines wins, everything else falls back to the prelude default.

**Helpers:** `task-list`/`task-done`/`task-todo`/`task-plain`, `blockquote`, `thematic-break`, `inline-code`, `code-block`, `markdown-table`, `ordered-list`, `internal-link`, `title-block`, `mark`, `superscript`, `subscript`, `definition-list`, `markdown-toc`, `figure-image`.

The probe body exercises all helpers except `figure-image` (its image file cannot exist during validation).

### Font loading

The engine is always given embedded DejaVu Sans (regular + bold). A custom template loads only the font families it references via `#set text(font: ...)` (`template::referenced_font_families` parses the source; `typst-kit-fonts` + `TypstKitFontOptions`), plus any font files in the template's own folder. The scan runs once per engine (re)build, not per keystroke render.

## 6. Engine & compilation

### TypstEngine build

```rust
pub fn build_engine(source: &str, fonts: &FontOptions) -> Result<TypstEngine<TypstTemplateMainFile>, EngineError>
```

Builder: `.main_file(source)`, `.fonts([FONT_BYTES, FONT_BOLD_BYTES])`, `.search_fonts_with(opts)` for system/extra dirs, `.comemo_evict_max_age(Some(30))` to keep memoization cache warm across renders.

### Render preview (SVG)

```rust
pub fn compile_preview_with_inputs(engine: &TypstEngine<TypstTemplateMainFile>, inputs: &Dict) -> Result<PreviewOutcome, EngineError>
```

Compiles with `sys.inputs`, walks pages for `Destination::Location` targets, resolves via `LateLinkResolver` + `introspector`, passes anchors to `svg_in_bundle`.

### SVG internal link resolution

The plain `typst_svg::svg(page, opts)` does not resolve internal `Destination::Location` links. `compile_preview_pages` collects all `Destination::Location` targets from frame items, resolves them via the introspector, and passes anchors + resolver to `svg_in_bundle`. This enables:
- Footnote markers ↔ notes (bidirectional links)
- Heading anchors (e.g. `[Section](#section-id)`)
- Any other internal link the document contains

**typst 0.15 limitation:** the paged introspector (typst-layout) implements
`anchor()` as `None` — anchors exist only in the HTML export path — so
`LateLinkResolver` never resolves a Location link and the SVG renderer emits
`<a>` elements without `href`. The machinery stays in place unchanged: PDF
links work today (typst-pdf resolves destinations natively), and SVG links
start working as soon as typst grows paged anchors. The
`footnote_markers_have_svg_href_anchors` test is `#[ignore]`d until then.

```rust
fn compile_preview_pages(document: &PagedDocument) -> Result<Vec<String>, EngineError> {
    let resolver = LateLinkResolver::new(None, document.introspector().as_ref());
    let tracked = resolver.track();

    let mut anchors_by_page: HashMap<usize, Vec<(Point, EcoString)>> = HashMap::new();
    for (page_idx, page) in document.pages().iter().enumerate() {
        for (_, item) in page.frame.items() {
            if let FrameItem::Link(Destination::Location(loc), _) = item {
                if let (Some(anchor), Some(pos)) = (
                    document.introspector().anchor(loc),
                    document.introspector().position(loc),
                ) {
                    anchors_by_page.entry(pos.page.get() as usize)
                        .or_default().push((pos.point, anchor.clone()));
                }
            }
        }
    }

    let mut pages = Vec::with_capacity(document.pages().len());
    for (page_idx, page) in document.pages().iter().enumerate() {
        let anchors = anchors_by_page.get(&page_idx).cloned().unwrap_or_default();
        pages.push(svg_in_bundle(page, &SvgOptions::default(), &anchors, tracked));
    }
    Ok(pages)
}
```

Fragment links (`#anchor`) scroll natively in the webview.

### Export PDF

```rust
pub fn compile_pdf_with_inputs(engine: &TypstEngine<TypstTemplateMainFile>, inputs: &Dict) -> Result<Vec<u8>, EngineError>
```

Compiles with `sys.inputs`, calls `typst_pdf::pdf(&doc, &Default::default())`.

### API verification (0.15/0.16 line)

1. `TypstEngine::builder()` → `TypstTemplateEngineBuilder`; `.main_file(source)` takes content; `.fonts([&[u8]])` takes byte slices; `.build()` → `TypstEngine<TypstTemplateMainFile>`.
2. `engine.compile_with_input(dict)` → `Warned<Result<PagedDocument, TypstAsLibError>>`; unwrap via `.output`.
3. `builder.search_fonts_with(TypstKitFontOptions)` loads system fonts / extra dirs.
4. `typst_pdf::pdf(&doc, &PdfOptions) -> Result<Vec<u8>, _>`.
5. `typst_svg::svg(page: &Page, opts: &SvgOptions) -> String`.
6. `PagedDocument` in typst-layout 0.15; pages via `pages()` accessor.

## 7. Frontend

### Component tree

```
src/
├── main.jsx          # font imports, MantineProvider, <App/>
├── App.jsx           # app state, debounce loop, seq guard, layout
├── components/
│   ├── Toolbar.jsx   # file buttons, flavor/paper/margins/align selects, template, export
│   ├── Editor.jsx    # CodeMirror 6 via @uiw/react-codemirror
│   ├── Preview.jsx   # SVG pages from render_preview
│   ├── StatusBar.jsx # page count / warnings / Ready
│   └── ErrorBanner.jsx
├── lib/
│   ├── api.js        # invoke() wrappers (one per command)
│   ├── settings.js   # localStorage-backed settings + applyOptionsPatch
│   └── presets.js    # FLAVORS, PAPERS, MARGIN_PRESETS, ALIGNS tables
└── styles.css        # app-level layout only
```

### App.jsx state

```jsx
const [settings, setSettings] = useState(() => loadSettings(window.localStorage));
const { options, editorFontSize } = settings;
const [markdown, setMarkdown] = useState(SAMPLE_DOC);
const [pages, setPages] = useState([]);
const [warnings, setWarnings] = useState([]);
const [error, setError] = useState(null);
const filePath = useRef(null);
const directory = useState(null);
const seq = useRef(0);
const [title, setTitle] = useState(() => docTitle(SAMPLE_DOC, null, "Sample"));
```

### changeOption

Merges a partial patch through `applyOptionsPatch` (drops `null` values for non-nullable fields — Mantine Select deselect guard). `templatePath: null` stays a legitimate patch (clearing the custom template).

### Preview link handling

The frontend intercepts every click (`Preview.jsx` `preventDefault` on `closest("a")`). http(s) opens in the system browser; any other scheme is rejected with a banner error. Fragment links (`href` starting with `#`) are internal — left to the webview to scroll natively.

## 8. Implementation decisions

### Use `svg_in_bundle` instead of `svg()` for preview

`svg()` has no link resolver; internal links emit `<a>` without `href`. `svg_in_bundle` accepts a `LateLinkResolver` and anchor points, enabling clickable footnote markers and heading anchors in the preview. Tradeoff: slightly more code to collect anchors, but enables bidirectional footnote links and heading cross-references.

### Heading IDs are always-on, not flavor-gated

Heading IDs (auto slug + `{#custom-id}` override) are core app behavior (anchor links, TOC), not a flavor extension. Labels are invisible metadata — they don't affect rendering. Enabling `header_attributes` in all flavors keeps the feature consistent and avoids confusing flavor-specific behavior.

### Replace `eprintln!` with `tracing`

`eprintln!` output goes to stderr unconditionally. `tracing` + `EnvFilter` reads `RUST_LOG` from the environment, defaults to `warn`. `debug!` for cache rebuilds and preview timing; `warn!` for PDF open failures. No new tests needed — tracing is a side-effect replacement for `eprintln!`.

### `ConvertedDocument.warnings` for non-fatal issues

Duplicate heading IDs, and any future non-fatal warnings, are collected in `ConvertedDocument.warnings` and merged with engine warnings in the command layer. This keeps the error path (compile errors) separate from the warning path (non-fatal issues that still produce output).

### Template system is a prelude, not a full replacement

`default.typ` provides default implementations of every helper the converter emits. A user template is composed **after** the prelude, so Typst shadowing gives "default unless redefined" — a template that only restyles (or defines nothing) is valid. This is simpler than requiring a full template rewrite and more flexible for theme authors.

### Engine cache holds one engine per static source

The static source is the resolved template (prelude + user, or default) plus toolbar options — it never contains the markdown body, which travels as `sys.inputs` per render. One `TypstEngine` per static source lives in `tauri::State<Arc<Mutex<TemplateEngineCache>>>`. The engine is rebuilt (re-running the font scan) only when the template or toolbar options change.

### `comemo_evict_max_age(Some(30))` to keep cache warm

The default `Some(0)` wipes the comemo cache after every compile, re-reading font files and re-doing shared work on each keystroke render (~7× slower with system fonts). A bounded eviction age keeps font loading and recent compiles warm while bounding memory. The cache keys on `sys.inputs`, so distinct bodies never serve stale previews.