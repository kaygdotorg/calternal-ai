---
title: "plugin-sdk: operation/functions/defineOperation"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation](/docs/dev/typescript/plugin-sdk/operation/) / defineOperation

# Function: defineOperation()

> **defineOperation**\<`I`, `O`, `M`\>(`declaration`): [`DefinedOperation`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/definedoperation/)\<[`SchemaType`](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schematype/)\<`I`\>, [`SchemaType`](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schematype/)\<`O`\>, `M`\[`number`\]\>

Declare an operation and, optionally, its handler. A string `target` names
the target kind; the field is the input's single ItemRef property. Throws at
authoring time when that field is missing or ambiguous. The return value is
the full manifest entry, with the inferred target written out.

## Type Parameters

| Type Parameter |
| ------ |
| <a id="type-parameter-i"></a> `I` *extends* [`Schema`](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schema/) |
| <a id="type-parameter-o"></a> `O` *extends* [`Schema`](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schema/) |
| <a id="type-parameter-m"></a> `M` *extends* readonly `HostMethod`[] |

## Parameters

| Parameter | Type |
| ------ | ------ |
| `declaration` | `OperationDeclaration`\<`I`, `O`, `M`\> |

## Returns

[`DefinedOperation`](/docs/dev/typescript/plugin-sdk/operation/type-aliases/definedoperation/)\<[`SchemaType`](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schematype/)\<`I`\>, [`SchemaType`](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schematype/)\<`O`\>, `M`\[`number`\]\>
