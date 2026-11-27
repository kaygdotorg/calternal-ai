---
title: "plugin-sdk: author-v2/functions/defineView"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [author-v2](/docs/dev/typescript/plugin-sdk/author-v2/) / defineView

# Function: defineView()

> **defineView**(`__namedParameters`): `object`

Build view content and its Binds. Defaults each Bind independently in linear time.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `__namedParameters` | `ViewInput` |

## Returns

### binds

> **binds**: `Bind`[]

Queries this view contributes to the app's shared Bind list.

### view

> **view**: `ViewBody`

Content rendered in the app's main area.
