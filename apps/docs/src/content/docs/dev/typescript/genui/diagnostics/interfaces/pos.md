---
title: "genui: diagnostics/interfaces/Pos"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [diagnostics](/docs/dev/typescript/genui/diagnostics/) / Pos

# Interface: Pos

A 1-based source position. Tokens and AST nodes carry one.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="column"></a> `column` | `readonly` | `number` | One-based UTF-16 source column. |
| <a id="line"></a> `line` | `readonly` | `number` | One-based source line. |
