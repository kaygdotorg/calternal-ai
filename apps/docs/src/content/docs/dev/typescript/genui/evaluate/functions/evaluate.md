---
title: "genui: evaluate/functions/evaluate"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [evaluate](/docs/dev/typescript/genui/evaluate/) / evaluate

# Function: evaluate()

> **evaluate**(`program`, `state?`, `overrides?`): [`Evaluation`](/docs/dev/typescript/genui/evaluate/interfaces/evaluation/)

Initialize/evaluate a validated program, with optional external state. Limits are rechecked on
every call, including AST, state, produced nodes, steps, value depth and serialized output.
Costs O(expression steps × bounded value bytes + local name lookups), bounded by Budgets; allocates a local
state copy, named-value cache and output tree. No locks, I/O, fsyncs, network or effects.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `program` | [`Program`](/docs/dev/typescript/genui/ast/interfaces/program/) |
| `state?` | `Readonly`\<`Record`\<`string`, [`Value`](/docs/dev/typescript/genui/types/type-aliases/value/)\>\> |
| `overrides?` | `Partial`\<[`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/)\> |

## Returns

[`Evaluation`](/docs/dev/typescript/genui/evaluate/interfaces/evaluation/)
