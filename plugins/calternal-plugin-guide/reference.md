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
