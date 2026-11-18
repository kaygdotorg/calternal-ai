/**
 * An in-memory transport to a `Room`, for unit tests. Messages are delivered
 * asynchronously (like a socket), in order.
 */

import type { Transport } from '../src/sync/transport'
import type { Conn, Room } from './room'

export interface MemoryTransport extends Transport {
  /** Simulates the network dropping (no clean close from the client). */
  drop(): void
}

export function memoryTransport(
  getRoom: () => Room,
  lineage: () => string | null = () => null,
  onLineage: (lineage: string) => void = () => {},
): MemoryTransport {
  let conn: (Conn & { alive: boolean }) | null = null
  const opens: Array<() => void> = []
  const closes: Array<(code: number, reason: string) => void> = []
  const messages: Array<(data: Uint8Array) => void> = []
  const later = (f: () => void) => queueMicrotask(f)

  const close = (code: number, reason: string) => {
    if (!conn) return
    const c = conn
    c.alive = false
    conn = null
    getRoom().leave(c)
    later(() => closes.forEach((f) => f(code, reason)))
  }

  const open = () => {
    const c: Conn & { alive: boolean } = {
      alive: true,
      send: (data) => later(() => c.alive && messages.forEach((f) => f(data))),
      close: (code, reason) => {
        if (conn === c) close(code, reason)
      },
    }
    conn = c
    later(() => {
      if (!c.alive) return
      // Like the live socket: "joined" (open) comes before any frame, and
      // a refused join never opens.
      const room = getRoom()
      const mine = lineage()
      if (mine && mine !== room.lineage) return c.close(4409, room.lineage)
      onLineage(room.lineage)
      opens.forEach((f) => f())
      room.join(c, mine)
    })
  }

  return {
    get connected() {
      return conn?.alive ?? false
    },
    connect() {
      if (!conn) open()
    },
    disconnect() {
      close(1000, '')
    },
    drop() {
      close(1006, '')
    },
    send(data) {
      const c = conn
      if (!c?.alive) return
      later(() => c.alive && getRoom().receive(c, data))
    },
    onOpen: (f) => void opens.push(f),
    onClose: (f) => void closes.push(f),
    onMessage: (f) => void messages.push(f),
  }
}
