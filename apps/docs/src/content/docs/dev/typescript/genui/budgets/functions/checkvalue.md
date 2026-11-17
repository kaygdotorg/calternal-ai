---
title: "genui: budgets/functions/checkValue"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [budgets](/docs/dev/typescript/genui/budgets/) / checkValue

# Function: checkValue()

> **checkValue**(`value`, `limits`, `pos`): `void`

Validate JSON before recursive evaluation. O(visited JSON bytes), with a traversal stack
and active-object set; throws Failure for ceilings, cycles, unsupported values or forbidden keys.
No locks, I/O, fsyncs or network. Shared acyclic values are visited once per occurrence.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `value` | `unknown` |
| `limits` | [`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/) |
| `pos` | [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/) |

## Returns

`void`
