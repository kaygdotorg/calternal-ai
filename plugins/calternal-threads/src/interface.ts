import type { AlarmDelivery, AlarmReceipt, AlarmSet, AuthorInvocationContext, ContributionCall, ContributionCallReceipt,
  ContributionRead, ContributionReadResult, ExecutionJobContext, ExecutionJobReceipt, ExecutionJobStart, ExecutionStart,
  ExecutionStatus, ItemAppend, ItemAppendReceipt, ItemViewContext, ItemViewContextRequest, ItemViewPublish,
  ItemViewPublishReceipt, ItemContentReceipt, ItemCreate, ItemHeadRequest, OwnedItemHead, ItemRange, ItemRangeResult,
  ItemSave, ItemSaveReply, ItemRef, HarnessRef, ChoiceRef } from "../generated/wire.js";

export type Send = { item: ItemRef; text: string; harness: HarnessRef; placement: ChoiceRef };
export type RunInput = { item: ItemRef; text: string; if_missed?: "run" | "skip" };

type Methods = {
  item_create: [ItemCreate, ItemContentReceipt];
  item_head: [ItemHeadRequest, OwnedItemHead];
  item_range: [ItemRange, ItemRangeResult];
  item_save: [ItemSave, ItemSaveReply];
  start_job: [ExecutionJobStart, ExecutionJobReceipt];
  execution_start: [ExecutionStart, ExecutionStatus];
  contribution_call: [ContributionCall, ContributionCallReceipt];
  contribution_read: [ContributionRead, ContributionReadResult];
  item_append: [ItemAppend, ItemAppendReceipt];
  item_view_context: [ItemViewContextRequest, ItemViewContext];
  item_view_publish: [ItemViewPublish, ItemViewPublishReceipt];
  alarm_set: [AlarmSet, AlarmReceipt];
};

export interface Context {
  call<M extends keyof Methods>(method: M, input: Methods[M][0]): Promise<Methods[M][1]>;
  author: AuthorInvocationContext;
  job?: ExecutionJobContext;
  alarm?: AlarmDelivery;
}
