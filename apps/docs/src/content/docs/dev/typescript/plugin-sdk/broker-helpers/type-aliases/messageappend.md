---
title: "plugin-sdk: broker-helpers/type-aliases/MessageAppend"
---

<!-- Generated from: packages/plugin-sdk/src/author-v2.ts, packages/plugin-sdk/src/broker-helpers.ts, packages/plugin-sdk/src/fake-host/execution.ts, packages/plugin-sdk/src/fake-host/host.ts, packages/plugin-sdk/src/fake-host/invocation.ts, packages/plugin-sdk/src/fake-host/items.ts, packages/plugin-sdk/src/fake-host/refusal.ts, packages/plugin-sdk/src/fake-host/schemas.ts, packages/plugin-sdk/src/generated/broker-methods.ts, packages/plugin-sdk/src/generated/limits.ts, packages/plugin-sdk/src/index.ts, packages/plugin-sdk/src/job-waits.ts, packages/plugin-sdk/src/limits.ts, packages/plugin-sdk/src/manifest-v1.ts, packages/plugin-sdk/src/operation.ts, packages/plugin-sdk/src/schema.ts, packages/plugin-sdk/src/space-paths.ts, packages/plugin-sdk/src/stdio-worker.ts, packages/plugin-sdk/src/testing.ts, packages/plugin-sdk/src/typed-handlers.ts, packages/plugin-sdk/src/worker-v2.ts. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

[**@calternal/plugin-sdk**](/docs/dev/typescript/plugin-sdk/)

***

[@calternal/plugin-sdk](/docs/dev/typescript/plugin-sdk/) / [broker-helpers](/docs/dev/typescript/plugin-sdk/broker-helpers/) / MessageAppend

# Type Alias: MessageAppend

> **MessageAppend** = `object`

One framed message for `appendMessage`.

## Properties

| Property | Type | Description |
| ------ | ------ | ------ |
| <a id="entry_anchor"></a> `entry_anchor?` | `AttestedBody`\[`"entry_anchor"`\] | Item-local entry anchor bound to the new span. |
| <a id="expected_revision"></a> `expected_revision` | `RevisionRef` | Exact head the append extends. |
| <a id="item"></a> `item` | `ItemRef` | Target append-file item. |
| <a id="opaque"></a> `opaque?` | () => `Uint8Array` | Opaque bytes to append when the message cannot be attested. Defaults to prefix, text and suffix. |
| <a id="prefix"></a> `prefix?` | `string` | Plugin framing before the middle, at most ATTESTED_FRAMING_CHARACTERS when attested. |
| <a id="purpose"></a> `purpose` | `string` | Subeffect purpose; the request ID is `ctx.key(purpose)`. |
| <a id="quote"></a> `quote?` | `AttestedBody`\[`"quote"`\] | Checked quote the host composes ahead of the middle. |
| <a id="rewrite"></a> `rewrite?` | () => `Uint8Array` | A complete replacement body to save instead of an opaque append, for formats that must rewrite earlier bytes. |
| <a id="source"></a> `source?` | `AttestedBody`\[`"source"`\] | Where an attested middle comes from; omit to write opaque bytes. |
| <a id="suffix"></a> `suffix?` | `string` | Plugin framing after the middle. |
| <a id="text"></a> `text` | `string` | The message middle: captured input or delivered output text when attested. |
