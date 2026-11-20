---
title: "plugin-sdk: fake-host/invocation/interfaces/Invocation"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/invocation](/docs/dev/typescript/plugin-sdk/fake-host/invocation/) / Invocation

# Interface: Invocation

One running invocation of a declared operation.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="actor"></a> `actor` | `readonly` | `string` | Acting account. |
| <a id="attempt"></a> `attempt?` | `readonly` | [`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/) | The job whose attempt runs this contribution entry; Stop fences its calls. |
| <a id="captured"></a> `captured?` | `readonly` | [`Captured`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/captured/) | Captured submission, when the host captured one. |
| <a id="created"></a> `created` | `readonly` | `Set`\<`string`\> | Items created in this invocation, for the `creates` check. |
| <a id="index"></a> `index` | `readonly` | `number` | Its index in `manifest.operations`, for diagnostics. |
| <a id="job"></a> `job?` | `readonly` | [`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/) | The job this invocation runs, if any. |
| <a id="operation"></a> `operation` | `readonly` | `Operation` | The declared operation. |
| <a id="pkg"></a> `pkg` | `readonly` | [`InstalledPackage`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/installedpackage/) | Package whose operation runs. |
| <a id="request_id"></a> `request_id` | `readonly` | `string` | Stable request ID. |
