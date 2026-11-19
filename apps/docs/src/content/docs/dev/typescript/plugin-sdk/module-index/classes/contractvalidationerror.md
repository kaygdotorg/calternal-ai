---
title: "plugin-sdk: index/classes/ContractValidationError"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / ContractValidationError

# Class: ContractValidationError

Contract violation raised by SDK parsers and validators; contains a safe diagnostic.

## Extends

- `Error`

## Constructors

### Constructor

> **new ContractValidationError**(`message`): `ContractValidationError`

Create a validation failure; callers must supply a diagnostic without credentials or private input.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `message` | `string` |

#### Returns

`ContractValidationError`

#### Overrides

`Error.constructor`
