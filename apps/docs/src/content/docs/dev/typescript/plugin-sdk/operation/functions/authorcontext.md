---
title: "plugin-sdk: operation/functions/authorContext"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation](/docs/dev/typescript/plugin-sdk/operation/) / authorContext

# Function: authorContext()

> **authorContext**\<`M`\>(`message`, `methods`, `call`, `signal?`): [`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`M`\>

Adapt an authenticated, validated host invoke to the handler context. The
caller supplies the operation's declared methods and a broker `call`
already bound to that invocation. Freezes the stable author data.

## Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-m"></a> `M` *extends* `string` | [`BrokerMethod`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokermethod/) |

## Parameters

| Parameter | Type |
| ------ | ------ |
| `message` | \{ `alarm?`: `AlarmDelivery`; `author_context`: `AuthorInvocationContext`; `job_context?`: `ExecutionJobContext`; \} |
| `message.alarm?` | `AlarmDelivery` |
| `message.author_context` | `AuthorInvocationContext` |
| `message.job_context?` | `ExecutionJobContext` |
| `methods` | readonly `M`[] |
| `call` | \<`K`\>(`method`, `input`) => `Promise`\<[`BrokerOutput`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokeroutput/)\<`K`\>\> |
| `signal?` | `AbortSignal` |

## Returns

[`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`M`\>
