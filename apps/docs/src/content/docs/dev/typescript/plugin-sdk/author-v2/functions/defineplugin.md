---
title: "plugin-sdk: author-v2/functions/definePlugin"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [author-v2](/docs/dev/typescript/plugin-sdk/author-v2/) / definePlugin

# Function: definePlugin()

> **definePlugin**(`__namedParameters`): `AuthoredPlugin`

Assemble v2 declarations for the offline package build. Adds own kinds to
reads and defaults to no network and no worker operations. Performs no I/O.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `__namedParameters` | `PluginInput` |

## Returns

`AuthoredPlugin`
