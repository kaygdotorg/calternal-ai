---
title: "genui: types/interfaces/UiNode"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [types](/docs/dev/typescript/genui/types/) / UiNode

# Interface: UiNode

Evaluated native component description; this package never draws views or performs actions.

## Indexable

> \[`key`: `string`\]: [`Value`](/docs/dev/typescript/genui/types/type-aliases/value/)

Structural compatibility with JSON values.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="key"></a> `key` | `string` | Source/iteration-derived identity, stable across state changes. |
| <a id="kind"></a> `kind` | `"node"` | Interpreter-owned value discriminator. |
| <a id="props"></a> `props` | `Record`\<`string`, [`Value`](/docs/dev/typescript/genui/types/type-aliases/value/)\> | Normalized arguments keyed by catalog parameter names. |
| <a id="type"></a> `type` | `string` | Catalog name, or Error when a component expression fails. |
