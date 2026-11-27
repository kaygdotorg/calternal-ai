/** Worker executable for the primitives-only operations pipe. @module */
import { Console } from "node:console";
import { isAbsolute } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { newlineFrames } from "./newline-frames.js";
import { operationCodes, type Change, type Operation, type OperationFailure, type OperationsContext } from "./operation-api.js";

const maximum = 64 * 1024 * 1024;
type Input = Record<string, unknown>;
type Write = (line: string) => Promise<void>;
type Send = (call: string, input: Input) => Promise<unknown>;

interface Invoke {
  v: 1;
  invocation: string;
  type: "invoke";
  selector: string;
  input: unknown;
  me: OperationsContext["me"];
  space: OperationsContext["space"];
  event: OperationsContext["event"];
}

function object(value: unknown): value is Input {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function failure(value: unknown): OperationFailure {
  if (object(value) && operationCodes.some(code => code === value.code)
    && typeof value.message === "string" && typeof value.next === "string") {
    return {
      code: value.code as OperationFailure["code"],
      message: value.message.slice(0, 512),
      next: value.next.slice(0, 512),
    };
  }
  return { code: "invalid", message: "The plugin could not complete the operation.", next: "Check the input or update the plugin." };
}

function frame(value: unknown): string {
  const text = JSON.stringify(value);
  if (Buffer.byteLength(text) > maximum) {
    throw { code: "too-large", message: "The plugin frame is too large.", next: "Reduce the operation input or result." };
  }
  return text + "\n";
}

function context(invoke: Invoke, send: Send): OperationsContext {
  function request<T>(call: string, input: Input): Promise<T> {
    return send(call, input) as Promise<T>;
  }
  function changeInput(change: Change): Input {
    const entries = Object.entries(change);
    const entry = entries[0];
    if (entries.length !== 1 || !object(entry[1]) || Object.hasOwn(entry[1], "op")) {
      throw { code: "invalid", message: "A write entry must contain one change.", next: "Fix the change list." };
    }
    const [op, data] = entry;
    return { op, ...data };
  }

  return {
    query: query => request("s.query", query),
    read: item => request("s.read", { item }),
    write: async (changes, options) => request("s.write", { changes: changes.map(changeInput), preview: options?.preview }),
    state: {
      get: async (item, key) => (await request<{ value: unknown }>("s.state", { op: "get", item, key })).value,
      set: async (item, key, value) => (await request<{ value: unknown }>("s.state", { op: "set", item, key, value })).value,
    },
    call: (operation, input) => request("s.call", { operation, input: input ?? null }),
    run: async (program, options) => {
      const { id, status } = await request<{ id: string; status: string }>("s.run", { program, scope: options.scope ?? "session", targets: options.targets });
      return {
        id, status,
        async send(input) { await send("s.run.send", { id, input: input ?? null }); },
        async stop() { await send("s.run.stop", { id }); },
      };
    },
    me: Object.freeze(invoke.me),
    space: Object.freeze(invoke.space),
    event: Object.freeze(invoke.event),
  };
}

class BrokenPipe extends Error {}

async function invokeOperation(
  invoke: Invoke,
  handler: Operation<unknown>["handler"] | undefined,
  read: () => Promise<unknown>,
  write: Write,
): Promise<void> {
  const envelope = { v: 1, invocation: invoke.invocation };
  let closed = false;
  let id = 0;
  let tail = Promise.resolve();
  let broken: BrokenPipe | undefined;

  const send: Send = (call, input) => {
    if (closed) return Promise.reject({ code: "stopped", message: "This event ended.", next: "Start a new operation." });
    const result = tail.then(async () => {
      if (broken) throw broken;
      const nextId = id + 1;
      const line = frame({ ...envelope, type: "call", id: nextId, call, input });
      let reply;
      try {
        await write(line);
        id = nextId;
        reply = await read();
      } catch {
        throw broken = new BrokenPipe();
      }
      if (!object(reply) || reply.v !== 1 || reply.type !== "reply" || reply.invocation !== invoke.invocation
        || reply.id !== id || Object.hasOwn(reply, "result") === Object.hasOwn(reply, "error")) {
        throw broken = new BrokenPipe();
      }
      if (Object.hasOwn(reply, "error")) throw failure(reply.error);
      return reply.result;
    });
    tail = result.then(() => {}, () => {});
    return result;
  };

  let outcome: { result: unknown } | { error: OperationFailure };
  try {
    if (!handler) throw { code: "not-allowed", message: "The plugin has no matching handler.", next: "Check the installed plugin." };
    const result = await handler(invoke.input, context(invoke, send));
    outcome = { result: invoke.event.kind === "change" ? null : result ?? null };
  } catch (error) {
    outcome = { error: failure(error) };
  }
  closed = true;
  await tail;
  if (broken) throw broken;
  const complete = { ...envelope, type: "complete" };
  let line: string;
  try {
    line = frame({ ...complete, ...outcome });
  } catch (error) {
    line = frame({ ...complete, error: failure(error) });
  }
  await write(line);
}

/** Run one installed module over the host pipe. Parses one frame per line and
 * keeps one invocation and one in-flight callback. Malformed host frames
 * terminate the worker. The launcher supplies isolation and owns the deadline.
 */
export async function serveOperations(
  exports: Record<string, unknown>,
  input: AsyncIterable<Uint8Array>,
  write: Write,
): Promise<void> {
  const byEvent = new Map<string, Operation<unknown>["handler"]>();
  for (const value of Object.values(exports)) {
    if (!object(value) || (value.kind !== "call" && value.kind !== "change")
      || typeof value.selector !== "string" || typeof value.handler !== "function") continue;
    const key = `${value.kind}:${value.selector}`;
    if (byEvent.has(key)) throw new Error("duplicate operation registration");
    byEvent.set(key, value.handler as Operation<unknown>["handler"]);
  }
  const frames = newlineFrames(input, maximum);
  const decoder = new TextDecoder("utf-8", { fatal: true });
  const read = async () => {
    const line = await frames.next();
    return line.done ? undefined : JSON.parse(decoder.decode(line.value)) as unknown;
  };
  for (let value = await read(); value !== undefined; value = await read()) {
    if (!object(value) || value.v !== 1 || value.type !== "invoke" || typeof value.invocation !== "string"
      || typeof value.selector !== "string" || !Object.hasOwn(value, "input") || !object(value.event)
      || (value.event.kind !== "call" && value.event.kind !== "change")
      || !object(value.me) || !object(value.space)) throw new BrokenPipe();
    const invoke = value as unknown as Invoke;
    await invokeOperation(invoke, byEvent.get(`${invoke.event.kind}:${invoke.selector}`), read, write);
  }
}

async function main() {
  const entry = process.argv[2];
  if (process.argv.length !== 3 || !isAbsolute(entry)) throw new Error("expected absolute operations.ts path");
  globalThis.console = new Console(process.stderr);
  const operations = await import(pathToFileURL(entry).href) as Record<string, unknown>;
  await serveOperations(operations, process.stdin, line => new Promise<void>((resolve, reject) => {
    process.stdout.write(line, error => error ? reject(error) : resolve());
  }));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch(() => { process.exit(1); });
}
