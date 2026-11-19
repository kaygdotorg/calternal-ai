---
title: "plugin-sdk: worker-v2/interfaces/InvocationAuthority"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / InvocationAuthority

# Interface: InvocationAuthority

Invocation authority issued by the trusted host; never a grant by itself.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="account_id"></a> `account_id` | `string` | Acting account UUIDv7. |
| <a id="grant"></a> `grant` | `string` | Opaque reference to host-owned current session/grant attribution. |
| <a id="space_id"></a> `space_id` | `string` | Independently allocated space UUIDv7. |
