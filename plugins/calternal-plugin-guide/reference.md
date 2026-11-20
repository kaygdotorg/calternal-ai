# Calternal Plugin API v2

Use the TypeScript SDK to declare a package, then validate its manifest and
host-drawn templates with `calternal-build-plugin`. Begin with the
[walkthrough](walkthrough.md). The manifest lists what the package asks
to do. The host evaluates the caller, space and declared authority for
each operation.

## Packages and templates

- `definePlugin` collects apps, kinds, external reads, and optional operations.
  Package ID and version identify the release. App and kind IDs identify
  entries within it. The SDK emits the package's own kinds as readable.
- `defineKind` accepts a metadata schema and placement. It maps the declared
  string `title` property to the host's generic title field. Set
  `title_field` for a different string property. A searchable kind opts in
  with `claimed_text: true` and `searchable: true`.
- `defineView` and `defineChrome` describe the same app. Supply each Bind once
  and reference it by ID from lists and sidebar sections. Override a window's
  size or count mode and supply custom state text as needed. Search defaults
  to no count. Set a row's `icon` to `false` to omit the default kind icon.
- `defineApp` combines the parts of the template. The build adds its path and
  content digest to the manifest.

Describe actions with an operation ID and input sources. A form field comes
from `{ form: "add", field: "title" }`. A host-owned row action can target
`{ row: "id" }`. `then.route` opens the host-created reference on success.
The shell draws the template and the host resolves data and targets. Full
swipe belongs on the first eligible action on that side, such as a host
metadata patch with atomic Undo.

## Worker operations and refusals

Use `defineOperation` to declare an input and output schema, capability, risk,
idempotency policy and exact broker methods. Declare the typed item target
using a kind and the input field containing its reference. Use the same
request ID and target for an idempotent retry. Broker scope comes from the host
invocation. A client-side selector may choose an authorized space before
invoking the operation.

An undeclared method or kind returns `403 not_declared`. For authorized
authors the host diagnostic includes the manifest field to change. Check the
operation's `methods` or the package's `reads`, review the requested authority,
and rebuild the manifest. Foreign targets return an opaque response. A stale
write needs a fresh head and a new decision before attempting a new effect.
The offline author build verifies declarations and does not run these flows.

## Durable jobs and progress

Declare durable jobs in the manifest with a `run_operation`, an explicit
`replayable` choice and the wait kinds the job uses. The host starts only
declared jobs and checks authority again at resume.

### Wait for a signal

A replayable job can checkpoint and park for a declared signal. For a question,
declare the operation that defines the answer schema. Resume from the saved
checkpoint and use the typed answer the host delivers. Replays, deadline
races, and refusing a revoked target need runtime conformance tests.

For a one-date reading reminder, store the reminder in the person's own
space. Declare the due operation as a replayable job with `alarm: true`.
Use `alarm_set` with the reminder as the job's checked target and one due
time. A reminder can reference a shared item. Opening it still requires
current access to that item. A viewer filter controls what the Bind displays.

## Network permissions

Omitting `network` emits `"none"`. Declare only the network routes the plugin
expects to use. Fixed HTTPS sites, account-configured sites and arbitrary
sites have different approval requirements. Remote rendered images use the
separate `content_fetch` declaration and affect the No network access badge.
A URL stored by Reading List is item data.

## Interface and addresses

Use a Bind for host-projected data and a typed target reference for actions.
The host validates current authority each time it resolves a reference.
Templates fill values from their declared sources and can be shared before
they receive any viewer data.
