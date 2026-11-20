---
title: "plugin-sdk: fake-host/invocation/interfaces/Job"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/invocation](/docs/dev/typescript/plugin-sdk/fake-host/invocation/) / Job

# Interface: Job

One admitted execution job and, once started, its attempt and contribution call.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="alarm"></a> `alarm?` | `public` | `AlarmDelivery` | Alarm evidence when an alarm admitted the job. |
| <a id="attempt"></a> `attempt?` | `public` | `ExecutionStatus` | The attempt, once execution_start admitted one. |
| <a id="call"></a> `call?` | `public` | [`ContributionCallState`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/contributioncallstate/) | The contribution call, once made. |
| <a id="captured"></a> `captured?` | `readonly` | [`Captured`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/captured/) | Captured submission inherited from the invocation that admitted the job. |
| <a id="contributioninput"></a> `contributionInput` | `readonly` | `unknown` | Pinned contribution input. |
| <a id="harness"></a> `harness` | `readonly` | `string` | Checked harness ref the job was admitted with. |
| <a id="id"></a> `id` | `readonly` | `string` | Job ref (ExecutionIntentRef). |
| <a id="identity"></a> `identity` | `readonly` | `ContributionIdentity` | Logical contribution the harness ref resolved to. |
| <a id="input"></a> `input` | `readonly` | `Record`\<`string`, `unknown`\> | Input for the run operation, without the target field. |
| <a id="item"></a> `item` | `readonly` | `string` | Target item. |
| <a id="job"></a> `job` | `readonly` | `string` | Declared job ID. |
| <a id="outputs"></a> `outputs` | `readonly` | `Map`\<`number`, `unknown`\> | Output data delivered to this attempt by sequence, for output-attested writes. |
| <a id="package"></a> `package` | `readonly` | `string` | Owning package. |
| <a id="starter"></a> `starter` | `readonly` | `string` | Account that started the job; its run invocations act as this account. |
| <a id="state"></a> `state` | `public` | `"done"` \| `"running"` \| `"stopped"` \| `"pending"` | Lifecycle state. |
| <a id="teardown"></a> `teardown` | `public` | `boolean` | Whether the previous attempt has torn down, so a successor may start. |
