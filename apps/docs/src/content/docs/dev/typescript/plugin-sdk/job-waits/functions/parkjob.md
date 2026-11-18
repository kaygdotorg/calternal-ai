---
title: "plugin-sdk: job-waits/functions/parkJob"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [job-waits](/docs/dev/typescript/plugin-sdk/job-waits/) / parkJob

# Function: parkJob()

> **parkJob**\<`Checkpoint`\>(`wait`): [`ParkedJob`](/docs/dev/typescript/plugin-sdk/job-waits/interfaces/parkedjob/)\<`Checkpoint`\>

Return this result from a replayable job handler. The host validates and parks
the job before releasing its run slot. O(1), with no I/O in the SDK.

## Type Parameters

| Type Parameter |
| ------ |
| <a id="type-parameter-checkpoint"></a> `Checkpoint` |

## Parameters

| Parameter | Type | Description |
| ------ | ------ | ------ |
| `wait` | [`JobPark`](/docs/dev/typescript/plugin-sdk/job-waits/interfaces/jobpark/)\<`Checkpoint`\> | Checkpoint and signal contract validated by the host. |

## Returns

[`ParkedJob`](/docs/dev/typescript/plugin-sdk/job-waits/interfaces/parkedjob/)\<`Checkpoint`\>

A job completion envelope to return to the host.
