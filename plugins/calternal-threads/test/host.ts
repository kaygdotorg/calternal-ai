import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { AlarmDelivery, AlarmSet, ExecutionJobContext, ExecutionJobStart, ExecutionStatus,
  ExecutionStopRequest, ExecutionStopResult, HostWorkerInvokeFrame, ItemCreate, ItemSave, ItemViewPublish } from "../generated/wire.js";
import { encodeContributionFrame } from "../generated/contribution-frame.mjs";
import { authorContext } from "../generated/author-context.mjs";
import type { ManifestV2 } from "../generated/author.js";
import type { Context, RunInput } from "../src/interface.ts";
import type { HarnessInput, HarnessOutput } from "../src/harnesses-v1.ts";
import type { handlers as threadHandlers } from "../src/worker.ts";
import type { InvitedInput, PeopleContext, Scope, peopleHandlers } from "../src/people.ts";
import type { handlers as harnessHandlers } from "../../calternal-threads-claude-code/src/worker.ts";

async function load<H, E = object>(directory: URL) {
  const manifest: ManifestV2 = JSON.parse(readFileSync(new URL("manifest.json", directory), "utf8"));
  assert.ok(manifest.worker);
  const worker = await import(new URL(manifest.worker.path, directory).href) as { handlers: H } & E;
  return { manifest, ...worker };
}
const application = await load<typeof threadHandlers, { peopleHandlers: typeof peopleHandlers }>(new URL("../dist/", import.meta.url));
const contribution = await load<typeof harnessHandlers>(new URL("../../calternal-threads-claude-code/dist/", import.meta.url));
export const handlers = application.handlers;

const exchanges = {
  item_create: ["ItemCreate", "ItemContentReceipt"],
  item_head: ["ItemHeadRequest", "OwnedItemHead"],
  item_range: ["ItemRange", "ItemRangeResult"],
  item_save: ["ItemSave", "ItemSaveReply"],
  contribution_call: ["ContributionCall", "ContributionCallReceipt"],
  contribution_read: ["ContributionRead", "ContributionReadResult"],
  start_job: ["ExecutionJobStart", "ExecutionJobReceipt"],
  execution_start: ["ExecutionStart", "ExecutionStatus"],
  item_append: ["ItemAppend", "ItemAppendReceipt"],
  item_view_context: ["ItemViewContextRequest", "ItemViewContext"],
  item_view_publish: ["ItemViewPublish", "ItemViewPublishReceipt"],
  alarm_set: ["AlarmSet", "AlarmReceipt"],
} as const;
const trace: [name: string, value: unknown][] = [];

/** Check the fake's revision 14.32 exchanges against the pinned schemas. */
export function checkTrace() {
  const check = fileURLToPath(new URL("../check-shapes.py", import.meta.url));
  const contract = fileURLToPath(new URL("../.build/contract/", import.meta.url));
  execFileSync("python3", ["-B", check, contract], { input: JSON.stringify(trace), stdio: ["pipe", "inherit", "inherit"] });
}

type File = { creator: string; discovery: "creator_only" | "consent_only"; owner: "threads" | "claude-code-run"; history: Map<string, Buffer>; revision: string; path?: string; archived: boolean; title: string };
type Job = {
  context: ExecutionJobContext;
  input: RunInput;
  state: "pending" | "running" | "done" | "stopped";
  attempt?: ExecutionStatus;
  teardown: boolean;
  result?: HarnessOutput;
  contribution?: { call: string; input: HarnessInput };
};
type Owner = File["owner"];
const actor = "019a0000-0000-7000-8000-000000000001";
const space = "019a0000-0000-7000-8000-000000000010";
const now = "2026-11-20T12:00:00Z";
const logicalHarness = {
  package: contribution.manifest.id,
  contribution: contribution.manifest.contributions![0].id,
};
const checkedPeople = [
  actor, "019a0000-0000-7000-8000-000000000002", "019a0000-0000-7000-8000-000000000003",
];
const peopleMethods = new Set(["item_create", "item_head", "item_range", "item_save", "item_append", "item_view_context", "item_view_publish"]);

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
  invitedInputs: InvitedInput[] = [];
  private personState = new Map<string, string>();
  beforeTerminal = async () => {};
  claude = async (text: string) => `Reply to ${text}`;

  choose() {
    const app = application.manifest.apps[0];
    const point = app.extension_points![0];
    const selected = contribution.manifest.contributions![0];
    assert.deepEqual(selected.app, { package: application.manifest.id, app: app.id });
    assert.equal(selected.point, point.id);
    assert.equal(selected.version, point.version);
    assert.equal(selected.operation, "threads-claude-code.run");
    const operation = contribution.manifest.operations.find(op => op.id === selected.operation);
    assert.deepEqual(operation?.input_schema, point.input_schema);
    assert.deepEqual(operation?.output_schema, point.output_schema);
    trace.push(
      ["RendererExtensionChoiceRequest", { type: "extension_choices", app: app.id, point: point.id, activation_context: "ctx", slot: "harness" }],
      ["ExtensionChoicePage", { options: [{ ref: "option", label: selected.harness!.name, harness_ref: this.selected.harness, contribution_identity: logicalHarness }] }],
      ["ChoiceOptionsRequest", { type: "choice_options", activation_context: "ctx", slot: "placement", harness: this.selected.harness, activation_id: "send" }],
      ["ChoiceOptions", { host: "execution_placement", options: [{ ref: this.selected.placement, label: "Desk" }], default_ref: this.selected.placement }],
    );
    return { ...this.selected };
  }

  rotateHarness() {
    this.selected = { ...this.selected, harness: this.id("checked-harness") };
    return this.choose();
  }

  discoverAsAttempt(person: string) {
    return [...this.files.entries()].filter(([, file]) =>
      file.owner === "threads" && file.creator === person && file.discovery === "creator_only",
    ).map(([item]) => item);
  }

  peopleContext(person: string, request: string): PeopleContext {
    const call: Context["call"] = async (method, input) => {
      assert.ok(peopleMethods.has(method));
      const wire = exchanges[method as keyof typeof exchanges];
      if (wire) trace.push([wire[0], input]);
      const result = await this.call("threads", request, method, input, undefined, person);
      if (wire) trace.push([wire[1], result]);
      return result as never;
    };
    return {
      author: { request_id: request, actor: person, authored_at: now }, checkedPeople, call,
      privateState: {
        read: async item => this.personState.get(JSON.stringify([person, item])),
        write: async (item, body, expected) => {
          const key = JSON.stringify([person, item]);
          assert.equal(this.personState.get(key), expected, "private state changed");
          this.personState.set(key, body);
        },
      },
    };
  }

  invitedContext(person: string, request: string, input: { item: string; scope: Scope; consent: string[] }): PeopleContext {
    const ctx = this.peopleContext(person, request);
    const { item, scope, consent } = input;
    const invitation = {
      item, scope, consent: [...consent], revision: this.files.get(item)!.revision,
      inviter: person, contribution_identity: logicalHarness,
    };
    return {
      ...ctx, invitation,
      runInvited: async (grant, payload) => {
        assert.equal(grant, invitation, "unchecked invitation");
        assert.equal(this.files.get(item)?.revision, invitation.revision, "invitation changed");
        this.invitedInputs.push(structuredClone(payload));
        const harness = this.context(request, "threads-claude-code.run", undefined, undefined, "claude-code-run");
        const reply = await contribution.handlers["threads-claude-code.run"]({ text: JSON.stringify(payload) }, {
          ...harness, claude: async () => "Reply to the permitted context",
        });
        assert.equal(this.files.get(item)?.revision, invitation.revision, "invitation changed");
        return reply;
      },
    };
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
    assert.ok(!Object.hasOwn(payload.input, "item"), "duplicate target field");
    const intent = this.id("job");
    const context = { intent, item: payload.item, harness: payload.harness, contribution_identity: logicalHarness };
    const input = { ...payload.input as Omit<RunInput, "item">, item: payload.item };
    this.jobs.set(intent, { context, input, state: "pending", teardown: true });
    return { job_id: intent };
  }

  context(request: string, operation: string, job?: Job, alarm?: AlarmDelivery, owner: Owner = "threads", frame?: HostWorkerInvokeFrame["message"]): Context {
    const declared = (owner === "threads" ? application : contribution).manifest.operations.find(op => op.id === operation);
    assert.ok(declared);
    const call: Context["call"] = async (method, input) => {
      assert.ok((declared.methods as readonly string[]).includes(method), "undeclared method");
      const wire = exchanges[method as keyof typeof exchanges];
      if (wire) trace.push([wire[0], input]);
      const result = await this.call(owner, request, method, input, job, actor, operation === "threads.create");
      if (wire) trace.push([wire[1], result]);
      return result as never;
    };
    const message = frame ?? { author_context: { request_id: request, actor, authored_at: now }, job_context: job?.context, alarm };
    const author = authorContext(message, call as Parameters<typeof authorContext>[1]);
    return { ...author, call };
  }

  // This dispatch is the only place that unpacks fixture broker method operands.
  private async call(owner: Owner, request: string, method: Parameters<Context["call"]>[0], input: unknown, job?: Job, receiptActor = actor, checkedSoleCreator = false): Promise<unknown> {
    if (job) assert.equal(job.state, "running", "attempt retired");
    const args = input as Record<string, string>;
    const file = this.files.get(args.item);
    const writable = () => {
      assert.ok(file?.owner === owner && !file.archived, "target unavailable");
      return file;
    };
    const requestId = args.request_id ?? request;
    const commit = (effect: () => unknown) => this.receipt(`${owner}:${receiptActor}:${method}:${requestId}`, input, effect);
    switch (method) {
      case "item_create": return commit(() => {
        const create = input as ItemCreate;
        assert.equal(create.kind, owner);
        const item = this.id("item");
        const revision = this.id("rev");
        const body = Buffer.from(create.body ?? "", "base64");
        const root = owner === "threads" ? "Threads" : ".calternal";
        const folders = create.folders ?? [];
        const used = new Set([...this.files.values()].map(file => file.path));
        let path = [root, ...folders, create.name].join("/");
        for (let suffix = 2; used.has(path); suffix++) {
          path = folders.length
            ? [root, `${folders[0]} (${suffix})`, ...folders.slice(1), create.name].join("/")
            : [root, `${suffix} ${create.name}`].join("/");
        }
        const discovery = create.attempt_discovery ?? "consent_only";
        if (discovery === "creator_only") {
          assert.ok(checkedSoleCreator, "unchecked sole creator");
          assert.equal(application.manifest.kinds.find(kind => kind.id === create.kind)?.attempt_discovery, "item_policy");
        }
        this.files.set(item, { owner, creator: receiptActor, discovery, revision, history: new Map([[revision, body]]),
          path, archived: false, title: String(create.metadata.title) });
        return { item, revision, locator: { space, id: item }, receipt: requestId };
      });
      case "item_save": {
        const save = input as ItemSave;
        const target = writable();
        if (save.expected !== target.revision && !this.receipts.has(`${owner}:${receiptActor}:${method}:${requestId}`)) {
          return { status: 409, code: "head_conflict", current: this.head(save.item, target) };
        }
        return commit(() => {
          const revision = this.id("rev");
          const body = save.body === undefined ? target.history.get(target.revision)! : Buffer.from(save.body, "base64");
          target.history.set(revision, body);
          target.revision = revision;
          if (typeof save.patch?.title === "string") target.title = save.patch.title;
          if (save.attempt_discovery === "consent_only" || receiptActor !== target.creator) target.discovery = "consent_only";
          return { item: save.item, revision, locator: { space, id: save.item }, receipt: requestId };
        });
      }
      case "item_head": {
        assert.equal(file?.owner, owner);
        return this.head(args.item, file);
      }
      case "item_range": {
        assert.equal(file?.owner, owner);
        const body = file.history.get(args.revision);
        assert.ok(body, "obsolete range");
        const { offset, length } = input as { offset: number; length: number };
        assert.ok(offset >= 0 && length <= 65536);
        const page = body.subarray(offset, offset + length);
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
      case "contribution_call": return commit(() => {
        assert.ok(job?.attempt && job.attempt.execution === args.execution && job.context.harness === args.contribution);
        assert.equal(job.contribution, undefined);
        const call = this.id("call");
        const payload = (input as { input: HarnessInput }).input;
        encodeContributionFrame({ call, input: payload });
        job.contribution = { call, input: payload };
        return { call };
      });
      case "contribution_read": {
        assert.ok(job?.attempt && job.contribution?.call === args.call);
        const { after } = input as { after?: number };
        if (after === 1) {
          await this.beforeTerminal();
          assert.equal(job.state, "running", "attempt retired");
          return { frames: [{ seq: 2, kind: "terminal", state: "ok" }] };
        }
        assert.equal(after, undefined);
        if (!job.result) {
          const entry = job.contribution;
          trace.push(["ContributionEntryInvoke", entry]);
          const ctx = this.context(entry.call, "threads-claude-code.run", job, undefined, "claude-code-run");
          job.result = await contribution.handlers["threads-claude-code.run"](entry.input, {
            ...ctx, claude: async text => {
              const reply = await this.claude(text);
              assert.equal(job.state, "running", "attempt retired");
              return reply;
            },
          });
        }
        return { frames: [{ seq: 1, kind: "output", data: job.result }] };
      }
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
        const set = input as AlarmSet;
        assert.equal(set.harness, this.selected.harness);
        assert.ok(set.item && this.files.get(set.item)?.owner === "threads");
        assert.ok(!Object.hasOwn(set.input, "item"), "duplicate target field");
        const alarm = this.id("alarm");
        this.alarms.set(alarm, set);
        return { alarm, receipt: requestId };
      });
    }
  }

  private head(item: string, file: File) {
    return { item, revision: file.revision, locator: { space, id: item }, bytes: file.history.get(file.revision)!.length,
      metadata: { title: file.title } };
  }

  async run(id: string, alarm?: AlarmDelivery) {
    const job = this.jobs.get(id)!;
    const head = [...this.jobs.values()].find(candidate => candidate.context.item === job.context.item && ["pending", "running"].includes(candidate.state));
    assert.equal(head, job, "successor must wait for the head");
    assert.equal(job.state, "pending");
    assert.equal(job.context.harness, this.selected.harness, "stale claimed harness");
    assert.ok([...this.jobs.values()].every(candidate =>
      candidate.context.item !== job.context.item || candidate.teardown), "successor must wait for teardown");
    const frame: HostWorkerInvokeFrame = {
      v: 1, epoch: "fake", seq: ++this.serial, package: application.manifest.id, digest: "0".repeat(64), ticket: id,
      message: { type: "invoke", operation: "threads.run", input: job.input, deadline: now,
        author_context: { request_id: id, actor, authored_at: now }, job_context: job.context, ...(alarm && { alarm }) },
    };
    trace.push(["HostWorkerInvokeFrame", frame]);
    job.state = "running";
    try {
      return await handlers["threads.run"](job.input, this.context(id, "threads.run", job, alarm, "threads", frame.message));
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
    const { job, item, at, input, harness, grace_seconds } = armed;
    assert.ok(item && harness);
    const delivery = this.admit({ job, item, input, harness });
    this.alarms.delete(alarm);
    return this.run(delivery.job_id, { alarm, scheduled_at: at, actual_at, missed: grace_seconds !== undefined && Date.parse(actual_at) > Date.parse(at) + 1000 * grace_seconds });
  }

  markdown(item: string) {
    const file = this.files.get(item)!;
    return file.history.get(file.revision)!.toString();
  }
}

/** Load the prototype worker from the pinned package, like the declared handlers. */
export function peopleFromBundle() {
  return application.peopleHandlers;
}
