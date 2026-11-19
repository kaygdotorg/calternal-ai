---
name: calternal-plugin-authoring
description: Scaffold, implement, test and live-load a TypeScript Calternal plugin.
---

## Start in the development shell

If the task supplies an environment file and project directory, first source
that file on the development machine, then change into the supplied project
directory. Every SSH command starts a fresh shell. Source the file again in
each block that uses the tools. Check `command -v calternal node npm` in that
same shell before proceeding. The env file supplies the tool PATH, SDK root
and server connection. Avoid guessing binary paths or printing credentials.
Use a quoted here-document into the remote shell for multiline source so
local quoting does not rewrite the code. Substitute the host and env file
from the task in this form.

```sh
ssh HOST_FROM_TASK bash -s <<'REMOTE'
source /path/from/task/env
cd "$PROJECT_DIR"
calternal describe --json
REMOTE
```

Outside a prepared environment, install Node.js 22.13 or later and npm,
put `calternal` on PATH, and use a built Calternal SDK. In a full Calternal
checkout, `nix develop` supplies the Node toolchain. The SDK is built by
`npm run build:plugins` in that checkout.

From the project directory, run `calternal plugin new my-plugin`. It creates
`./my-plugin` relative to your current directory. Change into it and run
`npm install`. The starter's `worker.ts`, `worker.test.mjs`, `manifest.json`
and README are in the package root. Build generates
`manifest.generated.ts` and `dist/worker.js`. Change the manifest and worker
together and update the tests when operation IDs or output contracts change.

## Work from the public contracts

For an Inbox question or deadline, use the reference's **Wait for a signal**
section. The public guide's `jobs` and `waits` topics include the complete
question job example.

For a sealed image preview, see **B. Content fetch** in the reference. It
shows the exact `network.content_fetch` declaration, the worker and the
authenticated frame check.

Read the bundled reference at `../../reference.md` and walkthrough at
`../../walkthrough.md`, relative to this skill's directory. They cover
broker calls, GenUI, jobs, settings and the fixture protocol. You can also
invoke the guide in the selected space.

```sh
jq -n --arg space "$SPACE_ID" '{space_id:$space, topic:"reference"}' |
  calternal invoke calternal-plugin-guide.explain --input-file -
```

A space-targeted plugin invocation includes `space_id` in its HTTP or CLI
input. The host resolves it and removes it before invoking your handler.
Your handler reads fields declared in the operation schema. Item-targeted
operations can resolve the space from the item ID. For a job returned as
`JOB_ID`, see the reference's `jobs.get` example. `calternal describe --json`
shows the schemas for the host operations. Use `--input-file PATH` for JSON
from a file or `--input-file -` for a pipe. The examples use `jq` to encode
shell variables and HTML as JSON.

Declare capabilities and broker calls in the manifest. Durable writes
need a mutating operation, the matching capability and a risk description.
Keep fixture tests in step with the handler inputs and outputs. Use portable
GenUI with its required labels, and encode dynamic text with `JSON.stringify`.

## Check and use the running server

Run `calternal plugin check .` and `calternal plugin test .`. Install
dependencies if a required package is missing. If the fixture worker
exited, read its reported status or signal and stderr. If the worker
reported a handler failure, check the operation name, input shape and
callback fixtures. An unhandled host callback is denied. A thrown handler
error yields a failed invocation.

When renaming a scaffold operation, replace its ID in the README as well
as in the manifest, worker and fixture test.

With the development env sourced, run `calternal plugin dev . --once`.
Without env defaults, pass `--appdata PATH --space UUID --account UUID`.
Use the supplied server built for development drafts and its enrolled
worker machine. Confirm Ready, invoke the authored operation through the
supplied connection and verify its effect or generated view. Omit
`--once` to watch the package. A rejected build leaves the last ready
draft available. If an invocation reports a busy worker, wait for its
current invocations to finish. Reuse the request ID when retrying the
same durable write. If an operation returns `committed_write`, the
save happened. Use its ID and string revision to reconcile the result.

When an operation fails and the message is not enough, run
`calternal plugin diagnostics <package-id> <operation>` with the same env.
It shows the loaded revision, the validate, handshake and publish times, the
last five errors for you and that operation with the schema field path, and
why the last rejected edit stopped (validation, handshake or publication).
It never shows values your worker returned. Nothing is stored on disk.

## Shared settings and generated contributions

Declare the key, primitive `value_type` and default in `settings`. Declare
`read_setting_v1` in `host_calls`, call `readSetting(context, key)` from the
worker and narrow the result to the declared type. A generated
`composer_row` slot targets the operation that returns the view. The
reference describes the slot and the GenUI grammar.

Use `plugins.settings.get` as Manager with `space_id`, `plugin_id` and
`key`. For `plugins.settings.set`, include those fields, the read
result's stored `value` as `base`, and the new `value`. Reset with null
and check the result's `conflicted` flag. Invoke the view again and check
`plugins.list` for its resolved contribution.
