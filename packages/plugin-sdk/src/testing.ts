/** Fixture host for exercising a plugin's actual worker wire protocol. */
import { encodeWorkerFrameV2, parseWorkerFrameV2, type WorkerFrameV2 } from "./worker-v2.js";
import type { HostCallKind } from "./manifest-v1.js";

type Child = {
  stdin: { write(line: string): unknown; end(): void };
  stdout: AsyncIterable<Uint8Array>;
  kill(): unknown;
};

/** Exercise operations using a disposable worker and local callback fixtures.
 * Runs one invocation at a time. Only the worker receives the dummy authority.
 * Unhandled callbacks fail closed. The deadline bounds fixture waits.
 */
export function fixtureWorker(
  child: Child,
  fixtures: Partial<Record<HostCallKind, (input: unknown) => unknown>> = {},
): {
  /** Send one invocation and await the worker's result. */
  invoke(operation: string, input: unknown): Promise<unknown>;
  /** Dispose the disposable subprocess and release its input stream. */
  close(): Promise<void>;
} {
  let next = 0;
  let waiting: { id: string; success(value: unknown): void; fail(error: Error): void } | undefined;
  const send = (frame: WorkerFrameV2): void => { child.stdin.write(encodeWorkerFrameV2(frame)); };
  const collect = async () => {
    // Decode only complete lines, preserving multibyte characters across chunks.
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let tail = "";
    for await (const chunk of child.stdout) {
      tail += decoder.decode(chunk, { stream: true });
      let newline: number;
      while ((newline = tail.indexOf("\n")) >= 0) {
        const { invocation_id, message } = parseWorkerFrameV2(tail.slice(0, newline));
        tail = tail.slice(newline + 1);
        if (!waiting || waiting.id !== invocation_id) throw new Error("unexpected fixture reply");
        if (message.type === "complete") waiting.success(message.data);
        else if (message.type === "host_call") {
          const fixture = fixtures[message.method];
          send({
            protocol_version: 2, invocation_id,
            message: { type: "host_reply", call_id: message.call_id, ok: Boolean(fixture),
              data: fixture ? (await fixture(message.input) ?? null) : null },
          });
        } else throw new Error("fixture operation did not complete");
      }
      if (tail.length > 256 * 1024) throw new Error("oversized fixture reply");
    }
    throw new Error("fixture worker closed");
  };
  const done = collect().catch(error => { waiting?.fail(error); });
  return {
    async invoke(operation, input) {
      if (waiting) throw new Error("fixture already has an active invocation");
      const id = `fixture-${next++}`;
      let deadline: ReturnType<typeof setTimeout> | undefined;
      try {
        return await new Promise<unknown>((success, fail) => {
          waiting = { id, success, fail };
          deadline = setTimeout(() => fail(new Error("fixture operation timed out")), 3_000);
          send({
            protocol_version: 2, invocation_id: id,
            message: { type: "invoke", operation, input, authority: {
              account_id: "01900000-0000-7000-8000-000000000001",
              space_id: "01900000-0000-7000-8000-000000000002",
              grant: "fixture",
            } },
          });
        });
      } finally {
        clearTimeout(deadline);
        waiting = undefined;
      }
    },
    async close() {
      child.stdin.end();
      child.kill();
      await done;
    },
  };
}
