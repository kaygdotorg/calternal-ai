---
title: "plugin-sdk: worker-v2/interfaces/WorkerContextV2"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / WorkerContextV2

# Interface: WorkerContextV2

Safe broker/stream context passed to a TypeScript operation handler.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="signal"></a> `signal` | `AbortSignal` | Local cancellation signal; effects still require host reconciliation. |

## Methods

### call()

> **call**(`method`, `input`): `Promise`\<`unknown`\>

Cost: one bounded RPC round trip and pending promise; no direct I/O outside transport.
Call a declared broker method; the host rechecks live authority and targets.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `method` | [`HostCallKind`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/hostcallkind/) |
| `input` | `unknown` |

#### Returns

`Promise`\<`unknown`\>

***

### callAll()

> **callAll**(`calls`): `Promise`\<`unknown`[]\>

Submit independent calls together. Every call retains its own live host authorization and reply.
Costs one existing wire round trip per call. Limited to MAX_V2_PENDING entries.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `calls` | readonly (readonly \[[`HostCallKind`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/hostcallkind/), `unknown`\])[] |

#### Returns

`Promise`\<`unknown`[]\>

***

### chunk()

> **chunk**(`jobId`, `data`): `Promise`\<`void`\>

Cost: one bounded transport write, O(chunk bytes); waits for transport back pressure.
Emit one ordered chunk for a job admitted through start_job, or for the job a run
invocation names in `input.job_id`; await before emitting another.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `jobId` | `string` |
| `data` | `unknown` |

#### Returns

`Promise`\<`void`\>
