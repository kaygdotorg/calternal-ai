---
title: "genui: budgets/classes/Failure"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [budgets](/docs/dev/typescript/genui/budgets/) / Failure

# Class: Failure

A deterministic interpreter/validation failure with a source position.

## Extends

- `Error`

## Constructors

### Constructor

> **new Failure**(`pos`, `message`, `limit?`): `Failure`

Construct a source-positioned failure. O(message bytes) error/stack allocation,
no locks, I/O, fsyncs or network. Resource failures terminate the complete pass.

#### Parameters

| Parameter | Type | Default value |
| ------ | ------ | ------ |
| `pos` | [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/) | `undefined` |
| `message` | `string` | `undefined` |
| `limit?` | `boolean` | `false` |

#### Returns

`Failure`

#### Overrides

`Error.constructor`

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="limit"></a> `limit` | `readonly` | `boolean` | True for a resource ceiling: terminate the complete evaluation. |
| <a id="pos"></a> `pos` | `readonly` | [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/) | Position of the expression that failed. |
