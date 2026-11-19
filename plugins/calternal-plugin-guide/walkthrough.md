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

To see a two-field form, search, and a Mark read swipe together, open the
[Reading List declaration](../calternal-reading-list/src/plugin.ts). Its Add
form supplies just the values the user entered. The host resolves the target
of each swipe and commits the metadata patch against the current item. An
authorized create outcome supplies the new item reference to `then.route`.
The fixture build validates these declarations offline. Host effects require
the v2 runtime.
