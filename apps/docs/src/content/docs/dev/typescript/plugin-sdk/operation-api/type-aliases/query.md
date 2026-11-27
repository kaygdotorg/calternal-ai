---
title: "plugin-sdk: operation-api/type-aliases/Query"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / Query

# Type Alias: Query

> **Query** = \{ `cursor?`: `string`; `from`: `string`; `limit?`: `number`; `search?`: `string`; `sort?`: readonly `string`[]; `where?`: `string`; \} \| \{ `collection`: `string`; `cursor?`: `string`; `limit?`: `number`; `search?`: `string`; \}

A kind query or a stored collection query accepted by the standalone host.

## Union Members

### Type Literal

\{ `cursor?`: `string`; `from`: `string`; `limit?`: `number`; `search?`: `string`; `sort?`: readonly `string`[]; `where?`: `string`; \}

#### cursor?

> `optional` **cursor?**: `string`

Opaque cursor from the previous page.

#### from

> **from**: `string`

Qualified kind name.

#### limit?

> `optional` **limit?**: `number`

Page size up to 200.

#### search?

> `optional` **search?**: `string`

Case-insensitive substring in the kind’s declared search fields.

#### sort?

> `optional` **sort?**: readonly `string`[]

Ordered sort expressions, up to eight for an ad hoc query.

#### where?

> `optional` **where?**: `string`

Formula predicate evaluated by the host.

***

### Type Literal

\{ `collection`: `string`; `cursor?`: `string`; `limit?`: `number`; `search?`: `string`; \}

#### collection

> **collection**: `string`

Qualified collection name.

#### cursor?

> `optional` **cursor?**: `string`

Opaque cursor from the previous page.

#### limit?

> `optional` **limit?**: `number`

Page size up to 200.

#### search?

> `optional` **search?**: `string`

Case-insensitive substring in the kind’s declared search fields.
