/**
 * An in-process v2 host for plugin tests: installs packages from their
 * manifests and worker handlers, dispatches operations with a typed context,
 * and runs every broker call through the catalogue schemas, the operation's
 * declarations, request receipts and the item and execution engines.
 *
 * What it proves: schema-valid broker traffic, declared methods, kinds, jobs,
 * targets and slots, exact-revision writes, attested composition and spans,
 * receipt replay, staged saves, viewer-state compare-and-set, the job queue,
 * Stop fences, alarms and contribution framing. What it does not prove: real
 * sessions and tickets, live authority (roles, grants, sharing, guests,
 * revocation), audiences, quotas and rate limits beyond sizes, durability and
 * crash recovery, renderer behavior, real harness images, egress, and the
 * host's exact path normalization. Qualify those on the real host.
 */
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import type { AlarmDelivery, ExecutionStopResult, ManifestV2 } from "../../generated/author-types.js";
import { BROKER_METHODS } from "../generated/broker-methods.js";
import { authorContext, handlersOf, type BrokerMethod, type ManifestOperation, type OperationContext, type OperationHandler } from "../operation.js";
import { Execution, type Choice, type HarnessChoice } from "./execution.js";
import type { Captured, InstalledPackage, Invocation, Job } from "./invocation.js";
import { Items, type StoredItem } from "./items.js";
import { ensure, HostRefusal, notDeclared } from "./refusal.js";
import { ContractSchemas } from "./schemas.js";

/** One app as `definePlugin` returns it. */
export interface AuthoredApp {
  /** The manifest app entry without its pinned description. */
  readonly metadata: Omit<ManifestV2["apps"][number], "description">;
}

/** A package as `definePlugin` returns it, before the build pins assets. */
export interface AuthoredPackage {
  /** Manifest fields other than apps and worker. */
  readonly metadata: Omit<ManifestV2, "apps" | "worker">;
  /** Apps with their templates. */
  readonly apps: readonly AuthoredApp[];
}

/** Construction options. */
export interface FakeHostOptions {
  /** Fixed host clock used for authored_at and dated placement. Default 2026-11-20T12:00:00Z. */
  now?: string;
  /** Display names of synthetic accounts; the first acts by default. Default Alice, Bob and Cara. */
  people?: readonly string[];
}

/** Per-invocation choices for `invoke` and `context`. */
export interface InvokeOptions {
  /** Acting account; defaults to the first person. */
  actor?: string;
  /** Stable request ID; defaults to a fresh one. Reuse it to replay. */
  request_id?: string;
  /** Treat the operation's `write_origins.host_input` field as a host-captured composer or editor submission. */
  captured?: boolean;
}

/** One recorded broker call, for assertions. */
export interface BrokerCall {
  /** Calling package. */
  readonly package: string;
  /** Calling operation. */
  readonly operation: string;
  /** Broker method. */
  readonly method: string;
  /** Input as sent. */
  readonly input: unknown;
  /** Reply, when the call succeeded. */
  output?: unknown;
  /** Refusal message, when it failed. */
  error?: string;
}

/** One committed request receipt. */
export interface Receipt {
  /** Canonical JSON of the input the receipt was committed for. */
  readonly fingerprint: string;
  /** The committed reply. */
  readonly result: unknown;
}

type Failure = "lose_ack" | HostRefusal;

/** Load an emitted package directory: `manifest.json` and the pinned worker module's `handlers` export. */
export async function loadPackage(directory: string): Promise<InstalledPackage> {
  const manifest = JSON.parse(await readFile(join(directory, "manifest.json"), "utf8")) as ManifestV2;
  const worker = manifest.worker ? await import(pathToFileURL(join(directory, manifest.worker.path)).href) as { handlers?: Record<string, OperationHandler> } : {};
  return { manifest, handlers: worker.handlers ?? {} };
}

function installed(entry: InstalledPackage | AuthoredPackage): InstalledPackage {
  if ("manifest" in entry) return entry;
  const apps = entry.apps.map(app => ({ ...app.metadata, description: { id: "app", template: { path: "unbuilt.json", sha256: "0".repeat(64) } } }));
  return { manifest: { ...entry.metadata, apps } as ManifestV2, handlers: handlersOf(entry.metadata.operations) };
}

/** The in-process fake host. See the module comment for what it does and does not prove. */
export class FakeHost {
  /** Synthetic accounts by display name. */
  readonly people: Readonly<Record<string, string>>;
  /** Every broker call in order. */
  readonly calls: BrokerCall[] = [];
  /** Committed request receipts by package, actor, method and request ID. */
  readonly receipts = new Map<string, Receipt>();
  /** The item engine; tune `spanPageSize` or `mentions` here. */
  readonly items: Items;
  /** The execution engine; set `beforeTerminal` or `readWaitMs` here. */
  readonly execution: Execution;
  /** The single fake space. */
  readonly space = "019a0000-0000-7000-8000-000000000010";
  private packages: InstalledPackage[];
  private schemas = ContractSchemas.get();
  private serial = 0;
  private extras = new Map<string, Record<string, unknown>>();
  private failures = new Map<string, Failure>();
  private labels = new Map<string, string>();
  private now: string;

  /** Install packages from emitted manifests or straight from `definePlugin` output, which needs no build. */
  constructor(packages: readonly (InstalledPackage | AuthoredPackage)[], options: FakeHostOptions = {}) {
    this.packages = packages.map(installed);
    this.now = options.now ?? "2026-11-20T12:00:00Z";
    const names = options.people ?? ["Alice", "Bob", "Cara"];
    this.people = Object.fromEntries(names.map((name, index) => [name, `019a0000-0000-7000-8000-${String(index + 1).padStart(12, "0")}`]));
    for (const [name, id] of Object.entries(this.people)) this.labels.set(id, name);
    this.items = new Items(prefix => this.id(prefix), () => this.now, this.space);
    this.execution = new Execution(prefix => this.id(prefix), () => this.packages, this.schemas, (job, call, input, choice) => this.entry(job, call, input, choice));
  }

  /** Load emitted package directories and install them. */
  static async load(directories: readonly string[], options?: FakeHostOptions): Promise<FakeHost> {
    return new FakeHost(await Promise.all(directories.map(loadPackage)), options);
  }

  private id(prefix: string) { return `${prefix}-${++this.serial}`; }

  private find(operationId: string) {
    for (const pkg of this.packages) {
      const index = pkg.manifest.operations.findIndex(op => op.id === operationId);
      if (index >= 0) return { pkg, operation: pkg.manifest.operations[index]!, index };
    }
    throw new HostRefusal(404, "unknown_operation", `No installed package declares operation ${operationId}.`);
  }

  /** Merge test-only properties, such as a scripted provider, into every context of one package. */
  provide(packageId: string, extras: Record<string, unknown>): void {
    this.extras.set(packageId, { ...this.extras.get(packageId), ...extras });
  }

  /**
   * Make the next call of `method` refuse before any effect, or commit and
   * then lose its acknowledgement. `packageId` limits it to one package.
   */
  failNext(method: BrokerMethod, failure: Failure, packageId?: string): void {
    this.failures.set(packageId === undefined ? method : `${packageId}/${method}`, failure);
  }

  /** Issue a checked harness and placement for an installed harness contribution. */
  choose(contribution?: string): HarnessChoice {
    return this.execution.choose(contribution);
  }

  private invocation(operationId: string, input: unknown, options: InvokeOptions, job?: Job): Invocation {
    const { pkg, operation, index } = this.find(operationId);
    const actor = job?.starter ?? options.actor ?? Object.values(this.people)[0]!;
    let captured: Captured | undefined = job?.captured;
    if (options.captured) {
      const field = operation.write_origins?.host_input;
      if (!field) throw notDeclared("A captured submission", `operations[${index}].write_origins.host_input`, "Name the input field the host captures.");
      const text = (input as Record<string, unknown>)[field];
      ensure(typeof text === "string", 400, "invalid_capture", `Captured field ${field} must be a string.`);
      captured = { field, text };
    }
    return { pkg, operation, index, actor, request_id: options.request_id ?? job?.id ?? this.id("request"), created: new Set(), ...(captured && { captured }), ...(job && { job }) };
  }

  private contextFor<M extends string>(inv: Invocation, alarm?: AlarmDelivery): OperationContext<M> & Record<string, unknown> {
    const label = this.labels.get(inv.actor);
    const message = {
      author_context: { request_id: inv.request_id, actor: inv.actor, authored_at: this.now, ...(label && { actor_label: label }) },
      ...(inv.job && { job_context: { intent: inv.job.id, item: inv.job.item, harness: inv.job.harness, contribution_identity: inv.job.identity, contribution_input: structuredClone(inv.job.contributionInput) } }),
      ...(alarm && { alarm }),
    };
    const call = ((method: BrokerMethod, input: unknown) => this.dispatch(inv, method, input)) as OperationContext<M>["call"];
    const ctx = authorContext(message, inv.operation.methods as unknown as M[], call);
    return { ...ctx, ...this.extras.get(inv.pkg.manifest.id) } as OperationContext<M> & Record<string, unknown>;
  }

  private async execute(inv: Invocation, input: unknown, ctx: OperationContext): Promise<unknown> {
    const { pkg, operation } = inv;
    const handler = pkg.handlers[operation.id];
    ensure(handler, 501, "no_handler", `Package ${pkg.manifest.id} exports no handler for ${operation.id}.`);
    const output = await handler(input, ctx);
    this.schemas.checkDeclared(`${pkg.manifest.id}/${operation.id}/output`, operation.output_schema, output, `The ${operation.id} result`);
    if (operation.creates) {
      const created = (output as { created?: string }).created;
      ensure(created && inv.created.has(created) && this.items.items.get(created)?.kind.id === operation.creates, 400, "unrecorded_created",
        `The ${operation.id} result must name the ${operation.creates} item this invocation created.`);
    }
    return output;
  }

  /**
   * Invoke a declared operation as the host would after a renderer action:
   * check the input schema and the declared target, stamp the author context
   * and check the result schema and `creates`. Returns the result.
   */
  async invoke(operationId: string, input: unknown, options: InvokeOptions = {}): Promise<unknown> {
    const inv = this.invocation(operationId, input, options);
    this.schemas.checkDeclared(`${inv.pkg.manifest.id}/${operationId}/input`, inv.operation.input_schema, input, `The ${operationId} input`);
    const target = inv.operation.target;
    if (target && "field" in target) {
      const item = this.items.items.get((input as Record<string, string>)[target.field]!);
      ensure(item && item.kind.id === target.kind && !item.archived, 404, "target_unavailable", "The target is unavailable (target unavailable).");
    }
    return this.execute(inv, input, this.contextFor(inv));
  }

  /** A typed context for calling a handler or the broker directly as `operationId` would, without schema-checking an input. */
  context<M extends string = BrokerMethod>(operationId: string, options: InvokeOptions & { input?: unknown; job?: string } = {}): OperationContext<M> & Record<string, unknown> {
    const job = options.job === undefined ? undefined : this.jobs.get(options.job);
    if (options.job !== undefined) ensure(job, 404, "no_job", "Unknown job.");
    return this.contextFor<M>(this.invocation(operationId, options.input ?? {}, options, job), job?.alarm);
  }

  /** Admitted jobs by ref. */
  get jobs(): ReadonlyMap<string, Job> { return this.execution.jobs; }

  /** Run a pending job's run operation as its starter, once it heads the item's queue. */
  async run(jobId: string): Promise<unknown> {
    const job = this.jobs.get(jobId);
    ensure(job, 404, "no_job", "Unknown job.");
    this.execution.startable(job);
    const pkg = this.packages.find(entry => entry.manifest.id === job.package)!;
    const declared = pkg.manifest.jobs!.find(entry => entry.id === job.job)!;
    const run = pkg.manifest.operations.find(op => op.id === declared.run_operation);
    if (!run) throw notDeclared(`Run operation ${declared.run_operation}`, "operations", "Declare the job's run_operation.");
    const field = run.target && "field" in run.target ? run.target.field : undefined;
    const input = { ...job.input, ...(field && { [field]: job.item }) };
    const inv = this.invocation(run.id, input, {}, job);
    this.schemas.checkDeclared(`${pkg.manifest.id}/${run.id}/input`, run.input_schema, input, `The ${run.id} input`);
    job.state = "running";
    try {
      return await this.execute(inv, input, this.contextFor(inv, job.alarm));
    } finally {
      if (job.state === "running") {
        job.state = "done";
        job.teardown = true;
      }
    }
  }

  /** Run a finished job again with the same request ID, as recovery after a lost acknowledgement would. */
  async replay(jobId: string): Promise<unknown> {
    const job = this.jobs.get(jobId);
    ensure(job?.state === "done", 409, "not_done", "Only a finished job replays.");
    job.state = "pending";
    return this.run(jobId);
  }

  /** Stop an exact attempt or queued job, as the host's Stop control does. */
  stop(item: string, target: { execution: string } | { intent: string }): ExecutionStopResult { return this.execution.stop(item, target); }

  /** Mark a stopped attempt torn down. */
  settle(jobId: string): void { this.execution.settle(jobId); }

  /** Deliver a pending alarm at `actual_at` and run the job it admits. Returns undefined when the alarm is gone. */
  async deliver(alarm: string, actual_at: string): Promise<unknown> {
    const job = this.execution.deliver(alarm, actual_at);
    return job ? this.run(job.id) : undefined;
  }

  /** Archive an item, as the generic host action does: stop its jobs and refuse later writes and admission. */
  archive(item: string): void {
    const stored = this.items.items.get(item);
    ensure(stored, 404, "not_found", "Item unavailable.");
    stored.archived = true;
    this.execution.archive(item);
  }

  /** One stored item, for assertions. */
  item(ref: string): StoredItem {
    const stored = this.items.items.get(ref);
    ensure(stored, 404, "not_found", `No item ${ref}.`);
    return stored;
  }

  /** The head bytes of an item as UTF-8 text. */
  text(ref: string): string {
    const stored = this.item(ref);
    return stored.revisions.get(stored.head)!.toString();
  }

  /** The published page of one item view slot, if any. */
  view(item: string, slot: string) { return this.items.views.get(`${item}/${slot}`); }

  /** Items attempts started by `person` may discover without consent. */
  discoverable(person: string): string[] {
    return [...this.items.items.values()].filter(item => item.creator === person && item.discovery === "creator_only").map(item => item.id);
  }

  private executionTarget(inv: Invocation, item: string): void {
    const stored = this.items.writable({ ...inv, job: undefined }, item);
    const index = inv.pkg.manifest.kinds.findIndex(kind => kind.id === stored.kind.id);
    if (!stored.kind.execution_target) throw notDeclared(`Execution on kind ${stored.kind.id}`, `kinds[${index}].execution_target`, "Set execution_target: true on the kind.");
  }

  private checkMetadata = (kind: ManifestV2["kinds"][number], metadata: unknown) => {
    const owner = this.packages.find(pkg => pkg.manifest.kinds.includes(kind))!;
    this.schemas.checkDeclared(`${owner.manifest.id}/kind/${kind.id}`, kind.schema ?? true, metadata, `Metadata for kind ${kind.id}`);
  };

  private async effect(inv: Invocation, method: string, input: any): Promise<unknown> {
    const items = this.items;
    switch (method) {
      case "item_create": {
        const receipt = items.create(inv, input, this.checkMetadata);
        inv.created.add(receipt.item);
        return receipt;
      }
      case "item_head": return items.head(items.readable(inv, input.item));
      case "item_range": return items.range(inv, input);
      case "item_spans": return items.spansPage(inv, input);
      case "item_append": return items.append(inv, input);
      case "item_save": return items.save(inv, input, this.checkMetadata);
      case "item_save_begin": return items.begin(inv, input);
      case "item_save_chunk": return items.chunk(inv, input);
      case "item_save_seal": return items.seal(inv, input);
      case "viewer_state_read": return items.viewerRead(inv, input.item);
      case "viewer_state_write": return items.viewerWrite(inv, input);
      case "item_view_context": return items.viewContext(inv, input);
      case "item_view_publish": return items.publish(inv, input);
      case "start_job": return this.execution.startJob(inv, input, { executionTarget: (i, item) => this.executionTarget(i, item) });
      case "execution_start": return this.execution.executionStart(inv, input);
      case "contribution_call": return this.execution.contributionCall(inv, input);
      case "contribution_read": return this.execution.contributionRead(inv, input);
      case "alarm_set": return this.execution.alarmSet(inv, input, { executionTarget: (i, item) => this.executionTarget(i, item) });
      case "alarm_cancel": return this.execution.alarmCancel(inv, input);
      default: throw new HostRefusal(501, "not_implemented", `The fake host does not implement ${method} yet.`);
    }
  }

  private async dispatch(inv: Invocation, method: BrokerMethod, input: unknown): Promise<unknown> {
    const record: BrokerCall = { package: inv.pkg.manifest.id, operation: inv.operation.id, method, input: structuredClone(input) };
    this.calls.push(record);
    try {
      const fence = inv.job ?? inv.attempt;
      if (fence) ensure(fence.state === "running", 409, "attempt_retired", "This attempt was stopped (attempt retired).");
      if (!(inv.operation.methods as readonly string[]).includes(method)) {
        throw notDeclared(`Method ${method}`, `operations[${inv.index}].methods`, `Add "${method}" to the methods of ${inv.operation.id}.`);
      }
      const exchange = (BROKER_METHODS as Record<string, { input: string; output: string }>)[method];
      ensure(exchange, 501, "not_pinned", `The pinned contract has no exchange for ${method}.`);
      this.schemas.check(exchange.input, input, `The ${method} input`);
      const scoped = `${inv.pkg.manifest.id}/${method}`;
      const failureKey = this.failures.has(scoped) ? scoped : method;
      const failure = this.failures.get(failureKey);
      if (failure instanceof HostRefusal) {
        this.failures.delete(failureKey);
        throw failure;
      }
      const request = (input as { request_id?: string }).request_id ?? (method === "start_job" ? inv.request_id : undefined);
      const key = request === undefined ? undefined : JSON.stringify([inv.pkg.manifest.id, inv.actor, method, request]);
      const fingerprint = JSON.stringify(input);
      const saved = key === undefined ? undefined : this.receipts.get(key);
      let output: unknown;
      if (saved) {
        ensure(saved.fingerprint === fingerprint, 409, "request_conflict", "This request ID was used with different input (conflicting replay).");
        output = structuredClone(saved.result);
        // An exact replay of a create still names this invocation's created item.
        if (method === "item_create") inv.created.add((output as { item: string }).item);
      } else {
        output = await this.effect(inv, method, input);
        this.schemas.check(exchange.output, output, `The ${method} reply`);
        const conflict = output && typeof output === "object" && "code" in output;
        if (key !== undefined && !conflict) this.receipts.set(key, { fingerprint, result: structuredClone(output) });
        // A replayed begin of a committed staged save returns the save's receipt.
        const begin = method === "item_save" && (input as { staged?: string }).staged !== undefined && !conflict
          ? this.receipts.get(JSON.stringify([inv.pkg.manifest.id, inv.actor, "item_save_begin", request])) : undefined;
        if (begin) this.receipts.set(JSON.stringify([inv.pkg.manifest.id, inv.actor, "item_save_begin", request]), { ...begin, result: structuredClone(output) });
      }
      if (failure === "lose_ack") {
        this.failures.delete(failureKey);
        throw new HostRefusal(503, "ack_lost", "The connection dropped after the host committed the call (acknowledgement lost).");
      }
      record.output = output;
      return structuredClone(output);
    } catch (error) {
      record.error = error instanceof Error ? error.message : String(error);
      throw error;
    }
  }

  private async *entry(job: Job, call: string, input: unknown, choice: Choice): AsyncIterable<unknown> {
    const inv: Invocation = {
      pkg: choice.contributor, operation: choice.operation, index: choice.contributor.manifest.operations.indexOf(choice.operation),
      actor: job.starter, request_id: call, created: new Set(), attempt: job,
    };
    const ctx = this.contextFor(inv);
    const handler = choice.contributor.handlers[choice.operation.id];
    ensure(handler, 501, "no_handler", `Package ${choice.contributor.manifest.id} exports no handler for ${choice.operation.id}.`);
    const result = await handler(input, ctx);
    if (result && typeof result === "object" && Symbol.asyncIterator in result) yield* result as AsyncIterable<unknown>;
    else yield result;
  }
}
