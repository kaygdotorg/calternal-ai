---
title: "plugin-sdk: index/type-aliases/ProviderCommand"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / ProviderCommand

# Type Alias: ProviderCommand

> **ProviderCommand** = \{ `type`: `"check"`; \} \| \{ `cwd`: `string`; `instructions`: `string`; `model`: `string`; `options`: `{ [key in string]?: unknown }`; `resume`: `string` \| `null`; `type`: `"start"`; \} \| \{ `context`: [`ProviderContext`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providercontext/)[]; `prompt`: `string`; `type`: `"send"`; \} \| \{ `prompt`: `string`; `type`: `"steer"`; \} \| \{ `approved`: `boolean`; `request_id`: `string`; `type`: `"approve"`; \} \| \{ `type`: `"stop"`; \}

Commands accepted by an adapter; one active turn per adapter.

## Union Members

### Type Literal

\{ `type`: `"check"`; \}

***

### Type Literal

\{ `cwd`: `string`; `instructions`: `string`; `model`: `string`; `options`: `{ [key in string]?: unknown }`; `resume`: `string` \| `null`; `type`: `"start"`; \}

#### cwd

> **cwd**: `string`

Sandbox workspace; the machine supplies this, never a provider login path.

#### instructions

> **instructions**: `string`

Trusted instruction layers, appended through the harness's documented channel.

#### model

> **model**: `string`

Model name understood by the official CLI.

#### options

> **options**: `{ [key in string]?: unknown }`

Provider-declared typed options; unknown keys are refused by its adapter.

#### resume

> **resume**: `string` \| `null`

Official CLI session to resume; null starts a new session.

#### type

> **type**: `"start"`

***

### Type Literal

\{ `context`: [`ProviderContext`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providercontext/)[]; `prompt`: `string`; `type`: `"send"`; \}

#### context

> **context**: [`ProviderContext`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providercontext/)[]

At most 32 contributions, bounded by the command budget.

#### prompt

> **prompt**: `string`

Person's message, preserved verbatim.

#### type

> **type**: `"send"`

***

### Type Literal

\{ `prompt`: `string`; `type`: `"steer"`; \}

#### prompt

> **prompt**: `string`

New message, bounded by the command budget.

#### type

> **type**: `"steer"`

***

### Type Literal

\{ `approved`: `boolean`; `request_id`: `string`; `type`: `"approve"`; \}

#### approved

> **approved**: `boolean`

Authorize this request within the existing sandbox boundary.

#### request\_id

> **request\_id**: `string`

Opaque ID emitted by this run's adapter.

#### type

> **type**: `"approve"`

***

### Type Literal

\{ `type`: `"stop"`; \}
