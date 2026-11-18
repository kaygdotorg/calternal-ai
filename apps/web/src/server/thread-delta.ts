/**
 * Applying one checkpoint's addition to a thread held in memory (docs/threads.md, "Read one checkpoint addition"). The server sends only the bytes a checkpoint added and the byte offset in the
 * Markdown body (the file without its frontmatter) where they go; the client keeps the body as UTF-8 bytes, splices the addition in, and parses again from the message the insertion touched.
 * Cost of one checkpoint: a backward scan to the touching message's marker, a move of the bytes after the insertion point (the tail of the last message: a few hundred bytes), and a decode and parse of that message to the end, never the whole file
 * and never a copy of it: the body lives in a buffer with spare room that grows by a quarter when full (a 5 MB body that was copied twice per checkpoint cost a garbage collection, and a late frame, each time).
 */
import type { Message } from "../apps/threads/model";
import { parseMessages } from "./thread-file";

/** A thread's Markdown held for splicing: the frontmatter as text (it never changes by a checkpoint) and the body as bytes (offsets count bytes): the first `len` bytes of `buf`, which has spare room after them. */
export interface Body { front: string; buf: Uint8Array; len: number }

const enc = new TextEncoder();
const dec = new TextDecoder();
/** The marker that opens every message, as bytes, for the backward scan. */
const MARK = enc.encode("<!-- message:");

/** Splits a full Markdown document into its frontmatter (through the closing `---` line) and its body bytes. A document with no frontmatter is all body. */
export function bodyOf(markdown: string): Body {
  const front = markdown.startsWith("---\n") ? markdown.slice(0, Math.max(0, markdown.indexOf("\n---\n", 3) + 5)) : "";
  const bytes = enc.encode(markdown.slice(front.length));
  // Spare room from the start (a quarter and 64 KiB), so the first checkpoint of a turn does not have to move a 5 MB body into a bigger buffer.
  const buf = new Uint8Array(bytes.length + (bytes.length >> 2) + 65536);
  buf.set(bytes);
  return { front, buf, len: bytes.length };
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
 * Inserts `add` into `body` at byte `offset` and returns the body (the same object, its buffer changed in place) with the messages that result: the messages before the one the insertion touched are
 * kept as they are, the rest are parsed again. Returns null when the insertion cannot be placed (an offset past the end, or the touched message is not one we hold): the body may then be half changed and
 * the caller reads the whole thread and drops it.
 */
export function applyAppend(body: Body, messages: Message[], offset: number, add: string, createdAt: number, updatedAt: number): { body: Body; messages: Message[] } | null {
  if (!Number.isInteger(offset) || offset < 0 || offset > body.len) return null;
  const from = markerBefore(body.buf.subarray(0, body.len), offset);
  const bytes = enc.encode(add);
  const need = body.len + bytes.length;
  if (need > body.buf.length) { const grown = new Uint8Array(Math.max(need, Math.ceil(body.buf.length * 1.25)) + 4096); grown.set(body.buf.subarray(0, body.len)); body.buf = grown; }
  body.buf.copyWithin(offset + bytes.length, offset, body.len);
  body.buf.set(bytes, offset);
  body.len = need;
  const tail = parseMessages(dec.decode(body.buf.subarray(from, body.len)), createdAt, updatedAt);
  if (!tail.length) return from === 0 ? { body, messages: [] } : null;
  let at = messages.length;
  for (let i = messages.length - 1; i >= 0; i--) if (messages[i]!.id === tail[0]!.id) { at = i; break; }
  if (from > 0 && at === messages.length) return null;
  return { body, messages: messages.slice(0, at).concat(tail) };
}
