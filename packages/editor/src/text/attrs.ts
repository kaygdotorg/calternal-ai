/**
 * What the browser is told about a text surface, in one place: spelling is checked and the first letter of a sentence is capitalised for prose (a note, a message, a title); a search field, a
 * code field, a secret and an address are left as typed. A field and the block editor read the same table.
 */
export type TextKind = "prose" | "search" | "code" | "secret" | "url";

/** The attributes for a kind. */
export function textAttrs(kind: TextKind = "prose"): Record<string, string> {
  if (kind === "prose") return { spellcheck: "true", autocapitalize: "sentences", autocorrect: "on" };
  return { spellcheck: "false", autocapitalize: "off", autocorrect: "off" };
}

/** Sets the attributes on a field, leaving any the author already set. */
export function applyTextAttrs(el: HTMLElement, kind: TextKind = "prose"): void {
  for (const [k, v] of Object.entries(textAttrs(kind))) if (!el.hasAttribute(k)) el.setAttribute(k, v);
}
