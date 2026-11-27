---
title: "plugin-sdk: newline-frames/functions/newlineFrames"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [newline-frames](/docs/dev/typescript/plugin-sdk/newline-frames/) / newlineFrames

# Function: newlineFrames()

> **newlineFrames**(`input`, `maximum`): `AsyncGenerator`\<`Uint8Array`\<`ArrayBuffer`\>, `void`, `unknown`\>

Yield one bounded frame at a time from UTF-8 bytes. Consumers must
decode each frame before asking for the next one, as the buffer is reused.
Work is linear in bytes received. Overflow or truncation rejects the stream.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `input` | `AsyncIterable`\<`Uint8Array`\<`ArrayBufferLike`\>\> |
| `maximum` | `number` |

## Returns

`AsyncGenerator`\<`Uint8Array`\<`ArrayBuffer`\>, `void`, `unknown`\>
