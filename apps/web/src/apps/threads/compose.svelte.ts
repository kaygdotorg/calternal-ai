/**
 * Hand-offs between parts of the thread view that do not own each other: selected text going into the composer ("Comment") or into the right
 * sidebar's side chat ("Ask in Side Chat"). A counter makes the same text insertable twice in a row.
 */

/** A selection as a Markdown quote: every line prefixed with `> `, then one blank line, ready for the person to type under it. */
export function quoteOf(text: string): string {
  return text.trim().split(/\r?\n/).map((l) => `> ${l}`.trimEnd()).join("\n") + "\n\n";
}

class Bridge {
  /** Text waiting to be put in the composer, with a counter so a repeat still triggers. */
  composer = $state<{ quote: string; n: number } | null>(null);
  /** Text waiting to open the side chat. */
  side = $state<{ quote: string; n: number } | null>(null);
  private n = 0;
  /** "Comment": the selection goes into the composer as a quote. */
  comment(text: string): void {
    this.composer = { quote: quoteOf(text), n: ++this.n };
  }
  /** "Ask in Side Chat": the selection opens the side chat, quoted. */
  ask(text: string): void {
    this.side = { quote: quoteOf(text), n: ++this.n };
  }
}

/** The shared bridge. */
export const bridge = new Bridge();
