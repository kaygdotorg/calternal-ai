---
title: "plugin-sdk: operation-api/interfaces/RunHandle"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / RunHandle

# Interface: RunHandle

Handle for a run in the current operation.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="id"></a> `id` | `string` | Host-assigned run ID. |
| <a id="status"></a> `status` | `string` | Status snapshot returned when the run was opened. |

## Methods

### send()

> **send**(`input`): `Promise`\<`void`\>

Send input as the run actor, subject to the host check.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `input` | `unknown` |

#### Returns

`Promise`\<`void`\>

***

### stop()

> **stop**(): `Promise`\<`void`\>

Stop the run if the current caller may read the target.

#### Returns

`Promise`\<`void`\>
