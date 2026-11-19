---
title: "plugin-sdk: index/functions/parseOperationRequest"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / parseOperationRequest

# Function: parseOperationRequest()

> **parseOperationRequest**(`input`): `OperationRequest`

Decode at most 2 MiB of strict UTF-8 JSON, then validate the operation envelope and input. Allocates O(input bytes).

## Parameters

| Parameter | Type |
| ------ | ------ |
| `input` | `string` \| `Uint8Array`\<`ArrayBufferLike`\> |

## Returns

`OperationRequest`

The validated value without an additional clone.

## Throws

[ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/) if decoding, shape, value, correlation or budget checks fail.
