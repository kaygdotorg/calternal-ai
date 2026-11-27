---
title: "plugin-sdk: operation-api/variables/operationMethods"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [operation-api](/docs/dev/typescript/plugin-sdk/operation-api/) / operationMethods

# Variable: operationMethods

> `const` **operationMethods**: `object`

Callback wire names accepted by the host.

## Type Declaration

### call

> `readonly` **call**: `"s.call"` = `"s.call"`

Invoke a declared operation through the host.

### query

> `readonly` **query**: `"s.query"` = `"s.query"`

Query rows through the host.

### read

> `readonly` **read**: `"s.read"` = `"s.read"`

Read an item through the host.

### run

> `readonly` **run**: `"s.run"` = `"s.run"`

Start a run through the host.

### runSend

> `readonly` **runSend**: `"s.run.send"` = `"s.run.send"`

Send input to a run through the host.

### runStop

> `readonly` **runStop**: `"s.run.stop"` = `"s.run.stop"`

Stop a run through the host.

### state

> `readonly` **state**: `"s.state"` = `"s.state"`

Get or set viewer state through the host.

### write

> `readonly` **write**: `"s.write"` = `"s.write"`

Commit or preview an ordered list atomically through the host.
