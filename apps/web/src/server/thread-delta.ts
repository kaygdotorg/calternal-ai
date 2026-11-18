/**
 * Applying one checkpoint's addition to a thread held in memory (docs/threads.md, "Read one checkpoint addition"). The server sends only the bytes a checkpoint added and the byte offset in the
 * Markdown body (the file without its frontmatter) where they go; the client keeps the body as UTF-8 bytes, splices the addition in, and parses again from the message the insertion touched.
 * Cost of one checkpoint: one copy of the body's bytes (a memcpy: about 1 ms for 5 MB), a backward scan to the touching message's marker, and a decode and parse of that message to the end, never the whole file.
 */
import type { Message } from "../apps/threads/model";
import { parseMessages } from "./thread-file";

/** A thread's Markdown held for splicing: the frontmatter as text (it never changes by a checkpoint) and the body as bytes (offsets count bytes). */
export interface Body { front: string; bytes: Uint8Array }

const enc = new TextEncoder();
const dec = new TextDecoder();
/** The marker that opens every message, as bytes, for the backward scan. */
const MARK = enc.encode("<!-- message:");

/** Splits a full Markdown document into its frontmatter (through the closing `---` line) and its body bytes. A document with no frontmatter is all body. */
export function bodyOf(markdown: string): Body {
  if (!markdown.startsWith("---\n")) return { front: "", bytes: enc.encode(markdown) };
  const end = markdown.indexOf("\n---\n", 3);
  const cut = end < 0 ? 0 : end + 5;
  return { front: markdown.slice(0, cut), bytes: enc.encode(markdown.slice(cut)) };
}

/** Where the message marker at or before byte `at` begins, or 0 when none does (parsing then starts at the top). */
function markerBefore(bytes: Uint8Array, at: number): number {
  outer: for (let i = Math.min(at, bytes.length - MARK.length); i >= 0; i--) {
    for (let k = 0; k < MARK.length; k++) if (bytes[i + k] !== MARK[k]) continue outer;
    return i;
  }
  return 0;
}

/**
 * Inserts `add` into `body` at byte `offset` and returns the new body with the messages that result: the messages before the one the insertion touched are kept as they are, the rest are parsed again.
 * Returns null when the insertion cannot be placed (an offset past the end, or the touched message is not one we hold): the caller then reads the whole thread.
 */
export function applyAppend(body: Body, messages: Message[], offset: number, add: string, createdAt: number, updatedAt: number): { body: Body; messages: Message[] } | null {
  if (!Number.isInteger(offset) || offset < 0 || offset > body.bytes.length) return null;
  const bytes = enc.encode(add);
  const next = new Uint8Array(body.bytes.length + bytes.length);
  next.set(body.bytes.subarray(0, offset), 0);
  next.set(bytes, offset);
  next.set(body.bytes.subarray(offset), offset + bytes.length);
  const from = markerBefore(body.bytes, offset);
  const tail = parseMessages(dec.decode(next.subarray(from)), createdAt, updatedAt);
  if (!tail.length) return from === 0 ? { body: { front: body.front, bytes: next }, messages: [] } : null;
  let at = messages.length;
  for (let i = messages.length - 1; i >= 0; i--) if (messages[i]!.id === tail[0]!.id) { at = i; break; }
  if (from > 0 && at === messages.length) return null;
  return { body: { front: body.front, bytes: next }, messages: messages.slice(0, at).concat(tail) };
}
