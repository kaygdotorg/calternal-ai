---
title: "plugin-sdk: testing/functions/fixtureWorker"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [testing](/docs/dev/typescript/plugin-sdk/testing/) / fixtureWorker

# Function: fixtureWorker()

> **fixtureWorker**(`child`, `fixtures?`): `object`

Exercise operations using a disposable worker and local callback fixtures.
Runs one invocation at a time. Only the worker receives the dummy authority.
Unhandled callbacks fail closed. The deadline bounds fixture waits.
A child that exits before answering reports its exit status or signal and
the last 4 KiB of stderr. Spawn with piped stdin, stdout and stderr.
Under `calternal plugin test`, each parked question uses the host validator.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `child` | `Child` |
| `fixtures?` | `Partial`\<`Record`\<[`HostCallKind`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/hostcallkind/), (`input`) => `unknown`\>\> |

## Returns

### close()

> **close**(): `Promise`\<`void`\>

Dispose the disposable subprocess and release its input stream.

#### Returns

`Promise`\<`void`\>

### invoke()

> **invoke**(`operation`, `input`): `Promise`\<`unknown`\>

Send one invocation and await the worker's result.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `operation` | `string` |
| `input` | `unknown` |

#### Returns

`Promise`\<`unknown`\>
