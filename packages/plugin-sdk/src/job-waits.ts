import type { GenUiView } from "@calternal/contracts";

/** The single signal a parked job accepts before its deadline. */
export type JobWaitSource =
  | { kind: "deadline" }
  | { kind: "question"; form: GenUiView; schema_operation: string }
  | { kind: "approval" }
  | { kind: "webhook"; route: string; correlation: string };

/** Signal delivered with the checkpoint on the next invocation. */
export type JobSignal<Answer = unknown, Webhook = unknown> =
  | { kind: "deadline" }
  | { kind: "approval"; approved: boolean }
  | { kind: "answer"; value: Answer }
  | { kind: "webhook"; value: Webhook };

/** Checkpoint and acceptance window to persist as one job transition. */
export interface JobPark<Checkpoint = unknown> {
  /** Durable state for the next invocation. */
  checkpoint: Checkpoint;
  /** Displayed to the job's viewer. Keep sensitive values out of this text. */
  reason: string;
  /** Unix milliseconds within the next seven days. */
  deadline_ms: number;
  /** Signal's declaration and scope. */
  source: JobWaitSource;
}

/** Return this result from a replayable job handler. The host validates and parks
 * the job before releasing its run slot. O(1), with no I/O in the SDK. */
export function parkJob<Checkpoint>(wait: JobPark<Checkpoint>) {
  return { status: "parked" as const, wait };
}
