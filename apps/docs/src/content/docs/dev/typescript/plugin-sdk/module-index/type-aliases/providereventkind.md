---
title: "plugin-sdk: index/type-aliases/ProviderEventKind"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/newline-frames.ts, packages/plugin-sdk/src/operation-api.ts, packages/plugin-sdk/src/operations-worker.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [index](/docs/dev/typescript/plugin-sdk/module-index/) / ProviderEventKind

# Type Alias: ProviderEventKind

> **ProviderEventKind** = `object` & `ProviderCheck` \| \{ `session_id`: `string`; `type`: `"session"`; \} \| \{ `text`: `string`; `type`: `"text_delta"`; \} \| \{ `type`: `"reasoning_started"`; \} \| \{ `text`: `string`; `type`: `"reasoning_delta"`; \} \| \{ `duration_ms`: `number`; `type`: `"reasoning_finished"`; \} \| \{ `id`: `string`; `name`: `string`; `summary`: `string`; `type`: `"tool_started"`; \} \| \{ `bytes`: `number`; `id`: `string`; `name`: `string`; `summary`: `string`; `type`: `"tool_output"`; \} \| \{ `detail`: `string`; `failed`: `boolean`; `id`: `string`; `name`: `string`; `summary`: `string`; `type`: `"tool_finished"`; \} \| \{ `text`: `string`; `type`: `"question"`; \} \| \{ `can_approve?`: `boolean`; `request_id?`: `string`; `summary`: `string`; `type`: `"approval_request"`; \} \| \{ `input_tokens`: `number`; `output_tokens`: `number`; `type`: `"usage"`; \} \| \{ `billing`: [`ProviderBilling`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerbilling/); `class`: [`ProviderFailureClass`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerfailureclass/); `code`: `string`; `message`: `string`; `reset_at_ms?`: `number`; `type`: `"error"`; \} \| \{ `outcome`: `string`; `type`: `"done"`; \}

Normalized provider output. Untrusted provider text conveys no authority.

## Union Members

`object` & `ProviderCheck`

***

### Type Literal

\{ `session_id`: `string`; `type`: `"session"`; \}

#### session\_id

> **session\_id**: `string`

Opaque provider session identifier; never a login or access token.

#### type

> **type**: `"session"`

***

### Type Literal

\{ `text`: `string`; `type`: `"text_delta"`; \}

#### text

> **text**: `string`

At most 4 KiB of UTF-8 text.

#### type

> **type**: `"text_delta"`

***

### Type Literal

\{ `type`: `"reasoning_started"`; \}

***

### Type Literal

\{ `text`: `string`; `type`: `"reasoning_delta"`; \}

#### text

> **text**: `string`

Text the official provider chose to expose.

#### type

> **type**: `"reasoning_delta"`

***

### Type Literal

\{ `duration_ms`: `number`; `type`: `"reasoning_finished"`; \}

#### duration\_ms

> **duration\_ms**: `number`

Monotonic elapsed milliseconds observed by the adapter.

#### type

> **type**: `"reasoning_finished"`

***

### Type Literal

\{ `id`: `string`; `name`: `string`; `summary`: `string`; `type`: `"tool_started"`; \}

#### id

> **id**: `string`

Provider-issued step identity.

#### name

> **name**: `string`

Tool name, at most 128 bytes.

#### summary

> **summary**: `string`

Short description, at most 512 bytes.

#### type

> **type**: `"tool_started"`

***

### Type Literal

\{ `bytes`: `number`; `id`: `string`; `name`: `string`; `summary`: `string`; `type`: `"tool_output"`; \}

#### bytes

> **bytes**: `number`

UTF-8 bytes of tool text identified by the adapter.

#### id

> **id**: `string`

Provider-issued step identity.

#### name

> **name**: `string`

Tool name, at most 128 bytes.

#### summary

> **summary**: `string`

Short target or description, at most 512 bytes.

#### type

> **type**: `"tool_output"`

***

### Type Literal

\{ `detail`: `string`; `failed`: `boolean`; `id`: `string`; `name`: `string`; `summary`: `string`; `type`: `"tool_finished"`; \}

#### detail

> **detail**: `string`

Bounded detail, at most 4 KiB; never raw stderr or credentials.

#### failed

> **failed**: `boolean`

Whether the tool reported failure.

#### id

> **id**: `string`

Provider-issued step identity.

#### name

> **name**: `string`

Tool name, at most 128 bytes.

#### summary

> **summary**: `string`

Short target or description, at most 512 bytes.

#### type

> **type**: `"tool_finished"`

***

### Type Literal

\{ `text`: `string`; `type`: `"question"`; \}

#### text

> **text**: `string`

Short question, at most 4 KiB.

#### type

> **type**: `"question"`

***

### Type Literal

\{ `can_approve?`: `boolean`; `request_id?`: `string`; `summary`: `string`; `type`: `"approval_request"`; \}

#### can\_approve?

> `optional` **can\_approve?**: `boolean`

Whether an interactive confirmation can proceed within the tool sandbox.

#### request\_id?

> `optional` **request\_id?**: `string`

ID of an interactive, run-scoped approval. Legacy notices omit it.

#### summary

> **summary**: `string`

Short action description, at most 4 KiB.

#### type

> **type**: `"approval_request"`

***

### Type Literal

\{ `input_tokens`: `number`; `output_tokens`: `number`; `type`: `"usage"`; \}

#### input\_tokens

> **input\_tokens**: `number`

Input tokens reported for the completed turn.

#### output\_tokens

> **output\_tokens**: `number`

Output tokens reported for the completed turn.

#### type

> **type**: `"usage"`

***

### Type Literal

\{ `billing`: [`ProviderBilling`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerbilling/); `class`: [`ProviderFailureClass`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerfailureclass/); `code`: `string`; `message`: `string`; `reset_at_ms?`: `number`; `type`: `"error"`; \}

#### billing

> **billing**: [`ProviderBilling`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerbilling/)

Billing route from the official CLI status, without login details.

#### class

> **class**: [`ProviderFailureClass`](/docs/dev/typescript/plugin-sdk/module-index/type-aliases/providerfailureclass/)

Stable class; auth/refusal/unknown failures are never retried.

#### code

> **code**: `string`

Stable category, at most 128 bytes.

#### message

> **message**: `string`

Fixed human-readable description, at most 512 bytes.

#### reset\_at\_ms?

> `optional` **reset\_at\_ms?**: `number`

Reported subscription reset in Unix milliseconds; absent if unknown.

#### type

> **type**: `"error"`

***

### Type Literal

\{ `outcome`: `string`; `type`: `"done"`; \}

#### outcome

> **outcome**: `string`

`done`, `failed`, `cancelled` or `outcome_unknown`.

#### type

> **type**: `"done"`
