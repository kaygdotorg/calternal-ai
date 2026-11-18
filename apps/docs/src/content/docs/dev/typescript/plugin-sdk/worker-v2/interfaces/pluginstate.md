---
title: "plugin-sdk: worker-v2/interfaces/PluginState"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / PluginState

# Interface: PluginState\<T\>

Private JSON state read from the invoking package's namespace.

## Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-t"></a> `T` | `unknown` |

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="revision"></a> `revision` | `string` | Exact decimal host revision used for compare and set. |
| <a id="schema"></a> `schema` | `number` | Package-defined format version. |
| <a id="value"></a> `value` | `T` | Package-defined JSON. Validate it before use. |
