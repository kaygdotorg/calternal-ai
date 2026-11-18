/**
 * Editing a sent message forks the thread in place (docs/threads-ui.md, "Editing a sent message"): the edited message and what follows it become another version of the tail from that message on; the old tail is kept, nothing is deleted, and
 * the thread stays one row in the list. Exactly one version is on screen at a time, and which one is per thread (every viewer sees the same). This is the fixtures' model of it (the server's file shape is the per-turn lane's): one fork point per thread,
 * version 0 is the original (its messages stay where they were, in the base), the others are stored tails starting with the edited message. Pure functions over `Fork`, so the rules are testable without a client.
 */
import type { Message } from "./model";

/** The fork point of a thread. */
export interface Fork {
  /** The index of the edited message: every version agrees before it and differs from it on. */
  at: number;
  /** How many messages the original version has, kept while another version is shown. */
  saved: number;
  /** The versions: `tails[0]` is the original (empty here: its messages are the thread's base), the rest start with the edited message. */
  tails: Message[][];
  /** Which version is shown (an index into `tails`). */
  active: number;
}

/** A fork of a thread of `count` messages that already shows a new version, whose first message is `first`. */
export const newFork = (at: number, count: number, first: Message): Fork => ({ at, saved: count, tails: [[], [first]], active: 1 });

/** Adds another version to a fork at the same message and shows it; `count` is the message count of the version being left. */
export function addVersion(f: Fork, count: number, first: Message): void {
  if (f.active === 0) f.saved = count;
  f.tails.push([first]);
  f.active = f.tails.length - 1;
}

/** Shows `version` and returns the message count of the thread then. `count` is the count of the version being left (the original's is remembered). */
export function switchTo(f: Fork, version: number, count: number): number {
  if (f.active === 0) f.saved = count;
  f.active = Math.max(0, Math.min(f.tails.length - 1, version));
  return f.active === 0 ? f.saved : f.at + f.tails[f.active]!.length;
}

/** The message at index `i` when a version other than the original is shown and `i` lies in its tail; otherwise undefined (the base has it). */
export const tailAt = (f: Fork | undefined, i: number): Message | undefined => (f && f.active !== 0 && i >= f.at ? f.tails[f.active]![i - f.at] : undefined);

/** The edited message with its version marker (`n` of `of`, one-based) when it has more than one version. */
export function stamp(f: Fork | undefined, i: number, m: Message): Message {
  return f && i === f.at && f.tails.length > 1 ? { ...m, version: { n: f.active + 1, of: f.tails.length } } : m;
}
