---
title: "plugin-sdk: operation/type-aliases/DefinedOperation"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation](/docs/dev/typescript/plugin-sdk/operation/) / DefinedOperation

# Type Alias: DefinedOperation\<I, O, M\>

> **DefinedOperation**\<`I`, `O`, `M`\> = [`ManifestOperation`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/manifestoperation/) & `object`

A manifest operation entry carrying its handler under a symbol key, which JSON serialization omits.

## Type Parameters

| Type Parameter | Default type |
| ------ | ------ |
| <a id="type-parameter-i"></a> `I` | `unknown` |
| <a id="type-parameter-o"></a> `O` | `unknown` |
| <a id="type-parameter-m"></a> `M` *extends* `string` | [`BrokerMethod`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokermethod/) |

## Type Declaration

### \[handlerKey\]?

> `readonly` `optional` **\[handlerKey\]?**: [`OperationHandler`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/operationhandler/)\<`I`, `O`, `M`\>

The handler, when the operation has worker code.
