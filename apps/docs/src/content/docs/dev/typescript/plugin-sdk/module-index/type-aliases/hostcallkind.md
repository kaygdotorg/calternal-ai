---
title: "plugin-sdk: index/type-aliases/HostCallKind"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / HostCallKind

# Type Alias: HostCallKind

> **HostCallKind** = `"read_item"` \| `"list_items_v1"` \| `"read_setting_v1"` \| `"content_prepare_v1"` \| `"write_item"` \| `"list_collection"` \| `"emit_event"` \| `"start_job"` \| `"checkpoint_job"` \| `"request_secret_handle"`

The only broker methods a worker may request.
