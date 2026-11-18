---
title: "plugin-sdk: worker-v2/type-aliases/ResourceAdmissionDecision"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / ResourceAdmissionDecision

# Type Alias: ResourceAdmissionDecision

> **ResourceAdmissionDecision** = \{ `decision`: `"allow"`; `memory_mib?`: `number`; \} \| \{ `decision`: `"queue"`; `reason`: `string`; `retry_after_ms`: `number`; \} \| \{ `decision`: `"refuse"`; `reason`: `string`; \}

A block hook's validated decision. The host still enforces its own authority and ceilings.
