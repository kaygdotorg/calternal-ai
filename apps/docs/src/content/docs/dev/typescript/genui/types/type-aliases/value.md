---
title: "genui: types/type-aliases/Value"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [types](/docs/dev/typescript/genui/types/) / Value

# Type Alias: Value

> **Value** = [`Scalar`](/docs/dev/typescript/genui/types/type-aliases/scalar/) \| `Value`[] \| \{\[`key`: `string`\]: `Value`; \}

JSON-only evaluator value; tagged nodes, bindings and actions are ordinary records.

## Union Members

[`Scalar`](/docs/dev/typescript/genui/types/type-aliases/scalar/)

***

`Value`[]

***

### Type Literal

\{\[`key`: `string`\]: `Value`; \}

### Index Signature

\[`key`: `string`\]: `Value`

JSON object entries keyed by literal property names.
