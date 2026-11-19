---
title: "plugin-sdk: worker-v2/functions/readRuntimeWindow"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / readRuntimeWindow

# Function: readRuntimeWindow()

> **readRuntimeWindow**(`context`, `sinceMs`, `untilMs?`): `Promise`\<[`RuntimeWindow`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimewindow/)\>

Read a retained runtime window. Declares `read_runtime_window_v1` and needs
an operator resource binding. Pass `untilMs` for a closed interval. An
unresolved count means a clock could still add time to that interval.
Costs one bounded broker round trip.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `context` | [`WorkerContextV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/) |
| `sinceMs` | `number` |
| `untilMs?` | `number` |

## Returns

`Promise`\<[`RuntimeWindow`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimewindow/)\>
