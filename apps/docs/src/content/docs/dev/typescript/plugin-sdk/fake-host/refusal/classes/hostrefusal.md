---
title: "plugin-sdk: fake-host/refusal/classes/HostRefusal"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/refusal](/docs/dev/typescript/plugin-sdk/fake-host/refusal/) / HostRefusal

# Class: HostRefusal

A broker or admission refusal: HTTP-like `status`, stable `code`, a plain
message and, for a missing declaration, the manifest path and the change
that fixes it.

## Extends

- `Error`

## Constructors

### Constructor

> **new HostRefusal**(`status`, `code`, `message`, `path?`, `change?`): `HostRefusal`

Create a refusal; `message` says what happened and what to do next.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `status` | `number` |
| `code` | `string` |
| `message` | `string` |
| `path?` | `string` |
| `change?` | `string` |

#### Returns

`HostRefusal`

#### Overrides

`Error.constructor`

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="change"></a> `change?` | `readonly` | `string` | For a missing declaration, the declaration to add. |
| <a id="code"></a> `code` | `readonly` | `string` | Stable machine-readable code, such as `not_declared`. |
| <a id="path"></a> `path?` | `readonly` | `string` | For a missing declaration, the manifest path to change. |
| <a id="status"></a> `status` | `readonly` | `number` | HTTP-like status, such as 403 or 409. |
