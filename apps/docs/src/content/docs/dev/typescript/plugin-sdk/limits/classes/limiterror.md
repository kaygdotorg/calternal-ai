---
title: "plugin-sdk: limits/classes/LimitError"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [limits](/docs/dev/typescript/plugin-sdk/limits/) / LimitError

# Class: LimitError

A value over a contract limit, raised by an SDK preflight before any
effect. `limit` is the exported constant name, `value` the measured size.

## Extends

- `RangeError`

## Constructors

### Constructor

> **new LimitError**(`limit`, `value`, `what`): `LimitError`

Create a refusal naming the limit, its bound and the measured value.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `limit` | keyof [`Limits`](/docs/dev/typescript/plugin-sdk/generated/limits/interfaces/limits/) |
| `value` | `number` |
| `what` | `string` |

#### Returns

`LimitError`

#### Overrides

`RangeError.constructor`

## Properties

| Property | Modifier | Type | Default value | Description |
| ------ | ------ | ------ | ------ | ------ |
| <a id="code"></a> `code` | `readonly` | `"limit_exceeded"` | `"limit_exceeded"` | Stable code shared by every preflight refusal. |
| <a id="limit"></a> `limit` | `readonly` | keyof [`Limits`](/docs/dev/typescript/plugin-sdk/generated/limits/interfaces/limits/) | `undefined` | The limit constant that refused the value. |
| <a id="value"></a> `value` | `readonly` | `number` | `undefined` | The measured value. |
