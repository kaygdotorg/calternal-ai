---
title: "genui: ast/type-aliases/Statement"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [ast](/docs/dev/typescript/genui/ast/) / Statement

# Type Alias: Statement

> **Statement** = `object` & [`Decl`](/docs/dev/typescript/genui/ast/interfaces/decl/) \| `object` & [`Decl`](/docs/dev/typescript/genui/ast/interfaces/decl/) \| \{ `kind`: `"root"`; `value`: [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/); \}

A state declaration, named declaration or bare render root.

## Union Members

`object` & [`Decl`](/docs/dev/typescript/genui/ast/interfaces/decl/)

***

`object` & [`Decl`](/docs/dev/typescript/genui/ast/interfaces/decl/)

***

### Type Literal

\{ `kind`: `"root"`; `value`: [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/); \}

#### kind

> `readonly` **kind**: `"root"`

Statement or interpreter-value discriminator.

#### value

> `readonly` **value**: [`Expr`](/docs/dev/typescript/genui/ast/type-aliases/expr/)

Literal value or unevaluated expression belonging to this syntax item.
