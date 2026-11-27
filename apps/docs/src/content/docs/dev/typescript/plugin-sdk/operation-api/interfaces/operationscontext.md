---
title: "plugin-sdk: operation-api/interfaces/OperationsContext"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / OperationsContext

# Interface: OperationsContext

Host calls and context available while handling one event.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="event"></a> `event` | `readonly` | `Readonly`\<[`OperationEvent`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/operationevent/)\> | Metadata scoped to this delivery. |
| <a id="me"></a> `me` | `readonly` | `Readonly`\<\{ `id`: [`PersonId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/personid/); `role`: `string`; \}\> | Verified actor and current role for this delivery. |
| <a id="space"></a> `space` | `readonly` | `Readonly`\<\{ `id`: `string`; \}\> | Space in which the plugin is installed. |
| <a id="state"></a> `state` | `public` | `object` | Get or set declared viewer state. |
| `state.get` | `public` | (`item`, `key`) => `Promise`\<`unknown`\> | Read the acting person's current value. |
| `state.set` | `public` | (`item`, `key`, `value`) => `Promise`\<`unknown`\> | Store the acting person's value. |

## Methods

### call()

> **call**(`operation`, `input`): `Promise`\<`unknown`\>

Invoke a declared operation using its plugin.kind.operation identifier.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `operation` | `string` |
| `input` | `unknown` |

#### Returns

`Promise`\<`unknown`\>

***

### query()

> **query**(`query`): `Promise`\<[`Page`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/page/)\>

Query visible items through the host.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `query` | [`Query`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/query/) |

#### Returns

`Promise`\<[`Page`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/page/)\>

***

### read()

> **read**(`item`): `Promise`\<[`Item`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/item/)\>

Read one item through the host.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `item` | `string` |

#### Returns

`Promise`\<[`Item`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/item/)\>

***

### run()

> **run**(`program`, `options`): `Promise`\<[`RunHandle`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/runhandle/)\>

Start a declared program in the current session. The host needs an
operator-configured run service to admit the program.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `program` | `string` |
| `options` | \{ `scope?`: `"session"`; `targets`: readonly `string`[]; \} |
| `options.scope?` | `"session"` |
| `options.targets` | readonly `string`[] |

#### Returns

`Promise`\<[`RunHandle`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/runhandle/)\>

***

### write()

> **write**(`changes`, `options?`): `Promise`\<[`WriteResult`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/writeresult/)\>

Commit or preview a list as one atomic effect.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `changes` | readonly [`Change`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/change/)[] |
| `options?` | \{ `preview?`: `boolean`; \} |
| `options.preview?` | `boolean` |

#### Returns

`Promise`\<[`WriteResult`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/writeresult/)\>
