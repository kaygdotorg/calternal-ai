---
title: "plugin-sdk: schema/type-aliases/SchemaType"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [schema](/docs/dev/typescript/plugin-sdk/schema/) / SchemaType

# Type Alias: SchemaType\<S\>

> **SchemaType**\<`S`\> = `S` *extends* [`CatalogueRefSchema`](/docs/dev/typescript/plugin-sdk/schema/interfaces/cataloguerefschema/)\<infer Name\> ? `Name` *extends* keyof `CatalogueDefinitions` ? `CatalogueDefinitions`\[`Name`\] : `unknown` : `S` *extends* [`ConstSchema`](/docs/dev/typescript/plugin-sdk/schema/interfaces/constschema/)\<infer Value\> ? `Value` : `S` *extends* [`EnumSchema`](/docs/dev/typescript/plugin-sdk/schema/interfaces/enumschema/)\<readonly infer Value[]\> ? `Value` : `S` *extends* [`OneOfSchema`](/docs/dev/typescript/plugin-sdk/schema/interfaces/oneofschema/)\<readonly infer Variant[]\> ? `SchemaType`\<`Variant`\> : `S` *extends* [`AnyOfSchema`](/docs/dev/typescript/plugin-sdk/schema/interfaces/anyofschema/)\<readonly infer Variant[]\> ? `SchemaType`\<`Variant`\> : `S` *extends* [`PropertiesKeyword`](/docs/dev/typescript/plugin-sdk/schema/interfaces/propertieskeyword/)\<infer P\> ? `ObjectValue`\<`S`, `P`\> : `S` *extends* [`ItemsKeyword`](/docs/dev/typescript/plugin-sdk/schema/interfaces/itemskeyword/)\<infer Items\> ? `SchemaType`\<`Items`\>[] : `S` *extends* [`TypeKeyword`](/docs/dev/typescript/plugin-sdk/schema/interfaces/typekeyword/)\<readonly ...[]\> ? `Primitive`\<`T`\> : `S` *extends* [`TypeKeyword`](/docs/dev/typescript/plugin-sdk/schema/interfaces/typekeyword/)\<infer T\> ? `Primitive`\<`T`\> : `unknown`

The value type a JSON Schema literal describes. Resolves host catalogue
references to the generated catalogue types, `const`, `enum`, `oneOf` and
`anyOf` unions, typed arrays and objects with required and optional
properties. Anything it cannot see statically is `unknown`; the host still
validates the full schema at runtime.

## Type Parameters

| Type Parameter |
| ------ |
| <a id="type-parameter-s"></a> `S` |
