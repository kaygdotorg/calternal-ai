# Plugin SDK

The v2 SDK describes, implements, tests and builds a plugin package without a
running space. Run `npm run build -w @calternal/plugin-sdk` from the checkout
root to generate the contract types, tables and limits and compile the SDK.

## Declare

Import `definePlugin`, `defineApp`, `defineKind`, `defineOperation`,
`defineView` and `defineChrome` from `@calternal/plugin-sdk`. Wire types for
every catalogue and description definition are at
`@calternal/plugin-sdk/types`.

`defineOperation` declares an operation once. Write its input and output with
the schema builders (`object`, `ref`, `string`, `integer`, `boolean`,
`oneOfValues`, `literal`, `array`, `oneOf`) or plain `as const` JSON Schema. The
handler's input and return types follow the schemas, and `ctx.call` accepts
only the methods listed in `methods`, with catalogue-typed input and reply.
A string `target` names the kind; the field is the input's single `ItemRef`
property. The returned value is the full manifest entry, so the emitted
manifest stays explicit for install review. A worker exports
`handlers = handlersOf(operations)`. `formField(operation, property, { label })`
derives a description form field from an input property.

```ts
const send = defineOperation({
  id: "chat.send", title: "Send", required_capability: "plugins.use", risk: "external", idempotency: "request_id",
  target: "chats", methods: ["start_job"], write_origins: { host_input: "text" },
  input_schema: object({ item: ref("ItemRef"), text: string({ maxLength: 16384 }), harness: ref("HarnessRef") }, "item", "text", "harness"),
  output_schema: ref("ExecutionJobReceipt"),
  handler: ({ item, text, harness }, ctx) =>
    ctx.call("start_job", { job: "run", item, harness, input: {}, contribution_input: { text } }),
});
```

`ctx.key(purpose)` derives a deterministic subeffect request ID under the
invocation's stable request ID, so a replay reuses every receipt.

## Helpers

Built only on existing broker methods, each requiring the methods it uses:
`readBody` (ranged reads), `readSpans` (paged spans, joining continued host
ranges), `saveBody` (inline or staged by size), `appendMessage` (attested,
opaque, or a whole-body rewrite), `streamContribution` (sequence-checked
contribution output) and `publishView` (publishes only for the current head).

## Limits

`LIMITS` holds every numeric limit an author can hit, each read from one
contract pointer; `LIMIT_SOURCES` names the pointer. `checkLimit`,
`encodeContributionFrame` and `checkItemView` refuse an oversized value with a
`LimitError` naming the limit, the bound and the value.
`calternal-plugin limits` lists them.

## Test

`FakeHost` from `@calternal/plugin-sdk/testing` runs packages in process,
straight from `definePlugin` output or from built `dist` directories. Every
broker call is checked against the pinned catalogue schemas, the operation's
declarations, request receipts and in-memory item and execution engines
(items, ranges, spans, attested writes, staged saves, viewer state, view
publication, harness choices, jobs, attempts, contributions, alarms, Stop).
A missing declaration refuses `403 not_declared` with the manifest path and
the change to make. It does not prove real sessions, live authority and
sharing, audiences, quotas beyond sizes, durability or crash recovery,
renderer behavior, real harness images or egress.

```ts
const host = new FakeHost([plugin, harness]);
const { created } = await host.invoke("chat.create", { title: "Lease" }, { captured: true });
const { job_id } = await host.invoke("chat.send", { item: created, text: "Hi", harness: host.choose().harness }, { captured: true });
await host.run(job_id);
```

## Build

`calternal-plugin build [<plugin.ts> [<output-dir>]] [--worker <file>]`
type-checks the package when it has a `tsconfig.json`, bundles a TypeScript
worker with esbuild, emits `manifest.json` with SHA-256 pins for templates,
worker and harness entries, and verifies the result with the pinned contract
tools. Defaults are `src/plugin.ts`, `dist` and `src/worker.ts`. `--json`
prints the same summary for scripts. Build output is disposable.

For an already emitted package, `verifyPackage` from
`@calternal/plugin-sdk/verify-package` checks the manifest, pins and
descriptions without loading the plugin source.

The [author walkthrough](../../plugins/calternal-plugin-guide/walkthrough.md)
has a complete first list. The [reading-list package](../../plugins/calternal-reading-list/)
shows search, forms and host-owned row actions.
