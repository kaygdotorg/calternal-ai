---
title: "genui: lexer/functions/tokenize"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [lexer](/docs/dev/typescript/genui/lexer/) / tokenize

# Function: tokenize()

> **tokenize**(`source`, `diagnostics`): [`Token`](/docs/dev/typescript/genui/lexer/type-aliases/token/)[]

Tokenize bounded source in O(source bytes), allocating tokens and decoded strings; no I/O, locks or effects.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `source` | `string` |
| `diagnostics` | [`DiagnosticList`](/docs/dev/typescript/genui/diagnostics/classes/diagnosticlist/) |

## Returns

[`Token`](/docs/dev/typescript/genui/lexer/type-aliases/token/)[]
