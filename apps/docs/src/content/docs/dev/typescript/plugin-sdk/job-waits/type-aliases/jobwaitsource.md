---
title: "plugin-sdk: job-waits/type-aliases/JobWaitSource"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [job-waits](/docs/dev/typescript/plugin-sdk/job-waits/) / JobWaitSource

# Type Alias: JobWaitSource

> **JobWaitSource** = \{ `kind`: `"deadline"`; \} \| \{ `form`: `GenUiView`; `kind`: `"question"`; `schema_operation`: `string`; \} \| \{ `kind`: `"approval"`; \} \| \{ `correlation`: `string`; `kind`: `"webhook"`; `route`: `string`; \}

The single signal a parked job accepts before its deadline.

## Union Members

### Type Literal

\{ `kind`: `"deadline"`; \}

#### kind

> **kind**: `"deadline"`

Resume when the deadline passes.

***

### Type Literal

\{ `form`: `GenUiView`; `kind`: `"question"`; `schema_operation`: `string`; \}

#### form

> **form**: `GenUiView`

One action-free GenUI Form displayed to the job's recipient.

#### kind

> **kind**: `"question"`

Wait for the job's recipient to answer the Inbox form.

#### schema\_operation

> **schema\_operation**: `string`

Declared package operation whose input schema validates the answer.

***

### Type Literal

\{ `kind`: `"approval"`; \}

#### kind

> **kind**: `"approval"`

Wait for a one-time decision from the job's recipient.

***

### Type Literal

\{ `correlation`: `string`; `kind`: `"webhook"`; `route`: `string`; \}

#### correlation

> **correlation**: `string`

Fresh UUID that identifies this wait to the sender.

#### kind

> **kind**: `"webhook"`

Wait for a signed delivery to a package-declared route.

#### route

> **route**: `string`

Manifest webhook ID in this package.
