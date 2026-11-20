import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { AlarmDelivery, AlarmSet, ExecutionJobContext, ExecutionJobStart, ExecutionStatus,
  ExecutionStopRequest, ExecutionStopResult, HostWorkerInvokeFrame, ItemViewPublish } from "../generated/wire.js";
import type { ManifestV2 } from "../generated/author.js";
import type { Context, RunInput } from "../src/interface.ts";
import type { HarnessInput, HarnessOutput } from "../src/harnesses-v1.ts";
import type { handlers as threadHandlers } from "../src/worker.ts";
import type { handlers as harnessHandlers } from "../../calternal-threads-claude-code/src/worker.ts";

async function load<H>(directory: URL) {
  const manifest: ManifestV2 = JSON.parse(readFileSync(new URL("manifest.json", directory), "utf8"));
  assert.ok(manifest.worker);
  const { handlers } = await import(new URL(manifest.worker.path, directory).href) as { handlers: H };
  return { manifest, handlers };
}
const application = await load<typeof threadHandlers>(new URL("../dist/", import.meta.url));
const contribution = await load<typeof harnessHandlers>(new URL("../../calternal-threads-claude-code/dist/", import.meta.url));
export const handlers = application.handlers;

const exchanges = {
  start_job: ["ExecutionJobStart", "ExecutionJobReceipt"],
  execution_start: ["ExecutionStart", "ExecutionStatus"],
  item_append: ["ItemAppend", "ItemAppendReceipt"],
  item_view_context: ["ItemViewContextRequest", "ItemViewContext"],
  item_view_publish: ["ItemViewPublish", "ItemViewPublishReceipt"],
  alarm_set: ["AlarmSet", "AlarmReceipt"],
} as const;
const trace: [name: string, value: unknown][] = [];

/** Check the fake's revision 14.30 exchanges against the pinned schemas. */
export function checkTrace() {
  const check = fileURLToPath(new URL("../check-shapes.py", import.meta.url));
  const contract = fileURLToPath(new URL("../.build/contract/", import.meta.url));
  execFileSync("python3", ["-B", check, contract], { input: JSON.stringify(trace), stdio: ["pipe", "inherit", "inherit"] });
}

type File = { owner: "threads" | "claude-code-run"; history: Map<string, Buffer>; revision: string; path?: string; archived: boolean; title: string };
type Job = {
  context: ExecutionJobContext;
  input: RunInput;
  state: "pending" | "running" | "done" | "stopped";
  attempt?: ExecutionStatus;
  teardown: boolean;
  result?: HarnessOutput;
};
type Owner = File["owner"];
const actor = "019a0000-0000-7000-8000-000000000001";
const now = "2026-11-20T12:00:00Z";

/** One in-memory space. Validates package ownership, exact revisions and
 * attempt retirement around the calls used by the spike. */
export class FakeHost {
  files = new Map<string, File>();
  jobs = new Map<string, Job>();
  snapshots = new Map<string, ItemViewPublish>();
  alarms = new Map<string, AlarmSet>();
  receipts = new Map<string, { fingerprint: string; result: unknown }>();
  private scopes = new Map<string, { item: string; revision: string }>();
  private serial = 0;
  private selected = { harness: "checked-harness", placement: "checked-desk" };
  loseNextAppendAck = false;
  claude = async (text: string) => `Reply to ${text}`;

  choose() {
    const app = application.manifest.apps[0];
    const point = app.extension_points![0];
    const selected = contribution.manifest.contributions![0];
    assert.equal(selected.app, app.id);
    assert.equal(selected.point, point.id);
    assert.equal(selected.version, point.version);
    assert.equal(selected.operation, "threads-claude-code.run");
    const operation = contribution.manifest.operations.find(op => op.id === selected.operation);
    assert.deepEqual(operation?.input_schema, point.input_schema);
    assert.deepEqual(operation?.output_schema, point.output_schema);
    trace.push(
      ["RendererExtensionChoiceRequest", { type: "extension_choices", app: app.id, point: point.id, activation_context: "ctx", slot: "harness" }],
      ["ExtensionChoicePage", { options: [{ ref: "option", label: selected.harness!.name, harness_ref: this.selected.harness }] }],
      ["ChoiceOptionsRequest", { type: "choice_options", activation_context: "ctx", slot: "placement", harness: this.selected.harness, activation_id: "send" }],
      ["ChoiceOptions", { host: "execution_placement", options: [{ ref: this.selected.placement, label: "Desk" }], default_ref: this.selected.placement }],
    );
    return { ...this.selected };
  }

  private id(prefix: string) { return `${prefix}-${++this.serial}`; }

  private receipt(key: string, input: unknown, effect: () => unknown) {
    const fingerprint = JSON.stringify(input);
    const saved = this.receipts.get(key);
    if (saved) {
      assert.equal(saved.fingerprint, fingerprint, "conflicting replay");
      return saved.result;
    }
    const result = effect();
    this.receipts.set(key, { fingerprint, result });
    return result;
  }

  private admit(payload: ExecutionJobStart) {
    assert.equal(payload.harness, this.selected.harness, "unchecked harness");
    if (payload.placement !== undefined) assert.equal(payload.placement, this.selected.placement, "unchecked placement");
    const file = this.files.get(payload.item);
    assert.ok(file?.owner === "threads" && !file.archived, "target unavailable");
    const active = [...this.jobs.values()].filter(job => job.context.item === payload.item && ["pending", "running"].includes(job.state));
    assert.ok(active.length < 2, "pending_limit");
    const intent = this.id("job");
    this.jobs.set(intent, { context: { intent, item: payload.item, harness: payload.harness }, input: payload.input as RunInput, state: "pending", teardown: true });
    return { job_id: intent };
  }

  context(request: string, operation: string, job?: Job, alarm?: AlarmDelivery, owner: Owner = "threads"): Context {
    const declared = (owner === "threads" ? application : contribution).manifest.operations.find(op => op.id === operation);
    assert.ok(declared);
    const call: Context["call"] = async (method, input) => {
      assert.ok((declared.methods as readonly string[]).includes(method), "undeclared method");
      const wire = exchanges[method as keyof typeof exchanges];
      if (wire) trace.push([wire[0], input]);
      const result = await this.call(owner, request, method, input, job);
      if (wire) trace.push([wire[1], result]);
      return result as never;
    };
    return { request, actor, now, job: job?.context, alarm, call, ...(job && {
      harnessResult: async (execution: string, input: HarnessInput) => {
        assert.equal(job.attempt?.execution, execution);
        assert.equal(job.state, "running");
        if (job.result) return job.result;
        const ctx = this.context(request, "threads-claude-code.run", job, undefined, "claude-code-run");
        const result = await contribution.handlers["threads-claude-code.run"](input, {
          ...ctx, claude: async text => {
            const reply = await this.claude(text);
            assert.equal(job.state, "running", "attempt retired");
            return reply;
          },
        });
        assert.equal(job.state, "running", "attempt retired");
        job.result = result;
        return result;
      },
    }) };
  }

  // This dispatch is the only place that unpacks fixture broker method operands.
  private async call(owner: Owner, request: string, method: Parameters<Context["call"]>[0], input: unknown, job?: Job): Promise<unknown> {
    if (job) assert.equal(job.state, "running", "attempt retired");
    const args = input as Record<string, string>;
    const file = this.files.get(args.item);
    const writable = () => {
      assert.ok(file?.owner === owner && !file.archived, "target unavailable");
      return file;
    };
    const requestId = args.request_id ?? request;
    const commit = (effect: () => unknown) => this.receipt(`${owner}:${method}:${requestId}`, input, effect);
    switch (method) {
      case "item_save": return commit(() => {
        const revision = this.id("rev");
        const body = Buffer.from(args.bytes, "base64");
        const { title } = (input as { metadata: { title: string } }).metadata;
        if (!args.item) {
          assert.equal(args.kind, owner);
          const item = this.id("item");
          let path = args.path;
          if (path) {
            const parts = path.split("/");
            const used = new Set([...this.files.values()].map(file => file.path));
            for (let suffix = 2; used.has(path); suffix++) {
              path = `${parts[0]}/${parts[1]} (${suffix})/${parts[2]}`;
            }
          }
          this.files.set(item, { owner, revision, history: new Map([[revision, body]]), path, archived: false, title });
          return { item, revision };
        }
        const target = writable();
        assert.equal(args.expected_revision, target.revision, "stale head");
        target.history.set(revision, body);
        target.revision = revision;
        target.title = title;
        return { item: args.item, revision };
      });
      case "item_head": {
        assert.equal(file?.owner, owner);
        return { item: args.item, revision: file.revision };
      }
      case "item_range": {
        assert.equal(file?.owner, owner);
        const body = file.history.get(args.revision);
        assert.ok(body, "obsolete range");
        const { offset, max_bytes } = input as { offset: number; max_bytes: number };
        assert.ok(offset >= 0 && max_bytes <= 65536);
        const page = body.subarray(offset, offset + max_bytes);
        const next = offset + page.length;
        return { bytes: page.toString("base64"), ...(next < body.length && { next_offset: next }) };
      }
      case "start_job": return commit(() => this.admit(input as ExecutionJobStart));
      case "execution_start": return commit(() => {
        assert.ok(job?.state === "running");
        assert.equal(args.harness, job.context.harness);
        job.teardown = false;
        job.attempt = { execution: this.id("attempt"), phase: "running", transition_cursor: this.id("transition") };
        return job.attempt;
      });
      case "item_append": {
        const receipt = commit(() => {
          const target = writable();
          assert.equal(args.expected_revision, target.revision, "stale head");
          const revision = this.id("rev");
          const prior = target.history.get(target.revision)!;
          const addition = Buffer.from(args.bytes, "base64");
          assert.ok(addition.length <= 65536);
          const body = Buffer.concat([prior, addition]);
          target.history.set(revision, body);
          target.revision = revision;
          return { revision, end_offset: body.length, receipt: requestId };
        });
        if (owner === "threads" && this.loseNextAppendAck) {
          this.loseNextAppendAck = false;
          throw new Error("Append acknowledgement lost");
        }
        return receipt;
      }
      case "item_view_context": {
        const target = writable();
        const audience_scope = this.id("scope");
        this.scopes.set(audience_scope, { item: args.item, revision: target.revision });
        return { audience_scope, source_revision: target.revision };
      }
      case "item_view_publish": {
        const target = writable();
        assert.equal(args.source_revision, target.revision, "stale snapshot");
        assert.deepEqual(this.scopes.get(args.audience_scope), { item: args.item, revision: target.revision });
        this.snapshots.set(args.item, input as ItemViewPublish);
        return { source_revision: target.revision, receipt: requestId };
      }
      case "alarm_set": return commit(() => {
        assert.equal(args.harness, this.selected.harness);
        const alarm = this.id("alarm");
        this.alarms.set(alarm, input as AlarmSet);
        return { alarm, receipt: requestId };
      });
    }
  }

  async run(id: string, alarm?: AlarmDelivery) {
    const job = this.jobs.get(id)!;
    const head = [...this.jobs.values()].find(candidate => candidate.context.item === job.context.item && ["pending", "running"].includes(candidate.state));
    assert.equal(head, job, "successor must wait for the head");
    assert.equal(job.state, "pending");
    assert.ok([...this.jobs.values()].every(candidate =>
      candidate.context.item !== job.context.item || candidate.teardown), "successor must wait for teardown");
    const frame: HostWorkerInvokeFrame = {
      v: 1, epoch: "fake", seq: ++this.serial, package: application.manifest.id, digest: "0".repeat(64), ticket: id,
      message: { type: "invoke", operation: "threads.run", input: job.input, deadline: now,
        job_context: job.context, ...(alarm && { alarm }) },
    };
    trace.push(["HostWorkerInvokeFrame", frame]);
    job.state = "running";
    try {
      return await handlers["threads.run"](job.input, this.context(id, "threads.run", job, alarm));
    } finally {
      if (this.jobs.get(id)!.state === "running") {
        job.state = "done";
        job.teardown = true;
      }
    }
  }

  async replay(id: string) {
    const job = this.jobs.get(id)!;
    assert.equal(job.state, "done");
    job.state = "pending";
    await this.run(id);
  }

  stop(item: string, target: { execution: string } | { intent: string }) {
    const input: ExecutionStopRequest = { request_id: this.id("stop"), ...target };
    trace.push(["ExecutionStopRequest", input]);
    const job = "intent" in target ? this.jobs.get(target.intent) :
      [...this.jobs.values()].find(candidate => candidate.attempt?.execution === target.execution);
    assert.ok(job?.context.item === item && ["pending", "running"].includes(job.state), "stale Stop");
    job.state = "stopped";
    const result: ExecutionStopResult = job.attempt
      ? { ...job.attempt, phase: "stopped", transition_cursor: this.id("transition") }
      : { intent: job.context.intent, phase: "stopped" };
    trace.push(["ExecutionStopResult", result]);
  }

  settle(id: string) {
    const job = this.jobs.get(id)!;
    assert.equal(job.state, "stopped");
    job.teardown = true;
  }

  archive(item: string) {
    const file = this.files.get(item)!;
    for (const job of this.jobs.values()) {
      if (job.context.item === item && ["pending", "running"].includes(job.state)) job.state = "stopped";
    }
    file.archived = true;
  }

  deliver(alarm: string, actual_at: string) {
    const armed = this.alarms.get(alarm);
    if (!armed) return;
    const { job, at, input, harness } = armed;
    assert.ok(harness);
    const delivery = this.admit({ job, item: (input as RunInput).item, input, harness });
    this.alarms.delete(alarm);
    return this.run(delivery.job_id, { alarm, scheduled_at: at, actual_at });
  }

  markdown(item: string) {
    const file = this.files.get(item)!;
    return file.history.get(file.revision)!.toString();
  }
}
