---
title: "plugin-sdk: typed-handlers/functions/typedHandlers"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [typed-handlers](/docs/dev/typescript/plugin-sdk/typed-handlers/) / typedHandlers

# Function: typedHandlers()

> **typedHandlers**\<`O`\>(`manifest`, `handlers`): `Record`\<`string`, [`WorkerHandlerV2`](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workerhandlerv2/)\>

Bind each declared operation to one handler. Literal manifest declarations infer
basic properties and discriminated unions of object schemas, and require matching handler keys. Runtime validation
still comes from the host's compiled schemas. Cost: return the map unchanged.

## Type Parameters

| Type Parameter |
| ------ |
| <a id="type-parameter-o"></a> `O` *extends* readonly `Operation`[] |

## Parameters

| Parameter | Type |
| ------ | ------ |
| `manifest` | \{ `operations`: `O`; \} |
| `manifest.operations` | `O` |
| `handlers` | `NoInfer`\<`{ [Id in string]: Handler<Extract<O[number], { id: Id }>> }`\> |

## Returns

`Record`\<`string`, [`WorkerHandlerV2`](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workerhandlerv2/)\>
