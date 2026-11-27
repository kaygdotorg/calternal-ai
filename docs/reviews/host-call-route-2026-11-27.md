# Host call route · 2026-11-27

Browser and CLI calls go through `POST /v1/spaces/{space}/call`. The
space is a UUID. The HTTP adapter resolves a bearer session through the
auth crate. The verified caller must own the personal space. The request body carries no person, role, plugin identity or
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
| `s.query` | `{from, where?, sort?, cursor?, limit?}` or `{collection, cursor?, limit?}`. Collection names are qualified plugin and collection IDs. | `{rows, totals, cursor}`. Rows contain `id`, `kind`, `fields`, `computed` and `version`. `cursor` is opaque and null on the last page. |
| `s.read` | `{item}` | `{id, kind, fields, computed, body, records, version}` |
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
key from the host. Worker effects return through the same checked route.

The isolated install probe discovers `on.change` selectors. A selector
names a kind or collection visible to the installing plugin's code.
Use `collection:` to disambiguate a collection sharing a kind name.
After a successful write the host sends an item observation to matching
registered plugins, excluding plugins already in the write's ancestry.
The worker callback is outside the space lock.

The append path already supplies the record link consumed by Chat's
optional harness call. The selected harness operation must be listed
in the calling manifest. The run adapter and durable delivery of
reactions across a host restart remain separate integration work.
