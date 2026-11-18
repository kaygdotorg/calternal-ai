# Calternal plugin API

A TypeScript plugin packages a strict manifest v1 menu, an optional bundled
worker and any declared assets. A manifest describes authority requests. A
space grant and the caller's current capabilities authorize each effect.

## Manifest

| Declaration | Purpose |
| --- | --- |
| kinds | Register addressable items with a schema and storage placement |
| operations | Declare JSON Schema input/output, required capability, mutation and retry policy, risk and optional item target |
| collections | Create standard collection templates when a plugin is enabled |
| views and slots | Declare portable views and host mount points |
| attention | Identify operations that provide attention items |
| settings | Declare typed defaults for space settings |
| schedules and webhooks | Declare intended triggers and target operations |
| secrets | Declare names for brokered handles |
| hooks | Request watch or block behavior within the space |
| themes | Declare a packaged stylesheet |
| host_calls | List the broker methods the worker needs |
| provider | Describe a machine-side provider executable and model catalog |

A declared trigger, renderer or broker method only works when the deployment
implements it. Read-only operations use `risk: read` and
`idempotency: read_only`. Writes declare a risk of `reversible`, `irreversible`,
`external` or `widens_access` and require the matching live capability.

## Worker calls and jobs

Use `serveWorker` with a map of operation handlers. The isolated worker receives
one schema-checked operation input. Use `context.call` for brokered effects
and `context.callAll` for independent reads that can be submitted together.
The host keeps item reads and writes within the invocation's authorized space.
Workers may only write their own kinds. Raw network, host files and credentials
are outside the worker contract. A job has a durable checkpoint and may publish
bounded chunks through `context.chunk`. The caller's current access determines
which operations and results it can see.

To list an ordinary kind, add `list_items_v1` to `host_calls` and call
`listItems(context, { kind, limit })` from the SDK. The kind must be
declared by this package. The host takes the space from the invocation.
The result includes item IDs, revisions, timestamps, optional titles
and a `next_cursor`. Pass that cursor on the next call with the same
kind and optional exact label. A null cursor marks the end.

Hooks and context contributions are scoped to the enabling space. Context is
labelled evidence bounded by a deadline. It cannot rewrite the person's prompt.
The plugin and host apply the operation, risk and scope rules at every boundary.

## Interface and addresses

Return `view: { genui_version: 2, source: "..." }` for a portable generated view.
Use catalog components with accessible labels. A web-only sealed component is
a possible escape hatch where the server and client support it. Keep stable
references as `/<kind>/<item-id>`. An address proves no authority.

See [the project API decisions](https://git.kayg.org/kayg/calternal-ai/src/branch/dev/docs/plugin-api.md).

## Shared settings

Declare typed defaults in the manifest and add `read_setting_v1` to
`host_calls`. The SDK's `readSetting(context, key)` returns the effective
value of that declaration for the calling package and space. A Manager
can read or edit that value using `plugins.settings.get` and
`plugins.settings.set`. A write echoes the last stored value as its base.
Overlapping edits remain available for an explicit resolution.

## Generated views

Return `{ view: { genui_version: 2, source } }`. The source is a GenUI
expression. Component names are case-sensitive. These are usable signatures:

| Component | Source shape |
| --- | --- |
| Text | `Text("Details")` |
| Badge | `Badge("Ready")` |
| Metric | `Metric("Remaining", "3")` |
| Stack | `Stack([Text("Details"), Badge("Ready")])` |
| Row | `Row([Metric("Remaining", "3"), Badge("Ready")])` |
| List | `List([{title: "First item", detail: "Optional detail"}], "Items")` |

Use a required accessible label as the second argument to `List`. Make
dynamic text a literal with `JSON.stringify(text)`. Build lists from
strings such as `{title: ${JSON.stringify(title)}}` and join the rows with
commas inside the array. A generated contribution declares its slot ID,
label, slot name and renderer. For a composer accessory the slot name is
`composer_row`, the renderer type is `generated`, and its `operation`
is a manifest-declared operation returning the view. Check the resolved
slot in `plugins.list`.

## Worker and fixture signatures

`typedHandlers(manifest, { "package.operation": async (input, context) => result })`
binds handlers by operation ID. The first argument is the schema-checked
operation input and the second has `call`, `callAll`, `chunk` and
`signal`. Use the generated manifest import from the scaffold. The
helper infers simple literal types, including object properties with a
`required` list. It has limited TypeScript inference for enum-only
schemas, schema unions and other JSON Schema forms. If the helper
rejects a valid schema, keep its runtime validation and pass
`satisfies Record<string, WorkerHandlerV2>` with explicit input narrowing
to `serveWorker`.

Use Node's `spawn(process.execPath, ["dist/worker.js"])` with piped
streams for `fixtureWorker`. Give it a map keyed by broker method. The
callback receives the method input and returns its response value.
`fixture.invoke(operation, input)` receives worker input fields, without
`space_id` or HTTP request metadata. Close the fixture in a `finally`
block. A fixture exercises the worker protocol. It does not schedule
jobs, persist items, render the browser UI or grant access. Mock
successful callback responses and exercise denied callbacks separately.
Worker handler failures are invocation failures. Worker crashes report
process exit status or signal and local stderr when available.

## Stored items

A Markdown kind declares its ID, a storage folder, a metadata schema and
`claimed_text: true`. Declare `write_item` in `host_calls` and use a
mutating operation with `content.write`. Creation takes
`{kind, text, metadata?}`. For an update, also supply the returned
`item_id` and `base_revision`. The response is `{id, revision}`.
Put a title in `metadata.title` if it should appear in the listing.
The broker accepts writes only to the worker's own kinds and uses the
outer operation's request identity for retries. Caller space and
request identifiers stay outside the callback body.

`read_item` takes `{kind, item_id}`. The broker supplies the
authorized current item, with its ID, revision, metadata and text.
`listItems(context, {kind, limit, cursor?, label?})` pages the plugin's
own kinds using `list_items_v1`. The result has `items` and
`next_cursor`. Each summary contains `id`, `revision`, `updated_at`
and an optional title. Pass the cursor unchanged with the same kind
and filter. Render a GenUI list from the resulting summaries.

The broker checks operation input and write arguments before applying a
write. `write_item` returns a UUID `id` and an opaque **string**
`revision`. Match those types in your output schema and fixture replies.
If a validated mutation commits a content write but the worker response
fails validation or does not arrive, the server returns its durable
acknowledgment as `{ "committed_write": { "id": "...", "revision": "..." } }`.
Treat this as a successful save. Retrying with the same request ID and
input returns the same acknowledgment. A new request ID can create a new
item. This envelope acknowledges the first committed content write and
does not certify that subsequent callbacks ran.

Omit `kinds` or use `"kinds": []` for a plugin with no stored kinds. A
generated slot targets a read-only operation whose output schema both
declares `properties.view` and includes `"view"` in `required`. For
`typedHandlers` inference, use single primitive `type` names on properties.
For example, declare a cursor as an optional string and omit it when
there is no next page.

## Durable jobs and progress

Use a mutating start operation with a request ID. Declare `start_job` in
`host_calls`, then call it with
`{operation: "package.run", input: { /* payload */ }}`. The reply has
`job_id`. The named run operation must be declared by the same plugin
and have no item target. The broker admits the durable job and starts a
separate run invocation. Returning the admission result does not run it
inside the start handler.

A run operation receives this host envelope, including all four keys:

| Key | Shape |
| --- | --- |
| `job_id` | Job UUID string |
| `input` | The start callback's submitted payload |
| `checkpoint` | Null on the first run. On a later run, the last saved checkpoint |
| `children` | Array of child job records, empty with no children |

An empty JSON Schema `{}` accepts a null checkpoint. An object-only
checkpoint schema rejects the very first invocation. If you narrow the
schema, explicitly allow null as well as the saved checkpoint shape.
Resume after the observed checkpoint. Declare `checkpoint_job` if you
call it with `{job_id, checkpoint}`. Its receipt covers the currently
running job. Use `await context.chunk(job_id, data)` for each progress
update. Return `{status: "done", result: { /* final payload */ }}` to
store a final result. `{status: "waiting"}` parks the job.

Read state and the final result with the ordinary CLI operation.

```sh
calternal invoke jobs.get --json --input "{\"job_id\":\"$JOB_ID\"}"
```
Subscribe to `GET /api/v1/jobs/<job_id>/events` using the normal
authenticated same-origin connection. The SSE `chunk` event includes
`seq` and `data`. The `snapshot` and `end` events include the job.
For disposable development bearer credentials, read the bearer from
the supplied token file without echoing it and send the server's base
URL as the `Origin` header. Keep both outside fixture logs.
