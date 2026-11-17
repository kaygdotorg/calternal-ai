---
title: "genui: index/functions/formatDiagnostics"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [index](/docs/dev/typescript/genui/module-index/) / formatDiagnostics

# Function: formatDiagnostics()

> **formatDiagnostics**(`diagnostics`): `string`

Render diagnostics as source-positioned lines. O(total message bytes), one result string
allocation plus line strings; no locks, I/O, fsyncs or network round trips.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `diagnostics` | readonly [`Diagnostic`](/docs/dev/typescript/genui/types/interfaces/diagnostic/)[] |

## Returns

`string`
