---
title: "plugin-sdk: broker-helpers/functions/publishView"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / publishView

# Function: publishView()

> **publishView**(`ctx`, `__namedParameters`): `Promise`\<`ItemViewPublishReceipt` \| `undefined`\>

Publish an item view page for exactly `revision`. Asks the host for the
writer scope first and publishes nothing when the head has moved on.
Checks the page against the snapshot bounds before sending. The request ID
is keyed by slot, revision and scope, so a replay reuses the receipt.
Cost: two calls plus `render`.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `ctx` | [`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`"item_view_context"` \| `"item_view_publish"`\> |
| `__namedParameters` | \{ `item`: `string`; `render`: () => \{ `anchor?`: `string`; `anchor_unavailable?`: `true`; `older?`: `string`; `view`: `ItemSnapshotView`; \} \| `Promise`\<\{ `anchor?`: `string`; `anchor_unavailable?`: `true`; `older?`: `string`; `view`: `ItemSnapshotView`; \}\>; `revision`: `string`; `slot`: `string`; \} |
| `__namedParameters.item` | `string` |
| `__namedParameters.render` | () => \{ `anchor?`: `string`; `anchor_unavailable?`: `true`; `older?`: `string`; `view`: `ItemSnapshotView`; \} \| `Promise`\<\{ `anchor?`: `string`; `anchor_unavailable?`: `true`; `older?`: `string`; `view`: `ItemSnapshotView`; \}\> |
| `__namedParameters.revision` | `string` |
| `__namedParameters.slot` | `string` |

## Returns

`Promise`\<`ItemViewPublishReceipt` \| `undefined`\>
