/** Fixture host for exercising a plugin's actual worker wire protocol. */
import { spawnSync } from "node:child_process";
import { encodeWorkerFrameV2, parseWorkerFrameV2, type WorkerFrameV2 } from "./worker-v2.js";
import type { HostCallKind } from "./manifest-v1.js";

type Child = {
  stdin: { write(line: string): unknown; end(): void; on(event: "error", listener: (error: Error) => void): unknown };
  stdout: AsyncIterable<Uint8Array>;
  stderr: AsyncIterable<Uint8Array> | null;
  once(event: "close", listener: (code: number | null, signal: string | null) => void): unknown;
  once(event: "error", listener: (error: Error) => void): unknown;
  kill(): unknown;
};


function validateQuestion(data: unknown): void {
  const bin = process.env.CALTERNAL_PLUGIN_TEST_BIN;
  if (!bin || !data || typeof data !== "object" || !("status" in data) || data.status !== "parked"
    || !("wait" in data) || !data.wait || typeof data.wait !== "object" || !("source" in data.wait)) return;
  const source = data.wait.source;
  if (!source || typeof source !== "object" || !("kind" in source) || source.kind !== "question") return;
  const check = spawnSync(bin, ["plugin", "check-question", "--json"], {
    input: "form" in source ? JSON.stringify(source.form) : "", encoding: "utf8",
    timeout: 2_000, maxBuffer: 64 * 1024,
  });
  if (check.error) throw check.error;
  if (check.status !== 0) {
    const error = JSON.parse(check.stdout).error;
    throw new Error(error.message);
  }
}

/** Exercise operations using a disposable worker and local callback fixtures.
 * Runs one invocation at a time. Only the worker receives the dummy authority.
 * Unhandled callbacks fail closed. The deadline bounds fixture waits.
 * A child that exits before answering reports its exit status or signal and
 * the last 4 KiB of stderr. Spawn with piped stdin, stdout and stderr.
 * Under `calternal plugin test`, each parked question uses the host validator.
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
  let failure: Error | undefined;
  let spawnError: Error | undefined;
  const fail = (error: Error) => { failure ??= error; waiting?.fail(failure); };
  const exited = new Promise<string>(resolve => {
    child.once("error", error => { spawnError = error; });
    child.once("close", (code, signal) => resolve(
      spawnError ? `could not start: ${spawnError.message}` : signal ? `signal ${signal}` : `exit code ${code}`,
    ));
  });
  // Closing a crashed child's pipe may report EPIPE before its stderr arrives.
  // The close event supplies the useful exit reason.
  child.stdin.on("error", () => {});
  const diagnostic = (async () => {
    const limit = 4096;
    let tail: Uint8Array = new Uint8Array();
    try {
      if (child.stderr) {
        for await (const chunk of child.stderr) {
          const latest = chunk.subarray(-limit);
          const before = tail.subarray(Math.max(0, tail.length + latest.length - limit));
          tail = new Uint8Array(before.length + latest.length);
          tail.set(before);
          tail.set(latest, before.length);
        }
      }
    } catch { /* Preserve stderr received before the pipe closed. */ }
    return new TextDecoder().decode(tail).trim();
  })();
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
        if (message.type === "complete") {
          try {
            validateQuestion(message.data);
            waiting.success(message.data);
          } catch (error) {
            waiting.fail(error instanceof Error ? error : new Error("question validation failed"));
          }
        }
        else if (message.type === "host_call") {
          const fixture = fixtures[message.method];
          send({
            protocol_version: 2, invocation_id,
            message: { type: "host_reply", call_id: message.call_id, ok: Boolean(fixture),
              data: fixture ? (await fixture(message.input) ?? null) : null },
          });
        } else if (message.type === "failed") {
          waiting.fail(new Error("fixture operation did not complete: the worker reported a handler failure"));
        } else throw new Error("unexpected fixture worker message");
      }
      if (tail.length > 256 * 1024) throw new Error("oversized fixture reply");
    }
    const [reason, stderr] = await Promise.all([exited, diagnostic]);
    throw new Error(`fixture worker closed (${reason})${stderr ? `\nWorker stderr:\n${stderr}` : ""}`);
  };
  const done = collect().catch(error => { fail(error instanceof Error ? error : new Error("fixture worker failed")); });
  return {
    async invoke(operation, input) {
      if (waiting) throw new Error("fixture already has an active invocation");
      if (failure) throw failure;
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
