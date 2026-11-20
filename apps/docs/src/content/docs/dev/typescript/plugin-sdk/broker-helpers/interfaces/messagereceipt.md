---
title: "plugin-sdk: broker-helpers/interfaces/MessageReceipt"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / MessageReceipt

# Interface: MessageReceipt

The receipt of `appendMessage` and the write path it chose.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="end_offset"></a> `end_offset` | `readonly` | `number` | Body length after the write. |
| <a id="mode"></a> `mode` | `readonly` | `"attested"` \| `"staged"` \| `"append"` \| `"inline"` | `attested` (host-composed span), `append` (opaque bytes) or `inline`/`staged` (whole-body rewrite). |
| <a id="receipt"></a> `receipt` | `readonly` | `string` | Committed receipt ref. |
| <a id="revision"></a> `revision` | `readonly` | `string` | New head revision. |
| <a id="span"></a> `span?` | `readonly` | `string` | Recorded span of an attested write. |
