/** Bounded worker-v2 peer; effects and current authority remain in the trusted host.
 * @module
 */
import type { ManifestV1 } from "@calternal/contracts";
import { validateManifestV1, type HostCallKind } from "./manifest-v1.js";

/** Maximum serialized frame bytes, excluding newline. */
export const MAX_V2_FRAME_BYTES = 256 * 1024;
/** Bounded concurrent handlers and pending callbacks. */
export const MAX_V2_PENDING = 32;
/** Invocation authority issued by the trusted host; never a grant by itself. */
export interface InvocationAuthority {
  /** Acting account UUIDv7. */
  account_id: string;
  /** Independently allocated space UUIDv7. */
  space_id: string;
  /** Opaque reference to host-owned current session/grant attribution. */
  grant: string;
}
/** Strict bidirectional worker-v2 messages. */
export type WorkerMessageV2 =
  | {
    /** Host operation dispatch. */
    type: "invoke";
    /** Declared operation. */
    operation: string;
    /** Schema-checked input. */
    input: unknown;
    /** Host authority. */
    authority: InvocationAuthority
  }
  | {
    /** Worker callback. */
    type: "host_call";
    /** Callback ID. */
    call_id: string;
    /** Declared method. */
    method: HostCallKind;
    /** Broker input. */
    input: unknown;
    /** Echoed authority. */
    authority: InvocationAuthority
  }
  | {
    /** Callback result. */
    type: "host_reply";
    /** Callback ID. */
    call_id: string;
    /** Success marker. */
    ok: boolean;
    /** JSON result or null. */
    data: unknown
  }
  | {
    /** Job output. */
    type: "chunk";
    /** Host-issued job handle. */
    job_id: string;
    /** Zero-based sequence per invocation. */
    sequence: number;
    /** Chunk data. */
    data: unknown
  }
  | {
    /** Successful completion. */
    type: "complete";
    /** Operation output. */
    data: unknown
  }
  | {
    /** Safe failure. */
    type: "failed"
  }
  | {
    /** Host cancellation. */
    type: "cancel"
  };
/** Newline-delimited JSON frame. Transport adapters must cap buffering before decoding. */
export interface WorkerFrameV2 {
  /** Exactly 2. */
  protocol_version: 2;
  /** Host-issued invocation ID. */
  invocation_id: string;
  /** Direction-specific message. */
  message: WorkerMessageV2;
}
function invalid(): never { throw new Error("invalid worker v2 frame"); }
const id = /^[a-z0-9][a-z0-9._-]{0,79}$/;
const uuid7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
function exact(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return invalid();
  const r = value as Record<string, unknown>;
  if (Object.keys(r).length !== keys.length || keys.some(k => !Object.hasOwn(r, k))) invalid();
  return r;
}
/** Parse a bounded strict UTF-8 JSON frame and reject unknown versions, fields and variants.
 * Cost: O(frame bytes), allocates decoded fields; no I/O.
 * @returns A validated frame; authorization is the host broker's responsibility.
 */
export function parseWorkerFrameV2(input: Uint8Array | string): WorkerFrameV2 {
  const bytes = typeof input === "string" ? new TextEncoder().encode(input) : input;
  const body = bytes[bytes.length - 1] === 10 ? bytes.subarray(0, -1) : bytes;
  if (body.length > MAX_V2_FRAME_BYTES) invalid();
  return validateFrame(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body)));
}

function validateFrame(value: unknown): WorkerFrameV2 {
  const frame = exact(value, ["protocol_version", "invocation_id", "message"]);
  if (frame.protocol_version !== 2 || typeof frame.invocation_id !== "string" || !id.test(frame.invocation_id)) invalid();
  const raw = frame.message as Record<string, unknown> | null;
  const fields: Record<string, string[]> = { invoke: ["operation", "input", "authority"], host_call: ["call_id", "method", "input", "authority"], host_reply: ["call_id", "ok", "data"], chunk: ["job_id", "sequence", "data"], complete: ["data"], failed: [], cancel: [] };
  const type = raw && typeof raw.type === "string" ? raw.type : "";
  if (!Object.hasOwn(fields, type)) invalid();
  const m = exact(raw, ["type", ...fields[type]!]);
  for (const k of ["operation", "call_id", "job_id"]) if (Object.hasOwn(m, k) && (typeof m[k] !== "string" || !id.test(m[k] as string))) invalid();
  if (Object.hasOwn(m, "authority")) { const a = exact(m.authority, ["account_id", "space_id", "grant"]); if (typeof a.account_id !== "string" || !uuid7.test(a.account_id) || typeof a.space_id !== "string" || !uuid7.test(a.space_id) || a.account_id === a.space_id || typeof a.grant !== "string" || !a.grant || new TextEncoder().encode(a.grant).length > 256) invalid(); }
  if (type === "host_call" && !["read_item", "write_item", "list_collection", "emit_event", "start_job", "checkpoint_job", "request_secret_handle"].includes(String(m.method))) invalid();
  if (type === "host_reply" && typeof m.ok !== "boolean") invalid();
  if (type === "chunk" && (typeof m.sequence !== "number" || !Number.isInteger(m.sequence) || m.sequence < 0 || m.sequence > 0xffff_ffff)) invalid();
  return frame as unknown as WorkerFrameV2;
}
/** Encode one strict frame including its newline, respecting the byte budget.
 * Cost: O(frame bytes), serialization and UTF-8 allocation, no I/O.
 * @returns JSON line suitable for a back-pressure-aware transport.
 */
export function encodeWorkerFrameV2(frame: WorkerFrameV2): string {
  validateFrame(frame);
  const text = JSON.stringify(frame);
  if (new TextEncoder().encode(text).byteLength > MAX_V2_FRAME_BYTES) invalid();
  return text + "\n";
}
/** Safe broker/stream context passed to a TypeScript operation handler. */
export interface WorkerContextV2 {
  /** Local cancellation signal; effects still require host reconciliation. */
  signal: AbortSignal;
  /** Cost: one bounded RPC round trip and pending promise; no direct I/O outside transport.
   * Call a declared broker method; the host rechecks live authority and targets. */
  call(method: HostCallKind, input: unknown): Promise<unknown>;
  /** Submit independent calls together. Every call retains its own live host authorization and reply.
   * Costs one existing wire round trip per call. Limited to MAX_V2_PENDING entries. */
  callAll(calls: ReadonlyArray<readonly [HostCallKind, unknown]>): Promise<unknown[]>;
  /** Cost: one bounded transport write, O(chunk bytes); waits for transport back pressure.
   * Emit one ordered chunk for a job admitted through start_job, or for the job a run
   * invocation names in `input.job_id`; await before emitting another. */
  chunk(jobId: string, data: unknown): Promise<void>;
}
/** Handler implementing one declared operation. Cost is plugin-defined and sandbox-limited; each host effect uses a bounded broker RPC. */
export type WorkerHandlerV2 = (input: unknown, context: WorkerContextV2) => Promise<unknown>;
/** Long-lived peer for one plugin/space pair. Transport send promises must apply back pressure.
 * The sandbox must deny ambient storage, network and credentials and enforce resource limits.
 */
export class WorkerRuntimeV2 {
  private active = new Map<string, { controller: AbortController; pending: Map<string, { resolve: (v: unknown) => void; reject: (e: Error) => void }> }>();
  private space: string | undefined;
  private seen = new Set<string>();
  private sending = 0;
  private manifest: ManifestV1;
  /** Cost: O(menu bytes), bounded state allocation, no I/O. Validate the menu and bind declared handlers to a bounded transport. */
  constructor(manifest: unknown, private handlers: Record<string, WorkerHandlerV2>, private send: (frame: WorkerFrameV2) => Promise<void>) {
    this.manifest = validateManifestV1(manifest);
  }
  private async emit(frame: WorkerFrameV2): Promise<void> {
    if (this.sending >= MAX_V2_PENDING) throw new Error("worker back pressure");
    this.sending++; try { await this.send(frame); } finally { this.sending--; }
  }
  /** Consume one host frame without waiting for handler completion, allowing cancellation and replies.
   * Cost: O(frame bytes), bounded maps; handlers run asynchronously and each effect uses a broker RPC.
   * @returns When admission/control processing has completed; handler results use the send callback.
   */
  async receive(frame: WorkerFrameV2): Promise<void> {
    validateFrame(frame);
    const id = frame.invocation_id; const m = frame.message;
    if (m.type === "cancel") { const state = this.active.get(id); if (!state) return; state.controller.abort(); for (const p of state.pending.values()) p.reject(new Error("cancelled")); this.active.delete(id); return; }
    if (m.type === "host_reply") { const pending = this.active.get(id)?.pending; const p = pending?.get(m.call_id); if (!p) invalid(); pending!.delete(m.call_id); if (m.ok) p.resolve(m.data); else p.reject(new Error("host call refused")); return; }
    if (m.type !== "invoke" || this.seen.has(id) || this.active.size >= MAX_V2_PENDING || this.seen.size >= 4096 || (this.space && this.space !== m.authority.space_id) || !this.manifest.operations.some(x => x.id === m.operation) || !Object.hasOwn(this.handlers, m.operation)) invalid();
    this.space = m.authority.space_id; this.seen.add(id);
    const state = { controller: new AbortController(), pending: new Map<string, { resolve: (v: unknown) => void; reject: (e: Error) => void }>() }; this.active.set(id, state);
    let calls = 0, sequence = 0, chunkPending = false;
    const jobs = new Set<string>();
    // A job's run invocation names its job in `input.job_id`; the host attached it.
    const input = m.input as { job_id?: unknown } | null;
    if (input && typeof input === "object" && typeof input.job_id === "string" && /^[a-z0-9][a-z0-9._-]{0,79}$/.test(input.job_id)) jobs.add(input.job_id);
    const ensure = (): void => { if (state.controller.signal.aborted) throw new Error("cancelled"); };
    const context: WorkerContextV2 = {
      signal: state.controller.signal,
      call: async (method, input) => {
        ensure();
        if (method === "write_item" && !this.manifest.operations.some(op => op.id === m.operation && op.mutates)) throw new Error("host call refused");
        if (method === "request_secret_handle" && (!input || typeof input !== "object" || Array.isArray(input) || Object.keys(input).length !== 1 || !("name" in input) || typeof input.name !== "string" || !this.manifest.secrets.includes(input.name))) throw new Error("host call refused");
        if (!this.manifest.host_calls.includes(method) || state.pending.size >= MAX_V2_PENDING || calls >= 128) throw new Error("host call refused");
        const callId = `call-${calls++}`;
        const result = await new Promise<unknown>((resolve, reject) => { state.pending.set(callId, { resolve, reject }); void this.emit({ protocol_version: 2, invocation_id: id, message: { type: "host_call", call_id: callId, method, input, authority: m.authority } }).catch(error => { state.pending.delete(callId); reject(error); }); });
        if (method === "start_job" && result && typeof result === "object" && "job_id" in result && typeof result.job_id === "string" && /^[a-z0-9][a-z0-9._-]{0,79}$/.test(result.job_id)) jobs.add(result.job_id);
        return result;
      },
      callAll: async batch => {
        ensure();
        if (batch.length > MAX_V2_PENDING - state.pending.size || batch.length > 128 - calls) {
          throw new Error("host call batch is too large");
        }
        return await Promise.all(batch.map(([method, input]) => context.call(method, input)));
      },
      chunk: async (jobId, data) => {
        ensure(); if (chunkPending || !jobs.has(jobId)) throw new Error("chunk refused");
        chunkPending = true;
        try { await this.emit({ protocol_version: 2, invocation_id: id, message: { type: "chunk", job_id: jobId, sequence, data } }); sequence++; }
        finally { chunkPending = false; }
      },
    };
    void (async () => {
      try { const data = await this.handlers[m.operation]!(m.input, context); ensure(); await this.emit({ protocol_version: 2, invocation_id: id, message: { type: "complete", data } }); }
      catch { if (!state.controller.signal.aborted) await this.emit({ protocol_version: 2, invocation_id: id, message: { type: "failed" } }); }
      finally { for (const p of state.pending.values()) p.reject(new Error("invocation ended")); this.active.delete(id); }
    })().catch(() => { state.controller.abort(); this.active.delete(id); });
  }
}
