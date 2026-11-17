---
title: "plugin-sdk: index/functions/parseWorkerResponse"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / parseWorkerResponse

# Function: parseWorkerResponse()

> **parseWorkerResponse**(`input`, `invocationId`): `WorkerResponse`

Decode at most 256 KiB of strict UTF-8 JSON and validate response correlation with the host-issued invocationId. Allocates O(input bytes).

## Parameters

| Parameter | Type |
| ------ | ------ |
| `input` | `string` \| `Uint8Array`\<`ArrayBufferLike`\> |
| `invocationId` | `string` |

## Returns

`WorkerResponse`

The validated value without an additional clone.

## Throws

[ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/) if decoding, shape, value, correlation or budget checks fail.
