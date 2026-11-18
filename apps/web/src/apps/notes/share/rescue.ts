/**
 * Keeping a person's edits when access to a shared note ends while they were writing (the write comes back `forbidden` or `not_found`). Only the blocks they changed are sent (`sharing.rescue`), with the cached title; the
 * server saves them as a note in their personal space, "Your edits to <title>". A block is the text between blank lines.
 */

/** The Markdown blocks of `text`: trimmed, non-empty, in order. */
export const blocksOf = (text: string): string[] => text.split(/\n[ \t]*\n/).map((b) => b.trim()).filter(Boolean);

/** The blocks of `local` that are not blocks of `base`, in order: what the person wrote. Cost: one pass over each text. */
export function changedBlocks(base: string, local: string): string[] {
  const had = new Set(blocksOf(base));
  return blocksOf(local).filter((b) => !had.has(b));
}

/** A short stable hash of a text (FNV-1a), to make the same unsaved text the same request again. */
export function textKey(text: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) { h ^= text.charCodeAt(i); h = Math.imul(h, 0x01000193); }
  return (h >>> 0).toString(36);
}

/** What to tell the person, in one line: where the edits went. */
export const rescueLine = (title: string, saved: string): string => `You can no longer open “${title}”. Your edits were kept as “${saved}” in your personal space.`;
