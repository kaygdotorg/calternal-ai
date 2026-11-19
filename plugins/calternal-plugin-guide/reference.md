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

## Network permissions

The network design distinguishes four classes by how the plugin chooses a
destination and who approves it. A worker makes requests through the host
broker. Its own network sockets and ambient credentials are unavailable.

### A. Named connections

**Example: a portfolio balance reader.** The package declares the brokerage's
HTTPS service and requests a read-only credential. The space manager reviews
that destination, the balance operation and the data sent to it. The person
adds their credential in the host. On a balance read, the host attaches the
credential for that service and returns the response to the worker. A redirect
to another service or a trade request is outside that approval. Named outbound
connections are designed into the contract. This preview has no named
connection manifest field or callable broker method.

### B. Content fetch

**Example: a stored mail message.** Register a package-owned text kind named
`mail-message`. Declare `content_prepare_v1` in `host_calls` and add this
manifest field.

```json
{
  "content_fetch": {
    "profile": "images_fonts",
    "sources": [{ "kind": "mail-message", "field": "text" }]
  }
}
```

This is the value of `network`. On review, explain that the message can
reference remote images and fonts. Enable the reviewed package in the space.
Store the HTML in a `mail-message` item and use the saved item ID and revision
to call the SDK's `prepareContent(context, { kind: "mail-message", item_id,
revision, field: "text" })` from a declared operation. Return its `frame_id`.
The client calls `plugins.network.content.get` with the space ID and frame ID,
then mounts the returned `frame_url` with an empty iframe sandbox. The host
sanitizes the HTML and checks the viewer, package activation and item revision
on delivery. Preparing another kind or revision is refused.

The `images_fonts` profile retrieves anonymous HTTPS media in the
background. The sealed frame displays cached media through signed host
references after checking the viewer's current remote-content preference.
The profile serves supported images and fonts and excludes linked page titles.

Content fetch can disclose data through the hostname, path or query that a
plugin puts in a stored media URL. Always may send that URL at ingest even
when nobody opens the item. There are no caller cookies, credentials or
referrer headers. The proxy limits each plugin to 16 distinct hostnames per
space per minute, including redirects. Managers can see the latest 32
attempted hostnames in `plugins.diagnostics` through `fetch_hosts`.

### C. Open internet

**Example: an RSS reader with user-added feeds.** The author discloses that the
worker can choose public destinations from subscriptions at runtime. The space
Owner or Manager approves the broad public scope. On refresh, the proposed
broker checks the addresses resolved for each feed and redirect, performs
bounded public HTTP retrieval, and returns the result. A private target remains
refused under the public grant. Public arbitrary HTTP is a design example.
This preview has no callable open-internet broker method.

### D. Private network and execution origin

**Example: a read-only home automation dashboard.** The package requests
HTTPS access to the controller on a chosen enrolled machine's network.
Approval names the origin, destination, protocol, port, package and space.
The machine owner approves a personal machine, or an operator approves a pool
machine. When the dashboard refreshes, the host checks the invocation and the
selected origin, and that machine's broker dials the approved controller.
Removing the machine revokes that origin, and the request cannot fall back to
the server. Private outbound access is a design example. This preview has no
callable private-network broker method.

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
`host_calls`, then use the SDK helper `startJob(context, {operation: "package.run", input})`. The reply has
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

### Wait for a signal

Return `parkJob` from a replayable job run. Declare `parked` in the run's
output schema and allow an optional `signal` property in its input schema.
Use a deadline source when no other signal should wake the job. The question
example below also handles deadlines.

#### A job with one text answer

Scaffold the complete example, including its manifest, handler, form, README
and fixture test.

```sh
calternal plugin new question-job --template question
cd question-job
npm install
calternal plugin check .
calternal plugin test .
```

The public `waits` topic prints the complete example files. The `jobs` topic
includes admission and progress as well.

```sh
calternal invoke calternal-plugin-guide.explain --input "{\"space_id\":\"$SPACE_ID\",\"topic\":\"waits\"}"
```

The example keeps the form in `question.form.json`. Import this file as the
form in the job's parked output so `plugin check` can validate the same form
that the worker sends. The JSON's `source` is GenUI source. Form arguments are
the controls and the label in that order. `Input` takes a bound string and a
label. Here `$text` creates the submitted field named `text`.

The start operation uses `plugins.use` and declares the `start_job` host call.
Use the SDK's `startJob` to get a typed `{ job_id }` reply. `content.write`
is for editing content, and `space.read` is for reads. After adding or
renaming an operation, document its full ID under the title in `README.md`.

The run's manifest uses `oneOf` on its optional `signal` field and a
`const` on each branch's `kind`. This lets `typedHandlers` infer
`signal.value.text` after checking for `"answer"`. Park the first run with
a saved checkpoint and a future deadline in Unix milliseconds. Return the
answer or a timeout result on the next run.

<!-- BEGIN GENERATED question -->

##### manifest.json

```json
{
  "id": "question-job",
  "name": "question-job",
  "icon": "puzzle",
  "version": "0.1.0",
  "api_version": 1,
  "kinds": [],
  "collections": [],
  "views": [],
  "attention": [],
  "settings": [],
  "schedules": [],
  "webhooks": [],
  "secrets": [],
  "hooks": [],
  "themes": [],
  "host_calls": ["start_job"],
  "operations": [
    {
      "id": "question-job.start",
      "input_schema": { "type": "object", "additionalProperties": false },
      "output_schema": {
        "type": "object",
        "properties": { "job_id": { "type": "string" } },
        "required": ["job_id"]
      },
      "required_capability": "plugins.use",
      "mutates": true,
      "idempotency": "request_id",
      "risk": "reversible"
    },
    {
      "id": "question-job.run",
      "input_schema": {
        "type": "object",
        "properties": {
          "job_id": { "type": "string" },
          "input": { "type": "object" },
          "checkpoint": {},
          "children": { "type": "array" },
          "signal": {
            "oneOf": [
              {
                "type": "object",
                "properties": {
                  "kind": { "const": "answer" },
                  "value": {
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                  }
                },
                "required": ["kind", "value"]
              },
              {
                "type": "object",
                "properties": { "kind": { "const": "deadline" } },
                "required": ["kind"]
              }
            ]
          }
        },
        "required": ["job_id", "input", "checkpoint", "children"],
        "additionalProperties": false
      },
      "output_schema": {
        "type": "object",
        "properties": { "status": { "type": "string", "enum": ["parked", "done"] } },
        "required": ["status"]
      },
      "required_capability": "plugins.use",
      "mutates": false,
      "idempotency": "read_only",
      "risk": "read"
    },
    {
      "id": "question-job.answer",
      "input_schema": {
        "type": "object",
        "properties": { "text": { "type": "string" } },
        "required": ["text"],
        "additionalProperties": false
      },
      "output_schema": { "type": "object" },
      "required_capability": "plugins.use",
      "mutates": false,
      "idempotency": "read_only",
      "risk": "read"
    }
  ]
}
```

##### question.form.json

```json
{
  "genui_version": 2,
  "source": "$text=\"\"\nForm([Input($text, \"What should the job use?\")], \"Job question\")"
}
```

##### worker.ts

```ts
import { parkJob, serveWorker, startJob, typedHandlers, validateGenUiView } from "@calternal/plugin-sdk";
import manifest from "./manifest.generated.js";
import question from "./question.form.json" with { type: "json" };

const form = validateGenUiView(question);

const handlers = typedHandlers(manifest, {
  "question-job.start": async (_, context) =>
    startJob(context, { operation: "question-job.run", input: {} }),

  "question-job.run": async ({ signal }) => {
    if (signal?.kind === "answer") {
      return { status: "done", result: { answer: signal.value.text } };
    }
    if (signal?.kind === "deadline") {
      return { status: "done", result: { timed_out: true } };
    }
    return parkJob({
      checkpoint: { phase: "asked" },
      reason: "Waiting for a text answer",
      deadline_ms: Date.now() + 20_000,
      source: { kind: "question", form, schema_operation: "question-job.answer" },
    });
  },

  // The host validates submitted fields against this operation's input schema.
  "question-job.answer": async () => ({}),
});

serveWorker(manifest, handlers, process.stdin, line =>
  new Promise<void>((resolve, reject) =>
    process.stdout.write(line, error => error ? reject(error) : resolve())),
).catch(() => {
  process.stderr.write("Worker protocol error. Check that the host speaks worker-v2.\n");
  process.exitCode = 1;
  process.stdin.destroy();
});
```

##### worker.test.mjs

```js
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";

test("starts a job and handles a typed answer or deadline", async () => {
  const job_id = "01900000-0000-7000-8000-000000000008";
  const fixture = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]), {
    start_job(request) {
      assert.deepEqual(request, { operation: "question-job.run", input: {} });
      return { job_id };
    },
  });
  try {
    assert.deepEqual(await fixture.invoke("question-job.start", {}), { job_id });
    const run = { job_id, input: {}, checkpoint: null, children: [] };
    const parked = await fixture.invoke("question-job.run", run);
    assert.equal(parked.status, "parked");
    assert.equal(parked.wait.source.kind, "question");
    const resumed = { ...run, checkpoint: parked.wait.checkpoint };
    assert.deepEqual(
      await fixture.invoke("question-job.run", { ...resumed, signal: { kind: "answer", value: { text: "Blue" } } }),
      { status: "done", result: { answer: "Blue" } },
    );
    assert.deepEqual(
      await fixture.invoke("question-job.run", { ...resumed, signal: { kind: "deadline" } }),
      { status: "done", result: { timed_out: true } },
    );
  } finally {
    await fixture.close();
  }
});
```

<!-- END GENERATED question -->

With the development environment sourced, load the draft and start a job.

```sh
calternal plugin dev . --once
calternal invoke question-job.start --json --input "{\"space_id\":\"$SPACE_ID\"}"
calternal invoke requests.list --json --input '{}'
```

In the Inbox output, find the row whose `job_id` matches the start reply.
Use that row's `id` for `REQUEST_ID` below, and use the ID from the start
reply for `JOB_ID`. Answer before the deadline.

```sh
calternal invoke requests.answer --json --input '{"id":"REQUEST_ID","answer":{"text":"Blue"}}'
calternal invoke jobs.get --json --input '{"job_id":"JOB_ID"}'
```

The job finishes with `{"answer":"Blue"}`. Start a second job, leave it
unanswered, and check for `{"timed_out":true}` after the deadline.

If an emitted question is malformed, `jobs.get` returns
`result.diagnostics` with the path and source position, what was expected
and what was found. `calternal plugin test` applies the same GenUI validator
when a fixture invocation returns a parked question. Exercise at least the
first run in the fixture to verify a form constructed by the handler.

Use `{kind: "question", form, schema_operation}` to ask the job's starter
a question in the existing Inbox. `form` is a GenUI view containing one
Form with no actions. The host validates the answer against the input
schema of `schema_operation` from this package and sends
`{kind: "answer", value}` to the next run. An approval uses
`{kind: "approval"}` and resumes with `{kind: "approval", approved}`.
The decision is scoped to this wait. It confers no host permissions.

`jobs.get` and the job stream show an active wait's reason and deadline.
`jobs.cancel` cancels the wait. Keep private data out of the reason and
shared result. The next run receives `{kind: "deadline"}` if no valid
signal reaches the job first. A package can park eight waits per space,
within a space-wide maximum of 32. Deadlines are future Unix milliseconds
within seven days. A rejected park fails the job with `result.wait_error`.

The host commits one winning signal and resumes from the saved checkpoint.
A process crash can re-invoke a replayable handler with the same signal
and checkpoint. Design that handler for replay and use the broker's
receipt-backed calls for durable effects.

To inspect a parked job or cancel it from the CLI, use the job ID from
the start result. Inbox decisions use the request ID from
`calternal invoke requests.list --json --input '{}'`. The listing
identifies plugin prompts with `job_id`. Submit the answer with
`requests.answer` and `{"id":"…","answer":{…}}`, or use
`requests.deny` with `{"id":"…"}`. `requests.approve` takes
`{"id":"…","lifetime":"once"}` and requires a fresh passkey session.


## Durable plugin state

This counter keeps one value for the package in the current space. Declare
`read_plugin_state_v1` and `cas_plugin_state_v1` in the worker's host calls.
Give the increment operation `space.settings`, set `mutates: true`, and
declare `idempotency: "request_id"`. Pass a stable request ID for retries.

```ts
import { compareAndSetPluginState, readPluginState, type WorkerContextV2 } from "@calternal/plugin-sdk";

async function incrementCounter(context: WorkerContextV2) {
  const current = await readPluginState(context, "counter");
  if (current && (current.schema !== 1 || typeof current.value !== "number" ||
      !Number.isSafeInteger(current.value))) {
    throw new Error("Unsupported counter state");
  }
  const next = (typeof current?.value === "number" ? current.value : 0) + 1;
  if (!Number.isSafeInteger(next)) throw new Error("Counter is full");
  const revision = await compareAndSetPluginState(
    context, "counter", current?.revision ?? null, 1, next,
  );
  return { revision };
}
```

Concurrent changes can make the revision stale. The host refuses the stale
write, and an identical retry of a committed request returns its stored
revision. State has no time-based expiry. One package can store up to 128 keys
per space, each with a 64 KiB JSON value. Each package in a space can retain
up to 16,384 receipts over the installation's receipt window, which defaults
to seven days.
