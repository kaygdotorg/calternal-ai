---
title: "plugin-sdk: broker-helpers/README"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / broker-helpers

# broker-helpers

Helpers built only from existing broker methods. Each one takes a context
whose operation declares the methods it uses, so a missing declaration is a
compile error. They add no authority: the host checks every call.

## Classes

- [BrokerReplyError](/docs/dev/typescript/plugin-sdk/broker-helpers/classes/brokerreplyerror/)
- [ContributionEndedError](/docs/dev/typescript/plugin-sdk/broker-helpers/classes/contributionendederror/)

## Interfaces

- [ContributionOutput](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/contributionoutput/)
- [MessageReceipt](/docs/dev/typescript/plugin-sdk/broker-helpers/interfaces/messagereceipt/)

## Type Aliases

- [BodySave](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/bodysave/)
- [MessageAppend](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/messageappend/)
- [SpanIndex](/docs/dev/typescript/plugin-sdk/broker-helpers/type-aliases/spanindex/)

## Functions

- [appendMessage](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/appendmessage/)
- [publishView](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/publishview/)
- [readBody](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/readbody/)
- [readSpans](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/readspans/)
- [saveBody](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/savebody/)
- [streamContribution](/docs/dev/typescript/plugin-sdk/broker-helpers/functions/streamcontribution/)
