---
title: "genui: budgets/functions/byteLength"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [budgets](/docs/dev/typescript/genui/budgets/) / byteLength

# Function: byteLength()

> **byteLength**(`text`): `number`

Count UTF-8 bytes rather than UTF-16 positions. O(text length), allocates an encoded
byte buffer; no locks, I/O, fsyncs or round trips.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `text` | `string` |

## Returns

`number`
