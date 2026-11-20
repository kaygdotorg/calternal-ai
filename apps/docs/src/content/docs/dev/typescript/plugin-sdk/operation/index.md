---
title: "plugin-sdk: operation/README"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / operation

# operation

Operations declared once: the manifest entry, the handler's input and
output types and the broker methods its context may call all come from one
`defineOperation` call. The returned value is the explicit manifest entry.

## Interfaces

- [OperationContext](/docs/dev/typescript/plugin-sdk/operation/interfaces/operationcontext/)

## Type Aliases

- [Broker](/docs/dev/typescript/plugin-sdk/operation/type-aliases/broker/)
- [BrokerInput](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokerinput/)
- [BrokerMethod](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokermethod/)
- [BrokerOutput](/docs/dev/typescript/plugin-sdk/operation/type-aliases/brokeroutput/)
- [DefinedOperation](/docs/dev/typescript/plugin-sdk/operation/type-aliases/definedoperation/)
- [HostMethod](/docs/dev/typescript/plugin-sdk/operation/type-aliases/hostmethod/)
- [InputOf](/docs/dev/typescript/plugin-sdk/operation/type-aliases/inputof/)
- [ManifestOperation](/docs/dev/typescript/plugin-sdk/operation/type-aliases/manifestoperation/)
- [OperationHandler](/docs/dev/typescript/plugin-sdk/operation/type-aliases/operationhandler/)
- [OutputOf](/docs/dev/typescript/plugin-sdk/operation/type-aliases/outputof/)

## Functions

- [authorContext](/docs/dev/typescript/plugin-sdk/operation/functions/authorcontext/)
- [defineOperation](/docs/dev/typescript/plugin-sdk/operation/functions/defineoperation/)
- [formField](/docs/dev/typescript/plugin-sdk/operation/functions/formfield/)
- [handlersOf](/docs/dev/typescript/plugin-sdk/operation/functions/handlersof/)
- [subeffectKey](/docs/dev/typescript/plugin-sdk/operation/functions/subeffectkey/)
