/**
 * Changing several `calternal-` keys of a note's frontmatter in ONE edit (a theme sets several at once; separate edits would be separate undo steps and redraws). Pure text in, one change out, so it is
 * tested without an editor. A key with `null` is removed; any other line is left exactly as it was; a note with no frontmatter gets a block only when a key has a value. Ported from the existing
 * app's `apps/web/src/lib/style-keys.ts` (origin/dev) unchanged in behaviour: the note's text is the only place a style lives.
 */

/** Keys to set (a string value) or remove (null). */
export type StylePatch = Record<string, string | null>;
/** One replacement in the note's text, in the shape the editor's `apply` takes. */
export type TextChange = { from: number; to: number; insert: string };

const FRONTMATTER = /^---\n([\s\S]*?\n)?---\n/;

/** The one change that applies `patch` to `text`, or null when nothing would change. */
export function patchFrontmatter(text: string, patch: StylePatch): TextChange | null {
  const keys = Object.keys(patch);
  const fm = FRONTMATTER.exec(text);
  if (!fm) {
    const lines = keys.filter((k) => patch[k] != null).map((k) => `${k}: ${patch[k]}\n`);
    return lines.length ? { from: 0, to: 0, insert: `---\n${lines.join("")}---\n` } : null;
  }
  const inner = fm[1] ?? "";
  const lines = inner === "" ? [] : inner.slice(0, -1).split("\n");
  for (const key of keys) {
    const at = lines.findIndex((l) => l.startsWith(`${key}:`));
    const value = patch[key];
    if (at >= 0) {
      if (value == null) lines.splice(at, 1);
      else lines[at] = `${key}: ${value}`;
    } else if (value != null) lines.push(`${key}: ${value}`);
  }
  const next = lines.length ? lines.join("\n") + "\n" : "";
  return next === inner ? null : { from: 4, to: 4 + inner.length, insert: next };
}
