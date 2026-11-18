---
title: "plugin-sdk: index/functions/validateOperationRequest"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / validateOperationRequest

# Function: validateOperationRequest()

> **validateOperationRequest**(`value`): `OperationRequest`

Validate an operation envelope, its known input schema and the 2 MiB serialized budget; does not authorize the caller. Costs O(request bytes). Unknown operations are rejected.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `value` | `unknown` |

## Returns

`OperationRequest`

The validated value without an additional clone.

## Throws

[ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/) if decoding, shape, value, correlation or budget checks fail.
