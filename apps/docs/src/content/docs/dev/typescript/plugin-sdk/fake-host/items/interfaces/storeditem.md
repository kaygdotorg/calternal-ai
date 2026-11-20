---
title: "plugin-sdk: fake-host/items/interfaces/StoredItem"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [fake-host/items](/docs/dev/typescript/plugin-sdk/fake-host/items/) / StoredItem

# Interface: StoredItem

One stored item. Exposed read-only to tests through `FakeHost.item`.

## Properties

| Property | Modifier | Type | Description |
| ------ | ------ | ------ | ------ |
| <a id="archived"></a> `archived` | `public` | `boolean` | Archived items refuse writes and job admission. |
| <a id="author"></a> `author?` | `public` | `string` | Account the host attributes the head write to, when it was attested. |
| <a id="bindings"></a> `bindings` | `readonly` | `Map`\<`string`, `Binding`\> | Record identity, entry anchor and quote binding per live span. |
| <a id="creator"></a> `creator` | `readonly` | `string` | Account that created the item. |
| <a id="discovery"></a> `discovery` | `public` | `"consent_only"` \| `"creator_only"` | Whether attempts started by the creator may discover the item without consent. |
| <a id="entries"></a> `entries` | `public` | `number` | Next entry ordinal. |
| <a id="head"></a> `head` | `public` | `string` | Current head revision. |
| <a id="id"></a> `id` | `readonly` | `string` | Item ref. |
| <a id="index"></a> `index` | `readonly` | `Map`\<`string`, `Pick`\<`ItemSpansPage`, `"spans"` \| `"body_provenance"`\>\> | The span index committed with each revision. |
| <a id="kind"></a> `kind` | `readonly` | `Kind` | Kind declaration from the owning manifest. |
| <a id="metadata"></a> `metadata` | `public` | `Record`\<`string`, `unknown`\> | Current metadata. |
| <a id="package"></a> `package` | `readonly` | `string` | Owning package ID. |
| <a id="path"></a> `path` | `public` | `string` | Readable path inside the space, from the kind's placement. |
| <a id="provenance"></a> `provenance` | `public` | `"attested"` \| `"unproven"` | Whether every byte of the head is explained by recorded spans. |
| <a id="revisions"></a> `revisions` | `readonly` | `Map`\<`string`, `Buffer`\<`ArrayBufferLike`\>\> | Every revision's bytes, oldest first. |
| <a id="spans"></a> `spans` | `public` | `ItemRecordedSpan`[] | Live recorded spans, ordered by offset. |
