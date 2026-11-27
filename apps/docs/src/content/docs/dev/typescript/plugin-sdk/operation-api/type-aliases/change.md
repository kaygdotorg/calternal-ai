---
title: "plugin-sdk: operation-api/type-aliases/Change"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / Change

# Type Alias: Change

> **Change** = \{ `add`: \{ `body?`: `string`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `kind`: `string`; \}; \} \| \{ `change`: \{ `base?`: `number`; `body?`: `string`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \} \| \{ `perform`: \{ `base?`: `number`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `operation`: `string`; \}; \} \| \{ `remove`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \} \| \{ `append record`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `record`: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/); \}; \} \| \{ `change record`: \{ `anchor`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `record`: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/); \}; \} \| \{ `remove record`: \{ `anchor`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

Changes supported by the host's atomic write route.

## Union Members

### Type Literal

\{ `add`: \{ `body?`: `string`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `kind`: `string`; \}; \}

#### add

> **add**: `object`

Create an item with initial values.

##### Type Declaration

##### add.body?

> `optional` **body?**: `string`

Markdown body to store.

##### add.fields?

> `optional` **fields?**: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/)

Declared field values to store.

##### add.kind

> **kind**: `string`

Qualified kind in which to create the item.

***

### Type Literal

\{ `change`: \{ `base?`: `number`; `body?`: `string`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

#### change

> **change**: `object`

Update fields or Markdown.

##### Type Declaration

##### change.base?

> `optional` **base?**: `number`

Required current version for a conditional change.

##### change.body?

> `optional` **body?**: `string`

Markdown body to store.

##### change.fields?

> `optional` **fields?**: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/)

Declared field values to store.

##### change.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

UUID of the item addressed by the change.

***

### Type Literal

\{ `perform`: \{ `base?`: `number`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `operation`: `string`; \}; \}

#### perform

> **perform**: `object`

Evaluate a declared item assignment and commit its changes atomically.

##### Type Declaration

##### perform.base?

> `optional` **base?**: `number`

Required current version for a conditional change.

##### perform.fields?

> `optional` **fields?**: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/)

Declared field values to store.

##### perform.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

UUID of the item addressed by the change.

##### perform.operation

> **operation**: `string`

Name of the declared operation to evaluate on the current item.

***

### Type Literal

\{ `remove`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

#### remove

> **remove**: `object`

Remove an item.

##### Type Declaration

##### remove.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

UUID of the item addressed by the change.

***

### Type Literal

\{ `append record`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `record`: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/); \}; \}

#### append record

> **append record**: `object`

Append one record and let the host stamp it.

##### Type Declaration

##### append record.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

UUID of the item addressed by the change.

##### append record.record

> **record**: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/)

Unstamped record fields and body.

***

### Type Literal

\{ `change record`: \{ `anchor`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `record`: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/); \}; \}

#### change record

> **change record**: `object`

Change the record identified by its anchor.

##### Type Declaration

##### change record.anchor

> **anchor**: `string`

Anchor in the item's record stream.

##### change record.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

UUID of the item addressed by the change.

##### change record.record

> **record**: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/)

Unstamped record fields and body.

***

### Type Literal

\{ `remove record`: \{ `anchor`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

#### remove record

> **remove record**: `object`

Remove the record identified by its anchor.

##### Type Declaration

##### remove record.anchor

> **anchor**: `string`

Anchor in the item's record stream.

##### remove record.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

UUID of the item addressed by the change.
