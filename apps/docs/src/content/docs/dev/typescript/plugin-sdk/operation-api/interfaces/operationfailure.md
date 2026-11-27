---
title: "plugin-sdk: operation-api/interfaces/OperationFailure"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / OperationFailure

# Interface: OperationFailure

Stable caller-safe failure supplied by the host or a coded operation.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="code"></a> `code` | `"conflict"` \| `"limit"` \| `"not-allowed"` \| `"not-found"` \| `"invalid"` \| `"too-large"` \| `"unreachable"` \| `"needs-sign-in"` \| `"stopped"` \| `"timeout"` | Category suitable for programmatic decisions. |
| <a id="message"></a> `message` | `string` | A safe human-readable reason. |
| <a id="next"></a> `next` | `string` | A safe next action. |
