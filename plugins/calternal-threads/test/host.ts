import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { AlarmDelivery, AlarmSet, AttestedBody, ExecutionJobContext, ExecutionJobStart, ExecutionStatus,
  ExecutionStopRequest, ExecutionStopResult, HostWorkerInvokeFrame, ItemAppend, ItemContentReceipt, ItemCreate, ItemSave, ItemStageBegin, ItemStageChunk, ItemStageSeal, ItemViewPublish } from "../generated/wire.js";
import { encodeContributionFrame } from "../generated/contribution-frame.mjs";
import { authorContext } from "../generated/author-context.mjs";
import type { ManifestV2 } from "../generated/author.js";
import type { Context, RunInput } from "../src/interface.ts";
import type { HarnessInput, HarnessOutput } from "../src/harnesses-v1.ts";
import type { handlers as threadHandlers } from "../src/worker.ts";
import { scoped } from "../src/people.ts";
import { decode } from "../src/format.ts";
import type { ConsultationTurn, InvitedInput, PeopleContext, Scope, peopleHandlers } from "../src/people.ts";
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
  item_save_begin: ["ItemStageBegin", "ItemStageBeginReply"],
  item_save_chunk: ["ItemStageChunk", "ItemStageChunkAck"],
  item_save_seal: ["ItemStageSeal", "ItemStageSealed"],
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

/** Check the fake's revision 14.37 exchanges against the pinned schemas. */
export function checkTrace() {
  const check = fileURLToPath(new URL("../check-shapes.py", import.meta.url));
  const contract = fileURLToPath(new URL("../.build/contract/", import.meta.url));
  execFileSync("python3", ["-B", check, contract], { input: JSON.stringify(trace), stdio: ["pipe", "inherit", "inherit"] });
}

type File = { space: string; people?: readonly string[]; creator: string; discovery: "creator_only" | "consent_only"; owner: "threads" | "claude-code-run"; history: Map<string, Buffer>; authors: Map<string, string>; spans: Map<string, { start: number; length: number }>; revision: string; path?: string; archived: boolean; title: string };
type Job = {
  context: ExecutionJobContext;
  starter: string;
  capturedInput?: CheckedText;
  input: RunInput;
  state: "pending" | "running" | "done" | "stopped";
  attempt?: ExecutionStatus;
  teardown: boolean;
  result?: HarnessOutput;
  contribution?: { call: string; input: HarnessInput };
};
type CheckedText = { item?: string; text: string };
type Owner = File["owner"];
type Stage = { spec: ItemStageBegin; chunks: Buffer[]; length: number; sealed?: Buffer };
type StagingMethod = "item_save_begin" | "item_save_chunk" | "item_save_seal";
const actor = "019a0000-0000-7000-8000-000000000001";
const space = "019a0000-0000-7000-8000-000000000010";
const now = "2026-11-20T12:00:00Z";
const logicalHarness = {
  package: contribution.manifest.id,
  contribution: contribution.manifest.contributions![0].id,
};
const directoryAccounts = [
  actor, "019a0000-0000-7000-8000-000000000002", "019a0000-0000-7000-8000-000000000003",
];
const personLabels: Record<string, string> = Object.fromEntries(directoryAccounts.map((id, index) => [id, ["Alice", "Bob", "Cara"][index]]));
const defaultGroup = "threads-group";
type DirectoryGroup = { id: string; source: "local" | "oidc"; members: readonly string[] };
const peopleMethods = new Set(["item_create", "item_head", "item_range", "item_save", "item_append", "item_view_context", "item_view_publish"]);

/** One in-memory space. Validates package ownership, exact revisions and
 * attempt retirement around the calls used by the spike. */
export class FakeHost {
  files = new Map<string, File>();
  jobs = new Map<string, Job>();
  snapshots = new Map<string, ItemViewPublish>();
  alarms = new Map<string, AlarmSet>();
  receipts = new Map<string, { fingerprint: string; result: unknown }>();
  private stages = new Map<string, Stage>();
  private scopes = new Map<string, { item: string; revision: string }>();
  private serial = 0;
  private selected = { harness: "checked-harness", placement: "checked-desk" };
  loseNextAppendAck = false;
  invitedInputs: InvitedInput[] = [];
  onInvited = async (_tools: { readMessage(id: string): { id: string; text: string } | undefined }) => {};
  consultationTurns: ConsultationTurn[] = [];
  private consent = new Map<string, { generation: number; always: boolean }>();
  invitationClock = (authoredAt: string) => Date.parse(authoredAt);
  private personState = new Map<string, string>();
  private personPreferences = new Map<string, string>();
  private directoryGroups: readonly DirectoryGroup[] = [{ id: defaultGroup, source: "local", members: directoryAccounts }];
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

  /** Read the host's attribution for the current or a specific committed revision. */
  writeAuthor(item: string, revision = this.files.get(item)!.revision) {
    return this.files.get(item)!.authors.get(revision);
  }

  /** Simulate the host renderer capturing the composer field before Send dispatch. */
  composerContext(request: string, item: string, text: string) {
    return this.context(request, "threads.send", undefined, undefined, "threads", undefined, { item, text });
  }

  /** Simulate checked new-thread form capture. */
  createContext(request: string, title: string) {
    return this.context(request, "threads.create", undefined, undefined, "threads", undefined, { text: title });
  }

  /** Return the newest issued span from the fixture writer's receipts. */
  lastSpan(item: string) {
    const span = [...this.files.get(item)!.spans.keys()].at(-1);
    assert.ok(span);
    return span;
  }

  /** Seed checked empty bytes for import tests independently of the app's formatted create. */
  async createEmptyCreatorItem() {
    const input: ItemCreate = {
      request_id: this.id("empty"), kind: "threads", name: "Empty.md", metadata: { title: "Empty" },
      attempt_discovery: "creator_only",
    };
    const result = await this.call("threads", input.request_id, "item_create", input, undefined, actor, true) as ItemContentReceipt;
    trace.push(["ItemCreate", input], ["ItemContentReceipt", result]);
    return result;
  }

  /** Exercise a synthetic checked editor save with independent capture and write bytes. */
  async saveCapturedText(item: string, text: string, bytes: Buffer) {
    const input: ItemSave = {
      request_id: this.id("editor"), item, expected: this.files.get(item)!.revision,
      ...(bytes.equals(Buffer.from(text)) ? { attested: { source: { kind: "input" } } } : { body: bytes.toString("base64") }),
    };
    const result = await this.call("threads", input.request_id, "item_save", input, undefined, actor, true, undefined, { item, text });
    trace.push(["ItemSave", input], ["ItemSaveReply", result]);
    return result;
  }

  async stageUnproven(item: string, bytes: Buffer) {
    const expected = this.files.get(item)!.revision;
    const request_id = this.id("import");
    const call = async <T>(method: StagingMethod | "item_save", operand: unknown): Promise<T> => {
      const [before, after] = exchanges[method];
      trace.push([before, operand]);
      const result = await this.call("threads", request_id, method, operand);
      trace.push([after, result]);
      return result as T;
    };
    const spec = {
      request_id, item, expected, length: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"),
    };
    const { handle } = await call<{ handle: string }>("item_save_begin", spec);
    for (let offset = 0; offset < bytes.length; offset += 65536) {
      await call("item_save_chunk", { handle, offset, bytes: bytes.subarray(offset, offset + 65536).toString("base64") });
    }
    const { staged } = await call<{ staged: string }>("item_save_seal", { handle });
    // The import has no trusted provenance. The wire save omits the marker.
    return {
      begin: (length = spec.length) => call("item_save_begin", { ...spec, length }),
      publish: (title?: string) => call("item_save", { request_id, item, expected, staged, ...(title && { patch: { title } }) }),
    };
  }

  attemptRestoreDiscovery(item: string) {
    return this.call("threads", "try-clear-marker", "item_save", {
      request_id: "try-clear-marker", item, expected: this.files.get(item)!.revision, attempt_discovery: "creator_only",
    });
  }

  personalSpace(person: string) {
    assert.ok(directoryAccounts.includes(person));
    return space.slice(0, -1) + directoryAccounts.indexOf(person);
  }

  /** Set fixture memberships supplied by the account system, including mirrored groups. */
  setDirectoryGroups(groups: readonly DirectoryGroup[]) {
    this.directoryGroups = groups;
  }

  // The package picker uses the checked group, separate from administrative visibility (ADR 0060).
  private canPickPerson(group: string, viewer: string, person: string) {
    const members = this.directoryGroups.find(entry => entry.id === group)?.members;
    return directoryAccounts.includes(viewer) && directoryAccounts.includes(person) &&
      members?.includes(viewer) && members.includes(person);
  }

  conversationAccess(item: string, person: string) {
    const people = this.files.get(item)?.people;
    if (!people?.includes(person)) return;
    return this.directoryGroups.some(group => people.every(id => group.members.includes(id))) ? "full" : "read";
  }

  setAlwaysConsent(item: string, person: string, always: boolean) {
    assert.equal(this.conversationAccess(item, person), "full");
    const key = JSON.stringify([item, person]);
    const previous = this.consent.get(key);
    this.consent.set(key, { generation: (previous?.generation ?? 0) + 1, always });
  }

  consultationContext(person: string, request: string, authored_at = now): PeopleContext {
    return {
      ...this.peopleContext(person, request, authored_at),
      consultation: {
        lead: logicalHarness, consultant: { package: "fixture-codex", contribution: "codex" },
        invoke: async (role, _prompt, prior) => {
          assert.deepEqual(prior, this.consultationTurns);
          const text = role === "summary" ? "Agreed recommendation" : `Private exchange ${prior.length + 1}`;
          this.consultationTurns.push({ role, text });
          return text;
        },
      },
    };
  }

  peopleContext(person: string, request: string, authored_at = now, actor_label = personLabels[person], group = defaultGroup,
    messageWrite?: PeopleContext["messageWrite"]): PeopleContext {
    const call: Context["call"] = async (method, input) => {
      assert.ok(peopleMethods.has(method));
      const wire = exchanges[method as keyof typeof exchanges];
      if (wire) trace.push([wire[0], input]);
      const captured = messageWrite?.text === undefined ? undefined : { item: messageWrite.item, text: messageWrite.text };
      const result = await this.call("threads", request, method, input, undefined, person, true, undefined, captured);
      if (wire) trace.push([wire[1], result]);
      return result as never;
    };
    return {
      author: { request_id: request, actor: person, authored_at, ...(actor_label && { actor_label }) }, call, messageWrite,
      canWriteConversation: async item => this.conversationAccess(item, person) === "full",
      visiblePeople: async (ids = directoryAccounts) => ids.filter(id => this.canPickPerson(group, person, id))
        .map(id => ({ id, name: personLabels[id] })),
      createShared: async (input, people) => {
        if (!people.includes(person) || people.some(id => !this.canPickPerson(group, person, id))) {
          throw new Error("Participants unavailable.");
        }
        assert.equal(input.kind, "threads");
        trace.push(["ItemCreate", input]);
        const result = await this.call("threads", request, "item_create", input, undefined, person, true, people);
        trace.push(["ItemContentReceipt", result]);
        return result as ItemContentReceipt;
      },
      privateState: {
        read: async item => this.personState.get(JSON.stringify([person, item])),
        write: async (item, body, expected) => {
          const key = JSON.stringify([person, item]);
          assert.equal(this.personState.get(key), expected, "private state changed");
          this.personState.set(key, body);
        },
      },
      privatePreferences: {
        read: async () => this.personPreferences.get(person),
        write: async (body, expected) => {
          assert.equal(this.personPreferences.get(person), expected, "private preferences changed");
          this.personPreferences.set(person, body);
        },
      },
    };
  }

  invitedContext(person: string, request: string, input: { item: string; scope: Scope; consent: string[] }, authored_at = now): PeopleContext {
    const ctx = this.peopleContext(person, request, authored_at);
    const { item, scope, consent } = input;
    const file = this.files.get(item)!;
    assert.equal(this.conversationAccess(item, person), "full");
    const invitation = { item, scope, revision: file.revision, inviter: person, contribution_identity: logicalHarness, call: request };
    const chosen = new Map(scoped(decode(this.markdown(item)).messages, scope).map(entry => [entry.id, entry]));
    const permission = (author: string) => this.consent.get(JSON.stringify([item, author])) ?? { generation: 0, always: false };
    const grants = new Map((file.people ?? []).flatMap(author => {
      const grant = permission(author);
      if (grant.always) return [[author, { ...grant, until: Infinity }] as const];
      if (consent.includes(author)) return [[author, { ...grant, until: Date.parse(authored_at) + 5 * 60_000 }] as const];
      return [];
    }));
    let state: "ready" | "running" | "done" = "ready";
    const canDisclose: NonNullable<PeopleContext["canDisclose"]> = (grant, author) => {
      const pinned = grants.get(author);
      return grant === invitation && state !== "done" && pinned !== undefined
        && this.conversationAccess(item, person) === "full"
        && pinned.generation === permission(author).generation && this.invitationClock(authored_at) < pinned.until;
    };
    return {
      ...ctx, invitation, canDisclose,
      runInvited: async (grant, payload) => {
        assert.equal(state, "ready", "invitation already used");
        assert.equal(grant, invitation, "unchecked invitation");
        assert.equal(payload.tools, true);
        assert.equal(this.conversationAccess(item, person), "full", "Conversation is read-only.");
        const selected = new Map(payload.messages.map(message => [message.id, message]));
        const readMessage = (id: string) => {
          const original = chosen.get(id);
          const message = selected.get(id);
          if (state !== "running" || !original || original.deleted || original.author.kind !== "person"
            || !message || original.text !== message.text || !canDisclose(grant, original.author.id)) return;
          const live = decode(this.markdown(item)).messages.find(entry => entry.id === id);
          if (live?.deleted || live?.text !== original.text || live.author.id !== original.author.id) return;
          return { ...message };
        };
        state = "running";
        try {
          assert.ok(payload.messages.every(message => readMessage(message.id)), "invitation scope changed");
          this.invitedInputs.push(structuredClone(payload));
          const harness = this.context(request, "threads-claude-code.run", undefined, undefined, "claude-code-run");
          const reply = await contribution.handlers["threads-claude-code.run"]({ text: JSON.stringify(payload) }, {
            ...harness, claude: async () => {
              await this.onInvited({ readMessage });
              assert.ok(payload.messages.every(message => readMessage(message.id)), "invitation consent changed");
              return "Reply to the permitted context";
            },
          });
          assert.equal(this.conversationAccess(item, person), "full", "Conversation is read-only.");
          assert.ok(payload.messages.every(message => readMessage(message.id)), "invitation consent changed");
          return reply;
        } finally {
          state = "done";
        }
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

  private preflight(harness: string, input: unknown) {
    assert.ok(input && typeof input === "object" && Object.keys(input).length === 1
      && "text" in input && typeof input.text === "string" && input.text.length > 0, "invalid contribution input");
    // Ref allows 128 Unicode code points, up to six JSON bytes each.
    const unknownRef = "\u0000".repeat(128);
    try {
      encodeContributionFrame({ request_id: unknownRef, execution: unknownRef, contribution: harness, input });
      encodeContributionFrame({ call: unknownRef, input });
    } catch (error) {
      if (error instanceof RangeError) throw Object.assign(error, { status: 413 });
      throw error;
    }
  }

  private admit(payload: ExecutionJobStart, starter = actor, capturedInput?: CheckedText) {
    assert.equal(payload.harness, this.selected.harness, "unchecked harness");
    if (payload.placement !== undefined) assert.equal(payload.placement, this.selected.placement, "unchecked placement");
    const file = this.files.get(payload.item);
    assert.ok(file?.owner === "threads" && !file.archived, "target unavailable");
    const active = [...this.jobs.values()].filter(job => job.context.item === payload.item && ["pending", "running"].includes(job.state));
    assert.ok(active.length < 2, "pending_limit");
    assert.ok(!Object.hasOwn(payload.input, "item"), "duplicate target field");
    this.preflight(payload.harness, payload.contribution_input);
    if (capturedInput) {
      assert.equal(capturedInput.item, payload.item);
      assert.deepEqual(payload.contribution_input, { text: capturedInput.text });
    }
    const intent = this.id("job");
    const context = { intent, item: payload.item, harness: payload.harness, contribution_identity: logicalHarness, contribution_input: structuredClone(payload.contribution_input) };
    const input = { ...payload.input as Omit<RunInput, "item">, item: payload.item };
    this.jobs.set(intent, { context, input, starter, capturedInput, state: "pending", teardown: true });
    return { job_id: intent };
  }

  context(request: string, operation: string, job?: Job, alarm?: AlarmDelivery, owner: Owner = "threads", frame?: HostWorkerInvokeFrame["message"], capturedInput?: CheckedText): Context {
    const declared = (owner === "threads" ? application : contribution).manifest.operations.find(op => op.id === operation);
    assert.ok(declared);
    const person = job?.starter ?? actor;
    const message = frame ?? { author_context: { request_id: request, actor: person, authored_at: now, actor_label: personLabels[person] }, job_context: job?.context, alarm };
    assert.equal(message.author_context?.actor, person);
    if (capturedInput) assert.equal(declared.write_origins?.host_input, operation === "threads.create" ? "title" : "text");
    const call: Context["call"] = async (method, input) => {
      assert.ok((declared.methods as readonly string[]).includes(method), "undeclared method");
      const wire = exchanges[method as keyof typeof exchanges];
      if (wire) trace.push([wire[0], input]);
      const result = await this.call(owner, request, method, input, job, person, true, undefined,
        capturedInput ?? job?.capturedInput, declared.write_origins?.attempt_output === "text");
      if (wire) trace.push([wire[1], result]);
      return result as never;
    };
    const author = authorContext(structuredClone(message), call as Parameters<typeof authorContext>[1]);
    return { ...author, call };
  }

  // This dispatch is the only place that unpacks fixture broker method operands.
  private async call(owner: Owner, request: string, method: Parameters<Context["call"]>[0] | StagingMethod, input: unknown, job?: Job, receiptActor = actor, checkedInvocation = false, shared?: readonly string[], captured?: CheckedText, allowsOutput = false): Promise<unknown> {
    if (job) {
      assert.ok(this.jobs.get(job.context.intent) === job && job.state === "running", "attempt retired");
      assert.equal(job.starter, receiptActor);
    }
    const args = input as Record<string, string>;
    const file = this.files.get(args.item);
    if (file?.people) assert.ok(file.people.includes(receiptActor), "Conversation unavailable.");
    const writable = () => {
      assert.ok(file?.owner === owner && !file.archived, "target unavailable");
      if (owner === "threads" && job) assert.equal(job.context.item, args.item, "wrong attempt target");
      if (file.people && this.conversationAccess(args.item, receiptActor) !== "full") {
        throw new Error("Conversation is read-only.");
      }
      return file;
    };
    const requestId = args.request_id ?? request;
    const receiptKey = (name: string) => `${owner}:${receiptActor}:${name}:${requestId}`;
    const commit = (effect: () => unknown) => this.receipt(receiptKey(method), input, effect);
    const compose = ({ source, prefix = "", suffix = "" }: AttestedBody) => {
      assert.ok(checkedInvocation);
      assert.ok(Buffer.byteLength(prefix) <= 512 && Buffer.byteLength(suffix) <= 512, "framing exceeds 512 bytes");
      let text = "";
      if (source.kind === "input") {
        assert.ok(captured && captured.item === args.item, "missing checked input");
        text = captured.text;
      } else if (source.kind === "output") {
        assert.ok(allowsOutput && job?.attempt && job.result && source.seq === 1, "missing delivered output");
        text = job.result.text;
      }
      return Buffer.from(prefix + text + suffix);
    };
    const attributeWrite = (target: File, revision: string, attested?: AttestedBody) => {
      if (attested && attested.source.kind !== "empty") target.authors.set(revision, receiptActor);
      if (!attested || receiptActor !== target.creator || args.attempt_discovery === "consent_only") target.discovery = "consent_only";
    };
    const recordSpan = (target: File, start: number, length: number) => {
      if (!length) return {};
      const span = this.id("span");
      target.spans.set(span, { start, length });
      return { span };
    };
    switch (method) {
      case "item_create": return commit(() => {
        const create = input as ItemCreate;
        assert.ok(create.kind === owner || (owner === "threads" && create.kind === "thread-consultations"));
        const item = this.id("item");
        const revision = this.id("rev");
        const body = create.attested ? compose(create.attested) : Buffer.from(create.body ?? "", "base64");
        const root = create.kind === "thread-consultations" ? ".calternal/thread-consultations" : owner === "threads" ? "Threads" : ".calternal";
        const folders = create.folders ?? [];
        const used = new Set([...this.files.values()].map(file => file.path));
        let path = [root, ...folders, create.name].join("/");
        for (let suffix = 2; used.has(path); suffix++) {
          path = folders.length
            ? [root, `${folders[0]} (${suffix})`, ...folders.slice(1), create.name].join("/")
            : [root, `${suffix} ${create.name}`].join("/");
        }
        const discovery = create.attempt_discovery === "creator_only" && checkedInvocation && !shared && (create.attested || body.length === 0)
          ? "creator_only" : "consent_only";
        if (create.attempt_discovery === "creator_only") {
          assert.equal(application.manifest.kinds.find(kind => kind.id === create.kind)?.attempt_discovery, "item_policy");
        }
        const target: File = { owner, space: this.personalSpace(receiptActor), people: shared && [...shared], creator: receiptActor, discovery, revision, history: new Map([[revision, body]]),
          authors: new Map(), spans: new Map(), path, archived: false, title: String(create.metadata.title) };
        const span = create.attested ? recordSpan(target, 0, body.length) : {};
        if (create.attested) attributeWrite(target, revision, create.attested);
        this.files.set(item, target);
        return { item, revision, locator: { space: target.space, id: item }, receipt: requestId, ...span };
      });
      case "item_save_begin": {
        const spec = input as ItemStageBegin;
        const target = writable();
        return commit(() => {
          assert.equal(spec.expected, target.revision);
          assert.ok(spec.length > 0 && spec.length <= 2 * 1024 * 1024);
          const handle = this.id("stage");
          this.stages.set(handle, { spec, chunks: [], length: 0 });
          return { handle };
        });
      }
      case "item_save_chunk": {
        const { handle, offset, bytes } = input as ItemStageChunk;
        const stage = this.stages.get(handle);
        assert.ok(stage && !stage.sealed && offset === stage.length);
        const chunk = Buffer.from(bytes, "base64");
        assert.ok(chunk.length > 0 && chunk.length <= 65536 && stage.length + chunk.length <= stage.spec.length);
        stage.chunks.push(chunk);
        stage.length += chunk.length;
        return { next_offset: stage.length };
      }
      case "item_save_seal": {
        const { handle } = input as ItemStageSeal;
        const stage = this.stages.get(handle);
        assert.ok(stage && stage.length === stage.spec.length);
        const body = Buffer.concat(stage.chunks);
        assert.equal(createHash("sha256").update(body).digest("hex"), stage.spec.sha256);
        stage.sealed = body;
        return { staged: handle };
      }
      case "item_save": {
        const save = input as ItemSave;
        assert.ok(args.attempt_discovery === undefined || args.attempt_discovery === "consent_only", "host-owned marker");
        const target = writable();
        if (save.expected !== target.revision && !this.receipts.has(receiptKey(method))) {
          return { status: 409, code: "head_conflict", current: this.head(save.item, target) };
        }
        return commit(() => {
          const revision = this.id("rev");
          const stage = save.staged === undefined ? undefined : this.stages.get(save.staged);
          if (save.staged !== undefined) {
            assert.ok(stage?.sealed && stage.spec.item === save.item && stage.spec.expected === save.expected
              && stage.spec.request_id === save.request_id, "invalid staged replacement");
          }
          const original = target.history.get(target.revision)!;
          let body = stage?.sealed ?? (save.body === undefined ? original : Buffer.from(save.body, "base64"));
          let span = {};
          if (save.attested) {
            const replacement = compose(save.attested);
            if (save.replace_span) {
              const prior = target.spans.get(save.replace_span);
              assert.ok(prior, "span unavailable");
              body = Buffer.concat([original.subarray(0, prior.start), replacement, original.subarray(prior.start + prior.length)]);
              target.spans.delete(save.replace_span);
              for (const other of target.spans.values()) {
                if (other.start > prior.start) other.start += replacement.length - prior.length;
              }
              span = recordSpan(target, prior.start, replacement.length);
            } else {
              body = replacement;
              target.spans.clear();
              span = recordSpan(target, 0, replacement.length);
            }
          } else if (stage || save.body !== undefined) target.spans.clear();
          if (save.staged !== undefined) this.stages.delete(save.staged);
          target.history.set(revision, body);
          target.revision = revision;
          if (typeof save.patch?.title === "string") target.title = save.patch.title;
          if (stage || save.body !== undefined || save.attested) attributeWrite(target, revision, save.attested);
          if (save.attempt_discovery === "consent_only") target.discovery = "consent_only";
          const result = { item: save.item, revision, locator: { space: target.space, id: save.item }, receipt: requestId, ...span };
          if (stage) this.receipts.get(receiptKey("item_save_begin"))!.result = result;
          return result;
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
      case "start_job": return commit(() => this.admit(input as ExecutionJobStart, receiptActor, captured));
      case "execution_start": return commit(() => {
        assert.ok(job?.state === "running");
        assert.equal(args.harness, job.context.harness);
        job.teardown = false;
        job.attempt = { execution: this.id("attempt"), phase: "running", transition_cursor: this.id("transition") };
        return job.attempt;
      });
      case "contribution_call": return commit(() => {
        assert.ok(job?.attempt && job.attempt.execution === args.execution && job.context.harness === args.contribution);
        const payload = (input as { input: HarnessInput }).input;
        assert.deepEqual(payload, job.context.contribution_input, "contribution input differs from admission");
        assert.equal(job.contribution, undefined);
        const call = this.id("call");
        encodeContributionFrame(input as Parameters<typeof encodeContributionFrame>[0]);
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
        const target = writable();
        const append = input as ItemAppend;
        const receipt = commit(() => {
          assert.equal(args.expected_revision, target.revision, "stale head");
          const revision = this.id("rev");
          const prior = target.history.get(target.revision)!;
          const addition = append.attested ? compose(append.attested) : Buffer.from(append.bytes ?? "", "base64");
          assert.ok(addition.length <= 65536);
          const body = Buffer.concat([prior, addition]);
          target.history.set(revision, body);
          target.revision = revision;
          attributeWrite(target, revision, append.attested);
          return { revision, end_offset: body.length, receipt: requestId,
            ...(append.attested ? recordSpan(target, prior.length, addition.length) : {}) };
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
        this.preflight(set.harness, set.contribution_input);
        const alarm = this.id("alarm");
        this.alarms.set(alarm, structuredClone(set));
        return { alarm, receipt: requestId };
      });
    }
  }

  private head(item: string, file: File) {
    return { item, revision: file.revision, locator: { space: file.space, id: item }, bytes: file.history.get(file.revision)!.length,
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
        author_context: { request_id: id, actor: job.starter, authored_at: now, actor_label: personLabels[job.starter] }, job_context: job.context, ...(alarm && { alarm }) },
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
    if (file.people && this.conversationAccess(item, file.creator) !== "full") throw new Error("Conversation is read-only.");
    for (const job of this.jobs.values()) {
      if (job.context.item === item && ["pending", "running"].includes(job.state)) job.state = "stopped";
    }
    file.archived = true;
  }

  deliver(alarm: string, actual_at: string) {
    const armed = this.alarms.get(alarm);
    if (!armed) return;
    const { job, item, at, input, harness, grace_seconds, contribution_input } = armed;
    assert.ok(item && harness && contribution_input !== undefined);
    const delivery = this.admit({ job, item, input, harness, contribution_input });
    this.alarms.delete(alarm);
    return this.run(delivery.job_id, { alarm, scheduled_at: at, actual_at, missed: grace_seconds !== undefined && Date.parse(actual_at) > Date.parse(at) + 1000 * grace_seconds });
  }

  /** Materialize exact broker bytes for the report, without serializing them again. */
  async materialize(directory: string, item: string, person: string) {
    const file = this.files.get(item)!;
    assert.ok(file.path);
    const transcript = join(directory, file.path);
    const privateFile = join(directory, ".calternal-private", person, "threads", `${item}.md`);
    const state = this.personState.get(JSON.stringify([person, item]));
    assert.ok(state);
    await mkdir(dirname(transcript), { recursive: true });
    await mkdir(dirname(privateFile), { recursive: true });
    await writeFile(transcript, file.history.get(file.revision)!);
    await writeFile(privateFile, state);
    for (const message of decode(this.markdown(item)).messages) {
      if (!message.consultation) continue;
      const consultation = this.files.get(message.consultation.id)!;
      const path = join(directory, consultation.path!);
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, consultation.history.get(consultation.revision)!);
    }
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
