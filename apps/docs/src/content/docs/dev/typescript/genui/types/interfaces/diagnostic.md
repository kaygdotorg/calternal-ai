---
title: "genui: types/interfaces/Diagnostic"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [types](/docs/dev/typescript/genui/types/) / Diagnostic

# Interface: Diagnostic

Positioned diagnostic; positions count UTF-16 code units, starting at one.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="column"></a> `column` | `number` | One-based UTF-16 source column. |
| <a id="line"></a> `line` | `number` | One-based source line. |
| <a id="message"></a> `message` | `string` | Stable language-neutral diagnostic text. |
| <a id="severity"></a> `severity` | `"error"` \| `"warning"` | Error prevents activation; warning is informational. |
