/**
 * The fixture room (fixtures mode, `?live=1`): an in-memory room for a note with one scripted collaborator, "Maya Okafor", who moves her cursor and types a line, so cursors, presence, undo and the save receipts can be seen and tested
 * without a server. It speaks the room contract of docs/live-rooms.md on lane/live-rooms (sync, save checks, awareness) through the same transport interface as the live socket. It is the editor package's in-memory
 * test room; loaded only here.
 */
import { Awareness } from "y-protocols/awareness";
import * as Y from "yjs";
import { Room } from "../../../../../../packages/editor/test-server/room";
import { memoryTransport } from "../../../../../../packages/editor/test-server/memory";
import { SyncClient } from "../../../../../../packages/editor/src/sync/client";
import { ImeGate } from "../../../../../../packages/editor/src/sync/gate";
import type { Transport } from "../../../../../../packages/editor/src/sync/transport";

const rooms = new Map<string, Room>();
const KEEP = (id: string) => `cal.fixture-room.${id}`;
const toB64 = (b: Uint8Array) => { let s = ""; for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000)); return btoa(s); };
const fromB64 = (t: string) => Uint8Array.from(atob(t), (c) => c.charCodeAt(0));

/** The room, as a server would keep it between visits: its lineage and state are kept in this browser (written a moment after a change, never per keystroke), so a reload finds the same room. */
function roomFor(noteId: string, text: string): Room {
  const room = new Room("");
  let saved: { lineage: string; state: string } | null = null;
  try { saved = JSON.parse(localStorage.getItem(KEEP(noteId)) ?? "null"); } catch { /* no kept room */ }
  if (saved) { room.lineage = saved.lineage; Y.applyUpdate(room.doc, fromB64(saved.state)); } else room.doc.getText("md").insert(0, text);
  let timer = 0;
  room.doc.on("update", () => {
    clearTimeout(timer);
    timer = window.setTimeout(() => { try { localStorage.setItem(KEEP(noteId), JSON.stringify({ lineage: room.lineage, state: toB64(Y.encodeStateAsUpdate(room.doc)) })); } catch { /* full: the room is just not kept */ } }, 400);
  });
  return room;
}

/** `?test=1`: the network, for the e2e checks: `cut()` drops the connection and keeps it down, `restore()` lets it come back, `revoke()` ends the membership as the room does when access is lost. */
interface Net { cut(): void; restore(): void; revoke(): void }

/** The transport for a note's fixture room, made (with its scripted collaborator) the first time it is asked for. */
export function fixtureTransport(noteId: string, text: string, lineage: () => string | null, onLineage: (l: string) => void): Transport {
  let room = rooms.get(noteId);
  if (!room) { room = roomFor(noteId, text); rooms.set(noteId, room); if (new URLSearchParams(location.search).get("peer") !== "0") collaborator(room); } // `?peer=0`: alone in the room (the typing measurement)
  const r = room;
  const t = memoryTransport(() => r, lineage, onLineage);
  let down = false;
  const connect = t.connect.bind(t);
  const closes: Array<(code: number, reason: string) => void> = [];
  const onClose = t.onClose.bind(t);
  t.onClose = (f) => { closes.push(f); onClose(f); };
  t.connect = () => { if (!down) connect(); };
  if (new URLSearchParams(location.search).has("test")) (globalThis as unknown as { __net: Net }).__net = { cut() { down = true; t.drop(); }, restore() { down = false; t.connect(); }, revoke() { down = true; closes.forEach((f) => f(4403, "access_revoked")); } };
  return t;
}

/** Maya: joins, moves her cursor around the first lines now and then, and types one line a little while after. */
function collaborator(room: Room): void {
  const doc = new Y.Doc();
  const awareness = new Awareness(doc);
  const client = new SyncClient({ doc, transport: memoryTransport(() => room), gate: new ImeGate(), channel: null, awareness, checkDelayMs: 50 });
  awareness.setLocalState({ account_id: "acc-maya" });
  client.connect();
  const text = doc.getText("md");
  const at = (i: number) => Y.relativePositionToJSON(Y.createRelativePositionFromTypeIndex(text, Math.max(0, Math.min(i, text.length))));
  let step = 0;
  const move = () => {
    if (!text.length) return;
    const from = Math.floor(((step++ * 37) % 100) / 100 * Math.min(text.length, 400));
    awareness.setLocalStateField("cursor", { anchor: at(from), head: at(step % 3 === 0 ? from + 12 : from) });
  };
  setInterval(move, 2500);
  // `?test=1`: the collaborator types on demand (a peer's edit arriving while this person has unconfirmed ones).
  if (new URLSearchParams(location.search).has("test")) (globalThis as unknown as { __peer: { type(t: string): void } }).__peer = { type: (t) => text.insert(text.length, t) };
  // She types at the end of the first paragraph, not in the title.
  setTimeout(() => { move(); if (text.length) { const t = text.toString(); const first = t.indexOf("\n\n"); const line = first < 0 ? -1 : t.indexOf("\n\n", first + 2); text.insert(line < 0 ? text.length : line, " (Maya: checked)"); } }, 3000);
}
