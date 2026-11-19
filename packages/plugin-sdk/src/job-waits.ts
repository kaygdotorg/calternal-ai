import type { GenUiView } from "@calternal/contracts";
import type { WorkerContextV2 } from "./worker-v2.js";

/** Admit a durable job from a handler. Declare `start_job` in the manifest.
 * Returns the host-issued job ID after one broker round trip. An invalid
 * reply rejects the promise instead of returning an unchecked value.
 */
export async function startJob(
  context: WorkerContextV2,
  input: { operation: string; input: unknown; parent_id?: string },
): Promise<{
  /** Host-issued ID for reading, cancelling, or reporting progress on this job. */
  job_id: string;
}> {
  const reply = await context.call("start_job", input);
  if (!reply || typeof reply !== "object" || !("job_id" in reply) || typeof reply.job_id !== "string") {
    throw new Error("Invalid start_job reply");
  }
  return { job_id: reply.job_id };
}

/** The single signal a parked job accepts before its deadline. */
export type JobWaitSource =
  | {
      /** Resume when the deadline passes. */
      kind: "deadline";
    }
  | {
      /** Wait for the job's recipient to answer the Inbox form. */
      kind: "question";
      /** One action-free GenUI Form displayed to the job's recipient. */
      form: GenUiView;
      /** Declared package operation whose input schema validates the answer. */
      schema_operation: string;
    }
  | {
      /** Wait for a one-time decision from the job's recipient. */
      kind: "approval";
    }
  | {
      /** Wait for a signed delivery to a package-declared route. */
      kind: "webhook";
      /** Manifest webhook ID in this package. */
      route: string;
      /** Fresh UUID that identifies this wait to the sender. */
      correlation: string;
    };

/** Signal delivered with the checkpoint on the next invocation.
 * @typeParam Answer - Payload accepted by the question's declared schema.
 * @typeParam Webhook - Payload accepted by the webhook's declared schema.
 */
export type JobSignal<Answer = unknown, Webhook = unknown> =
  | {
      /** The deadline passed before the job accepted a signal. */
      kind: "deadline";
    }
  | {
      /** The person made a one-time decision about this job. */
      kind: "approval";
      /** Whether the person approved continuing with the job. */
      approved: boolean;
    }
  | {
      /** The person answered the Inbox form. */
      kind: "answer";
      /** The validated answers for its declared fields. */
      value: Answer;
    }
  | {
      /** The host accepted a signed webhook delivery. */
      kind: "webhook";
      /** The validated payload of the accepted delivery. */
      value: Webhook;
    };

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

/** Completed run output asking the host to park a durable job. */
export interface ParkedJob<Checkpoint = unknown> {
  /** Selects the host's durable park transition. */
  status: "parked";
  /** Checkpoint and signal contract for the waiting job. */
  wait: JobPark<Checkpoint>;
}

/** Return this result from a replayable job handler. The host validates and parks
 * the job before releasing its run slot. O(1), with no I/O in the SDK.
 * @param wait - Checkpoint and signal contract validated by the host.
 * @returns A job completion envelope to return to the host.
 */
export function parkJob<Checkpoint>(wait: JobPark<Checkpoint>): ParkedJob<Checkpoint> {
  return { status: "parked", wait };
}
