---
title: "plugin-sdk: worker-v2/interfaces/WorkerFrameV2"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / WorkerFrameV2

# Interface: WorkerFrameV2

Newline-delimited JSON frame. Transport adapters must cap buffering before decoding.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="invocation_id"></a> `invocation_id` | `string` | Host-issued invocation ID. |
| <a id="message"></a> `message` | [`WorkerMessageV2`](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workermessagev2/) | Direction-specific message. |
| <a id="protocol_version"></a> `protocol_version` | `2` | Exactly 2. |
