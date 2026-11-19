---
title: "plugin-sdk: worker-v2/classes/WorkerRuntimeV2"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / WorkerRuntimeV2

# Class: WorkerRuntimeV2

Long-lived peer for one plugin/space pair. Transport send promises must apply back pressure.
The sandbox must deny ambient storage, network and credentials and enforce resource limits.

## Constructors

### Constructor

> **new WorkerRuntimeV2**(`manifest`, `handlers`, `send`): `WorkerRuntimeV2`

Cost: O(menu bytes), bounded state allocation, no I/O. Validate the menu and bind declared handlers to a bounded transport.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `manifest` | `unknown` |
| `handlers` | `Record`\<`string`, [`WorkerHandlerV2`](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workerhandlerv2/)\> |
| `send` | (`frame`) => `Promise`\<`void`\> |

#### Returns

`WorkerRuntimeV2`

## Methods

### assertReady()

> **assertReady**(): `void`

Verify that every declared operation has a handler before activation. Cost: O(operations).

#### Returns

`void`

***

### receive()

> **receive**(`frame`): `Promise`\<`void`\>

Consume one host frame without waiting for handler completion, allowing cancellation and replies.
Cost: O(frame bytes), bounded maps; handlers run asynchronously and each effect uses a broker RPC.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `frame` | [`WorkerFrameV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workerframev2/) |

#### Returns

`Promise`\<`void`\>

When admission/control processing has completed; handler results use the send callback.
