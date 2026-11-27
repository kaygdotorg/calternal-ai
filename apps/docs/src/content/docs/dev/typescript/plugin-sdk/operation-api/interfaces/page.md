---
title: "plugin-sdk: operation-api/interfaces/Page"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / Page

# Interface: Page

One page of rows and totals from the host.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="cursor"></a> `cursor` | `string` \| `null` | Opaque next-page cursor, or null on the last page. |
| <a id="groups"></a> `groups` | `object`[] | Group key, matched row count and pre-page aggregates for each visible group. |
| <a id="namedtotals"></a> `namedTotals` | [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/) | Computed collection header totals keyed by declared name. |
| <a id="rows"></a> `rows` | [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/)[] | Rows in the requested order. |
| <a id="totals"></a> `totals` | `unknown`[] \| `Record`\<`string`, `never`\> | Overall aggregates in declaration order, or an empty object without aggregates. |
