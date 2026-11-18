---
title: "plugin-sdk: job-waits/interfaces/ParkedJob"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [job-waits](/docs/dev/typescript/plugin-sdk/job-waits/) / ParkedJob

# Interface: ParkedJob\<Checkpoint\>

Completed run output asking the host to park a durable job.

## Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-checkpoint"></a> `Checkpoint` | `unknown` |

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="status"></a> `status` | `"parked"` | Selects the host's durable park transition. |
| <a id="wait"></a> `wait` | [`JobPark`](/docs/dev/typescript/plugin-sdk/job-waits/interfaces/jobpark/)\<`Checkpoint`\> | Checkpoint and signal contract for the waiting job. |
