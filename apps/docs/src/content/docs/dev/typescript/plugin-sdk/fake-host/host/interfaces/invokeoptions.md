---
title: "plugin-sdk: fake-host/host/interfaces/InvokeOptions"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/host](/docs/dev/typescript/plugin-sdk/fake-host/host/) / InvokeOptions

# Interface: InvokeOptions

Per-invocation choices for `invoke` and `context`.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="actor"></a> `actor?` | `string` | Acting account; defaults to the first person. |
| <a id="captured"></a> `captured?` | `boolean` | Treat the operation's `write_origins.host_input` field as a host-captured composer or editor submission. |
| <a id="request_id"></a> `request_id?` | `string` | Stable request ID; defaults to a fresh one. Reuse it to replay. |
