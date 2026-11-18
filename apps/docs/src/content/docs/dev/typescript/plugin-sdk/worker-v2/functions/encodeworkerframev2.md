---
title: "plugin-sdk: worker-v2/functions/encodeWorkerFrameV2"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / encodeWorkerFrameV2

# Function: encodeWorkerFrameV2()

> **encodeWorkerFrameV2**(`frame`): `string`

Encode one strict frame including its newline, respecting the byte budget.
Cost: O(frame bytes), serialization and UTF-8 allocation, no I/O.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `frame` | [`WorkerFrameV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workerframev2/) |

## Returns

`string`

JSON line suitable for a back-pressure-aware transport.
