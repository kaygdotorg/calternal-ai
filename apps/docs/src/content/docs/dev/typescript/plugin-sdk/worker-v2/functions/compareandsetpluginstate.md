---
title: "plugin-sdk: worker-v2/functions/compareAndSetPluginState"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / compareAndSetPluginState

# Function: compareAndSetPluginState()

> **compareAndSetPluginState**(`context`, `key`, `base`, `schema`, `value`): `Promise`\<`string`\>

Replace a private state key under a revision fence. Requires `cas_plugin_state_v1`
and a mutating operation with `space.settings`. Use one replacement per key
per request. The value and retry receipt commit together.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `context` | [`WorkerContextV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/) |
| `key` | `string` |
| `base` | `string` \| `null` |
| `schema` | `number` |
| `value` | `unknown` |

## Returns

`Promise`\<`string`\>
