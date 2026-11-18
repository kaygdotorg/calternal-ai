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

/** The transport for a note's fixture room, made (with its scripted collaborator) the first time it is asked for. */
export function fixtureTransport(noteId: string, text: string, lineage: () => string | null, onLineage: (l: string) => void): Transport {
  let room = rooms.get(noteId);
  if (!room) { room = new Room(text); rooms.set(noteId, room); if (new URLSearchParams(location.search).get("peer") !== "0") collaborator(room); } // `?peer=0`: alone in the room (the typing measurement)
  const r = room;
  return memoryTransport(() => r, lineage, onLineage);
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
  setTimeout(() => { move(); if (text.length) { const line = text.toString().indexOf("\n\n"); text.insert(line < 0 ? text.length : line, " (Maya: checked)"); } }, 3000);
}
