---
title: "genui: ast/interfaces/Decl"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [ast](/docs/dev/typescript/genui/ast/) / Decl

# Interface: Decl

Source-ordered state initializer or top-level named value.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="name"></a> `name` | `readonly` | `string` | Identifier without a state prefix. |
| <a id="pos"></a> `pos` | `readonly` | [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/) | Source location of this syntax item. |
| <a id="value"></a> `value` | `readonly` | [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/) | Unevaluated expression. |
