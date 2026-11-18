---
title: "plugin-sdk: worker-v2/interfaces/SpaceUsage"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / SpaceUsage

# Interface: SpaceUsage

Committed current-body bytes of a space or its owner group, without rounding.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="binary_bytes"></a> `binary_bytes` | `string` | Decimal binary bytes, including recoverable Trash. |
| <a id="text_bytes"></a> `text_bytes` | `string` | Decimal text bytes, including recoverable Trash. |
