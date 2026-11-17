---
title: "genui: evaluate/interfaces/Evaluation"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [evaluate](/docs/dev/typescript/genui/evaluate/) / Evaluation

# Interface: Evaluation

One complete evaluation snapshot, safe to hand to a native tree walker.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="diagnostics"></a> `diagnostics` | [`Diagnostic`](/docs/dev/typescript/genui/types/interfaces/diagnostic/)[] | Deterministic errors in evaluation order. |
| <a id="roots"></a> `roots` | [`UiNode`](/docs/dev/typescript/genui/types/interfaces/uinode/)[] | Root native descriptions; resource failures discard the entire output. |
| <a id="state"></a> `state` | `Record`\<`string`, [`Value`](/docs/dev/typescript/genui/types/type-aliases/value/)\> | Initialized state, or the supplied state snapshot. |
