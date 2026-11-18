---
title: "plugin-sdk: worker-v2/functions/readSpaceUsage"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / readSpaceUsage

# Function: readSpaceUsage()

> **readSpaceUsage**(`context`, `scope?`): `Promise`\<[`SpaceUsage`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/spaceusage/)\>

Read the current space's committed body bytes. Declare `read_space_usage_v1`
and require `space.settings`. With `scope: "owner"`, an exact operator
resource binding is also required. The host resolves the group and reads
one snapshot per member. Personal spaces keep their own byte counts.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `context` | [`WorkerContextV2`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/workercontextv2/) |
| `scope?` | `"owner"` |

## Returns

`Promise`\<[`SpaceUsage`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/spaceusage/)\>
