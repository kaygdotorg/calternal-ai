/**
 * Code-block grammars on demand. A fenced block with a language is coloured by that language's grammar, loaded from `@codemirror/language-data` the first time a note needs it (one small
 * chunk per language, fetched only when a fence names it) and coloured from the design tokens. A fence with no language, or one the list does not know, stays plain. Lazy chunk: loaded by
 * `features.ts` when a note holds a fence with a language.
 *
 * Deliberately not CodeMirror's usual nested-language parse (`parseMixed`): that re-walks the whole document tree on every edit, which costs every keystroke in a long note (measured: about
 * 1 ms per key at 4x in the 50,000-word note). Here each fence that is on screen is parsed on its own, and only when its text changes (a small cache keyed by language and text), so a key
 * typed anywhere else costs one walk over the top-level blocks on screen.
 */
import { HighlightStyle, LanguageDescription, syntaxTree } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import type { Extension, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { IterMode } from "@lezer/common";
import { highlightTree, tags as t } from "@lezer/highlight";

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

const find = (info: string): LanguageDescription | null => LanguageDescription.matchLanguageName(languages, info, true);

/** Loads the grammars the given languages name, so the first draw of a note is already coloured. Unknown names are skipped; a failed load leaves that fence plain. */
export async function loadGrammars(names: string[]): Promise<void> {
  await Promise.all(names.map((n) => find(n)?.load().catch(() => undefined)));
}

type Spans = [from: number, to: number, cls: string][];
const cache = new Map<string, Spans>();
/** The coloured spans of `code` in `lang` (offsets into `code`); cached by language and text, so an unchanged fence is never parsed twice. */
function spans(lang: LanguageDescription, code: string): Spans {
  const key = `${lang.name}\0${code}`;
  let out = cache.get(key);
  if (out) return out;
  out = [];
  const tree = lang.support!.language.parser.parse(code);
  highlightTree(tree, style, (from, to, cls) => out!.push([from, to, cls]));
  cache.set(key, out);
  if (cache.size > 64) cache.delete(cache.keys().next().value!);
  return out;
}

const colours = ViewPlugin.fromClass(class {
  decorations: DecorationSet;
  loading = new Set<string>();
  constructor(readonly view: EditorView) { this.decorations = this.build(); }
  update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged || syntaxTree(u.startState) !== syntaxTree(u.state)) this.decorations = this.build(); }
  build(): DecorationSet {
    const state = this.view.state, doc = state.doc, out: Range<Decoration>[] = [];
    for (const r of this.view.visibleRanges) {
      syntaxTree(state).iterate({
        from: r.from, to: r.to, mode: IterMode.IgnoreMounts,
        enter: (n) => {
          if (n.name !== "FencedCode") return /^(Document|Blockquote|BulletList|OrderedList|ListItem)$/.test(n.name);
          const info = n.node.getChild("CodeInfo"), text = n.node.getChildren("CodeText");
          if (!info || !text.length) return false;
          const name = doc.sliceString(info.from, info.to).trim().split(/\s/)[0]!;
          const lang = name ? find(name) : null;
          if (!lang) return false;
          if (!lang.support) {
            if (!this.loading.has(lang.name)) { this.loading.add(lang.name); lang.load().then(() => this.view.dispatch({}), () => undefined); }
            return false;
          }
          const from = text[0]!.from;
          for (const [a, b, cls] of spans(lang, doc.sliceString(from, text.at(-1)!.to))) out.push(Decoration.mark({ class: cls }).range(from + a, from + b));
          return false;
        },
      });
    }
    return Decoration.set(out, true);
  }
}, { decorations: (v) => v.decorations });

/** The code colouring: the token colours' style sheet and the plugin that applies them to the fences on screen. */
export const code: Extension = [EditorView.styleModule.of(style.module!), colours];
