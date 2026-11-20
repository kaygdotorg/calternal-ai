# Write a Calternal plugin

From a Calternal checkout, install the workspace dependencies with `npm ci`
and build the SDK with `npm run build -w @calternal/plugin-sdk`. Use Node 24
and Python 3.

Create a directory under `plugins/` for your plugin. Give it a `package.json`
with `"type": "module"` and `"@calternal/plugin-sdk": "*"` in `dependencies`.
Run `npm install` from the checkout root to link the new workspace. Save the
following example as `src/plugin.ts` in that directory.

<!-- BEGIN GENERATED first-list -->

```ts
import { defineApp, defineChrome, defineKind, definePlugin, defineView } from "@calternal/plugin-sdk";

export default definePlugin({
  api_version: 2,
  id: "bookmarks",
  name: "Bookmarks",
  icon: "book",
  version: "1.0.0",
  kinds: [defineKind({
    id: "bookmark",
    icon: "book",
    storage: { hidden: true },
    claimed_text: true,
    schema: {
      type: "object",
      required: ["title"],
      additionalProperties: false,
      properties: { title: { type: "string" } },
    },
  })],
  apps: [defineApp({
    id: "bookmarks",
    name: "Bookmarks",
    icon: "book",
    view: defineView({
      binds: [{ id: "items", source: { type: "list", kind: "bookmark" } }],
      root: { type: "list", bind: "items" },
    }),
    chrome: defineChrome({
      sidebar: { sections: [{ id: "items", title: "Bookmarks", bind: "items" }] },
    }),
  })],
});
```

<!-- END GENERATED first-list -->

From the plugin directory, build the plugin. The command type-checks it
when the directory has a `tsconfig.json`, then emits and verifies the package.

```sh
npx --no-install calternal-plugin build src/plugin.ts dist
```

Open `dist/manifest.json` and the app template named in its `apps` entry. The
manifest includes the plugin’s own kind in `reads`, a generated title
mapping, no network, and no worker operations. The template asks for 60 rows
and an exact count subject to host authority. It names the row title and
kind icon and includes empty, loading, and error messages.

The example has eight authoring concepts. The **plugin** groups declarations.
The **app** is the entry in the host shell. Its **kind** names a class of items.
**Placement** controls where they are stored. The **item schema** describes
their metadata. A **Bind** names the query that supplies rows. A **list** shows
those rows. The **sidebar section** refers to the same Bind by ID.

## Complete gear list

Save this as `src/plugin.ts`. The labeled comments locate the kind and
indexes, operation, list and swipes, form, search, and plugin assembly.
Both swipes patch the current row's item using `{ row: "id" }`. For an
action that reads a projected value, use `{ row: "field", field: "packed" }`.
Keep action objects inline in `defineView` so TypeScript preserves their
literal types. A manifest uses JSON Schema `"boolean"` for the stored
property and catalogue `"Boolean"` for the indexed projection.

<!-- BEGIN GENERATED gear-list -->

```ts
import { defineApp, defineChrome, defineKind, defineOperation, definePlugin, defineView } from "@calternal/plugin-sdk";

// Kind and indexed fields
const kind = defineKind({
  id: "gear",
  icon: "book",
  storage: { hidden: true },
  claimed_text: true,
  searchable: true,
  schema: {
    type: "object",
    additionalProperties: false,
    required: ["title", "location"],
    properties: {
      title: { type: "string" },
      location: { type: "string" },
      packed: { type: "boolean" },
    },
  },
  indexed_fields: { packed: { type: "Boolean", audience: "member_item" } },
});

// Operation declaration
const addGear = defineOperation({
  id: "gear.add",
  title: "Add gear",
  creates: kind.id,
  methods: ["item_create"],
  required_capability: "content.write",
  risk: "reversible",
  idempotency: "request_id",
  input_schema: {
    type: "object",
    additionalProperties: false,
    required: ["title", "location"],
    properties: { title: { type: "string" }, location: { type: "string" } },
  },
  output_schema: {
    type: "object",
    additionalProperties: false,
    required: ["created"],
    properties: { created: { $ref: "host-catalogue-v2.json#/$defs/ItemRef" } },
  },
});

// List Bind, empty state and row swipes
const view = defineView({
  binds: [{
    id: "gear",
    source: { type: "list", kind: kind.id },
    fields: ["packed"],
    states: { empty: { title: "No gear yet" } },
    row: {
      trailingActions: [{
        id: "pack",
        title: "Mark packed",
        operation: "host.item.patch-metadata",
        fullSwipe: true,
        inputs: { item: { row: "id" }, field: { literal: "packed" }, value: { literal: true } },
      }],
      leadingActions: [{
        id: "unpack",
        title: "Clear packed",
        operation: "host.item.patch-metadata",
        fullSwipe: true,
        inputs: { item: { row: "id" }, field: { literal: "packed" }, value: { literal: false } },
      }],
    },
  }],
  root: {
    type: "stack",
    children: [
      { type: "list", bind: "gear" },
      // Form fields and submit Invoke
      {
        type: "form",
        id: "add",
        fields: [
          { id: "title", label: "Name", input: "text", required: true },
          { id: "location", label: "Location", input: "text", required: true },
        ],
        submit: {
          id: "add",
          title: "Add gear",
          operation: addGear.id,
          inputs: { title: { form: "add", field: "title" }, location: { form: "add", field: "location" } },
          then: { route: { name: "item" } },
        },
      },
    ],
  },
});

// Search Bind and footer
const chrome = defineChrome({
  binds: [{ id: "gear-search", source: { type: "search", kind: kind.id } }],
  footer: { search: { bind: "gear-search", placeholder: "Search gear" } },
});

// App and plugin assembly
export default definePlugin({
  api_version: 2,
  id: "gear-list",
  name: "Gear List",
  version: "1.0.0",
  icon: "book",
  kinds: [kind],
  operations: [addGear],
  apps: [defineApp({ id: "gear", name: "Gear", icon: "book", view, chrome })],
});
```

<!-- END GENERATED gear-list -->

Save this as `worker.mjs`. Supply it with
`calternal-plugin build src/plugin.ts dist --worker worker.mjs`. The SDK adds the
plugin-level worker reference and digest. Each form selects its operation
by ID. The offline placeholder exits with failure if started.

<!-- BEGIN GENERATED gear-worker -->

```js
// Offline authoring example. Fail closed if launched.
process.exitCode = 1;
```

<!-- END GENERATED gear-worker -->

## Operation and form field choices

<!-- BEGIN GENERATED field-reference -->

| Field | Allowed values |
| --- | --- |
| `risk` | `read`, `reversible`, `irreversible`, `external`, `widens_access` |
| `idempotency` | `read_only`, `request_id`, `never` |
| `methods` | `item_create`, `item_save_begin`, `item_save_chunk`, `item_save_seal`, `contribution_call`, `contribution_read`, `item_head`, `item_range`, `item_spans`, `viewer_state_read`, `viewer_state_write`, `item_history`, `item_save`, `item_trash`, `item_restore`, `item_rename`, `item_rename_status`, `item_rename_undo`, `item_purge`, `empty_trash`, `collection_create`, `collection_membership`, `items_list`, `items_search`, `sharing_check`, `start_job`, `checkpoint_job`, `job_status`, `cancel_job`, `read_setting`, `set_setting`, `request_secret_handle`, `proxy_request`, `credential_call`, `execution_start`, `execution_status`, `execution_stop`, `execution_input`, `execution_spawn_child`, `execution_wait`, `alarm_set`, `alarm_cancel`, `item_append`, `item_view_publish`, `execution_session_bind`, `item_view_context`, `extension_choices` |
| `required_capability` | `space.read`, `content.write`, `plugins.use`, `plugins.manage`, `members.invite`, `members.manage`, `managers.manage`, `space.settings`, `space.transfer`, `space.delete`, `spaces.list`, `spaces.create` |
| `FormField.input` | `text`, `multiline`, `url`, `email`, `number`, `date`, `choice`, `toggle`, `file` |

<!-- END GENERATED field-reference -->

Use `methods` as an array containing only the broker methods the operation
needs. A `read` operation uses `read_only`. Other risks use `request_id`
or `never`. A kind must declare storage placement. `{ hidden: true }`
is one choice. In a form, `text` is single-line and `multiline` accepts
multiple lines.
