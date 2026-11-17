---
title: "genui: validate/functions/validate"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [validate](/docs/dev/typescript/genui/validate/) / validate

# Function: validate()

> **validate**(`statements`, `diagnostics`, `limits`): [`Program`](/docs/dev/typescript/genui/ast/interfaces/program/)

Check parser-produced statements against catalog/accessibility/dependency contracts.
Cost grows only with this bounded AST and its dependency walks, including transitive reads per
state initializer. Allocates local symbol/reference sets and diagnostics; no locks, I/O, fsyncs
or round trips. Reports errors into diagnostics; compile discards any resulting invalid program.

## Parameters

| Parameter | Type |
| ------ | ------ |
| `statements` | readonly [`Statement`](/docs/dev/typescript/genui/ast/type-aliases/statement/)[] |
| `diagnostics` | [`DiagnosticList`](/docs/dev/typescript/genui/diagnostics/classes/diagnosticlist/) |
| `limits` | [`Budgets`](/docs/dev/typescript/genui/budgets/interfaces/budgets/) |

## Returns

[`Program`](/docs/dev/typescript/genui/ast/interfaces/program/)
