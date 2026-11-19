---
title: "plugin-sdk: author-v2/functions/defineChrome"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [author-v2](/docs/dev/typescript/plugin-sdk/author-v2/) / defineChrome

# Function: defineChrome()

> **defineChrome**(`__namedParameters`): `object`

Build chrome and its Binds using the same defaults as view content.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `__namedParameters` | `ChromeInput` |

## Returns

### binds

> **binds**: `Bind`[]

Queries these controls contribute to the app's shared Bind list.

### chrome

> **chrome**: `ChromeBody`

Controls rendered around the app content.
