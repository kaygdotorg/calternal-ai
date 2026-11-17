/**
 * The lazy pieces of the block editor and how they are loaded. `detect` looks at text (cheap regular expressions, no parse) and names the pieces it needs; `prepare` imports those chunks and
 * the grammars its fences name, before an editor is created or a note replaced, so the first draw is already right (no raw frontmatter, marker or table flashing). The extensions of every piece
 * loaded so far are in `loadedExtensions`, which the editor puts in one compartment. A note that needs none of them never fetches any, so first paint is unchanged.
 */
import type { Extension } from "@codemirror/state";

export type Feature = "frontmatter" | "ids" | "tables" | "code";

const DETECT: Record<Feature, RegExp> = {
  frontmatter: /^---\r?\n/,
  ids: /(?:^|[\s[,{])\^[A-Za-z0-9-]{2,}(?=$|[\s\]},).;:])|^\s*[-*+] \[[ xX]\](?: |$)/m,
  tables: /^\s*\|.*\|\s*\n\s*\|?\s*:?-+:?\s*(?:\||$)/m,
  code: /^[ \t]*(?:```|~~~)[ \t]*[\w+#.-]+/m,
};
const LOAD: Record<Feature, () => Promise<Extension>> = {
  frontmatter: () => import("./frontmatter").then((m) => m.frontmatter),
  ids: () => import("./ids-view").then((m) => m.ids),
  tables: () => import("./tables").then((m) => m.tables),
  code: () => import("./code").then((m) => m.code),
};
const loaded = new Map<Feature, Extension>();
const pending = new Map<Feature, Promise<void>>();

/** The pieces this text needs. */
export function detect(text: string): Feature[] {
  return (Object.keys(DETECT) as Feature[]).filter((f) => DETECT[f].test(text));
}
/** The pieces this text needs that are not loaded yet. */
export const missing = (text: string): Feature[] => detect(text).filter((f) => !loaded.has(f));
/** The extensions of every piece loaded so far. */
export const loadedExtensions = (): Extension[] => [...loaded.values()];

function load(f: Feature): Promise<void> {
  let p = pending.get(f);
  if (!p) { p = LOAD[f]().then((e) => { loaded.set(f, e); }).finally(() => pending.delete(f)); pending.set(f, p); }
  return p;
}
/** The languages named by fences in the text (first word of the info string), without duplicates. */
export function fenceLanguages(text: string): string[] {
  const out = new Set<string>();
  for (const m of text.matchAll(/^[ \t]*(?:```|~~~)[ \t]*([\w+#.-]+)/gm)) out.add(m[1]!);
  return [...out];
}
/**
 * Loads what `text` needs: the missing pieces, and the grammars its fences name. A failed load is not fatal (the piece stays off and the note stays plain Markdown) and is tried again on the
 * next call. Returns true when something new was loaded, so the caller knows to apply `loadedExtensions` to a live editor.
 */
export async function prepare(text: string): Promise<boolean> {
  const need = missing(text);
  await Promise.all(need.map((f) => load(f).catch(() => undefined)));
  if (detect(text).includes("code")) await import("./code").then((m) => m.loadGrammars(fenceLanguages(text))).catch(() => undefined);
  return need.some((f) => loaded.has(f));
}
