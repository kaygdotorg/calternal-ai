---
title: "plugin-sdk: operation-api/variables/on"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / on

# Variable: on

> `const` **on**: `object`

Register handlers by exporting the returned declaration from operations.ts.

## Type Declaration

### call()

> **call**\<`T`\>(`selector`, `handler`): [`Operation`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/operation/)\<`T`\>

Bind a declared coded operation to its handler.

#### Type Parameters

| Type Parameter |
| ------ |
| <a id="type-parameter-callt"></a> `T` |

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `selector` | `string` |
| `handler` | (`input`, `s`) => `unknown` |

#### Returns

[`Operation`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/operation/)\<`T`\>

### change()

> **change**\<`T`\>(`selector`, `handler`): [`Operation`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/operation/)\<`T`\>

React to a committed change for a kind or collection.

#### Type Parameters

| Type Parameter |
| ------ |
| <a id="type-parameter-changet"></a> `T` |

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `selector` | `string` |
| `handler` | (`input`, `s`) => `unknown` |

#### Returns

[`Operation`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/operation/)\<`T`\>
