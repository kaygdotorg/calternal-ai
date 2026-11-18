---
title: "plugin-sdk: worker-v2/interfaces/RuntimeWindow"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / RuntimeWindow

# Interface: RuntimeWindow

Confirmed runtime within a retained window for the authorized resource scope.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="entitlement_groups"></a> `entitlement_groups` | `string`[] | Groups available to this granted entitlement policy. |
| <a id="owner"></a> `owner` | [`ResourceOwner`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/resourceowner/) | Resource owner of the granted observation. |
| <a id="running_ns"></a> `running_ns` | `string` | Exact decimal nanoseconds. |
| <a id="since_ms"></a> `since_ms` | `number` | Start requested by the package, in Unix milliseconds. |
| <a id="through_ms"></a> `through_ms` | `number` | UTC sample boundary from the server, in Unix milliseconds. |
| <a id="unresolved"></a> `unresolved` | `number` | Generation ends that still require evidence. |
