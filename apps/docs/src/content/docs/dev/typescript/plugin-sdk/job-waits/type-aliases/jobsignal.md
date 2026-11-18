---
title: "plugin-sdk: job-waits/type-aliases/JobSignal"
---

<!-- Generated from: packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [job-waits](/docs/dev/typescript/plugin-sdk/job-waits/) / JobSignal

# Type Alias: JobSignal\<Answer, Webhook\>

> **JobSignal**\<`Answer`, `Webhook`\> = \{ `kind`: `"deadline"`; \} \| \{ `approved`: `boolean`; `kind`: `"approval"`; \} \| \{ `kind`: `"answer"`; `value`: `Answer`; \} \| \{ `kind`: `"webhook"`; `value`: `Webhook`; \}

Signal delivered with the checkpoint on the next invocation.

## Type Parameters

| Type Parameter | Default type | Description |
| ------ | ------ | ------ |
| <a id="type-parameter-answer"></a> `Answer` | `unknown` | Payload accepted by the question's declared schema. |
| <a id="type-parameter-webhook"></a> `Webhook` | `unknown` | Payload accepted by the webhook's declared schema. |

## Union Members

### Type Literal

\{ `kind`: `"deadline"`; \}

#### kind

> **kind**: `"deadline"`

The deadline passed before the job accepted a signal.

***

### Type Literal

\{ `approved`: `boolean`; `kind`: `"approval"`; \}

#### approved

> **approved**: `boolean`

Whether the person approved continuing with the job.

#### kind

> **kind**: `"approval"`

The person made a one-time decision about this job.

***

### Type Literal

\{ `kind`: `"answer"`; `value`: `Answer`; \}

#### kind

> **kind**: `"answer"`

The person answered the Inbox form.

#### value

> **value**: `Answer`

The validated answers for its declared fields.

***

### Type Literal

\{ `kind`: `"webhook"`; `value`: `Webhook`; \}

#### kind

> **kind**: `"webhook"`

The host accepted a signed webhook delivery.

#### value

> **value**: `Webhook`

The validated payload of the accepted delivery.
