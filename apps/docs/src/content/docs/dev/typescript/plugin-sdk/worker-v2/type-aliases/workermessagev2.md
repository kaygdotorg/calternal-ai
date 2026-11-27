---
title: "plugin-sdk: worker-v2/type-aliases/WorkerMessageV2"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [worker-v2](/docs/dev/typescript/plugin-sdk/worker-v2/) / WorkerMessageV2

# Type Alias: WorkerMessageV2

> **WorkerMessageV2** = \{ `authority`: [`InvocationAuthority`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/); `input`: `unknown`; `operation`: `string`; `type`: `"invoke"`; \} \| \{ `authority`: [`InvocationAuthority`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/); `call_id`: `string`; `input`: `unknown`; `method`: [`HostCallKind`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/hostcallkind/); `type`: `"host_call"`; \} \| \{ `call_id`: `string`; `data`: `unknown`; `ok`: `boolean`; `type`: `"host_reply"`; \} \| \{ `data`: `unknown`; `job_id`: `string`; `sequence`: `number`; `type`: `"chunk"`; \} \| \{ `data`: `unknown`; `type`: `"complete"`; \} \| \{ `type`: `"failed"`; \} \| \{ `type`: `"cancel"`; \}

Strict bidirectional worker-v2 messages.

## Union Members

### Type Literal

\{ `authority`: [`InvocationAuthority`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/); `input`: `unknown`; `operation`: `string`; `type`: `"invoke"`; \}

#### authority

> **authority**: [`InvocationAuthority`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/)

Host authority.

#### input

> **input**: `unknown`

Schema-checked input.

#### operation

> **operation**: `string`

Declared operation.

#### type

> **type**: `"invoke"`

Host operation dispatch.

***

### Type Literal

\{ `authority`: [`InvocationAuthority`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/); `call_id`: `string`; `input`: `unknown`; `method`: [`HostCallKind`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/hostcallkind/); `type`: `"host_call"`; \}

#### authority

> **authority**: [`InvocationAuthority`](/docs/dev/typescript/plugin-sdk/worker-v2/interfaces/invocationauthority/)

Echoed authority.

#### call\_id

> **call\_id**: `string`

Callback ID.

#### input

> **input**: `unknown`

Broker input.

#### method

> **method**: [`HostCallKind`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/hostcallkind/)

Declared method.

#### type

> **type**: `"host_call"`

Worker callback.

***

### Type Literal

\{ `call_id`: `string`; `data`: `unknown`; `ok`: `boolean`; `type`: `"host_reply"`; \}

#### call\_id

> **call\_id**: `string`

Callback ID.

#### data

> **data**: `unknown`

JSON result or null.

#### ok

> **ok**: `boolean`

Success marker.

#### type

> **type**: `"host_reply"`

Callback result.

***

### Type Literal

\{ `data`: `unknown`; `job_id`: `string`; `sequence`: `number`; `type`: `"chunk"`; \}

#### data

> **data**: `unknown`

Chunk data.

#### job\_id

> **job\_id**: `string`

Host-issued job handle.

#### sequence

> **sequence**: `number`

Zero-based sequence per invocation.

#### type

> **type**: `"chunk"`

Job output.

***

### Type Literal

\{ `data`: `unknown`; `type`: `"complete"`; \}

#### data

> **data**: `unknown`

Operation output.

#### type

> **type**: `"complete"`

Successful completion.

***

### Type Literal

\{ `type`: `"failed"`; \}

#### type

> **type**: `"failed"`

Safe failure.

***

### Type Literal

\{ `type`: `"cancel"`; \}

#### type

> **type**: `"cancel"`

Host cancellation.
