---
title: "plugin-sdk: fake-host/execution/classes/Execution"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/execution](/docs/dev/typescript/plugin-sdk/fake-host/execution/) / Execution

# Class: Execution

Execution engine state and broker methods.

## Constructors

### Constructor

> **new Execution**(`id`, `packages`, `schemas`, `runEntry`): `Execution`

Create an engine using the host's ID source, package list, schemas and entry runner.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `id` | (`prefix`) => `string` |
| `packages` | () => readonly [`InstalledPackage`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/installedpackage/)[] |
| `schemas` | [`ContractSchemas`](/docs/dev/typescript/plugin-sdk/fake-host/schemas/classes/contractschemas/) |
| `runEntry` | [`EntryRunner`](/docs/dev/typescript/plugin-sdk/fake-host/execution/type-aliases/entryrunner/) |

#### Returns

`Execution`

## Properties

| Property | Modifier | Type | Default value | Description |
| ------ | ------ | ------ | ------ | ------ |
| <a id="alarms"></a> `alarms` | `readonly` | `Map`\<`string`, [`PendingAlarm`](/docs/dev/typescript/plugin-sdk/fake-host/execution/interfaces/pendingalarm/)\> | `...` | Pending alarms by ref. |
| <a id="beforeterminal"></a> `beforeTerminal` | `public` | (`job`) => `Promise`\<`void`\> | `...` | Awaited between a contribution's last output and its terminal frame; tests use it to Stop mid-call. |
| <a id="jobs"></a> `jobs` | `readonly` | `Map`\<`string`, [`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/)\> | `...` | Admitted jobs by ref. |
| <a id="readwaitms"></a> `readWaitMs` | `public` | `number` | `1000` | Real milliseconds a contribution_read may wait; the host caps the requested wait at this. |

## Methods

### alarmCancel()

> **alarmCancel**(`inv`, `input`): `AlarmReceipt`

`alarm_cancel`: drop a pending alarm this package set.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | \{ `alarm`: `string`; `request_id`: `string`; \} |
| `input.alarm` | `string` |
| `input.request_id` | `string` |

#### Returns

`AlarmReceipt`

***

### alarmSet()

> **alarmSet**(`inv`, `input`, `items`): `AlarmReceipt`

`alarm_set`: a pending alarm for a declared alarm job, with checked choices when it starts execution.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `AlarmSet` |
| `items` | \{ `executionTarget`(`inv`, `item`): `void`; \} |
| `items.executionTarget` | (`inv`, `item`) => `void` |

#### Returns

`AlarmReceipt`

***

### archive()

> **archive**(`item`): `void`

Stop every pending or running job on an archived item.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `item` | `string` |

#### Returns

`void`

***

### choose()

> **choose**(`contribution?`): [`HarnessChoice`](/docs/dev/typescript/plugin-sdk/fake-host/execution/interfaces/harnesschoice/)

Issue a checked harness ref and placement ref for one installed harness
contribution, as the renderer's extension choice and placement prompt
would. Issuing again revokes the previous harness ref for that
contribution. Checks that the contributing operation's schemas equal the
point's, as install would.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `contribution?` | `string` |

#### Returns

[`HarnessChoice`](/docs/dev/typescript/plugin-sdk/fake-host/execution/interfaces/harnesschoice/)

***

### contributionCall()

> **contributionCall**(`inv`, `input`): `ContributionCallReceipt`

`contribution_call`: the pinned input to the admitted harness, once per attempt; starts the entry.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ContributionCall` |

#### Returns

`ContributionCallReceipt`

***

### contributionRead()

> **contributionRead**(`inv`, `input`): `Promise`\<`ContributionReadResult`\>

`contribution_read`: committed frames after `after`, waiting up to `wait_ms` for more.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ContributionRead` |

#### Returns

`Promise`\<`ContributionReadResult`\>

***

### deliver()

> **deliver**(`alarm`, `actual_at`): [`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/) \| `undefined`

Consume a pending alarm into one admitted job, as delivery would. Returns undefined when it is gone.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `alarm` | `string` |
| `actual_at` | `string` |

#### Returns

[`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/) \| `undefined`

***

### executionStart()

> **executionStart**(`inv`, `input`): `ExecutionStatus`

`execution_start`: one attempt for the running job and its admitted harness.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ExecutionStart` |

#### Returns

`ExecutionStatus`

***

### settle()

> **settle**(`jobId`): `void`

Record that a stopped attempt finished tearing down, so a successor may start.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `jobId` | `string` |

#### Returns

`void`

***

### startable()

> **startable**(`job`): `void`

Check that `job` may start now: it is the item's head job and the previous attempt tore down.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `job` | [`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/) |

#### Returns

`void`

***

### startJob()

> **startJob**(`inv`, `input`, `items`): `ExecutionJobReceipt`

`start_job` for an execution job: admission, checked choices and the per-item queue bound.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ExecutionJobStart` |
| `items` | \{ `executionTarget`(`inv`, `item`): `void`; \} |
| `items.executionTarget` | (`inv`, `item`) => `void` |

#### Returns

`ExecutionJobReceipt`

***

### stop()

> **stop**(`item`, `target`): `ExecutionStopResult`

Stop one exact attempt or queued job on `item`. Refuses a stale target.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `item` | `string` |
| `target` | \{ `execution`: `string`; \} \| \{ `intent`: `string`; \} |

#### Returns

`ExecutionStopResult`
