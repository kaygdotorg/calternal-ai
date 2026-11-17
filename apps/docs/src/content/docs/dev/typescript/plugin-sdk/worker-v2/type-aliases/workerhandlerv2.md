---
title: "plugin-sdk: worker-v2/type-aliases/WorkerHandlerV2"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / WorkerHandlerV2

# Type Alias: WorkerHandlerV2

> **WorkerHandlerV2** = (`input`, `context`) => `Promise`\<`unknown`\>

Handler implementing one declared operation. Cost is plugin-defined and sandbox-limited; each host effect uses a bounded broker RPC.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `input` | `unknown` |
| `context` | [`WorkerContextV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/) |

## Returns

`Promise`\<`unknown`\>
