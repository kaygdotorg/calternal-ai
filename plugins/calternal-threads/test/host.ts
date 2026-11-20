import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join, posix } from "node:path";
import { fileURLToPath } from "node:url";
import type { AlarmDelivery, AlarmSet, AttestedBody, ExecutionJobContext, ExecutionJobStart, ExecutionStatus,
  ExecutionStopRequest, ExecutionStopResult, HostWorkerInvokeFrame, ItemAppend, ItemContentReceipt, ItemCreate, ItemSave,
  ItemSpansPage, ItemSpansRequest, ItemStageBegin, ItemStageChunk, ItemStageSeal, ItemViewPublish } from "../generated/wire.js";
import { encodeContributionFrame } from "../generated/contribution-frame.mjs";
import { authorContext } from "../generated/author-context.mjs";
import type { ManifestV2 } from "../generated/author.js";
import type { Context, RunInput } from "../src/interface.ts";
import type { HarnessInput, HarnessOutput } from "../src/harnesses-v1.ts";
import type { handlers as threadHandlers } from "../src/worker.ts";
import { scoped } from "../src/people.ts";
import { checkedTitle, decode, decodeIndexed } from "../src/format.ts";
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
  item_spans: ["ItemSpansRequest", "ItemSpansPage"],
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

/** Check the fake's revision 14.41 exchanges against the pinned schemas. */
export function checkTrace() {
  const check = fileURLToPath(new URL("../check-shapes.py", import.meta.url));
  const contract = fileURLToPath(new URL("../.build/contract/", import.meta.url));
  execFileSync("python3", ["-B", check, contract], { input: JSON.stringify(trace), stdio: ["pipe", "inherit", "inherit"] });
}

type SpanIndex = Pick<ItemSpansPage, "body_provenance" | "spans">;
type Span = ItemSpansPage["spans"][number];
type QuoteSource = NonNullable<AttestedBody["quote"]>;
type SpanBinding = { record: string; quotedRecord?: string; quoteSource?: QuoteSource };
type IndexedLink = { source: string; anchor: string; path: string; item: string };
type RelocationUndo = { item: string; title: string; folder: string[]; head: string; viewer: string };
type File = { space: string; people?: readonly string[]; creator: string; discovery: "creator_only" | "consent_only"; owner: "threads" | "claude-code-run"; history: Map<string, Buffer>; authors: Map<string, string>;
  spanHistory: Map<string, SpanIndex>; spans: Map<string, Span>; bindings: Map<string, SpanBinding>;
  withheldRevisions: Set<string>; provenance: SpanIndex["body_provenance"];
  revision: string; path?: string; archived: boolean; title: string; createdAt: string };
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

function titleFolder(createdAt: string, title: string) {
  let stem = "";
  for (const character of title.replace(/[^\p{L}\p{N} -]/gu, "_")) {
    if (Buffer.byteLength(stem + character) > 105) break;
    stem += character;
  }
  return `${createdAt.slice(0, 10)} ${stem}`;
}

function checkBoundTitle(body: Buffer, title: string, key: string) {
  const close = body.indexOf("\n---\n", 3);
  assert.ok(body.subarray(0, 4).equals(Buffer.from("---\n")) && close >= 0, "missing title metadata");
  const lines = body.subarray(4, close).toString().split("\n").filter(line => line.startsWith(`${key}:`));
  assert.deepEqual(lines, [`${key}: ${title}`], "incorrect title metadata");
}
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
const peopleMethods = new Set(["item_create", "item_head", "item_range", "item_spans", "item_save", "item_save_begin",
  "item_save_chunk", "item_save_seal", "item_append", "item_view_context", "item_view_publish"]);

/** One in-memory space. Validates plugin ownership, exact revisions and
 * attempt retirement around the calls used by the spike. */
export class FakeHost {
  files = new Map<string, File>();
  jobs = new Map<string, Job>();
  snapshots = new Map<string, ItemViewPublish>();
  alarms = new Map<string, AlarmSet>();
  receipts = new Map<string, { fingerprint: string; result: unknown }>();
  private stages = new Map<string, Stage>();
  private scopes = new Map<string, { item: string; revision: string }>();
  private spanPages = new Map<string, { item: string; revision: string; viewer: string; owner: Owner; after: number }>();
  private incomingLinks = new Map<string, IndexedLink[]>();
  private linksBySource = new Map<string, IndexedLink>();
  private relocationUndo = new Map<string, RelocationUndo>();
  private deferredRelocations = new Map<string, { activation: string; cursor: number; inverse: RelocationUndo }>();
  inlineBacklinkLimit = 63;
  linkedMarkdown = new Map<string, string>();
  private serial = 0;
  private selected = { harness: "checked-harness", placement: "checked-desk" };
  loseNextAppendAck = false;
  spanPageSize = 128;
  invitedInputs: InvitedInput[] = [];
  consentWaits: { authors: readonly string[]; seconds: number }[] = [];
  onConsentWait = async (_authors: readonly string[], _seconds: number): Promise<{ author: string; after: number }[]> => [];
  onInvited = async (_tools: { readMessage(id: string): { id: string; text: string } | undefined }) => {};
  consultationTurns: ConsultationTurn[] = [];
  consultationExchanges = 1;
  onConsultationTurn = async (_turn: ConsultationTurn) => {};
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

  // The plugin picker uses the checked group, separate from administrative visibility (ADR 0060).
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

  consultationContext(person: string, request: string, authored_at = now, signal = new AbortController().signal): PeopleContext {
    const host = this;
    return {
      ...this.peopleContext(person, request, authored_at),
      consultation: {
        lead: logicalHarness, consultant: { package: "fixture-codex", contribution: "codex" },
        signal,
        async *run() {
          for (let exchange = 0; exchange < host.consultationExchanges; exchange++) {
            for (const role of ["lead", "consultant"] as const) {
              const turn = { role, text: `Private exchange ${host.consultationTurns.length + 1}` };
              host.consultationTurns.push(turn);
              yield turn;
              await host.onConsultationTurn(turn);
              signal.throwIfAborted();
            }
          }
          const result = { role: "summary" as const, text: "Agreed recommendation" };
          host.consultationTurns.push(result);
          yield result;
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
    const chosen = new Map(scoped(this.thread(item).messages, scope).map(entry => [entry.id, entry]));
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
      waitForConsent: async (grant, authors, seconds) => {
        assert.equal(grant, invitation);
        this.consentWaits.push({ authors, seconds });
        const began = this.invitationClock(authored_at);
        for (const { author, after } of await this.onConsentWait(authors, seconds)) {
          if (authors.includes(author) && after >= 0 && after <= seconds) {
            grants.set(author, { ...permission(author), until: began + (after + 5 * 60) * 1000 });
          }
        }
      },
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
          const live = this.thread(item).messages.find(entry => entry.id === id);
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
    const compose = ({ source, prefix = "", suffix = "", quote }: AttestedBody, retiring?: string) => {
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
      let quoted: { text: string; record: string; source: QuoteSource; attribution: NonNullable<Span["quote"]>["attribution"] } | undefined;
      if (quote) {
        assert.ok(file && quote.item === args.item && file.history.has(quote.revision), "quote source unavailable");
        const origin = file.spanHistory.get(quote.revision)?.spans.find(candidate => candidate.span === quote.span);
        assert.ok(origin && file.spans.has(quote.span) && origin.middle_length > 0 && origin.attribution &&
          !file.withheldRevisions.has(quote.revision), "quote source unavailable");
        const record = file.bindings.get(origin.span)!.record;
        const copies = [...file.spans.values()].filter(entry => entry.span !== retiring &&
          file.bindings.get(entry.span)?.quotedRecord === record);
        assert.ok(copies.length < 64, "quote copy limit");
        const source = file.history.get(quote.revision)!.subarray(origin.middle_offset, origin.middle_offset + origin.middle_length);
        const block = source.toString().split("\n").map(line => `> ${line}`).join("\n") + "\n\n";
        assert.ok(Buffer.byteLength(block) <= 65536, "quote byte limit");
        quoted = { text: block, record, source: quote, attribution: origin.attribution };
      } else if (file && retiring && source.kind !== "empty") {
        const existing = file.spans.get(retiring)?.quote;
        if (existing) {
          const { quotedRecord, quoteSource } = file.bindings.get(retiring)!;
          assert.ok(quotedRecord && quoteSource && !file.withheldRevisions.has(quoteSource.revision), "quote cannot be retained");
          const bytes = file.history.get(file.revision)!;
          quoted = { text: bytes.subarray(existing.offset, existing.offset + existing.length).toString(),
            record: quotedRecord, source: quoteSource, attribution: existing.attribution };
        }
      }
      const bytes = Buffer.from(prefix + (quoted?.text ?? "") + text + suffix);
      return { bytes, prefixBytes: Buffer.byteLength(prefix), middleBytes: Buffer.byteLength(text), quoted };
    };
    const attributeWrite = (target: File, revision: string, attested?: AttestedBody) => {
      if (attested && attested.source.kind !== "empty") target.authors.set(revision, receiptActor);
      if (!attested || receiptActor !== target.creator || args.attempt_discovery === "consent_only") target.discovery = "consent_only";
    };
    const recordSpan = (target: File, offset: number, composed: ReturnType<typeof compose>, attested: AttestedBody, record?: string) => {
      if (!composed.bytes.length) return {};
      const span = this.id("span");
      const quoteLength = Buffer.byteLength(composed.quoted?.text ?? "");
      const attribution = attested.source.kind === "input" ? { kind: "person" as const, actor: receiptActor }
        : attested.source.kind === "output" ? { kind: "attempt" as const, attempt: job!.attempt!.execution }
        : undefined;
      target.spans.set(span, { span, offset, length: composed.bytes.length,
        middle_offset: offset + composed.prefixBytes + quoteLength, middle_length: composed.middleBytes, ...(attribution && { attribution }),
        ...(composed.quoted && { quote: { offset: offset + composed.prefixBytes, length: quoteLength, attribution: composed.quoted.attribution } }),
      });
      target.bindings.set(span, { record: record ?? span, quotedRecord: composed.quoted?.record, quoteSource: composed.quoted?.source });
      return { span };
    };
    const shiftAfter = (target: File, start: number, delta: number) => {
      for (const span of target.spans.values()) {
        if (span.offset <= start) continue;
        span.offset += delta;
        span.middle_offset += delta;
        if (span.quote) span.quote.offset += delta;
      }
    };
    const publishIndex = (target: File, revision: string, opaque = false) => {
      if (opaque) {
        target.provenance = "unproven";
        target.spans.clear();
      }
      target.spanHistory.set(revision, {
        body_provenance: target.provenance,
        spans: [...target.spans.values()].sort((a, b) => a.offset - b.offset).map(entry => structuredClone(entry)),
      });
    };
    switch (method) {
      case "item_create": return commit(() => {
        const create = input as ItemCreate;
        assert.ok(create.kind === owner || (owner === "threads" && create.kind === "thread-consultations"));
        const item = this.id("item");
        const revision = this.id("rev");
        const composed = create.attested && compose(create.attested);
        const body = composed ? composed.bytes : Buffer.from(create.body ?? "", "base64");
        const root = create.kind === "thread-consultations" ? ".calternal/thread-consultations" : owner === "threads" ? "Threads" : ".calternal";
        const binding = application.manifest.kinds.find(kind => kind.id === create.kind)?.file_format?.title_binding;
        const title = String(create.metadata.title);
        if (binding?.frontmatter_key) checkBoundTitle(body, title, binding.frontmatter_key);
        const folders = [...create.folders ?? []];
        if (binding?.placement === "utc_date_folder") folders.push(titleFolder(now, title));
        const used = new Set([...this.files.values()].map(file => file.path));
        let path = [root, ...folders, create.name].join("/");
        for (let suffix = 2; used.has(path); suffix++) {
          path = folders.length
            ? [root, ...folders.slice(0, -1), `${folders.at(-1)} (${suffix})`, create.name].join("/")
            : [root, `${suffix} ${create.name}`].join("/");
        }
        const discovery = create.attempt_discovery === "creator_only" && checkedInvocation && !shared && (create.attested || body.length === 0)
          ? "creator_only" : "consent_only";
        if (create.attempt_discovery === "creator_only") {
          assert.equal(application.manifest.kinds.find(kind => kind.id === create.kind)?.attempt_discovery, "item_policy");
        }
        const target: File = { owner, space: this.personalSpace(receiptActor), people: shared && [...shared], creator: receiptActor, discovery, revision, history: new Map([[revision, body]]),
          authors: new Map(), spans: new Map(), spanHistory: new Map(), bindings: new Map(), withheldRevisions: new Set(),
          provenance: "attested", path, archived: false, title, createdAt: now };
        const span = create.attested && composed ? recordSpan(target, 0, composed, create.attested) : {};
        if (create.attested) attributeWrite(target, revision, create.attested);
        publishIndex(target, revision, !create.attested && body.length !== 0);
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
        assert.ok(stage && offset >= 0 && offset <= stage.length);
        const chunk = Buffer.from(bytes, "base64");
        assert.ok(chunk.length > 0 && chunk.length <= 65536 && offset + chunk.length <= stage.spec.length);
        if (offset < stage.length) {
          assert.ok(offset + chunk.length <= stage.length && Buffer.concat(stage.chunks).subarray(offset, offset + chunk.length).equals(chunk),
            "conflicting staged chunk");
          return { next_offset: stage.length };
        }
        assert.ok(!stage.sealed);
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
            const replacement = compose(save.attested, save.replace_span);
            if (save.replace_span) {
              const prior = target.spans.get(save.replace_span);
              assert.ok(prior, "span unavailable");
              const { record } = target.bindings.get(prior.span)!;
              body = Buffer.concat([original.subarray(0, prior.offset), replacement.bytes, original.subarray(prior.offset + prior.length)]);
              target.spans.delete(save.replace_span);
              shiftAfter(target, prior.offset, replacement.bytes.length - prior.length);
              span = recordSpan(target, prior.offset, replacement, save.attested, record);
              if (save.attested.source.kind === "empty") {
                for (const dependent of target.spans.values()) {
                  const quote = dependent.quote;
                  const binding = target.bindings.get(dependent.span)!;
                  if (!quote || binding.quotedRecord !== record) continue;
                  body = Buffer.concat([body.subarray(0, quote.offset), body.subarray(quote.offset + quote.length)]);
                  dependent.length -= quote.length;
                  dependent.middle_offset -= quote.length;
                  delete dependent.quote;
                  shiftAfter(target, dependent.offset, -quote.length);
                }
                for (const [older, index] of target.spanHistory) {
                  if (index.spans.some(entry => {
                    const binding = target.bindings.get(entry.span)!;
                    return entry.attribution !== undefined &&
                      (binding.record === record || (entry.quote && binding.quotedRecord === record));
                  })) target.withheldRevisions.add(older);
                }
              }
            } else {
              body = replacement.bytes;
              target.spans.clear();
              span = recordSpan(target, 0, replacement, save.attested);
            }
          }
          if (save.staged !== undefined) this.stages.delete(save.staged);
          target.history.set(revision, body);
          target.revision = revision;
          if (typeof save.patch?.title === "string") target.title = save.patch.title;
          if (stage || save.body !== undefined || save.attested) attributeWrite(target, revision, save.attested);
          if (save.attempt_discovery === "consent_only") target.discovery = "consent_only";
          publishIndex(target, revision, Boolean(stage) || save.body !== undefined);
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
        assert.ok(!file.withheldRevisions.has(args.revision), "revision withheld after redaction");
        const body = file.history.get(args.revision);
        assert.ok(body, "obsolete range");
        const { offset, length } = input as { offset: number; length: number };
        assert.ok(offset >= 0 && length <= 65536);
        const page = body.subarray(offset, offset + length);
        const next = offset + page.length;
        return { bytes: page.toString("base64"), ...(next < body.length && { next_offset: next }) };
      }
      case "item_spans": {
        assert.equal(file?.owner, owner);
        const { item, revision, cursor } = input as ItemSpansRequest;
        assert.ok(!file.withheldRevisions.has(revision), "revision withheld after redaction");
        const index = file.spanHistory.get(revision);
        assert.ok(index, "span revision unavailable");
        const location = { item, revision, viewer: receiptActor, owner };
        let after = 0;
        if (cursor) {
          const pin = this.spanPages.get(cursor);
          assert.ok(pin, "invalid span cursor");
          after = pin.after;
          assert.deepEqual(pin, { ...location, after }, "wrong span cursor scope");
        }
        const spans = index.spans.slice(after, after + this.spanPageSize);
        const next = after + spans.length < index.spans.length ? this.id("spans-page") : undefined;
        if (next) this.spanPages.set(next, { ...location, after: after + spans.length });
        return { item, revision, body_provenance: index.body_provenance, spans, ...(next && { next }) };
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
          const composed = append.attested && compose(append.attested);
          const addition = composed ? composed.bytes : Buffer.from(append.bytes ?? "", "base64");
          assert.ok(addition.length <= 65536);
          const body = Buffer.concat([prior, addition]);
          target.history.set(revision, body);
          target.revision = revision;
          attributeWrite(target, revision, append.attested);
          const span = append.attested && composed ? recordSpan(target, prior.length, composed, append.attested) : {};
          publishIndex(target, revision, !append.attested);
          return { revision, end_offset: body.length, receipt: requestId,
            ...span };
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

  /** Register one indexed Markdown link for the generic rename fixture. */
  linkTo(item: string, source: string, anchor: string) {
    const destination = this.files.get(item)!.path!;
    this.linkedMarkdown.set(source, `[Read the conversation](<${posix.relative(posix.dirname(source), destination)}#${anchor}>)\n`);
    const link = { source, anchor, item, path: destination };
    const incoming = this.incomingLinks.get(item) ?? [];
    incoming.push(link);
    this.incomingLinks.set(item, incoming);
    this.linksBySource.set(source, link);
  }

  /** Resolve the indexed target even if the Markdown spelling is still waiting for rewrite. */
  followLink(source: string, viewer = actor) {
    const link = this.linksBySource.get(source);
    assert.ok(link);
    const file = this.files.get(link.item)!;
    assert.ok(file.people ? file.people.includes(viewer) : file.creator === viewer, "link unavailable");
    return { item: link.item, path: file.path, anchor: link.anchor };
  }

  private rewriteLinks(links: readonly IndexedLink[], path: string) {
    for (const link of links) {
      const stem = posix.dirname(link.source);
      const oldHref = `<${posix.relative(stem, link.path)}#${link.anchor}>`;
      const markdown = this.linkedMarkdown.get(link.source)!;
      if (!markdown.includes(oldHref)) continue;
      const newHref = `<${posix.relative(stem, path)}#${link.anchor}>`;
      this.linkedMarkdown.set(link.source, markdown.replace(oldHref, newHref));
      link.path = path;
    }
  }

  private relocationComplete(activation_id: string, inverse: RelocationUndo) {
    const ref = this.id("undo");
    this.relocationUndo.set(ref, inverse);
    const outcome = { activation_id, state: "ok" as const,
      undo: { ref, label: "Undo conversation move", expires_at: "2026-11-20T13:00:00Z" } };
    trace.push(["ActionOutcome", outcome]);
    return outcome;
  }

  /** Finish the deferred part of this move using the already committed target placement. */
  finishRelocation(status: string) {
    const job = this.deferredRelocations.get(status);
    assert.ok(job, "relocation status unavailable");
    const file = this.files.get(job.inverse.item)!;
    this.rewriteLinks((this.incomingLinks.get(job.inverse.item) ?? []).slice(job.cursor), file.path!);
    this.deferredRelocations.delete(status);
    return this.relocationComplete(job.activation, { ...job.inverse, head: file.revision });
  }

  /** Model folder placement and indexed link updates behind the generic host action. */
  async rename(item: string, title: string | undefined, activation_id: string, viewer = actor, destination?: string[]) {
    const file = this.files.get(item)!;
    if (file.people && this.conversationAccess(item, viewer) !== "full") throw new Error("Conversation is read-only.");
    assert.ok(title !== undefined || destination !== undefined);
    const nextTitle = title === undefined ? file.title : checkedTitle(title);
    const oldPath = file.path!;
    const priorTitle = file.title;
    const priorFolders = oldPath.split("/").slice(1, -2);
    const nextFolder = titleFolder(file.createdAt, nextTitle);
    const root = ["Threads", ...(destination ?? priorFolders), nextFolder].join("/");
    const occupied = new Set([...this.files.entries()].filter(([id]) => id !== item).map(([, sibling]) => sibling.path));
    let nextPath = `${root}/Conversation.md`;
    for (let suffix = 2; occupied.has(nextPath); suffix++) nextPath = `${root} (${suffix})/Conversation.md`;
    const links = this.incomingLinks.get(item) ?? [];
    const original = file.history.get(file.revision)!;
    const titleKey = application.manifest.kinds.find(kind => kind.id === "threads")?.file_format?.title_binding?.frontmatter_key;
    assert.ok(titleKey, "undeclared title binding");
    checkBoundTitle(original, file.title, titleKey);
    const oldTitleLine = Buffer.from(`\n${titleKey}: ${file.title}\n`);
    const newTitleLine = Buffer.from(`\n${titleKey}: ${nextTitle}\n`);
    const start = original.indexOf(oldTitleLine);
    assert.ok(start >= 0, "missing title field");
    const delta = newTitleLine.length - oldTitleLine.length;
    const bytes = Buffer.concat([original.subarray(0, start), newTitleLine, original.subarray(start + oldTitleLine.length)]);
    for (const span of file.spans.values()) {
      if (span.offset === 0) {
        span.length += delta;
        span.middle_length += delta;
        if (nextTitle !== priorTitle) span.attribution = { kind: "host" };
      } else {
        span.offset += delta;
        span.middle_offset += delta;
        if (span.quote) span.quote.offset += delta;
      }
    }
    const revision = this.id("rev");
    file.history.set(revision, bytes);
    file.spanHistory.set(revision, {
      body_provenance: file.provenance, spans: structuredClone([...file.spans.values()].sort((a, b) => a.offset - b.offset)),
    });
    file.revision = revision;
    file.path = nextPath;
    file.title = nextTitle;
    this.rewriteLinks(links.slice(0, this.inlineBacklinkLimit), nextPath);
    const snapshot = this.snapshots.get(item);
    if (snapshot) this.snapshots.set(item, { ...snapshot, source_revision: revision });
    const inverse = { item, title: priorTitle, folder: priorFolders, head: revision, viewer };
    if (links.length <= this.inlineBacklinkLimit) return this.relocationComplete(activation_id, inverse);
    const status_ref = this.id("relocation-status");
    this.deferredRelocations.set(status_ref, { activation: activation_id, cursor: this.inlineBacklinkLimit, inverse });
    const outcome = { activation_id, state: "pending" as const, status_ref };
    trace.push(["ActionOutcome", outcome]);
    return outcome;
  }

  /** Apply the generic inverse only for the same viewer and unchanged head. */
  async undoRename(ref: string, activation_id: string, viewer = actor) {
    const undo = this.relocationUndo.get(ref);
    assert.ok(undo?.viewer === viewer && this.files.get(undo.item)?.revision === undo.head, "stale relocation Undo");
    const result = await this.rename(undo.item, undo.title, activation_id, viewer, undo.folder);
    this.relocationUndo.delete(ref);
    return result;
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
    for (const message of this.thread(item).messages) {
      if (!message.consultation) continue;
      const consultation = this.files.get(message.consultation.id)!;
      const path = join(directory, consultation.path!);
      await mkdir(dirname(path), { recursive: true });
      await writeFile(path, consultation.history.get(consultation.revision)!);
    }
  }

  /** Decode the fixture's exact head using its committed index when available. */
  thread(item: string) {
    const file = this.files.get(item)!;
    const bytes = file.history.get(file.revision)!;
    const index = file.spanHistory.get(file.revision)!;
    return index.spans.length ? decodeIndexed(bytes, index.spans, index.body_provenance).thread : decode(bytes.toString());
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
