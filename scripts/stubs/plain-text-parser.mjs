// A `@lezer/common` parser that produces a single flat node.
//
// `@codemirror/lang-html` wires nested grammars into raw HTML blocks by
// handing concrete `Parser` objects to `configureNesting` (for `<script>` /
// `<style>` content and the `style` / `on*` attributes). The markdown editor
// highlights those nested regions as plain text — fenced code is not
// highlighted either (`markdown()` is configured without `codeLanguages`) —
// so the stub languages in this directory substitute this parser for the
// real js/css grammars.
//
// Contract (from `parseMixed` in @lezer/common): `createParse` returns a
// `PartialParse` (`parsedPos`, `advance`, `stopAt`, `finish`); the tree it
// yields is mounted onto the node being nested, in coordinates relative to
// that node, so it just has to cover the parsed range's span.
import { NodeType, Parser, Tree } from "@lezer/common";

export class PlainTextParser extends Parser {
  /** Nesting specs `.configure({ top })` their parsers at module load
   *  (lang-html's `jsonParser`, the `style=` attribute); a no-op is the
   *  point — nothing descends into plain text. */
  configure() {
    return this;
  }

  createParse(input, _fragments, ranges) {
    const from = ranges?.length ? ranges[0].from : 0;
    const to = ranges?.length ? ranges[ranges.length - 1].to : input.length;
    const tree = new Tree(NodeType.none, [], [], to - from);
    return {
      parsedPos: to,
      advance: () => tree,
      stopAt() {},
      finish: () => tree,
    };
  }
}

/** Shared instance; parser objects are stateless. */
export const plainText = new PlainTextParser();
