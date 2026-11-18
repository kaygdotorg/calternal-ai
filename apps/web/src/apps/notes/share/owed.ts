/**
 * Edits owed to the person's personal space after they lost access to a note. A rescue (`sharing.rescue`) must not depend on the person keeping a tab open or the server answering at once, so before it is tried the changed
 * blocks are written down on this device (IndexedDB, one small record per request) and the record is removed only when the rescue succeeded. It is tried again whenever the browser comes back online and the next time Notes loads;
 * the request id is stable, so the server saves the note once however often it is retried. Bounded: one record per refused note edit, each a few blocks, removed on success.
 */
import { sharing } from "./sharing";
import { changedBlocks, rescueLine } from "./rescue";

/** What is owed: the note, its cached title and the blocks the person wrote, under the request id the server de-duplicates on. */
export interface Owed { requestId: string; noteId: string; title: string; blocks: string[]; at: number }

/** Where records are kept: IndexedDB in the app, memory in tests. */
export interface OwedStore { put(o: Owed): Promise<void>; all(): Promise<Owed[]>; remove(requestId: string): Promise<void> }

/** The device's own store: one object store in a small database of its own. */
export function idbOwed(): OwedStore {
  const db = (): Promise<IDBDatabase> => new Promise((ok, no) => {
    const r = indexedDB.open("calternal-rescue-owed", 1);
    r.onupgradeneeded = () => r.result.createObjectStore("owed", { keyPath: "requestId" });
    r.onsuccess = () => ok(r.result);
    r.onerror = () => no(r.error);
  });
  const run = async <T,>(mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> => {
    const d = await db();
    return new Promise<T>((ok, no) => { const q = f(d.transaction("owed", mode).objectStore("owed")); q.onsuccess = () => { d.close(); ok(q.result); }; q.onerror = () => { d.close(); no(q.error); }; });
  };
  return { put: async (o) => void (await run("readwrite", (s) => s.put(o))), all: () => run("readonly", (s) => s.getAll() as IDBRequest<Owed[]>), remove: async (id) => void (await run("readwrite", (s) => s.delete(id))) };
}

let store: OwedStore | null = null;
/** The store in use (tests replace it). */
export const owedStore = (s?: OwedStore): OwedStore => (store = s ?? store ?? idbOwed());

/** What to tell the person while a rescue is still waiting. */
export const owedLine = (title: string): string => `You can no longer open “${title}”. Your edits are kept on this device and go to your personal space as soon as you are back online.`;

/**
 * Keeps the blocks the person changed (against `base`) and tries to send them now. Returns the one line to show: where the edits went, or that they are kept and will go. Nothing changed means nothing is owed.
 * `requestId` is stable for the same text, so a second attempt is the same request.
 */
export async function rescueEdits(noteId: string, title: string, base: string, local: string, requestId: string): Promise<string> {
  return rescueBlocks(noteId, title, changedBlocks(base, local), requestId);
}

/**
 * The same, for blocks already known to be this person's own (a live note's `LocalIntent`, which never includes what other people wrote): the room case must not diff the shared text, since that would hand other people's
 * edits to the rescue.
 */
export async function rescueBlocks(noteId: string, title: string, blocks: string[], requestId: string): Promise<string> {
  if (!blocks.length) return `You can no longer open “${title}”. You had changed nothing in it.`;
  const rec: Owed = { requestId, noteId, title, blocks, at: Date.now() };
  try { await owedStore().put(rec); } catch { /* no durable store: still try now */ }
  try { return await settleOne(rec); } catch { return owedLine(title); }
}

async function settleOne(o: Owed): Promise<string> {
  const { note } = await sharing.rescue(o.title, o.blocks, o.requestId);
  await owedStore().remove(o.requestId).catch(() => {});
  return rescueLine(o.title, note.title);
}

let settling: Promise<string[]> | null = null;
/** Tries every owed rescue, oldest first; returns the line for each one that went through (a failure stays owed and is tried again later). One pass at a time. */
export function settleOwed(): Promise<string[]> {
  return (settling ??= (async () => {
    const lines: string[] = [];
    try {
      for (const o of (await owedStore().all().catch(() => [])).sort((a, b) => a.at - b.at)) {
        try { lines.push(await settleOne(o)); } catch { break; } // the server is still away: the rest wait too
      }
    } finally { settling = null; }
    return lines;
  })());
}
