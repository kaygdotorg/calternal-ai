---
title: "genui: ast/interfaces/Program"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [ast](/docs/dev/typescript/genui/ast/) / Program

# Interface: Program

A validated program. Only `compile` creates one.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="named"></a> `named` | `readonly` | `ReadonlyMap`\<`string`, [`Decl`](/docs/dev/typescript/genui/ast/interfaces/decl/)\> | Top-level definitions in source insertion order. |
| <a id="roots"></a> `roots` | `readonly` | readonly [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)[] | Bare expressions in source order, or the named value `root`. |
| <a id="serializedbytes"></a> `serializedBytes` | `readonly` | `number` | Serialized statement bytes, retained so evaluation can recheck a lowered byte budget. |
| <a id="states"></a> `states` | `readonly` | readonly [`Decl`](/docs/dev/typescript/genui/ast/interfaces/decl/)[] | Local state initializers in source declaration order. |
