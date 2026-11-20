---
title: "plugin-sdk: limits/README"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / limits

# limits

Every numeric limit a plugin author can hit, generated from the pinned
contract, and preflight checks that name the limit and the offending value
before a broker call is made.

## Classes

- [LimitError](/docs/dev/typescript/plugin-sdk/limits/classes/limiterror/)

## Functions

- [checkItemView](/docs/dev/typescript/plugin-sdk/limits/functions/checkitemview/)
- [checkLimit](/docs/dev/typescript/plugin-sdk/limits/functions/checklimit/)
- [encodeContributionFrame](/docs/dev/typescript/plugin-sdk/limits/functions/encodecontributionframe/)
- [utf8Length](/docs/dev/typescript/plugin-sdk/limits/functions/utf8length/)

## References

### LIMIT\_SOURCES

Re-exports [LIMIT_SOURCES](/docs/dev/typescript/plugin-sdk/generated/limits/variables/limit_sources/)

***

### LimitName

Re-exports [LimitName](/docs/dev/typescript/plugin-sdk/generated/limits/type-aliases/limitname/)

***

### Limits

Re-exports [Limits](/docs/dev/typescript/plugin-sdk/generated/limits/interfaces/limits/)

***

### LIMITS

Re-exports [LIMITS](/docs/dev/typescript/plugin-sdk/generated/limits/variables/limits/)

***

### LimitSource

Re-exports [LimitSource](/docs/dev/typescript/plugin-sdk/generated/limits/interfaces/limitsource/)
