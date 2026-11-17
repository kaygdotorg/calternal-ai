---
title: "genui: validate/functions/children"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [validate](/docs/dev/typescript/genui/validate/) / children

# Function: children()

> **children**(`expr`): readonly [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)[]

Return immediate children in deterministic traversal order. O(immediate child count),
allocates at most one small result array; no locks, I/O, fsyncs or network.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `expr` | [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/) |

## Returns

readonly [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)[]
