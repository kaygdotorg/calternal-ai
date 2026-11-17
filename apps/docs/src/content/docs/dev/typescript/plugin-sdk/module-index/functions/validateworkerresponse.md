---
title: "plugin-sdk: index/functions/validateWorkerResponse"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / validateWorkerResponse

# Function: validateWorkerResponse()

> **validateWorkerResponse**(`value`, `invocationId`): `WorkerResponse`

Validate response correlation, version, safe failure shape or successful statistics and portable view. Costs O(response bytes). The expected invocationId must come from the host.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `value` | `unknown` |
| `invocationId` | `string` |

## Returns

`WorkerResponse`

The validated value without an additional clone.

## Throws

[ContractValidationError](/docs/dev/typescript/plugin-sdk/module-index/classes/contractvalidationerror/) if decoding, shape, value, correlation or budget checks fail.
