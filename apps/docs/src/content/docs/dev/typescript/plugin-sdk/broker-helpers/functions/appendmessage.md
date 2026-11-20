---
title: "plugin-sdk: broker-helpers/functions/appendMessage"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / appendMessage

# Function: appendMessage()

> **appendMessage**(`ctx`, `message`): `Promise`\<[`MessageReceipt`](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/messagereceipt/)\>

Append one framed message, choosing the strongest write the input allows:
an attested append when `source` is given and the framing fits; otherwise a
whole-body rewrite when `rewrite` is given (inline or staged by size);
otherwise an opaque append. Refuses an addition over ITEM_APPEND_BYTES and
a rewrite over ITEM_STAGED_BODY_BYTES before any call. A head conflict on a
rewrite throws. Cost: one call, or `saveBody`'s cost for a rewrite.

Replays: the request ID is `ctx.key(purpose)`, so a replay with the same
input returns the original receipt. A replay that re-read a head this
append already moved sends a different `expected_revision` and the host
refuses it as a conflicting replay; look for your own committed record
(for example by its entry anchor) before appending again.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `ctx` | [`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`"item_save"` \| `"item_save_begin"` \| `"item_save_chunk"` \| `"item_save_seal"` \| `"item_append"`\> |
| `message` | [`MessageAppend`](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/messageappend/) |

## Returns

`Promise`\<[`MessageReceipt`](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/messagereceipt/)\>
