/**
 * A reader for server-sent events over `fetch` (EventSource cannot send an Authorization header, and the development bearer needs one). It turns a response
 * body into events `{ event, id, data }` and stops when the body ends, the signal aborts or the consumer returns. Cost: one decoder and one buffer per stream.
 */
export interface SseEvent { event: string; id?: string; data: string }

/** Parses one block of lines (up to a blank line) into an event; null for a comment-only block. */
export function parseBlock(block: string): SseEvent | null {
  let event = "message", id: string | undefined;
  const data: string[] = [];
  for (const line of block.split("\n")) {
    if (!line || line.startsWith(":")) continue;
    const i = line.indexOf(":");
    const field = i < 0 ? line : line.slice(0, i);
    const value = i < 0 ? "" : line.slice(i + 1).replace(/^ /, "");
    if (field === "event") event = value;
    else if (field === "id") id = value;
    else if (field === "data") data.push(value);
  }
  return data.length || event !== "message" ? { event, id, data: data.join("\n") } : null;
}

/** Yields the events of a streaming response body. */
export async function* events(body: ReadableStream<Uint8Array>): AsyncGenerator<SseEvent> {
  const reader = body.getReader();
  const dec = new TextDecoder();
  let buf = "";
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true }).replace(/\r\n/g, "\n");
      for (let at = buf.indexOf("\n\n"); at >= 0; at = buf.indexOf("\n\n")) {
        const ev = parseBlock(buf.slice(0, at));
        buf = buf.slice(at + 2);
        if (ev) yield ev;
      }
    }
    const last = parseBlock(buf);
    if (last) yield last;
  } finally {
    reader.releaseLock();
  }
}
