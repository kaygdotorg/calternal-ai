---
title: "plugin-sdk: generated/limits/interfaces/Limits"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [generated/limits](/docs/dev/typescript/plugin-sdk/generated/limits/) / Limits

# Interface: Limits

Every numeric limit an author can hit, each read from one contract pointer.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="alarm_grace_seconds_default"></a> `ALARM_GRACE_SECONDS_DEFAULT` | `readonly` | `60` | Grace period the host applies when an alarm omits one. Source: `catalogue#/$defs/AlarmSet/properties/grace_seconds/default`. |
| <a id="alarm_grace_seconds_max"></a> `ALARM_GRACE_SECONDS_MAX` | `readonly` | `86400` | Longest grace period an alarm may declare before its delivery counts as missed. Source: `catalogue#/$defs/AlarmSet/properties/grace_seconds/maximum`. |
| <a id="alarm_input_bytes"></a> `ALARM_INPUT_BYTES` | `readonly` | `16384` | Largest encoded alarm input. Source: `catalogue#/alarms/limits/input_bytes`. |
| <a id="alarms_pending_per_package"></a> `ALARMS_PENDING_PER_PACKAGE` | `readonly` | `32` | Most pending alarms one package may hold in one space. Source: `catalogue#/alarms/limits/pending_per_package_per_space`. |
| <a id="attested_framing_characters"></a> `ATTESTED_FRAMING_CHARACTERS` | `readonly` | `512` | Longest attested prefix or suffix; the SDK also refuses framing over this many UTF-8 bytes. Source: `catalogue#/$defs/AttestedBody/properties/prefix/maxLength`. |
| <a id="contribution_frame_bytes"></a> `CONTRIBUTION_FRAME_BYTES` | `readonly` | `16384` | Largest encoded contribution call, entry invoke or output frame, counting JSON plus one LF. Source: `catalogue#/extension_points/calls/limits/encoded_frame_bytes`. |
| <a id="contribution_read_frames"></a> `CONTRIBUTION_READ_FRAMES` | `readonly` | `16` | Most frames one contribution_read returns. Source: `catalogue#/$defs/ContributionReadResult/properties/frames/maxItems`. |
| <a id="contribution_read_wait_ms"></a> `CONTRIBUTION_READ_WAIT_MS` | `readonly` | `1000` | Longest wait one contribution_read may request. Source: `catalogue#/$defs/ContributionRead/properties/wait_ms/maximum`. |
| <a id="item_append_bytes"></a> `ITEM_APPEND_BYTES` | `readonly` | `65536` | Largest decoded addition one item_append call commits, including attested framing, quote and middle. Source: `catalogue#/file_append/max_decoded_bytes`. |
| <a id="item_folders"></a> `ITEM_FOLDERS` | `readonly` | `4` | Most parent folder components an item_create may name. Source: `catalogue#/$defs/ItemCreate/properties/folders/maxItems`. |
| <a id="item_inline_body_bytes"></a> `ITEM_INLINE_BODY_BYTES` | `readonly` | `131072` | Largest body, in decoded bytes, that item_create or item_save accepts inline; stage anything larger. Source: `catalogue#/$defs/ItemSave/properties/body/maxLength`. |
| <a id="item_metadata_fields"></a> `ITEM_METADATA_FIELDS` | `readonly` | `64` | Most metadata fields one item may carry. Source: `catalogue#/$defs/ItemCreate/properties/metadata/maxProperties`. |
| <a id="item_range_bytes"></a> `ITEM_RANGE_BYTES` | `readonly` | `65536` | Largest length one item_range read returns. Source: `catalogue#/$defs/ItemRange/properties/length/maximum`. |
| <a id="item_spans_per_page"></a> `ITEM_SPANS_PER_PAGE` | `readonly` | `128` | Most recorded spans one item_spans page returns; follow `next` for the rest. Source: `catalogue#/$defs/ItemSpansPage/properties/spans/maxItems`. |
| <a id="item_stage_chunk_bytes"></a> `ITEM_STAGE_CHUNK_BYTES` | `readonly` | `65536` | Largest decoded chunk one item_save_chunk call carries. Source: `catalogue#/$defs/ItemStageChunk/properties/bytes/maxLength`. |
| <a id="item_staged_body_bytes"></a> `ITEM_STAGED_BODY_BYTES` | `readonly` | `2097152` | Largest whole item body, in bytes, a staged save accepts; this is also the size bound of one item. Source: `catalogue#/$defs/ItemStageBegin/properties/length/maximum`. |
| <a id="job_input_fields"></a> `JOB_INPUT_FIELDS` | `readonly` | `32` | Most fields in the input a job or alarm passes to its run operation. Source: `catalogue#/$defs/ExecutionJobStart/properties/input/maxProperties`. |
| <a id="methods_per_operation"></a> `METHODS_PER_OPERATION` | `readonly` | `64` | Most broker methods one operation may declare. Source: `manifest#/definitions/operation/properties/methods/maxItems`. |
| <a id="operations_per_package"></a> `OPERATIONS_PER_PACKAGE` | `readonly` | `128` | Most operations one manifest may declare. Source: `manifest#/properties/operations/maxItems`. |
| <a id="quote_bytes"></a> `QUOTE_BYTES` | `readonly` | `65536` | Largest emitted quote block one attested record may carry. Source: `catalogue#/item_engine/quotes/limits/bytes_per_quote`. |
| <a id="quote_copies_per_source"></a> `QUOTE_COPIES_PER_SOURCE` | `readonly` | `64` | Most live quote copies of one source record across all destinations. Source: `catalogue#/item_engine/quotes/limits/copies_per_source`. |
| <a id="ref_characters"></a> `REF_CHARACTERS` | `readonly` | `128` | Longest opaque reference, including request IDs and subeffect keys. Source: `catalogue#/$defs/Ref/maxLength`. |
| <a id="snapshot_text_characters"></a> `SNAPSHOT_TEXT_CHARACTERS` | `readonly` | `2000` | Most Unicode code points in one snapshot-text node; the page byte bound applies separately. Source: `description#/$defs/SnapshotText/properties/text/maxLength`. |
| <a id="view_snapshot_bytes"></a> `VIEW_SNAPSHOT_BYTES` | `readonly` | `65536` | Largest encoded item view page a worker may publish or return. Source: `catalogue#/view_snapshots/bounds/max_encoded_bytes`. |
| <a id="view_snapshot_depth"></a> `VIEW_SNAPSHOT_DEPTH` | `readonly` | `6` | Deepest nesting of one item view page. Source: `catalogue#/view_snapshots/bounds/max_depth`. |
| <a id="view_snapshot_nodes"></a> `VIEW_SNAPSHOT_NODES` | `readonly` | `200` | Most nodes in one item view page. Source: `catalogue#/view_snapshots/bounds/max_nodes`. |
| <a id="viewer_fields"></a> `VIEWER_FIELDS` | `readonly` | `16` | Most viewer-private fields one item may hold for one account. Source: `catalogue#/$defs/ViewerItemState/properties/fields/maxProperties`. |
