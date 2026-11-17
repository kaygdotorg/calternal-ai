---
title: "genui: validate/functions/slots"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [validate](/docs/dev/typescript/genui/validate/) / slots

# Function: slots()

> **slots**(`expr`, `spec`): `Map`\<`string`, [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)\>

Associate arguments with catalog parameter names; first occurrence wins after static duplicate checks.
O(argument count), one bounded Map allocation; no locks, I/O, fsyncs or round trips.

## Parameters

| Parameter | Type | Description |
| ------ | ------ | ------ |
| `expr` | \{ `args`: readonly [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)[]; `name`: `string`; `named`: readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[]; `pos`: [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/); `type`: `"call"`; \} | - |
| `expr.args` | readonly [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)[] | Positional call arguments in source order. |
| `expr.name` | `string` | Identifier without a state prefix. |
| `expr.named` | readonly [`NamedArg`](/docs/dev/typescript/genui/ast/interfaces/namedarg/)[] | Top-level definitions in source insertion order. |
| `expr.pos` | [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/) | Source location of this syntax item. |
| `expr.type` | `"call"` | Syntax or interpreter-value discriminator; determines the other fields. |
| `spec` | [`ComponentSpec`](/docs/dev/typescript/genui/catalog/interfaces/componentspec/) | - |

## Returns

`Map`\<`string`, [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)\>
