---
title: "genui: budgets/functions/checkState"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [budgets](/docs/dev/typescript/genui/budgets/) / checkState

# Function: checkState()

> **checkState**(`state`, `pos`): `void`

Reject interpreter-tagged external state after checkValue has established an acyclic JSON tree.
O(visited state entries), allocates a local traversal stack; throws Failure on a reserved tag.
No I/O, locks, fsyncs, round trips or effects.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `state` | `unknown` |
| `pos` | [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/) |

## Returns

`void`
