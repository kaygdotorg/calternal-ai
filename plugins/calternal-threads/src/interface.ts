import type { HarnessInput, HarnessOutput } from "./harnesses-v1.ts";
import type { AlarmDelivery, AlarmReceipt, AlarmSet, ExecutionJobContext, ExecutionJobReceipt, ExecutionJobStart, ExecutionStart,
  ExecutionStatus, ItemAppend, ItemAppendReceipt, ItemViewContext, ItemViewContextRequest, ItemViewPublish,
  ItemViewPublishReceipt, ItemRef, RevisionRef, AccountRef, HarnessRef, ChoiceRef, Timestamp } from "../generated/wire.js";

export type Send = { item: ItemRef; text: string; harness: HarnessRef; placement: ChoiceRef };
export type RunInput = { item: ItemRef; text: string; if_missed?: "run" | "skip" };
type FileHead = { item: ItemRef; revision: RevisionRef };
type NewFile = { kind: string; path?: string; metadata: { title: string } };
type Save = { request_id: string; bytes: string } &
  (NewFile | { item: ItemRef; expected_revision: RevisionRef; metadata: { title: string } });

// Draft §3 describes item bodies and broker metadata in prose. Save/range,
// author attribution and receipt delivery below are fixture assumptions.
type Methods = {
  item_head: [{ item: ItemRef }, FileHead];
  item_range: [{ item: ItemRef; revision: RevisionRef; offset: number; max_bytes: number },
    { bytes: string; next_offset?: number }];
  item_save: [Save, FileHead];
  start_job: [ExecutionJobStart, ExecutionJobReceipt];
  execution_start: [ExecutionStart, ExecutionStatus];
  item_append: [ItemAppend, ItemAppendReceipt];
  item_view_context: [ItemViewContextRequest, ItemViewContext];
  item_view_publish: [ItemViewPublish, ItemViewPublishReceipt];
  alarm_set: [AlarmSet, AlarmReceipt];
};

export interface Context {
  call<M extends keyof Methods>(method: M, input: Methods[M][0]): Promise<Methods[M][1]>;
  request: string;
  actor: AccountRef;
  now: Timestamp;
  job?: ExecutionJobContext;
  alarm?: AlarmDelivery;
  // Fixture-only port. Revision 14.30 has no harness reply delivery method.
  harnessResult?: (execution: string, input: HarnessInput) => Promise<HarnessOutput>;
}
