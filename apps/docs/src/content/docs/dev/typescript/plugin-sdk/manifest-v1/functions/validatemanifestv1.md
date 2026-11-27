---
title: "plugin-sdk: manifest-v1/functions/validateManifestV1"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [manifest-v1](/docs/dev/typescript/plugin-sdk/manifest-v1/) / validateManifestV1

# Function: validateManifestV1()

> **validateManifestV1**(`value`): `ManifestV1`

Validate the strict data-only menu, references, bounds, defaults and retry consistency.
Schemas remain data and must be compiled by the trusted execution adapter before activation.
Cost: O(menu bytes + declaration count), bounded sets and serialization, no I/O or execution.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `value` | `unknown` |

## Returns

`ManifestV1`

The validated menu without cloning or executing plugin code.
