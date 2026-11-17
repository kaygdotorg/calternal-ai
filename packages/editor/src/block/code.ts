/**
 * Code-block grammars on demand. A fenced block with a language is parsed by that language's grammar, loaded from `@codemirror/language-data` the first time a note needs it (one small chunk
 * per language, fetched only when a fence names it) and coloured from the design tokens. A fence with no language, or a language the list does not know, stays plain. The markdown language in
 * `editor.ts` asks `codeLanguage(info)` for each fence; this module answers once loaded. Lazy chunk: loaded by `features.ts` when a note holds a fence with a language.
 */
import { HighlightStyle, LanguageDescription, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { tags as t } from "@lezer/highlight";
import { setCodeLanguages } from "./code-registry";

const style = HighlightStyle.define([
  { tag: [t.keyword, t.operatorKeyword, t.modifier, t.controlKeyword], color: "var(--accent)" },
  { tag: [t.string, t.special(t.string), t.regexp], color: "var(--good)" },
  { tag: [t.number, t.bool, t.null, t.atom], color: "var(--warn)" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--ink-3)", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.definition(t.variableName)], color: "var(--info)" },
  { tag: [t.typeName, t.className, t.namespace], color: "var(--info)" },
  { tag: [t.propertyName, t.attributeName], color: "var(--ink)" },
  { tag: [t.invalid], color: "var(--danger)" },
]);

/** The grammar for a fence's info string, or null (plain). Loading starts when this first asks for it. */
const find = (info: string): LanguageDescription | null => LanguageDescription.matchLanguageName(languages, info, true);
setCodeLanguages(find);

/** Loads the grammars the given languages name, so the first draw of a note is already coloured. Unknown names are skipped; a failed load leaves that fence plain. */
export async function loadGrammars(names: string[]): Promise<void> {
  await Promise.all(names.map((n) => find(n)?.load().catch(() => undefined)));
}

/** The highlight style for code tokens. */
export const code = syntaxHighlighting(style);
