---
title: "plugin-sdk: schema/README"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / schema

# schema

Small JSON Schema builders whose TypeScript types carry the value type they
describe. Each builder returns plain JSON Schema, so the emitted manifest
stays explicit for install review; only the static type is extra.

## Interfaces

- [AnyOfSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/anyofschema/)
- [ArrayKeywords](/docs/dev/typescript/plugin-sdk/schema/interfaces/arraykeywords/)
- [CatalogueRefSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/cataloguerefschema/)
- [ConstSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/constschema/)
- [EnumSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/enumschema/)
- [IntegerKeywords](/docs/dev/typescript/plugin-sdk/schema/interfaces/integerkeywords/)
- [ItemsKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/itemskeyword/)
- [ObjectSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/objectschema/)
- [OneOfSchema](/docs/dev/typescript/plugin-sdk/schema/interfaces/oneofschema/)
- [PropertiesKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/propertieskeyword/)
- [RequiredKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/requiredkeyword/)
- [StringKeywords](/docs/dev/typescript/plugin-sdk/schema/interfaces/stringkeywords/)
- [TypeKeyword](/docs/dev/typescript/plugin-sdk/schema/interfaces/typekeyword/)

## Type Aliases

- [ArraySchema](/docs/dev/typescript/plugin-sdk/schema/type-aliases/arrayschema/)
- [CatalogueName](/docs/dev/typescript/plugin-sdk/schema/type-aliases/cataloguename/)
- [Schema](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schema/)
- [SchemaType](/docs/dev/typescript/plugin-sdk/schema/type-aliases/schematype/)
- [TypedSchema](/docs/dev/typescript/plugin-sdk/schema/type-aliases/typedschema/)

## Functions

- [array](/docs/dev/typescript/plugin-sdk/schema/functions/array/)
- [integer](/docs/dev/typescript/plugin-sdk/schema/functions/integer/)
- [literal](/docs/dev/typescript/plugin-sdk/schema/functions/literal/)
- [object](/docs/dev/typescript/plugin-sdk/schema/functions/object/)
- [oneOf](/docs/dev/typescript/plugin-sdk/schema/functions/oneof/)
- [oneOfValues](/docs/dev/typescript/plugin-sdk/schema/functions/oneofvalues/)
- [ref](/docs/dev/typescript/plugin-sdk/schema/functions/ref/)
- [string](/docs/dev/typescript/plugin-sdk/schema/functions/string/)
