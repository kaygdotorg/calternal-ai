# Calternal author guide

## First list in eight concepts

This complete `plugin.ts` declares a package, app, kind, storage
placement, item schema, Bind, list, and sidebar section. The SDK supplies
the title, icon, window, states, reads, and no-network defaults.

<!-- first-list.ts -->

## Complete gear list with form, swipe and search

Copy this as one file. Find each declaration by its labeled comment.
The JSON Schema field uses `"boolean"`. Its catalogue projection uses
`"Boolean"` and declares an audience. The swipe target is the row's item
`{ row: "id" }`. Use `{ row: "field", field: "packed" }` when reading a
projected field. Keep actions inline as shown so TypeScript preserves
literal types. The form's `then` route is optional and uses the
created reference returned by the host.

<!-- gear-list.ts -->

## Worker and build

When you declare operations, write a second file named `worker.mjs`.
Use this complete placeholder for the offline exercise.

<!-- gear-worker.mjs -->

The form names an operation ID from `operations`. The SDK's build takes
the worker path separately and emits its digest and package-level manifest
reference. The trial command supplies `worker.mjs` when present.

With Node 24, npm, and Python 3, run the install before the build command:

```sh
npm init -y
npm pkg set type=module
npm install --offline --ignore-scripts --no-audit --no-fund ./calternal-plugin-sdk-0.1.0.tgz
sh build-and-check.sh
```
