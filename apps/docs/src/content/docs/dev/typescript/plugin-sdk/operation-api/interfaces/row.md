---
title: "plugin-sdk: operation-api/interfaces/Row"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / Row

# Interface: Row

Projected item returned by a query.

## Extended by

- [`Item`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/item/)

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="computed"></a> `computed` | [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/) | Computed fields evaluated for the acting person. |
| <a id="fields"></a> `fields` | [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/) | Stored fields visible to the acting person. |
| <a id="groups"></a> `groups?` | `unknown`[] | Group keys for this row in a grouped collection. |
| <a id="id"></a> `id` | `string` | Item UUID. |
| <a id="kind"></a> `kind` | `string` | Qualified kind name. |
| <a id="records"></a> `records?` | `unknown`[] | Host-stamped records when the query requests them. |
| <a id="version"></a> `version` | `number` | Host version for a conditional write. |
