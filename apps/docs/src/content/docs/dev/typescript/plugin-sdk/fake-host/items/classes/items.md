---
title: "plugin-sdk: fake-host/items/classes/Items"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/items](/docs/dev/typescript/plugin-sdk/fake-host/items/) / Items

# Class: Items

Item engine state and the broker methods over it.

## Constructors

### Constructor

> **new Items**(`id`, `now`, `space`): `Items`

Create an engine using the host's ID source, clock and space ref.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `id` | (`prefix`) => `string` |
| `now` | () => `string` |
| `space` | `string` |

#### Returns

`Items`

## Properties

| Property | Modifier | Type | Default value | Description |
| ------ | ------ | ------ | ------ | ------ |
| <a id="items"></a> `items` | `readonly` | `Map`\<`string`, [`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/)\> | `...` | Items by ref. |
| <a id="mentions"></a> `mentions` | `public` | (`item`, `anchor`) => readonly `string`[] | `...` | Mention lookup for `entry_mentions` viewer fields: accounts mentioned by one entry. |
| <a id="spanpagesize"></a> `spanPageSize` | `public` | `number` | `128` | Recorded span page size; lower it to exercise paging. |
| <a id="views"></a> `views` | `readonly` | `Map`\<`string`, `ItemViewPublish`\> | `...` | Published item views by `item/slot`. |

## Methods

### append()

> **append**(`inv`, `input`): `ItemAppendReceipt`

`item_append`: attested or opaque addition under the exact expected head.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ItemAppend` |

#### Returns

`ItemAppendReceipt`

***

### begin()

> **begin**(`inv`, `input`): `ItemStageBeginReply`

`item_save_begin`: reserve a staged replacement under the expected head.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ItemStageBegin` |

#### Returns

`ItemStageBeginReply`

***

### chunk()

> **chunk**(`inv`, `__namedParameters`): `ItemStageChunkAck`

`item_save_chunk`: the next contiguous chunk; an identical resend is acknowledged.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `__namedParameters` | \{ `bytes`: `string`; `handle`: `string`; `offset`: `number`; \} |
| `__namedParameters.bytes` | `string` |
| `__namedParameters.handle` | `string` |
| `__namedParameters.offset` | `number` |

#### Returns

`ItemStageChunkAck`

***

### create()

> **create**(`inv`, `input`, `checkMetadata`): `ItemContentReceipt`

`item_create`: place, check title binding and metadata, compose attested bytes.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ItemCreate` |
| `checkMetadata` | (`kind`, `metadata`) => `void` |

#### Returns

`ItemContentReceipt`

***

### head()

> **head**(`stored`): `OwnedItemHead`

`item_head`: the current head, size and metadata.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `stored` | [`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/) |

#### Returns

`OwnedItemHead`

***

### markRead()

> **markRead**(`item`, `actor`, `anchor`): `void`

Test hook: record that `actor` has read through `anchor`, as the renderer's read mark would.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `item` | `string` |
| `actor` | `string` |
| `anchor` | `string` |

#### Returns

`void`

***

### publish()

> **publish**(`inv`, `input`): `ItemViewPublishReceipt`

`item_view_publish`: store one page for the exact head the scope was issued for.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ItemViewPublish` |

#### Returns

`ItemViewPublishReceipt`

***

### range()

> **range**(`inv`, `__namedParameters`): `ItemRangeResult`

`item_range`: one bounded slice of an exact revision.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `__namedParameters` | \{ `item`: `string`; `length`: `number`; `offset`: `number`; `revision`: `string`; \} |
| `__namedParameters.item` | `string` |
| `__namedParameters.length` | `number` |
| `__namedParameters.offset` | `number` |
| `__namedParameters.revision` | `string` |

#### Returns

`ItemRangeResult`

***

### readable()

> **readable**(`inv`, `item`): [`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/)

Look up an item the invocation's package may read.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `item` | `string` |

#### Returns

[`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/)

***

### save()

> **save**(`inv`, `input`, `checkMetadata`): `ItemSaveReply`

`item_save`: inline, staged, attested, span replacement, metadata patch.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ItemSave` |
| `checkMetadata` | (`kind`, `metadata`) => `void` |

#### Returns

`ItemSaveReply`

***

### seal()

> **seal**(`inv`, `__namedParameters`): `ItemStageSealed`

`item_save_seal`: check length and digest.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `__namedParameters` | \{ `handle`: `string`; \} |
| `__namedParameters.handle` | `string` |

#### Returns

`ItemStageSealed`

***

### spansPage()

> **spansPage**(`inv`, `__namedParameters`): `ItemSpansPage`

`item_spans`: paged recorded spans with cursors pinned to item, revision and actor.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `__namedParameters` | \{ `cursor?`: `string`; `item`: `string`; `revision`: `string`; \} |
| `__namedParameters.cursor?` | `string` |
| `__namedParameters.item` | `string` |
| `__namedParameters.revision` | `string` |

#### Returns

`ItemSpansPage`

***

### viewContext()

> **viewContext**(`inv`, `__namedParameters`): `ItemViewContext`

`item_view_context`: a writer scope for the current head of a declared slot.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `__namedParameters` | \{ `item`: `string`; `slot`: `string`; \} |
| `__namedParameters.item` | `string` |
| `__namedParameters.slot` | `string` |

#### Returns

`ItemViewContext`

***

### viewerRead()

> **viewerRead**(`inv`, `item`): `ViewerItemState`

`viewer_state_read`: the actor's private record plus host-maintained entry counts.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `item` | `string` |

#### Returns

`ViewerItemState`

***

### viewerWrite()

> **viewerWrite**(`inv`, `input`): `ViewerItemState`

`viewer_state_write`: compare-and-set of declared, author-writable fields.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `input` | `ViewerItemStateWrite` |

#### Returns

`ViewerItemState`

***

### writable()

> **writable**(`inv`, `item`): [`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/)

Look up an item the invocation's package owns and may write now.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `inv` | [`Invocation`](/docs/dev/typescript/plugin-sdk/fake-host/invocation/interfaces/invocation/) |
| `item` | `string` |

#### Returns

[`StoredItem`](/docs/dev/typescript/plugin-sdk/fake-host/items/interfaces/storeditem/)
