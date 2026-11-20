---
title: "plugin-sdk: fake-host/host/classes/FakeHost"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/host](/docs/dev/typescript/plugin-sdk/fake-host/host/) / FakeHost

# Class: FakeHost

The in-process fake host. See the module comment for what it does and does not prove.

## Constructors

### Constructor

> **new FakeHost**(`packages`, `options?`): `FakeHost`

Install packages straight from `definePlugin` output, which needs no build, or from a manifest and its handlers.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `packages` | readonly ([`InstalledPackage`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/installedpackage/) \| [`AuthoredPackage`](/docs/dev/typescript/plugin-sdk/fake-host/host/interfaces/authoredpackage/))[] |
| `options?` | [`FakeHostOptions`](/docs/dev/typescript/plugin-sdk/fake-host/host/interfaces/fakehostoptions/) |

#### Returns

`FakeHost`

## Properties

| Property | Modifier | Type | Default value | Description |
| ------ | ------ | ------ | ------ | ------ |
| <a id="calls"></a> `calls` | `readonly` | [`BrokerCall`](/docs/dev/typescript/plugin-sdk/fake-host/host/interfaces/brokercall/)[] | `[]` | Every broker call in order. |
| <a id="execution"></a> `execution` | `readonly` | [`Execution`](/docs/dev/typescript/plugin-sdk/fake-host/execution/classes/execution/) | `undefined` | The execution engine; set `beforeTerminal` or `readWaitMs` here. |
| <a id="items"></a> `items` | `readonly` | [`Items`](/docs/dev/typescript/plugin-sdk/fake-host/items/classes/items/) | `undefined` | The item engine; tune `spanPageSize` or `mentions` here. |
| <a id="people"></a> `people` | `readonly` | `Readonly`\<`Record`\<`string`, `string`\>\> | `undefined` | Synthetic accounts by display name. |
| <a id="receipts"></a> `receipts` | `readonly` | `Map`\<`string`, [`Receipt`](/docs/dev/typescript/plugin-sdk/fake-host/host/interfaces/receipt/)\> | `...` | Committed request receipts by package, actor, method and request ID. |
| <a id="space"></a> `space` | `readonly` | `"019a0000-0000-7000-8000-000000000010"` | `"019a0000-0000-7000-8000-000000000010"` | The single fake space. |

## Accessors

### jobs

#### Get Signature

> **get** **jobs**(): `ReadonlyMap`\<`string`, [`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/)\>

Admitted jobs by ref.

##### Returns

`ReadonlyMap`\<`string`, [`Job`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/job/)\>

## Methods

### archive()

> **archive**(`item`): `void`

Archive an item, as the generic host action does: stop its jobs and refuse later writes and admission.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `item` | `string` |

#### Returns

`void`

***

### choose()

> **choose**(`contribution?`): [`HarnessChoice`](/docs/dev/typescript/plugin-sdk/fake-host/execution/interfaces/harnesschoice/)

Issue a checked harness and placement for an installed harness contribution.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `contribution?` | `string` |

#### Returns

[`HarnessChoice`](/docs/dev/typescript/plugin-sdk/fake-host/execution/interfaces/harnesschoice/)

***

### context()

> **context**\<`M`\>(`operationId`, `options?`): [`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`M`\> & `Record`\<`string`, `unknown`\>

A typed context for calling a handler or the broker directly as `operationId` would, without schema-checking an input.

#### Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-m"></a> `M` *extends* `string` | [`BrokerMethod`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokermethod/) |

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `operationId` | `string` |
| `options?` | [`InvokeOptions`](/docs/dev/typescript/plugin-sdk/fake-host/host/interfaces/invokeoptions/) & `object` |

#### Returns

[`OperationContext`](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)\<`M`\> & `Record`\<`string`, `unknown`\>

***

### deliver()

> **deliver**(`alarm`, `actual_at`): `Promise`\<`unknown`\>

Deliver a pending alarm at `actual_at` and run the job it admits. Returns undefined when the alarm is gone.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `alarm` | `string` |
| `actual_at` | `string` |

#### Returns

`Promise`\<`unknown`\>

***

### discoverable()

> **discoverable**(`person`): `string`[]

Items attempts started by `person` may discover without consent.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `person` | `string` |

#### Returns

`string`[]

***

### failNext()

> **failNext**(`method`, `failure`, `packageId?`): `void`

Make the next call of `method` refuse before any effect, or commit and
then lose its acknowledgement. `packageId` limits it to one package.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `method` | [`BrokerMethod`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokermethod/) |
| `failure` | `Failure` |
| `packageId?` | `string` |

#### Returns

`void`

***

### invoke()

> **invoke**(`operationId`, `input`, `options?`): `Promise`\<`unknown`\>

Invoke a declared operation as the host would after a renderer action:
check the input schema and the declared target, stamp the author context
and check the result schema and `creates`. Returns the result.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `operationId` | `string` |
| `input` | `unknown` |
| `options?` | [`InvokeOptions`](/docs/dev/typescript/plugin-sdk/fake-host/host/interfaces/invokeoptions/) |

#### Returns

`Promise`\<`unknown`\>

***

### item()

> **item**(`ref`): [`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/)

One stored item, for assertions.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `ref` | `string` |

#### Returns

[`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/)

***

### provide()

> **provide**(`packageId`, `extras`): `void`

Merge test-only properties, such as a scripted provider, into every context of one package.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `packageId` | `string` |
| `extras` | `Record`\<`string`, `unknown`\> |

#### Returns

`void`

***

### replay()

> **replay**(`jobId`): `Promise`\<`unknown`\>

Run a finished job again with the same request ID, as recovery after a lost acknowledgement would.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `jobId` | `string` |

#### Returns

`Promise`\<`unknown`\>

***

### run()

> **run**(`jobId`): `Promise`\<`unknown`\>

Run a pending job's run operation as its starter, once it heads the item's queue.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `jobId` | `string` |

#### Returns

`Promise`\<`unknown`\>

***

### settle()

> **settle**(`jobId`): `void`

Mark a stopped attempt torn down.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `jobId` | `string` |

#### Returns

`void`

***

### stop()

> **stop**(`item`, `target`): `ExecutionStopResult`

Stop an exact attempt or queued job, as the host's Stop control does.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `item` | `string` |
| `target` | \{ `execution`: `string`; \} \| \{ `intent`: `string`; \} |

#### Returns

`ExecutionStopResult`

***

### text()

> **text**(`ref`): `string`

The head bytes of an item as UTF-8 text.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `ref` | `string` |

#### Returns

`string`

***

### view()

> **view**(`item`, `slot`): `ItemViewPublish` \| `undefined`

The published page of one item view slot, if any.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `item` | `string` |
| `slot` | `string` |

#### Returns

`ItemViewPublish` \| `undefined`
