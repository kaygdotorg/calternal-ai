import type { SnapshotNode } from "../draft";

const time = (ts: string | undefined): number => (ts ? new Date(ts).getTime() : NaN);

/** Whether the child at `i` starts a group of entries: no entry before it from the same author within five minutes (a marker between them breaks the group). */
export function startsGroup(kids: readonly SnapshotNode[], i: number): boolean {
  const a = kids[i], b = kids[i - 1];
  if (a?.type !== "snapshot-entry" || b?.type !== "snapshot-entry") return true;
  return a.author.key !== b.author.key || time(a.timestamp) - time(b.timestamp) > 5 * 60_000 || time(a.timestamp) < time(b.timestamp);
}

/** Whether the child at `i` ends its group: the next child does not continue it. */
export const endsGroup = (kids: readonly SnapshotNode[], i: number): boolean => i === kids.length - 1 || startsGroup(kids, i + 1);
