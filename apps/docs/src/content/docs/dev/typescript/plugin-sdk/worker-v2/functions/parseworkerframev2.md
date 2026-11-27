---
title: "plugin-sdk: worker-v2/functions/parseWorkerFrameV2"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / parseWorkerFrameV2

# Function: parseWorkerFrameV2()

> **parseWorkerFrameV2**(`input`): [`WorkerFrameV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workerframev2/)

Parse a bounded strict UTF-8 JSON frame and reject unknown versions, fields and variants.
Cost: O(frame bytes), allocates decoded fields; no I/O.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `input` | `string` \| `Uint8Array`\<`ArrayBufferLike`\> |

## Returns

[`WorkerFrameV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workerframev2/)

A validated frame; authorization is the host broker's responsibility.
