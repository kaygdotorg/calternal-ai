---
title: "plugin-sdk: broker-helpers/functions/readBody"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / readBody

# Function: readBody()

> **readBody**(`ctx`, `item`, `revision`): `Promise`\<`Buffer`\<`ArrayBufferLike`\>\>

Read the whole body of one exact revision with as many `item_range` calls
as it needs (one per ITEM_RANGE_BYTES). Refuses a range that does not
advance or a body over ITEM_STAGED_BODY_BYTES. Cost: ceil(bytes / 64 KiB)
round trips and one copy of the body.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `ctx` | [`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`"item_range"`\> |
| `item` | `string` |
| `revision` | `string` |

## Returns

`Promise`\<`Buffer`\<`ArrayBufferLike`\>\>
