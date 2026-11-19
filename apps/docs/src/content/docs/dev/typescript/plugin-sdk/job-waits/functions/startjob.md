---
title: "plugin-sdk: job-waits/functions/startJob"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [job-waits](/docs/dev/typescript/plugin-sdk/job-waits/) / startJob

# Function: startJob()

> **startJob**(`context`, `input`): `Promise`\<\{ `job_id`: `string`; \}\>

Admit a durable job from a handler. Declare `start_job` in the manifest.
Returns the host-issued job ID after one broker round trip. An invalid
reply rejects the promise instead of returning an unchecked value.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `context` | [`WorkerContextV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/) |
| `input` | \{ `input`: `unknown`; `operation`: `string`; `parent_id?`: `string`; \} |
| `input.input` | `unknown` |
| `input.operation` | `string` |
| `input.parent_id?` | `string` |

## Returns

`Promise`\<\{ `job_id`: `string`; \}\>
