---
title: "genui: catalog/interfaces/ComponentSpec"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [catalog](/docs/dev/typescript/genui/catalog/) / ComponentSpec

# Interface: ComponentSpec

Pure-data component/action contract shared with the Rust evaluator.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="accessibility"></a> `accessibility?` | `object` | Mandatory nonblank accessible names/descriptions; checked again after evaluation. |
| `accessibility.required` | readonly `string`[] | Accessible argument names that must be present and resolve to nonblank text. |
| <a id="group"></a> `group?` | `string` | Native catalog grouping; absent for actions. |
| <a id="params"></a> `params` | readonly [`ParamSpec`](/docs/dev/typescript/genui/catalog/interfaces/paramspec/)[] | Parameters in positional order. |
| <a id="summary"></a> `summary?` | `string` | Explanation of the component's data shape. |
