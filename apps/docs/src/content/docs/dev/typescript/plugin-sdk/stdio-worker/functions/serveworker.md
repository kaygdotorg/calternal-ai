---
title: "plugin-sdk: stdio-worker/functions/serveWorker"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [stdio-worker](/docs/dev/typescript/plugin-sdk/stdio-worker/) / serveWorker

# Function: serveWorker()

> **serveWorker**(`manifest`, `handlers`, `input`, `write`): `Promise`\<`void`\>

Serve one isolated worker until its input closes. Keep at most one frame of buffered input.
Cost: one decode per line, one serialized result per send, one reusable bounded buffer.
Rejects invalid, oversized or truncated frames. The caller controls the process and transport.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `manifest` | `unknown` |
| `handlers` | `Record`\<`string`, [`WorkerHandlerV2`](/docs/dev/typescript/plugin-sdk/worker-v2/type-aliases/workerhandlerv2/)\> |
| `input` | `AsyncIterable`\<`Uint8Array`\<`ArrayBufferLike`\>\> |
| `write` | (`line`) => `Promise`\<`void`\> |

## Returns

`Promise`\<`void`\>
