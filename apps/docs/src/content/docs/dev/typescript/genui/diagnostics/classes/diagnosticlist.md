---
title: "genui: diagnostics/classes/DiagnosticList"
---

<!-- Generated from: packages/genui/src/ast.ts, packages/genui/src/budgets.ts, packages/genui/src/catalog.ts, packages/genui/src/diagnostics.ts, packages/genui/src/evaluate.ts, packages/genui/src/index.ts, packages/genui/src/lexer.ts, packages/genui/src/parser.ts, packages/genui/src/types.ts, packages/genui/src/validate.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/genui**](/docs/dev/typescript/genui/)

***

[@calternal/genui](/docs/dev/typescript/genui/) / [diagnostics](/docs/dev/typescript/genui/diagnostics/) / DiagnosticList

# Class: DiagnosticList

Collects up to 20 diagnostics, dropping exact duplicates.

## Constructors

### Constructor

> **new DiagnosticList**(): `DiagnosticList`

#### Returns

`DiagnosticList`

## Properties

| Property | Modifier | Type | Default value | Description |
| ------ | ------ | ------ | ------ | ------ |
| <a id="haserrors"></a> `hasErrors` | `public` | `boolean` | `false` | True once any error has been reported, including after the item cap. |
| <a id="items"></a> `items` | `readonly` | [`Diagnostic`](/docs/dev/typescript/genui/types/interfaces/diagnostic/)[] | `[]` | Positioned unique diagnostics in reporting order, capped at twenty. |

## Methods

### report()

> **report**(`pos`, `message`, `severity?`): `void`

Record a unique diagnostic in O(message bytes), bounded to twenty stored items; no I/O or locks.

#### Parameters

| Parameter | Type | Default value |
| ------ | ------ | ------ |
| `pos` | [`Pos`](/docs/dev/typescript/genui/diagnostics/interfaces/pos/) | `undefined` |
| `message` | `string` | `undefined` |
| `severity?` | `"error"` \| `"warning"` | `"error"` |

#### Returns

`void`
