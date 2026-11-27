---
title: "plugin-sdk: operation-api/type-aliases/Change"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / Change

# Type Alias: Change

> **Change** = \{ `add`: \{ `body?`: `string`; `fields`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `kind`: `string`; \}; \} \| \{ `change`: \{ `base?`: `number`; `body?`: `string`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \} \| \{ `remove`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \} \| \{ `move`: \{ `file?`: `string`; `folder?`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \} \| \{ `append record`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `record`: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/); \}; \} \| \{ `change record`: \{ `anchor`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `record`: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/); \}; \} \| \{ `remove record`: \{ `anchor`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

One write in the atomic change list. Each entry names exactly one change.

## Union Members

### Type Literal

\{ `add`: \{ `body?`: `string`; `fields`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `kind`: `string`; \}; \}

#### add

> **add**: `object`

Create an item with initial values.

##### Type Declaration

##### add.body?

> `optional` **body?**: `string`

Initial Markdown body.

##### add.fields

> **fields**: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/)

Initial declared field values.

##### add.kind

> **kind**: `string`

Kind in which the item belongs.

***

### Type Literal

\{ `change`: \{ `base?`: `number`; `body?`: `string`; `fields?`: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/); `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

#### change

> **change**: `object`

Update fields or Markdown.

##### Type Declaration

##### change.base?

> `optional` **base?**: `number`

Required current version for a conditional update.

##### change.body?

> `optional` **body?**: `string`

Replacement Markdown body.

##### change.fields?

> `optional` **fields?**: [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/)

New values for the named fields.

##### change.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

Item to update.

***

### Type Literal

\{ `remove`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

#### remove

> **remove**: `object`

Remove an item.

##### Type Declaration

##### remove.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

Item to remove.

***

### Type Literal

\{ `move`: \{ `file?`: `string`; `folder?`: `string`; `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); \}; \}

#### move

> **move**: `object`

Move an item within its kind.

##### Type Declaration

##### move.file?

> `optional` **file?**: `string`

Destination filename within the kind.

##### move.folder?

> `optional` **folder?**: `string`

Destination subfolder within the kind.

##### move.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

Item to move.

***

### Type Literal

\{ `append record`: \{ `item`: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/); `record`: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/); \}; \}

#### append record

> **append record**: `object`

Append one record and let the host stamp it.

##### Type Declaration

##### append record.item

> **item**: [`ItemId`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/itemid/)

Item whose record stream receives the append.

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

Item holding the record.

##### change record.record

> **record**: [`RecordBody`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/recordbody/)

Updated declared fields and body.

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

Item holding the record.
