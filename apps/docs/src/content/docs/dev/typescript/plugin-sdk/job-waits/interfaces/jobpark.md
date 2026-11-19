---
title: "plugin-sdk: job-waits/interfaces/JobPark"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [job-waits](/docs/dev/typescript/plugin-sdk/job-waits/) / JobPark

# Interface: JobPark\<Checkpoint\>

Checkpoint and acceptance window to persist as one job transition.

## Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-checkpoint"></a> `Checkpoint` | `unknown` |

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="checkpoint"></a> `checkpoint` | `Checkpoint` | Durable state for the next invocation. |
| <a id="deadline_ms"></a> `deadline_ms` | `number` | Unix milliseconds within the next seven days. |
| <a id="reason"></a> `reason` | `string` | Displayed to the job's viewer. Keep sensitive values out of this text. |
| <a id="source"></a> `source` | [`JobWaitSource`](/docs/dev/typescript/plugin-sdk/job-waits/type-aliases/jobwaitsource/) | Signal's declaration and scope. |
