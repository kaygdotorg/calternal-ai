# Host call route · 2026-11-27

Browser and CLI calls go through `POST /v1/spaces/{space}/call`. The
space is a UUID. The HTTP adapter resolves a session through the auth crate. Browsers
use the strict HttpOnly host cookie and send the configured origin.
CLI calls can still supply a bearer session. The verified caller must own the personal space. The request body carries no person, role, plugin identity or
run scope. In-process adapters supply those from their trusted session or
worker context. The worker transport can wrap the same operation and input
in its own frame.

```json
{"call":"s.query","input":{"from":"calternal-chat.chats","where":"not archived","sort":["title asc"],"limit":50}}
```

Success is `{"result": ...}`. The error body is `{"error":{"code": "...",
"message": "...", "next": "..."}}`. The error codes follow format §8.1.

| Call | Input | Result |
| --- | --- | --- |
| `s.declarations` | `{}` | `{space, plugins}` with the reachable installed kinds, fields, operations and inputs, collections, sidebars, slots and fillers. |
| `s.query` | `{from, where?, sort?, cursor?, limit?}` or `{collection, cursor?, limit?}`. Collection names are qualified plugin and collection IDs. | `{rows, totals, cursor}`. Rows contain `id`, `kind`, `fields`, `computed`, viewer `state`, server-evaluated `when`, `edited` and `version`. `cursor` is opaque and null on the last page. |
| `s.read` | `{item}` | The same row metadata plus `body` and `records`. |
| `s.write` | `{changes, preview?}`. Each change carries its operation in `op`. | `{changes:[{id, version, anchor?, link?}]}` in input order. Preview performs validation and returns the planned changes. |
| `s.state` | `{op:"get",item,key}` or `{op:"set",item,key,value}` | `{value}` |
| `s.me` | `{}` | `{id,role}` |
| `s.space` | `{}` | `{id}` |
| `index.rebuild` | `{}` (shell only) | `{items}` |

`s.write` change tags and payloads are `add {kind,fields,body?}`,
`change {item,fields?,body?,base?}`, `remove {item}`,
`append record {item,record:{fields?,body?}}`,
`change record {item,anchor,record:{fields?,body?}}` and
`remove record {item,anchor}`. Example for a verified owning-plugin context:

```json
{"call":"s.write","idempotency_key":"gesture-1","input":{"changes":[{"op":"append record","item":"019a0000-0000-7000-8000-000000000001","record":{"body":"Hello"}}]}}
```

An append receipt includes the item's space-relative Markdown path and
the host-issued anchor in a Markdown link such as
`[Record](<Chats/A%20chat/chat.md#^0123456789abcdef>)`. Consumers resolve
the anchor within the authorized space if the path has changed. The SDK
translates a plugin author's single-key change object into the wire's
`op`-tagged object before sending the callback.

Output records add the host's
`anchor`, `author` and `at` fields. An adapter may pass
`idempotency_key` at the envelope level. The host scopes receipts to
the verified person, space and plugin. Reusing a live key with different
input yields `conflict`.

This is the adapter contract for slices 1 and 2. The plugin process lane
supplies operation dispatch and event context. The runs lane supplies the
trusted run context. Run calls require a scope binding before they can use
this route.

## Presentation declarations

`s.declarations` projects the checked, installed catalog. Kind fields carry
a boolean `badge` flag. Collection views carry their declared `type`, including
`strip`, and an ordered `columns` array that preserves commas inside formulas.
Collections and sidebar links each carry their own `icon` token or null.
Use the link's token first and then the collection's.

Slots carry their optional `label`, the existing `inputOperation` reference
and either null or a `composer` object. Its `text` and optional `quote`
are names in that operation's typed inputs, while `draft` is an optional
state key without the `state.` prefix. Chat declares the following composer binding.

```json
{"label":"Message","inputOperation":"chats.send","composer":{"text":"text","draft":"draft","quote":"quote"}}
```

Both the slot picker and the supplying plugin's fill projection carry
the fill's display `label`, falling back to the supplying plugin's name.
Picker values still use plugin IDs. An icon, label or composer input name
confers no authority. The record-link binding names an input type and
obtaining a link for an existing record still requires a host operation.

## Coded operations

The session and trusted asynchronous adapters also dispatch installed
coded operations. Name the plugin, kind and operation, as in
`calternal-chat.chats.send`. The two-part name is accepted when it names
one unambiguous coded operation in the plugin. The worker pipe gets the
kind-qualified selector, current acting person, space and stable event
key from the host. An item-scoped operation declares
`item: link(<its kind>), required` in its typed inputs and accepts the
target UUID under that key. An operation without that input has no
event item. Worker effects return through the same checked route.

The isolated install probe discovers `on.change` selectors. A selector
names a kind or collection visible to the installing plugin's code.
Use `collection:` to disambiguate a collection sharing a kind name.
After a successful write the host sends an item observation to matching
registered plugins, excluding plugins already in the write's ancestry.
The worker callback is outside the space lock.

The append path supplies the record link consumed by Chat's optional
harness call. The person selects a filler using the call envelope:

```json
{"call":"calternal-chat.chats.send","input":{"item":"019a0000-0000-7000-8000-000000000001","text":"Hello"},"idempotency_key":"gesture-1","slot_selection":{"slot":"harness","filler":"my-harness"}}
```

The route checks the calling plugin's person-picked slot and resolves
the installed filler's one declared coded operation. The worker sees
the qualified operation in `s.event.selected_operation`. Its `s.call`
is admitted only for that operation under the active selection. The
selection does not carry through reactions or further nested calls.
An operation name in plugin input supplies no selection. A selected harness starts a session-scoped run for the original Chat item's
UUID. Chat returns its append receipt with an additional `run` result
containing the ID and status. A replay preserves both the append and run IDs. A gesture that reused its key
with a different harness fails with `conflict`.
The restart sweep terminates old process families. Start receipts survive and
report those runs as `stopped` on replay. Live output is held in memory.

## Runs

The installed plugin's `programs:` binding supplies the executable and its
`sends out:` list supplies the exact CONNECT allowlist. Both run inside the
operator-configured Podman boundary with a disposable provider profile.

- `s.run.send {id,input}` sends from the starting person or their starting
  harness invocation. `s.run.stop {id}` requires current read access to the
  target. Both methods are available on the authenticated call route.
- Only the item of the originating coded operation establishes session scope.
  The target list must equal that one item. The run context follows nested
  callbacks and checks explicit targets before dispatch. It admits reads and
  state for the session, session-bound coded calls, and single record appends.
  A cross-session target receives `not-allowed`.
- A harness can emit a newline-delimited
  `{"host_call":{"id":1,"call":"s.read","input":{"item":"…"}}}` frame. The
  host checks its bound identity and scope and returns a `host_reply` input
  with the same ID, carrying `result` or an `error`. IDs rise within each
  process; the host derives retry keys itself.
- A successful adapter completion sends the buffered reply through
  `calternal-chat.chats.send` under the run context. The host stamps the
  run UUID into the authenticated record header and returns it as `run`
  on that record.
