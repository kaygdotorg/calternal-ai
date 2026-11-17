---
title: "genui: evaluate/functions/initialState"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [evaluate](/docs/dev/typescript/genui/evaluate/) / initialState

# Function: initialState()

> **initialState**(`program`, `overrides?`): `object`

Initialize local state without running roots. Costs O(initializer steps × bounded value bytes + local name lookups),
bounded by the supplied budgets; allocates state/cache only. No locks, I/O or effects.
A failed initializer discards state and returns positioned diagnostics.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `program` | [`Program`](/docs/dev/typescript/genui/ast/interfaces/program/) |
| `overrides?` | `Partial`\<[`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/)\> |

## Returns

### diagnostics

> **diagnostics**: [`Diagnostic`](/docs/dev/typescript/genui/types/interfaces/diagnostic/)[]

Positioned errors collected in deterministic order.

### state

> **state**: `Record`\<`string`, [`Value`](/docs/dev/typescript/genui/types/type-aliases/value/)\>

Local state snapshot; names exclude the dollar prefix.
