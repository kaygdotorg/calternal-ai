---
title: "plugin-sdk: worker-v2/functions/readPluginState"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / readPluginState

# Function: readPluginState()

> **readPluginState**(`context`, `key`): `Promise`\<[`PluginState`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/pluginstate/)\<`unknown`\> \| `null`\>

Read a private state key. Requires `read_plugin_state_v1` and `space.settings`.
Costs one bounded broker round trip.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `context` | [`WorkerContextV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/) |
| `key` | `string` |

## Returns

`Promise`\<[`PluginState`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/pluginstate/)\<`unknown`\> \| `null`\>
