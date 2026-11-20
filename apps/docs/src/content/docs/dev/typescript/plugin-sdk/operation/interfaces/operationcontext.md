---
title: "plugin-sdk: operation/interfaces/OperationContext"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation](/docs/dev/typescript/plugin-sdk/operation/) / OperationContext

# Interface: OperationContext\<M\>

What a handler receives. `call` accepts only the methods its operation
declares, with catalogue-typed input and reply. `key` derives a
deterministic subeffect request ID under the invocation's stable request
ID, so a replay reuses every receipt.

## Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-m"></a> `M` *extends* `string` | [`BrokerMethod`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokermethod/) |

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="alarm"></a> `alarm?` | `readonly` | `AlarmDelivery` | Delivery evidence when an alarm admitted this job. |
| <a id="author"></a> `author` | `readonly` | `AuthorInvocationContext` | Stable request ID, actor and authored time stamped by the host. |
| <a id="job"></a> `job?` | `readonly` | `ExecutionJobContext` | Checked job metadata when this invocation runs an admitted job. |
| <a id="methods"></a> `methods` | `readonly` | `Readonly`\<`Record`\<`M`, `true`\>\> | The broker methods this operation declares. Also makes a context that lacks a method unassignable to a helper that needs it. |
| <a id="signal"></a> `signal?` | `readonly` | `AbortSignal` | Local cancellation; effects still require host reconciliation. |

## Methods

### call()

> **call**\<`K`\>(`method`, `input`): `Promise`\<[`BrokerOutput`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokeroutput/)\<`K`\>\>

Call a declared broker method. Cost: one bounded broker round trip.

#### Type Parameters

| Type Parameter |
| ------ |
| <a id="type-parameter-k"></a> `K` *extends* `"start_job"` \| `"item_create"` \| `"item_head"` \| `"item_range"` \| `"item_spans"` \| `"viewer_state_read"` \| `"viewer_state_write"` \| `"item_save"` \| `"item_save_begin"` \| `"item_save_chunk"` \| `"item_save_seal"` \| `"execution_start"` \| `"execution_status"` \| `"execution_input"` \| `"execution_spawn_child"` \| `"execution_stop"` \| `"execution_wait"` \| `"child_request"` \| `"execution_session_bind"` \| `"contribution_call"` \| `"contribution_read"` \| `"alarm_set"` \| `"alarm_cancel"` \| `"item_append"` \| `"item_view_context"` \| `"item_view_publish"` |

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `method` | `K` |
| `input` | [`BrokerInput`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokerinput/)\<`K`\> |

#### Returns

`Promise`\<[`BrokerOutput`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokeroutput/)\<`K`\>\>

***

### key()

> **key**(`purpose`): `string`

Deterministic request ID for one named subeffect of this invocation. Cost: one SHA-256.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `purpose` | `string` |

#### Returns

`string`
