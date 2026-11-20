---
title: "plugin-sdk: broker-helpers/classes/ContributionEndedError"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / ContributionEndedError

# Class: ContributionEndedError

A contribution that ended with a state other than `ok`.

## Extends

- `Error`

## Constructors

### Constructor

> **new ContributionEndedError**(`state`): `ContributionEndedError`

Create an error naming the terminal state.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `state` | `"failed"` \| `"stopped"` |

#### Returns

`ContributionEndedError`

#### Overrides

`Error.constructor`

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="state"></a> `state` | `readonly` | `"failed"` \| `"stopped"` | The terminal state. |
