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
{"call":"calternal-chat.chats.send","input":{"chat":"019a0000-0000-7000-8000-000000000001","text":"Hello"},"idempotency_key":"gesture-1","slot_selection":{"slot":"harness","filler":"my-harness"}}
```

The route checks the calling plugin's person-picked slot and resolves
the installed filler's one declared coded operation. The worker sees
the qualified operation in `s.event.selected_operation`. Its `s.call`
is admitted only for that operation under the active selection. The
selection does not carry through reactions or further nested calls.
An operation name in plugin input supplies no selection. The run
adapter and durable reaction delivery across restart remain separate
integration work.
