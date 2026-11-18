---
title: "plugin-sdk: index/type-aliases/ProviderFailureClass"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / ProviderFailureClass

# Type Alias: ProviderFailureClass

> **ProviderFailureClass** = `"transient"` \| `"usage_limit"` \| `"authentication"` \| `"refusal"` \| `"permanent"` \| `"unknown"`

Provider-declared failure category; only transient and usage-limit failures
with no pending tool effect may be retried automatically.
