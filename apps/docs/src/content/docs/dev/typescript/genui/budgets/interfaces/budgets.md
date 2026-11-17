---
title: "genui: budgets/interfaces/Budgets"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [budgets](/docs/dev/typescript/genui/budgets/) / Budgets

# Interface: Budgets

Hard ceilings for one description and each evaluation; callers may lower these limits.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="depth"></a> `depth` | `number` | Maximum source/AST/value and evaluation dependency depth. |
| <a id="listlength"></a> `listLength` | `number` | Maximum list length, including range/map outputs. |
| <a id="nodes"></a> `nodes` | `number` | Maximum AST expressions and maximum produced native nodes separately. |
| <a id="serializedbytes"></a> `serializedBytes` | `number` | Maximum UTF-8 bytes in source, external state, AST or evaluated output separately. |
| <a id="stateentries"></a> `stateEntries` | `number` | Maximum declared or supplied top-level state entries. |
| <a id="steps"></a> `steps` | `number` | Maximum expression visits in a complete initialization/evaluation pass. |
