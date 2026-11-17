---
title: "plugin-sdk: index/functions/validateExtensionView"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / validateExtensionView

# Function: validateExtensionView()

> **validateExtensionView**(`value`, `manifest`): `GenUiView`

Validate a portable tree and require each action to be declared and read-only. Does not grant dispatch authority. Costs O(view bytes + declared operations).

## Parameters

| Parameter | Type |
| ------ | ------ |
| `value` | `unknown` |
| `manifest` | `PluginManifest` |

## Returns

`GenUiView`

The validated value without an additional clone.

## Throws

[ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/) if decoding, shape, value, correlation or budget checks fail.
