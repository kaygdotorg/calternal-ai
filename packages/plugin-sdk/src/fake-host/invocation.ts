/** Shared fake-host state types for one invocation and one admitted job. */
import type { AlarmDelivery, ContributionFrame, ContributionIdentity, ExecutionStatus, ManifestV2 } from "../../generated/author-types.js";
import type { ManifestOperation, OperationHandler } from "../operation.js";

/** One installed package: its manifest and the handlers its worker exports. */
export interface InstalledPackage {
  /** The package manifest, as emitted or as `definePlugin` assembles it. */
  readonly manifest: ManifestV2;
  /** Handlers by operation ID. */
  readonly handlers: Readonly<Record<string, OperationHandler>>;
}

/** Text a host-owned composer or editor captured for the invocation's `write_origins.host_input` field. */
export interface Captured {
  /** The operation input field the host captured. */
  readonly field: string;
  /** The captured text. */
  readonly text: string;
}

/** A contribution call made by one attempt and the frames its entry produced. */
export interface ContributionCallState {
  /** Call ref. */
  readonly id: string;
  /** Pinned input. */
  readonly input: unknown;
  /** Committed frames in sequence order. */
  readonly frames: ContributionFrame[];
  /** Whether the entry has finished. */
  done: boolean;
}

/** One admitted execution job and, once started, its attempt and contribution call. */
export interface Job {
  /** Job ref (ExecutionIntentRef). */
  readonly id: string;
  /** Owning package. */
  readonly package: string;
  /** Declared job ID. */
  readonly job: string;
  /** Target item. */
  readonly item: string;
  /** Account that started the job; its run invocations act as this account. */
  readonly starter: string;
  /** Checked harness ref the job was admitted with. */
  readonly harness: string;
  /** Logical contribution the harness ref resolved to. */
  readonly identity: ContributionIdentity;
  /** Pinned contribution input. */
  readonly contributionInput: unknown;
  /** Input for the run operation, without the target field. */
  readonly input: Record<string, unknown>;
  /** Captured submission inherited from the invocation that admitted the job. */
  readonly captured?: Captured;
  /** Lifecycle state. */
  state: "pending" | "running" | "done" | "stopped";
  /** Whether the previous attempt has torn down, so a successor may start. */
  teardown: boolean;
  /** The attempt, once execution_start admitted one. */
  attempt?: ExecutionStatus;
  /** The contribution call, once made. */
  call?: ContributionCallState;
  /** Output data delivered to this attempt by sequence, for output-attested writes. */
  readonly outputs: Map<number, unknown>;
  /** Alarm evidence when an alarm admitted the job. */
  alarm?: AlarmDelivery;
}

/** One running invocation of a declared operation. */
export interface Invocation {
  /** Package whose operation runs. */
  readonly pkg: InstalledPackage;
  /** The declared operation. */
  readonly operation: ManifestOperation;
  /** Its index in `manifest.operations`, for diagnostics. */
  readonly index: number;
  /** Acting account. */
  readonly actor: string;
  /** Stable request ID. */
  readonly request_id: string;
  /** Captured submission, when the host captured one. */
  readonly captured?: Captured;
  /** The job this invocation runs, if any. */
  readonly job?: Job;
  /** The job whose attempt runs this contribution entry; Stop fences its calls. */
  readonly attempt?: Job;
  /** Items created in this invocation, for the `creates` check. */
  readonly created: Set<string>;
}
