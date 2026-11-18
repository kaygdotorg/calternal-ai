/**
 * How sync bytes travel. The sync client only needs "send bytes" and "tell
 * me when bytes, opens and closes happen": in the app that is a room on the
 * one live socket (`live.ts`); in unit tests, an in-memory room.
 */

export interface Transport {
  readonly connected: boolean
  connect(): void
  /** Disconnects and stops reconnecting until `connect` is called again. */
  disconnect(): void
  send(data: Uint8Array): void
  /** A public room/join error; the client distinguishes retryable failures. */
  onError?(fn: (code: string) => void): void
  onOpen(fn: () => void): void
  onClose(fn: (code: number, reason: string) => void): void
  onMessage(fn: (data: Uint8Array) => void): void
}
