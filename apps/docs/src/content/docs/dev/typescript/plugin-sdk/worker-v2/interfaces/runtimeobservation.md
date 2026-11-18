---
title: "plugin-sdk: worker-v2/interfaces/RuntimeObservation"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / RuntimeObservation

# Interface: RuntimeObservation

One cumulative clock observation. An unknown end has no terminal timestamp.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="generation"></a> `generation` | `string` | Fenced guest generation. |
| <a id="running_ns"></a> `running_ns` | `string` | Exact decimal nanoseconds to date. |
| <a id="started_at_ms"></a> `started_at_ms` | `number` | UTC anchor in Unix milliseconds. |
| <a id="state"></a> `state` | `"unknown"` \| `"running"` \| `"stopped"` | State of the source observation. |
