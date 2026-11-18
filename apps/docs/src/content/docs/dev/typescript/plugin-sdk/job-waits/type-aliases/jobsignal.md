---
title: "plugin-sdk: job-waits/type-aliases/JobSignal"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [job-waits](/docs/dev/typescript/plugin-sdk/job-waits/) / JobSignal

# Type Alias: JobSignal\<Answer, Webhook\>

> **JobSignal**\<`Answer`, `Webhook`\> = \{ `kind`: `"deadline"`; \} \| \{ `approved`: `boolean`; `kind`: `"approval"`; \} \| \{ `kind`: `"answer"`; `value`: `Answer`; \} \| \{ `kind`: `"webhook"`; `value`: `Webhook`; \}

Signal delivered with the checkpoint on the next invocation.

## Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-answer"></a> `Answer` | `unknown` |
| <a id="type-parameter-webhook"></a> `Webhook` | `unknown` |
