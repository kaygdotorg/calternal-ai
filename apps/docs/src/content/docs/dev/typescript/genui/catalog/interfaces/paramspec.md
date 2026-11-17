---
title: "genui: catalog/interfaces/ParamSpec"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [catalog](/docs/dev/typescript/genui/catalog/) / ParamSpec

# Interface: ParamSpec

One positional or named argument in a native component/action signature.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="holds"></a> `holds?` | [`StateKind`](/docs/dev/typescript/genui/catalog/type-aliases/statekind/) | State kind required for binding arguments. |
| <a id="kind"></a> `kind` | [`ParamKind`](/docs/dev/typescript/genui/catalog/type-aliases/paramkind/) | Validation/normalization rule. |
| <a id="name"></a> `name` | `string` | Argument key in evaluated properties. |
| <a id="nullable"></a> `nullable?` | `boolean` | Required arguments that explicitly accept null. |
| <a id="optional"></a> `optional?` | `boolean` | Omitted or null means absent when true. |
| <a id="values"></a> `values?` | readonly `string`[] | Allowed strings for enum arguments. |
