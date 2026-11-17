---
title: "plugin-sdk: index/functions/parseWorkerRequest"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / parseWorkerRequest

# Function: parseWorkerRequest()

> **parseWorkerRequest**(`input`): `WorkerRequest`

Decode at most 2 MiB of strict UTF-8 JSON and validate a worker invocation. Allocates O(input bytes).

## Parameters

| Parameter | Type |
| ------ | ------ |
| `input` | `string` \| `Uint8Array`\<`ArrayBufferLike`\> |

## Returns

`WorkerRequest`

The validated value without an additional clone.

## Throws

[ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/) if decoding, shape, value, correlation or budget checks fail.
