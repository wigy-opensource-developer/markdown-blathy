// Stub replacement for `@codemirror/lang-javascript` (aliased in
// vite.config.js; see scripts/stubs/plain-text-parser.mjs for the why).
//
// Sole consumer: `@codemirror/lang-html`, which reads `.parser` off the
// language objects (to nest into `<script>` content and `on*=` attributes)
// and `.support` off the factory result. Script bodies stay plain text.
import { plainText } from "./plain-text-parser.mjs";

const language = { name: "javascript", parser: plainText };

export const javascriptLanguage = language;
export const typescriptLanguage = { ...language, name: "typescript" };
export const jsxLanguage = { ...language, name: "jsx" };
export const tsxLanguage = { ...language, name: "tsx" };

/** @codemirror/language `LanguageSupport` shape, minus the language. */
export function javascript() {
  return { name: "javascript", language, support: [] };
}
