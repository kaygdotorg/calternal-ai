---
title: "plugin-sdk: fake-host/schemas/classes/ContractSchemas"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/schemas](/docs/dev/typescript/plugin-sdk/fake-host/schemas/) / ContractSchemas

# Class: ContractSchemas

Validators for catalogue definitions and author-declared schemas. Author
schemas resolve `host-catalogue-v2.json#/$defs/...` against the pinned
catalogue. Compiled validators are cached by key.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="catalogue"></a> `catalogue` | `readonly` | `Record`\<`string`, `unknown`\> | The parsed pinned catalogue. |

## Methods

### check()

> **check**(`pointer`, `value`, `what`): `void`

Validate against one catalogue pointer such as `#/$defs/ItemCreate`; throws `400 invalid_shape`.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `pointer` | `string` |
| `value` | `unknown` |
| `what` | `string` |

#### Returns

`void`

***

### checkDeclared()

> **checkDeclared**(`key`, `schema`, `value`, `what`): `void`

Validate against an author-declared schema cached under `key`; throws `400 invalid_shape`.

#### Parameters

| Parameter | Type |
| ------ | ------ |
| `key` | `string` |
| `schema` | `unknown` |
| `value` | `unknown` |
| `what` | `string` |

#### Returns

`void`

***

### get()

> `static` **get**(): `ContractSchemas`

One shared instance; compiling the catalogue costs about a second.

#### Returns

`ContractSchemas`
