---
title: "genui: index/interfaces/Compilation"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [index](/docs/dev/typescript/genui/module-index/) / Compilation

# Interface: Compilation

Parsed/validated program; syntax or validation errors always leave program null.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="diagnostics"></a> `diagnostics` | [`Diagnostic`](/docs/dev/typescript/genui/types/interfaces/diagnostic/)[] | Source-ordered diagnostics, capped at twenty. |
| <a id="program"></a> `program` | [`Program`](/docs/dev/typescript/genui/ast/interfaces/program/) \| `null` | Accepted program; compile owns creation. |
| <a id="statements"></a> `statements` | [`Statement`](/docs/dev/typescript/genui/ast/type-aliases/statement/)[] | Parsed statement AST, including statements with semantic errors. |
