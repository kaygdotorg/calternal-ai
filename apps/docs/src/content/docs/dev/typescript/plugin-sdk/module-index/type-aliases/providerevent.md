---
title: "plugin-sdk: index/type-aliases/ProviderEvent"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

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
