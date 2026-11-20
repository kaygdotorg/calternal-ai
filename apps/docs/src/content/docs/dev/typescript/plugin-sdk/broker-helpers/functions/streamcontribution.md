---
title: "plugin-sdk: broker-helpers/functions/streamContribution"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / streamContribution

# Function: streamContribution()

> **streamContribution**(`ctx`, `__namedParameters`): `AsyncGenerator`\<[`ContributionOutput`](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/contributionoutput/), `void`\>

Call a contribution for the current attempt and yield each output frame in
order. Preflights the call against CONTRIBUTION_FRAME_BYTES, polls
`contribution_read` with the longest allowed wait, refuses a gap or repeat
in `seq`, and finishes on an `ok` terminal; any other terminal throws
ContributionEndedError. Returns the call ref so a caller can name the
output. Cost: one call plus one read per batch of frames.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `ctx` | [`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`"contribution_call"` \| `"contribution_read"`\> |
| `__namedParameters` | \{ `contribution`: `string`; `execution`: `string`; `input`: `unknown`; `purpose`: `string`; \} |
| `__namedParameters.contribution` | `string` |
| `__namedParameters.execution` | `string` |
| `__namedParameters.input` | `unknown` |
| `__namedParameters.purpose` | `string` |

## Returns

`AsyncGenerator`\<[`ContributionOutput`](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/contributionoutput/), `void`\>
