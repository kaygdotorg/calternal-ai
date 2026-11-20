---
title: "plugin-sdk: fake-host/execution/interfaces/Choice"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/execution](/docs/dev/typescript/plugin-sdk/fake-host/execution/) / Choice

# Interface: Choice

A resolved harness choice: the contributing package and the point it fills.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="contributor"></a> `contributor` | `readonly` | [`InstalledPackage`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/installedpackage/) | The contributing package. |
| <a id="identity"></a> `identity` | `readonly` | `ContributionIdentity` | Logical contribution identity. |
| <a id="operation"></a> `operation` | `readonly` | `Operation` | The contributing operation that runs as the entry. |
| <a id="owner"></a> `owner` | `readonly` | `string` | Package ID of the app that owns the point. |
| <a id="point"></a> `point` | `readonly` | `ExtensionPoint` | The extension point the contribution fills. |
