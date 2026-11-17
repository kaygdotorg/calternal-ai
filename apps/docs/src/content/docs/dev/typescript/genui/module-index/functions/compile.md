---
title: "genui: index/functions/compile"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [index](/docs/dev/typescript/genui/module-index/) / compile

# Function: compile()

> **compile**(`source`, `overrides?`): [`Compilation`](/docs/dev/typescript/genui/module-index/interfaces/compilation/)

Parse and validate the upstream expression syntax against the native data catalog.
Errors prevent activation. Costs O(source bytes + bounded AST/dependency walks); allocates
tokens, AST and per-program symbol tables. No locks, I/O, fsyncs or network round trips.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `source` | `string` |
| `overrides?` | `Partial`\<[`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/)\> |

## Returns

[`Compilation`](/docs/dev/typescript/genui/module-index/interfaces/compilation/)
