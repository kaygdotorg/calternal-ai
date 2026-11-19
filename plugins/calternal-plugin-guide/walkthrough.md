# Write a Calternal plugin

1. Build the SDK in a Calternal checkout with `npm run build:plugins`.
2. Run `calternal plugin new my-plugin --sdk-root /path/to/Calternal-AI`.
3. Open the generated manifest and worker. Keep the package ID stable. Change
   the operation schema and its handler together.
4. Use a generated view first. Request the narrowest broker calls and declare
   the risk of each operation in the manifest.
5. Run `npm install`, then `calternal plugin check` and `calternal plugin test`
   in the package. Tests run the actual bundled worker against fixture data.
6. With a `dev-plugin-drafts` build running in development mode and an enrolled
   machine, run `calternal plugin dev . --appdata "$APPDATA" --space "$SPACE_ID" --account "$ACCOUNT_ID"`.
   Wait for Ready and invoke the operation in that space. Change the view text,
   wait for Ready again, then invoke it again to verify the new result.
7. Run `calternal plugin bench` to compare fixture transport paths.
8. Show the manifest, operation risks and host calls to the administrator
   who will review and activate the package.

## Generated status view starter

The files below come from the default `calternal plugin new my-plugin`
scaffold. The fixture invokes its view operation over the worker protocol.

<!-- BEGIN GENERATED view -->

### manifest.json

```json
{
  "id": "my-plugin",
  "name": "my-plugin",
  "icon": "puzzle",
  "version": "0.1.0",
  "api_version": 1,
  "kinds": [
    {
      "id": "my-plugin",
      "schema": {
        "type": "object"
      },
      "storage": {
        "hidden": true
      },
      "claimed_text": true
    }
  ],
  "operations": [
    {
      "id": "my-plugin.hello",
      "input_schema": {
        "type": "object",
        "properties": {
          "reads": {
            "type": "boolean"
          }
        },
        "required": [],
        "additionalProperties": false
      },
      "output_schema": {
        "type": "object",
        "properties": {
          "view": {
            "type": "object"
          }
        },
        "required": [
          "view"
        ],
        "additionalProperties": false
      },
      "required_capability": "space.read",
      "mutates": false,
      "idempotency": "read_only",
      "risk": "read"
    }
  ],
  "slots": [
    {
      "id": "hello",
      "label": "Plugin status",
      "slot": "card_window",
      "renderer": {
        "type": "generated",
        "operation": "my-plugin.hello"
      }
    }
  ],
  "collections": [],
  "views": [],
  "attention": [],
  "settings": [],
  "schedules": [],
  "webhooks": [],
  "secrets": [],
  "hooks": [],
  "themes": [],
  "host_calls": [
    "read_item"
  ]
}
```

### worker.ts

```ts
/** Example plugin with a manifest-typed operation and a portable view. */
import { serveWorker, typedHandlers } from "@calternal/plugin-sdk";
import manifest from "./manifest.generated.js";

const handlers = typedHandlers(manifest, {
  "my-plugin.hello": async (input, context) => {
    if (input.reads) {
      await context.callAll(Array.from({ length: 3 }, () => ["read_item", {
        kind: "my-plugin",
        item_id: "01900000-0000-7000-8000-000000000003",
      }] as const));
    }
    return { view: { genui_version: 2, source: 'Row([Metric("Plugin", "Ready")])' } };
  },
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

### worker.test.mjs

```js
import assert from "node:assert/strict";
import { test } from "node:test";
import { fixtureWorker } from "@calternal/plugin-sdk/testing";
import { spawn } from "node:child_process";

test("returns a generated view over the worker protocol", async () => {
  const fixture = fixtureWorker(spawn(process.execPath, ["dist/worker.js"]));
  try {
    const view = await fixture.invoke("my-plugin.hello", {});
    assert.equal(view.view.genui_version, 2);
    assert.match(view.view.source, /Plugin/);
  } finally {
    await fixture.close();
  }
});
```

<!-- END GENERATED view -->

## Reading-list plugins

Declare the `reading-items` kind and the `list_items_v1` host call. Call
`listItems(context, { kind: "reading-items", limit: 25, cursor: input.cursor })`
from the SDK in the read operation.

Use `page.items` to populate the generated list and return
`page.next_cursor` in the declared result. Declare the read operation
with `space.read`, `read_only` idempotency and `read` risk.

## A setting on a composer chip

Declare `settings: [{ "id": "label", "value_type": "string", "default": "Quick note" }]`,
and include `read_setting_v1` in `host_calls`. Use `readSetting(context, "label")`
inside a read-only operation. Return a GenUI badge from that operation and target
it from a generated `composer_row` slot. The text argument can be encoded with
`JSON.stringify` to keep quotes inside the label.

Run `calternal plugin check .` and `calternal plugin test .`, then start
the draft in the supplied space with `calternal plugin dev`. Use the
normal Manager session to invoke `plugins.settings.get` with the space UUID,
package ID and key. Invoke `plugins.settings.set` with those fields, the current
override as `base`, and a new string as `value`. Invoke the view to see the
change. The view must still show the default in another enabled space.

## Check the whole path

Run each group of commands in the project directory with the development
env sourced in that shell. Edit the root-level `worker.ts` and
`worker.test.mjs` produced by the scaffold. The fixture tests do not execute
the host's job scheduler or consume progress chunks. For a job plugin,
mock admission to assert the operation and payload. Test pure step logic
separately. After live loading, check ordering through the real SSE
endpoint and confirm the durable result with `jobs.get`. The first real
run supplies `checkpoint: null`.

For a Markdown list, create a distinct item through the declared add
operation, call the view operation, and confirm the generated list shows
the saved title. For a settings view, check both the default and an edited
value with the current stored value as the edit base. See the public
reference for callback payloads and component signatures.
