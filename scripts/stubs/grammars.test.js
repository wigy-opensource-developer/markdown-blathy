// The vite aliases in vite.config.js replace `@codemirror/lang-javascript`
// and `@codemirror/lang-css` with the stubs next to this file (the editor
// highlights Markdown only — fenced code is not highlighted and `<script>`
// bodies are plain text). These tests run against the real
// `@codemirror/lang-markdown`/`lang-html` under the same aliases and pin the
// behaviour that must survive the swap: Markdown parsing, tag completion, and
// nested `<script>`/`<style>`/attribute regions resolving as flat plain text.
import { describe, expect, it } from "vitest";
import { EditorState } from "@codemirror/state";
import { CompletionContext } from "@codemirror/autocomplete";
import { ensureSyntaxTree } from "@codemirror/language";
import { html, htmlCompletionSource } from "@codemirror/lang-html";
import { markdown } from "@codemirror/lang-markdown";
import { css, cssLanguage } from "./codemirror-lang-css.mjs";
import { javascriptLanguage, javascript, typescriptLanguage } from "./codemirror-lang-javascript.mjs";

function parseWith(doc, extensions) {
  const state = EditorState.create({ doc, extensions });
  const tree = ensureSyntaxTree(state, doc.length, 5000);
  expect(tree, "full parse did not finish").not.toBeNull();
  return tree;
}

describe("grammar stubs", () => {
  it("parse any range as a single flat node", () => {
    const tree = javascriptLanguage.parser.parse("var x = 1; // anything\n");
    expect(tree.children).toHaveLength(0);
    expect(tree.length).toBe(23);
  });

  it("ignore nesting configs (lang-html sets `top` at module load)", () => {
    expect(typescriptLanguage.parser.configure({ top: "Script" })).toBe(typescriptLanguage.parser);
    expect(cssLanguage.parser.configure({ top: "Styles" })).toBe(cssLanguage.parser);
  });

  it("expose the LanguageSupport shape lang-html spreads", () => {
    expect(javascript().support).toEqual([]);
    expect(css().support).toEqual([]);
  });
});

describe("markdown editor with stubbed js/css grammars", () => {
  it("parses a plain markdown document", () => {
    const tree = parseWith("# Title\n\nSome *emphasised* text.\n", [markdown()]);
    const cursor = tree.cursor();
    cursor.firstChild();
    expect(cursor.name).toBe("ATXHeading1");
  });

  it("renders fenced code as plain text (no codeLanguages configured)", () => {
    const doc = "```js\nconst x = 1;\n```\n";
    const tree = parseWith(doc, [markdown()]);
    // The js body stays opaque CodeText — the js grammar never runs on it.
    const at = tree.resolveInner(doc.indexOf("const") + 2, 1);
    expect(at.name).toBe("CodeText");
  });

  it("keeps raw HTML blocks as markdown blocks", () => {
    const doc = '<div style="color: red">\n<script>var x = 1;</script>\n</div>\n\ntext\n';
    const tree = parseWith(doc, [markdown()]);
    const names = [];
    tree.iterate({ enter: (n) => names.push(n.name) });
    expect(names).toContain("HTMLBlock");
  });

  it("mounts script content as one plain-text node instead of a js tree", () => {
    // lang-html nests <script> bodies through the (stubbed) js parser; the
    // result must be a single flat node covering exactly the script body.
    const doc = "<script>var x = 1;</script>";
    const tree = parseWith(doc, [html()]);
    const at = tree.resolveInner(doc.indexOf("var") + 1, 1);
    expect(at.name).toBe(""); // NodeType.none — the plain-text mount
    expect([at.from, at.to]).toEqual([8, 18]);
  });

  it("parses style tags and style/event attributes without throwing", () => {
    const doc =
      '<div style="color: red" onclick="go()"><style>p { color: blue; }</style></div>\n';
    const tree = parseWith(doc, [html()]);
    for (const needle of ["color: red", "go()", "color: blue"]) {
      expect(tree.resolveInner(doc.indexOf(needle) + 1, 1)).toBeTruthy();
    }
  });

  it("still completes HTML tag names (lang-markdown's own call)", () => {
    // Mirrors htmlTagCompletions() in @codemirror/lang-markdown: an explicit
    // completion on an empty document lists every standard tag.
    const state = EditorState.create({ extensions: [html()] });
    const result = htmlCompletionSource(new CompletionContext(state, 0, true));
    const labels = result.options.map((option) => option.label);
    expect(labels).toContain("<table");
  });
});
