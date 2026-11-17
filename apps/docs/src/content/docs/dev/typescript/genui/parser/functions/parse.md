---
title: "genui: parser/functions/parse"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [parser](/docs/dev/typescript/genui/parser/) / parse

# Function: parse()

> **parse**(`tokens`, `diagnostics`): [`Statement`](/docs/dev/typescript/genui/ast/type-aliases/statement/)[]

Parse prebounded lexer tokens in O(token count), allocating a statement/expression tree.
Reports syntax errors and skips their statement; nesting is capped before recursive tree walks.
No locks, I/O, fsyncs or round trips. Compile applies caller-specific AST budgets afterward.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `tokens` | readonly [`Token`](/docs/dev/typescript/genui/lexer/type-aliases/token/)[] |
| `diagnostics` | [`DiagnosticList`](/docs/dev/typescript/genui/diagnostics/classes/diagnosticlist/) |

## Returns

[`Statement`](/docs/dev/typescript/genui/ast/type-aliases/statement/)[]
