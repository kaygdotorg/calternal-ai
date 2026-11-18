---
title: "plugin-sdk: index/type-aliases/ProviderRequest"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / ProviderRequest

# Type Alias: ProviderRequest

> **ProviderRequest** = `object`

A command envelope. `seq` starts at one and increases exactly once per command.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="command"></a> `command` | [`ProviderCommand`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providercommand/) | Action to perform. |
| <a id="seq"></a> `seq` | `number` | Consecutive command number, independent of event sequence numbers. |
| <a id="version"></a> `version` | `1` | Must equal [`PROVIDER_VERSION`]. |
