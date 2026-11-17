---
title: "plugin-sdk: index/functions/validatePluginManifest"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / validatePluginManifest

# ~~Function: validatePluginManifest()~~

> **validatePluginManifest**(`value`): `PluginManifest`

## Parameters

| Parameter | Type |
| ------ | ------ |
| `value` | `unknown` |

## Returns

`PluginManifest`

The validated value without an additional clone.

## Deprecated

Use validateManifestV1 for new plugin menus; this frozen preview remains for existing callers.
Validate plugin declarations, unique operation/view IDs and all read-only contributions. Does not load code or grant capabilities. Cost includes contributed views and action-input validation.

## Throws

[ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/) if decoding, shape, value, correlation or budget checks fail.
