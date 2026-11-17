---
title: "plugin-sdk: index/type-aliases/ProviderEvent"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / ProviderEvent

# Type Alias: ProviderEvent

> **ProviderEvent** = `object`

Ordered event envelope. Sequence numbers start at one for each adapter process.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="event"></a> `event` | [`ProviderEventKind`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providereventkind/) | Normalized output. |
| <a id="seq"></a> `seq` | `number` | Consecutive event number; duplicates or gaps fail the run. |
| <a id="version"></a> `version` | `1` | Must equal [`PROVIDER_VERSION`]. |
