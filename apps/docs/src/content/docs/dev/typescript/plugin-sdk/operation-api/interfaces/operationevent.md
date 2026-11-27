---
title: "plugin-sdk: operation-api/interfaces/OperationEvent"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / OperationEvent

# Interface: OperationEvent

Event context attached by the host. The key is stable across replay.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="before"></a> `before?` | `unknown` | Visible previous item data when available. |
| <a id="item"></a> `item` | `string` \| `null` | Addressed item, or null for an itemless operation. |
| <a id="key"></a> `key` | `string` | Opaque identifier for this logical delivery. |
| <a id="kind"></a> `kind` | `"call"` \| `"change"` | The registration that opened this event. |
| <a id="request"></a> `request?` | `string` | Host request identifier when available. |
