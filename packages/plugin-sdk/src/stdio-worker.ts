/** Bounded newline transport for worker-v2 subprocesses and fixture streams. */
import { WorkerRuntimeV2, MAX_V2_FRAME_BYTES, encodeWorkerFrameV2, parseWorkerFrameV2, type WorkerHandlerV2 } from "./worker-v2.js";

/** Serve one isolated worker until its input closes. Keep at most one frame of buffered input.
 * Cost: one decode per line, one serialized result per send, one reusable bounded buffer.
 * Rejects invalid, oversized or truncated frames. The caller controls the process and transport.
 */
export async function serveWorker(
  manifest: unknown,
  handlers: Record<string, WorkerHandlerV2>,
  input: AsyncIterable<Uint8Array>,
  write: (line: string) => Promise<void>,
): Promise<void> {
  const runtime = new WorkerRuntimeV2(manifest, handlers, frame => write(encodeWorkerFrameV2(frame)));
  const buffer = new Uint8Array(MAX_V2_FRAME_BYTES);
  let used = 0;
  let first = true;
  const decoder = new TextDecoder("utf-8", { fatal: true });
  for await (const chunk of input) {
    let start = 0;
    while (start < chunk.length) {
      const newline = chunk.indexOf(10, start);
      const end = newline < 0 ? chunk.length : newline;
      const part = chunk.subarray(start, end);
      if (used + part.length > buffer.length) throw new Error("worker frame exceeds its budget");
      buffer.set(part, used);
      used += part.length;
      if (newline < 0) break;
      const frame = buffer.subarray(0, used);
      if (first && used <= 128) {
        const line = decoder.decode(frame);
        if (/^\{"type":"ready","protocol_version":2,"nonce":"[0-9a-f-]{36}"\}$/.test(line)) {
          runtime.assertReady();
          first = false;
          used = 0;
          await write(line + "\n");
          start = end + 1;
          continue;
        }
      }
      first = false;
      await runtime.receive(parseWorkerFrameV2(frame));
      used = 0;
      start = end + 1;
    }
  }
  if (used !== 0) throw new Error("unterminated worker frame");
}
