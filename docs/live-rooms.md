# Live editing service

A Markdown kind requests live editing by adding `"x-calternal-live-editing":
true` to its root manifest kind schema. The host provisions that choice in
the selected space. Identity is the space UUID, registered kind and item UUID.
A path move keeps the item's identity. The host uses existing content
revisions, the space journal and checked file placement so plugin writes use
the same durable result and retry contract as ordinary content writes.
See [Plugin API](plugin-api.md) for write forms.

## Merge and durability

A replacement or patch names the base revision. The host restores the
Yjs checkpoint for that base, applies the smallest text edits there and
merges the update into the current document. Edits use UTF-16 positions
at grapheme boundaries. An append writes at the current end. Text-only
edits preserve the head's metadata. A stale replacement which also changes
metadata must agree with the current head or its base. A missing base or
a different lineage returns a conflict before the durable write.

The content revision, room checkpoint and publication intent commit in
one space-local SQLite transaction. Publication and request replay use the
existing space journal. A revision from a writer without checkpoints seeds
a new lineage on demand. An explicit purge clears room checkpoints through
the revision foreign key. See [Upgrades](upgrades.md) for the schema and
the recovery contract.

## Provisional transport limits

The proposed transport multiplexes rooms over a WebSocket upgrade at
`/api/v1/live` beside the existing SSE journal. The provisional limits are
32 live rooms per space, 32 participants per room and 512 MiB of room
memory across one host. Clean empty rooms retire after 60 seconds.
These values are subject to measurement before the first shared invite.
