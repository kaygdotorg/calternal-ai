---
title: "genui: budgets/functions/budgets"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [budgets](/docs/dev/typescript/genui/budgets/) / budgets

# Function: budgets()

> **budgets**(`overrides?`): [`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/)

Validate host-lowered limits; invalid fields throw Error. O(number of supplied fields),
one bounded record allocation; no locks, I/O, fsyncs or round trips.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `overrides?` | `Partial`\<[`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/)\> |

## Returns

[`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/)
