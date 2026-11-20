---
title: "plugin-sdk: broker-helpers/functions/saveBody"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / saveBody

# Function: saveBody()

> **saveBody**(`ctx`, `__namedParameters`): `Promise`\<`ItemSaveReply`\>

Replace a body inline when it fits ITEM_INLINE_BODY_BYTES, otherwise stage
it in ITEM_STAGE_CHUNK_BYTES chunks and save the sealed stage under the
same request ID. Returns the receipt or the host's head conflict. A replayed
staged save returns the original receipt without sending chunks again.
Cost: one call inline; staged, three plus one per chunk, and one SHA-256.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `ctx` | [`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`"item_save"` \| `"item_save_begin"` \| `"item_save_chunk"` \| `"item_save_seal"`\> |
| `__namedParameters` | [`BodySave`](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/bodysave/) |

## Returns

`Promise`\<`ItemSaveReply`\>
