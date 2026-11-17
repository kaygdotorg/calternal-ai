---
title: "genui: validate/functions/checkProgram"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [validate](/docs/dev/typescript/genui/validate/) / checkProgram

# Function: checkProgram()

> **checkProgram**(`program`, `limits`): `void`

Enforce AST node/depth/list/state/byte budgets before validation or evaluation.
O(AST size), allocating a bounded traversal stack; throws Failure on a ceiling, positioned at the
first expression (source order) crossing a node/depth/list ceiling, else at 1:1.
No locks, I/O, fsyncs or network. Accepts only parser-produced acyclic ASTs.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `program` | [`Program`](/docs/dev/typescript/genui/ast/interfaces/program/) |
| `limits` | [`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/) |

## Returns

`void`
