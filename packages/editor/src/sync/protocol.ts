/**
 * The note sync wire format.
 *
 * Messages are y-protocols messages, byte for byte: `0` is a Yjs sync
 * message (step 1, step 2 or an update) and `1` is awareness. Calternal adds
 * one pair on top, as y-protocols custom messages (a tag, then a
 * length-prefixed buffer), so the editor can tell the truth about saving:
 *
 * - `100 <buf: varuint id>` from the client: "tell me when everything I
 *   sent before this is durable".
 * - `101 <buf: varuint id>` from the server: "everything you sent before
 *   check `<id>` is durable". Rooms answer checks in order, only once the
 *   changes are in history, and cumulatively: one answer covers every
 *   earlier check, so a busy disk answers many checks with one message.
 *
 * A server that doesn't know `100` simply never answers, and the editor then
 * never claims to be saved.
 */

import * as decoding from 'lib0/decoding'
import * as encoding from 'lib0/encoding'
import * as syncProtocol from 'y-protocols/sync'
import * as Y from 'yjs'

export const MSG_SYNC = 0
export const MSG_AWARENESS = 1
export const MSG_SAVE_CHECK = 100
export const MSG_SAVED = 101
/** A refused room update, with the public error code as a JSON string. */
export const MSG_REFUSED = 102
/** A temporary room failure: leave, back off, then rejoin and resubmit. */
export const MSG_RETRY = 103

/** Public errors for which submitting the same request cannot succeed. */
export function permanentRefusal(code: string): boolean {
  return ['too_large', 'storage_quota', 'invalid_input', 'not_found', 'unauthorized', 'forbidden'].includes(code)
}

/**
 * The largest room message the server takes (one y-protocols message, e.g.
 * a pasted update). Messages over the socket's 1 MiB frame limit travel in
 * pieces (`live.ts`, `crates/server/src/rooms.rs`). Keep in step with
 * `MAX_ROOM_MESSAGE` there.
 */
export const MAX_ROOM_MESSAGE = 64 * 1024 * 1024

/**
 * Close code a transport reports when the server refused the client's copy
 * because it is from another lineage (`lineage_changed`).
 */
export const CLOSE_LINEAGE = 4409

/** A sync message, already decoded far enough to decide what to do. */
export type Incoming =
  | { kind: 'step1'; stateVector: Uint8Array }
  /** The answer to our step 1: everything the server has that we lack. */
  | { kind: 'step2'; update: Uint8Array }
  | { kind: 'update'; update: Uint8Array }
  | { kind: 'saved'; id: number }
  | { kind: 'refused'; code: string }
  | { kind: 'retry'; code: string }
  | { kind: 'other' }

/** Reads one message from the server. Unknown kinds are ignored, not fatal. */
export function decodeMessage(data: Uint8Array): Incoming {
  const d = decoding.createDecoder(data)
  const type = decoding.readVarUint(d)
  if (type === MSG_REFUSED || type === MSG_RETRY) {
    const kind = type === MSG_RETRY ? 'retry' : 'refused'
    try {
      const code: unknown = JSON.parse(new TextDecoder().decode(decoding.readVarUint8Array(d)))
      return { kind, code: typeof code === 'string' ? code : 'error' }
    } catch { return { kind, code: 'error' } }
  }
  if (type === MSG_SAVED) {
    const buf = decoding.readVarUint8Array(d)
    return { kind: 'saved', id: decoding.readVarUint(decoding.createDecoder(buf)) }
  }
  if (type !== MSG_SYNC) return { kind: 'other' }
  const sub = decoding.readVarUint(d)
  const payload = decoding.readVarUint8Array(d)
  if (sub === syncProtocol.messageYjsSyncStep1) return { kind: 'step1', stateVector: payload }
  if (sub === syncProtocol.messageYjsSyncStep2) return { kind: 'step2', update: payload }
  return { kind: 'update', update: payload }
}

export function encodeStep1(doc: Y.Doc): Uint8Array {
  const e = encoding.createEncoder()
  encoding.writeVarUint(e, MSG_SYNC)
  syncProtocol.writeSyncStep1(e, doc)
  return encoding.toUint8Array(e)
}

export function encodeStep2(doc: Y.Doc, stateVector: Uint8Array): Uint8Array {
  const e = encoding.createEncoder()
  encoding.writeVarUint(e, MSG_SYNC)
  syncProtocol.writeSyncStep2(e, doc, stateVector)
  return encoding.toUint8Array(e)
}

export function encodeUpdate(update: Uint8Array): Uint8Array {
  const e = encoding.createEncoder()
  encoding.writeVarUint(e, MSG_SYNC)
  syncProtocol.writeUpdate(e, update)
  return encoding.toUint8Array(e)
}

function custom(tag: number, id: number): Uint8Array {
  const inner = encoding.createEncoder()
  encoding.writeVarUint(inner, id)
  const e = encoding.createEncoder()
  encoding.writeVarUint(e, tag)
  encoding.writeVarUint8Array(e, encoding.toUint8Array(inner))
  return encoding.toUint8Array(e)
}

export const encodeSaveCheck = (id: number) => custom(MSG_SAVE_CHECK, id)
export const encodeSaved = (id: number) => custom(MSG_SAVED, id)

/** Reads the id of a save check (a test server's side of the pair). */
export function decodeSaveCheck(data: Uint8Array): number | null {
  const d = decoding.createDecoder(data)
  if (decoding.readVarUint(d) !== MSG_SAVE_CHECK) return null
  return decoding.readVarUint(decoding.createDecoder(decoding.readVarUint8Array(d)))
}
