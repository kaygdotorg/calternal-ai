---
title: "plugin-sdk: operation-api/interfaces/Item"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / Item

# Interface: Item

A visible item and its Markdown and records.

## Extends

- [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/)

## Properties

| Property | Type | Description | Inherited from |
| ------ | ------ | ------ | ------ |
| <a id="body"></a> `body` | `string` | Markdown outside the item frontmatter. | - |
| <a id="computed"></a> `computed` | [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/) | Computed fields evaluated for the acting person. | [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/).[`computed`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/#computed) |
| <a id="fields"></a> `fields` | [`Fields`](/docs/dev/typescript/plugin-sdk/operation-api/type-aliases/fields/) | Stored fields visible to the acting person. | [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/).[`fields`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/#fields) |
| <a id="groups"></a> `groups?` | `unknown`[] | Group keys for this row in a grouped collection. | [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/).[`groups`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/#groups) |
| <a id="id"></a> `id` | `string` | Item UUID. | [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/).[`id`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/#id) |
| <a id="kind"></a> `kind` | `string` | Qualified kind name. | [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/).[`kind`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/#kind) |
| <a id="records"></a> `records` | `unknown`[] | Records returned by the host, with ledger stamps. | - |
| <a id="version"></a> `version` | `number` | Host version for a conditional write. | [`Row`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/).[`version`](/docs/dev/typescript/plugin-sdk/operation-api/interfaces/row/#version) |
