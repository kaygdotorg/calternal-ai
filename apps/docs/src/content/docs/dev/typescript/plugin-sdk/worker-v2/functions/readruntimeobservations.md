---
title: "plugin-sdk: worker-v2/functions/readRuntimeObservations"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / readRuntimeObservations

# Function: readRuntimeObservations()

> **readRuntimeObservations**(`context`, `after?`): `Promise`\<[`RuntimeObservationPage`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimeobservationpage/)\>

Read up to 64 observations. The manifest declares `read_runtime_observations_v1`
and the exact installed package needs the operator's resource binding.
Cost is one bounded broker round trip. Replays carry cumulative time.

## Parameters

| Parameter | Type | Default value |
| ------ | ------ | ------ |
| `context` | [`WorkerContextV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/) | `undefined` |
| `after?` | `string` | `"0"` |

## Returns

`Promise`\<[`RuntimeObservationPage`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/runtimeobservationpage/)\>
