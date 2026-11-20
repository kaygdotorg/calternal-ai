/**
 * The fake host's execution engine: checked harness choices, job admission,
 * attempts, contribution calls and reads, alarms, Stop and archive fences.
 * In memory and deterministic; contribution entries run in-process.
 */
import { isDeepStrictEqual } from "node:util";
import type {
  AlarmReceipt, AlarmSet, ContributionCall, ContributionCallReceipt, ContributionFrame, ContributionIdentity, ContributionRead,
  ContributionReadResult, ExecutionJobReceipt, ExecutionJobStart, ExecutionStart, ExecutionStatus, ExecutionStopResult, ManifestV2,
} from "../../generated/author-types.js";
import { encodeContributionFrame, LimitError, LIMITS } from "../limits.js";
import type { ManifestOperation } from "../operation.js";
import { ensure, HostRefusal, notDeclared } from "./refusal.js";
import type { ContractSchemas } from "./schemas.js";
import type { InstalledPackage, Invocation, Job } from "./invocation.js";

const { ALARM_GRACE_SECONDS_DEFAULT, ALARMS_PENDING_PER_PACKAGE, CONTRIBUTION_READ_FRAMES, REF_CHARACTERS } = LIMITS;

type Point = NonNullable<ManifestV2["apps"][number]["extension_points"]>[number];

/** A resolved harness choice: the contributing package and the point it fills. */
export interface Choice {
  /** Logical contribution identity. */
  readonly identity: ContributionIdentity;
  /** The contributing package. */
  readonly contributor: InstalledPackage;
  /** The contributing operation that runs as the entry. */
  readonly operation: ManifestOperation;
  /** The extension point the contribution fills. */
  readonly point: Point;
  /** Package ID of the app that owns the point. */
  readonly owner: string;
}

/** Checked refs the renderer's agent and machine prompts would return. */
export interface HarnessChoice {
  /** Checked HarnessRef. */
  readonly harness: string;
  /** Checked placement ChoiceRef. */
  readonly placement: string;
}

/** One pending alarm. */
export interface PendingAlarm {
  /** The alarm as set. */
  readonly set: AlarmSet;
  /** Package that set it. */
  readonly package: string;
  /** Account that set it; the admitted job acts as this account. */
  readonly actor: string;
}

/** Runs one contribution entry in-process and yields its output data. */
export type EntryRunner = (job: Job, call: string, input: unknown, choice: Choice) => AsyncIterable<unknown>;

// A Ref allows REF_CHARACTERS code points of up to six JSON bytes each.
const widestRef = "\u0000".repeat(REF_CHARACTERS);

/** Execution engine state and broker methods. */
export class Execution {
  /** Admitted jobs by ref. */
  readonly jobs = new Map<string, Job>();
  /** Pending alarms by ref. */
  readonly alarms = new Map<string, PendingAlarm>();
  /** Awaited between a contribution's last output and its terminal frame; tests use it to Stop mid-call. */
  beforeTerminal: (job: Job) => Promise<void> = async () => {};
  /** Real milliseconds a contribution_read may wait; the host caps the requested wait at this. */
  readWaitMs = 1000;
  private choices = new Map<string, Choice>();
  private placements = new Set<string>();
  private wake = new Set<() => void>();

  /** Create an engine using the host's ID source, package list, schemas and entry runner. */
  constructor(
    private id: (prefix: string) => string,
    private packages: () => readonly InstalledPackage[],
    private schemas: ContractSchemas,
    private runEntry: EntryRunner,
  ) {}

  private notify() {
    for (const wake of [...this.wake]) wake();
  }

  /**
   * Issue a checked harness ref and placement ref for one installed harness
   * contribution, as the renderer's extension choice and placement prompt
   * would. Issuing again revokes the previous harness ref for that
   * contribution. Checks that the contributing operation's schemas equal the
   * point's, as install would.
   */
  choose(contribution?: string): HarnessChoice {
    const found = this.packages().flatMap(contributor => (contributor.manifest.contributions ?? [])
      .filter(entry => entry.kind === "harness" && (!contribution || entry.id === contribution))
      .map(entry => ({ contributor, entry })));
    ensure(found.length === 1, 404, "no_contribution", `Expected one installed harness contribution${contribution ? ` named ${contribution}` : ""}, found ${found.length}.`);
    const { contributor, entry } = found[0]!;
    const owner = this.packages().find(pkg => pkg.manifest.id === entry.app.package);
    const point = owner?.manifest.apps.find(app => app.id === entry.app.app)?.extension_points?.find(candidate => candidate.id === entry.point);
    ensure(owner && point && point.version === entry.version, 404, "no_point", `Extension point ${entry.app.package}/${entry.app.app}/${entry.point} v${entry.version} is not installed.`);
    const operation = contributor.manifest.operations.find(op => op.id === entry.operation);
    ensure(operation && isDeepStrictEqual(operation.input_schema, point.input_schema) && isDeepStrictEqual(operation.output_schema, point.output_schema),
      400, "point_mismatch", `Contribution ${entry.id} must declare exactly the point's input and output schemas.`);
    const identity = { package: contributor.manifest.id, contribution: entry.id };
    for (const [ref, choice] of this.choices) if (isDeepStrictEqual(choice.identity, identity)) this.choices.delete(ref);
    const harness = this.id("harness");
    const placement = this.id("placement");
    this.choices.set(harness, { identity, contributor, operation, point, owner: owner.manifest.id });
    this.placements.add(placement);
    return { harness, placement };
  }

  private checkedChoice(inv: Invocation, harness: string, placement: string | undefined, contributionInput: unknown): Choice {
    const choice = this.choices.get(harness);
    ensure(choice && choice.owner === inv.pkg.manifest.id, 403, "unchecked_harness", "That agent choice is not a current checked option (unchecked harness).");
    ensure(placement === undefined || this.placements.has(placement), 403, "unchecked_placement", "That machine choice is not a current checked option (unchecked placement).");
    this.schemas.checkDeclared(`${choice.owner}/point/${choice.point.id}/input`, choice.point.input_schema, contributionInput, "The contribution input");
    try {
      encodeContributionFrame({ request_id: widestRef, execution: widestRef, contribution: harness, input: contributionInput });
      encodeContributionFrame({ call: widestRef, input: contributionInput });
    } catch (error) {
      if (error instanceof LimitError) throw new HostRefusal(413, "contribution_frame_limit", error.message);
      throw error;
    }
    return choice;
  }

  private target(inv: Invocation, declared: string, item: string | undefined, input: Record<string, unknown>, need: "execution" | "alarm") {
    const jobs = inv.pkg.manifest.jobs ?? [];
    const index = jobs.findIndex(job => job.id === declared);
    if (index < 0) throw notDeclared(`Job ${declared}`, "jobs", `Declare { id: "${declared}", run_operation, replayable: true } in jobs.`);
    const job = jobs[index]!;
    if (need === "execution" && !job.execution) throw notDeclared(`Execution for job ${declared}`, `jobs[${index}].execution`, "Set execution: true.");
    if (need === "alarm" && !job.alarm) throw notDeclared(`Alarms for job ${declared}`, `jobs[${index}].alarm`, "Set alarm: true.");
    const run = inv.pkg.manifest.operations.find(op => op.id === job.run_operation);
    const field = run?.target && "field" in run.target ? run.target.field : undefined;
    ensure(!field || !Object.hasOwn(input, field), 400, "duplicate_target", `The job input must not repeat the target field ${field}; the host supplies it.`);
    return { job, field };
  }

  /** `start_job` for an execution job: admission, checked choices and the per-item queue bound. */
  startJob(inv: Invocation, input: ExecutionJobStart, items: { executionTarget(inv: Invocation, item: string): void }): ExecutionJobReceipt {
    this.target(inv, input.job, input.item, input.input, "execution");
    items.executionTarget(inv, input.item);
    const choice = this.checkedChoice(inv, input.harness, input.placement, input.contribution_input);
    const active = [...this.jobs.values()].filter(job => job.item === input.item && (job.state === "pending" || job.state === "running"));
    ensure(active.length < 2, 409, "pending_limit", "One job may run and one wait per item (pending_limit).");
    return { job_id: this.admit(inv.pkg.manifest.id, input.job, input.item, inv.actor, input.harness, choice, input.contribution_input, input.input, inv.captured).id };
  }

  private admit(pkg: string, declared: string, item: string, starter: string, harness: string, choice: Choice, contributionInput: unknown,
    input: Record<string, unknown>, captured?: Job["captured"]): Job {
    const job: Job = {
      id: this.id("job"), package: pkg, job: declared, item, starter, harness, identity: choice.identity,
      contributionInput: structuredClone(contributionInput), input: structuredClone(input), ...(captured && { captured }),
      state: "pending", teardown: true, outputs: new Map(),
    };
    this.jobs.set(job.id, job);
    return job;
  }

  /** `execution_start`: one attempt for the running job and its admitted harness. */
  executionStart(inv: Invocation, input: ExecutionStart): ExecutionStatus {
    const job = inv.job;
    ensure(job, 403, "no_job", "execution_start runs only inside an admitted execution job.");
    ensure(input.harness === job.harness, 409, "harness_mismatch", "The harness must match the admitted job.");
    job.teardown = false;
    job.attempt ??= { execution: this.id("attempt"), phase: "running", transition_cursor: this.id("transition") };
    return job.attempt;
  }

  /** `contribution_call`: the pinned input to the admitted harness, once per attempt; starts the entry. */
  contributionCall(inv: Invocation, input: ContributionCall): ContributionCallReceipt {
    const job = inv.job;
    ensure(job?.attempt && input.execution === job.attempt.execution && input.contribution === job.harness, 403, "wrong_attempt", "The call must name this job's attempt and harness.");
    ensure(isDeepStrictEqual(input.input, job.contributionInput), 409, "input_changed", "The contribution input differs from admission.");
    ensure(!job.call, 409, "call_exists", "This attempt already made its contribution call.");
    encodeContributionFrame(input);
    const call = { id: this.id("call"), input: input.input, frames: [] as ContributionFrame[], done: false };
    job.call = call;
    const choice = this.choices.get(job.harness) ?? [...this.choices.values()].find(entry => isDeepStrictEqual(entry.identity, job.identity))!;
    void (async () => {
      let seq = 0;
      const live = () => job.state === "running";
      try {
        for await (const data of this.runEntry(job, call.id, input.input, choice)) {
          if (!live()) return;
          this.schemas.checkDeclared(`${choice.owner}/point/${choice.point.id}/output`, choice.point.output_schema, data, "The contribution output");
          const frame = { seq: ++seq, kind: "output" as const, data };
          encodeContributionFrame(frame);
          call.frames.push(frame);
          this.notify();
        }
        await this.beforeTerminal(job);
        if (live()) call.frames.push({ seq: ++seq, kind: "terminal", state: "ok" });
      } catch {
        if (live()) call.frames.push({ seq: ++seq, kind: "terminal", state: "failed" });
      } finally {
        call.done = true;
        this.notify();
      }
    })();
    return { call: call.id };
  }

  /** `contribution_read`: committed frames after `after`, waiting up to `wait_ms` for more. */
  async contributionRead(inv: Invocation, input: ContributionRead): Promise<ContributionReadResult> {
    const job = inv.job;
    const call = job?.call;
    ensure(job && call && call.id === input.call, 403, "wrong_call", "Only the owning job may read this call.");
    const after = input.after ?? 0;
    const ready = () => call.frames.length > after || call.done || job.state !== "running";
    if (!ready() && input.wait_ms) {
      await new Promise<void>(resolve => {
        const wake = () => { if (ready()) finish(); };
        const timer = setTimeout(() => finish(), Math.min(input.wait_ms!, this.readWaitMs));
        const finish = () => { clearTimeout(timer); this.wake.delete(wake); resolve(); };
        this.wake.add(wake);
      });
    }
    ensure(job.state === "running", 409, "attempt_retired", "This attempt was stopped (attempt retired).");
    const frames = call.frames.slice(after, after + CONTRIBUTION_READ_FRAMES);
    for (const frame of frames) if (frame.kind === "output") job.outputs.set(frame.seq, frame.data);
    return { frames: structuredClone(frames) };
  }

  /** `alarm_set`: a pending alarm for a declared alarm job, with checked choices when it starts execution. */
  alarmSet(inv: Invocation, input: AlarmSet, items: { executionTarget(inv: Invocation, item: string): void }): AlarmReceipt {
    const { job } = this.target(inv, input.job, input.item, input.input, "alarm");
    if (input.harness !== undefined) {
      if (!job.execution) throw notDeclared(`Execution for job ${input.job}`, "jobs", "Set execution: true to start a harness from an alarm.");
      items.executionTarget(inv, input.item!);
      this.checkedChoice(inv, input.harness, undefined, input.contribution_input);
    }
    if (input.replace) {
      ensure(this.alarms.get(input.replace)?.package === inv.pkg.manifest.id, 404, "alarm_unavailable", "The alarm to replace is unavailable.");
      this.alarms.delete(input.replace);
    }
    const pending = [...this.alarms.values()].filter(alarm => alarm.package === inv.pkg.manifest.id).length;
    ensure(pending < ALARMS_PENDING_PER_PACKAGE, 409, "alarm_limit", `A package may hold ${ALARMS_PENDING_PER_PACKAGE} pending alarms per space.`);
    const alarm = this.id("alarm");
    this.alarms.set(alarm, { set: structuredClone(input), package: inv.pkg.manifest.id, actor: inv.actor });
    return { alarm, receipt: input.request_id };
  }

  /** `alarm_cancel`: drop a pending alarm this package set. */
  alarmCancel(inv: Invocation, input: { request_id: string; alarm: string }): AlarmReceipt {
    ensure(this.alarms.get(input.alarm)?.package === inv.pkg.manifest.id, 404, "alarm_unavailable", "That alarm is unavailable.");
    this.alarms.delete(input.alarm);
    return { alarm: input.alarm, receipt: input.request_id };
  }

  /** Consume a pending alarm into one admitted job, as delivery would. Returns undefined when it is gone. */
  deliver(alarm: string, actual_at: string): Job | undefined {
    const pending = this.alarms.get(alarm);
    if (!pending) return undefined;
    this.alarms.delete(alarm);
    const { set } = pending;
    const grace = set.grace_seconds ?? ALARM_GRACE_SECONDS_DEFAULT;
    const delivery = { alarm, scheduled_at: set.at, actual_at, missed: Date.parse(actual_at) > Date.parse(set.at) + grace * 1000 };
    ensure(set.harness && set.item, 501, "not_implemented", "The fake host delivers only execution alarms.");
    const choice = this.choices.get(set.harness);
    ensure(choice, 403, "unchecked_harness", "The alarm's agent choice is no longer current (unchecked harness).");
    const job = this.admit(pending.package, set.job, set.item, pending.actor, set.harness, choice, set.contribution_input, set.input);
    job.alarm = delivery;
    return job;
  }

  /** Check that `job` may start now: it is the item's head job and the previous attempt tore down. */
  startable(job: Job): void {
    const head = [...this.jobs.values()].find(candidate => candidate.item === job.item && (candidate.state === "pending" || candidate.state === "running"));
    ensure(head === job, 409, "successor_waits", "A successor must wait for the head job (successor must wait).");
    ensure(job.state === "pending", 409, "not_pending", "That job is not pending.");
    ensure(this.choices.has(job.harness), 409, "stale_harness", "The admitted agent choice was replaced (stale harness).");
    ensure([...this.jobs.values()].every(candidate => candidate.item !== job.item || candidate.teardown), 409, "teardown_pending",
      "The previous attempt has not torn down (successor must wait for teardown).");
  }

  /** Stop one exact attempt or queued job on `item`. Refuses a stale target. */
  stop(item: string, target: { execution: string } | { intent: string }): ExecutionStopResult {
    const job = "intent" in target ? this.jobs.get(target.intent)
      : [...this.jobs.values()].find(candidate => candidate.attempt?.execution === target.execution);
    ensure(job?.item === item && (job.state === "pending" || job.state === "running"), 409, "stale_stop", "That attempt is no longer active (stale Stop).");
    job.state = "stopped";
    this.notify();
    return job.attempt ? { ...job.attempt, phase: "stopped", transition_cursor: this.id("transition") } : { intent: job.id, phase: "stopped" };
  }

  /** Record that a stopped attempt finished tearing down, so a successor may start. */
  settle(jobId: string): void {
    const job = this.jobs.get(jobId);
    ensure(job?.state === "stopped", 409, "not_stopped", "Only a stopped job settles.");
    job.teardown = true;
  }

  /** Stop every pending or running job on an archived item. */
  archive(item: string): void {
    for (const job of this.jobs.values()) if (job.item === item && (job.state === "pending" || job.state === "running")) job.state = "stopped";
    this.notify();
  }
}
