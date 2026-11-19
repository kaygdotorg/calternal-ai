# Write a Calternal plugin

From a Calternal checkout, install the workspace dependencies with `npm ci`
and build the SDK with `npm run build -w @calternal/plugin-sdk`. Use Node 24
and Python 3.

Create a directory under `plugins/` for your package. Give it a `package.json`
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

From the plugin directory, check the TypeScript declarations and build the
package.

```sh
npx tsc --noEmit --strict --skipLibCheck --module nodenext --target es2022 src/plugin.ts
npx --no-install calternal-build-plugin src/plugin.ts dist
```

Open `dist/manifest.json` and the app template named in its `apps` entry. The
manifest includes the package’s own kind in `reads`, a generated title
mapping, no network, and no worker operations. The template asks for 60 rows
and an exact count subject to host authority. It names the row title and
kind icon and includes empty, loading, and error messages.

The example has eight authoring concepts. The **package** groups declarations.
The **app** is the entry in the host shell. Its **kind** names a class of items.
**Placement** controls where they are stored. The **item schema** describes
their metadata. A **Bind** names the query that supplies rows. A **list** shows
those rows. The **sidebar section** refers to the same Bind by ID.

## Complete gear list

Save this as `src/plugin.ts`. The labeled comments locate the kind and
indexes, operation, list and swipes, form, search, and package assembly.
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
  methods: ["item_save"],
  required_capability: "content.write",
  risk: "reversible",
  idempotency: "request_id",
  input_schema: {
    type: "object",
    additionalProperties: false,
    required: ["title", "location"],
    properties: { title: { type: "string" }, location: { type: "string" } },
  },
  output_schema: { type: "object", additionalProperties: false, properties: {} },
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
  binds: [{ id: "gear-search", source: { type: "search", kind: kind.id, mode: "terms" } }],
  footer: { search: { bind: "gear-search", placeholder: "Search gear" } },
});

// App and package assembly
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

Save this as `worker.mjs`. Supply it as the third argument to
`calternal-build-plugin src/plugin.ts dist worker.mjs`. The SDK adds the
package-level worker reference and digest. Each form selects its operation
by ID. The offline placeholder exits with failure if started.

<!-- BEGIN GENERATED gear-worker -->

```js
// Offline authoring example. Fail closed if launched.
process.exitCode = 1;
```

<!-- END GENERATED gear-worker -->
