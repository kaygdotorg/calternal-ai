/** Bounded newline transport for worker-v2 subprocesses and fixture streams. */
import { WorkerRuntimeV2, MAX_V2_FRAME_BYTES, encodeWorkerFrameV2, parseWorkerFrameV2, type WorkerHandlerV2 } from "./worker-v2.js";

import { newlineFrames } from "./newline-frames.js";

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
  let first = true;
  const decoder = new TextDecoder("utf-8", { fatal: true });
  for await (const frame of newlineFrames(input, MAX_V2_FRAME_BYTES)) {
    if (first && frame.length <= 128) {
      const line = decoder.decode(frame);
      if (/^\{"type":"ready","protocol_version":2,"nonce":"[0-9a-f-]{36}"\}$/.test(line)) {
        runtime.assertReady();
        first = false;
        await write(line + "\n");
        continue;
      }
    }
    first = false;
    await runtime.receive(parseWorkerFrameV2(frame));
  }
}
