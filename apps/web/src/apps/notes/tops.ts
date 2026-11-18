/** The top of notes that were opened, for the row press's preview (no fetch on a press: what was read is what it shows). At most `TOPS` notes, the oldest dropped first. */
export const TOPS = 40;
const tops = new Map<string, string>();
/** Remembers the top of a note that was just read. */
export function rememberTop(id: string, markdown: string): void {
  tops.delete(id); tops.set(id, markdown.slice(0, 900));
  if (tops.size > TOPS) tops.delete(tops.keys().next().value!);
}
/** The top of a note read earlier, or `undefined`. */
export const topOf = (id: string): string | undefined => tops.get(id);
