// Stub replacement for `@codemirror/lang-css` (aliased in vite.config.js;
// see scripts/stubs/plain-text-parser.mjs for the why).
//
// Sole consumer: `@codemirror/lang-html`, which reads `.parser` (for `<style>`
// content and `style=` attributes) and `.support` off the factory result.
// Stylesheet bodies stay plain text.
import { plainText } from "./plain-text-parser.mjs";

const language = { name: "css", parser: plainText };

export const cssLanguage = language;

/** @codemirror/language `LanguageSupport` shape, minus the language. */
export function css() {
  return { name: "css", language, support: [] };
}
