# Author a Calternal plugin

Use Node 24, npm, and Python 3. In this directory, initialize a package
and install the bundled SDK. The install works offline.

```sh
npm init -y
npm pkg set type=module
npm install --offline --ignore-scripts --no-audit --no-fund ./calternal-plugin-sdk-0.1.0.tgz
```

Save your TypeScript declarations in `plugin.ts`. Build the package and
check the task features with one command.

```sh
sh build-and-check.sh
```

The guide's first list below shows how the package, app, kind, storage
placement, item schema, Bind, content list, and sidebar section fit
together. The SDK emits the own-kind reads, title mapping, no network,
no worker operations, standard list window, title and icon, and state
messages.

## Add a form and a worker operation

A content root can be a `stack` with a list and a `form` in `children`.
Give the form an ID and fields with IDs, labels, and an input type such
as `text`. Set `required: true` for values the user must enter. In the
form's `submit`, supply an action ID, title, operation ID, and `inputs`.
For example, `{ form: "add", field: "title" }` sources the title from the
form with ID `add`.

Declare each worker operation in the package's `operations` using
`defineOperation`. Include `id`, `title`, `input_schema`,
`output_schema`, `required_capability`, `risk`, `idempotency`, and
`methods`. A create operation can declare `creates` with its kind ID,
`required_capability: "content.write"`, `risk: "reversible"`,
`idempotency: "request_id"`, and `methods: ["item_save"]`. Use object
schemas with declared properties and required keys. An empty result can
use `{ type: "object", properties: {}, additionalProperties: false }`.
On success the host can supply a created reference to `then.route`.

With worker operations, save a local `worker.mjs` and let the build
include it. A placeholder for this offline exercise can contain
`process.exitCode = 1;`. The build validates the manifest and pins the
worker file without running it.

## Bind row actions and search

Declare fields used by rows in the Bind's `fields` and index custom fields
on the kind with `indexed_fields`. Boolean metadata shown to an authorized
member uses `{ type: "Boolean", audience: "member_item" }`. Override an
empty state with `states: { empty: { title: "Nothing here yet" } }`.

A row action goes in `row.leadingActions` or `row.trailingActions`. The
first action on each side may set `fullSwipe: true` if the catalogue allows
it. For a Boolean metadata patch, use `operation: "host.item.patch-metadata"`
and bind the three inputs `item` from `{ row: "id" }`, `field` from the
literal metadata key, and `value` from a Boolean literal. The host resolves
the target and commit authority.

For search, register the kind with `claimed_text: true` and
`searchable: true`. Add a Bind with `source.type: "search"` and the
searchable kind and reference its ID from `footer.search.bind` in
`defineChrome`. The bundled contract specifies whether the source needs an
explicit mode. If required, use `mode: "terms"`.

## First list
