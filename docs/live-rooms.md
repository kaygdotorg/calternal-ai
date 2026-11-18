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

While a writer is connected, the host can check the underlying file
at most once every five seconds. A write that races an outside change
also runs the check before retrying. The host hashes the exact file,
finds its last materialized revision, checks its stable identity and
merges the changed body against that base. It preserves the file's
BOM and dominant line ending. A moved, invalid or unprovable file is
preserved for recovery. Live replacement uses recoverable fsio
exchange so a racing write is preserved.

## Provisional transport limits

The proposed transport multiplexes rooms over a WebSocket upgrade at
`/api/v1/live` beside the existing SSE journal. The provisional limits are
32 live rooms per space, 32 participants per room and 512 MiB of room
memory across one host. Clean empty rooms retire after 60 seconds.
These values are subject to measurement before the first shared invite.

## Browser binding

Open a same-origin WebSocket at `/api/v1/live` with subprotocol
`calternal-live-rooms.v1`. The ordinary GET at that URL still uses the
space journal SSE query contract. Use one socket per page. Each room is
`<space UUID>/<kind>/<item UUID>`. Use increasing positive safe-integer
membership nonces across the socket. Join with type, room, membership,
and the lineage of any persisted local copy. A joined control echoes
room and membership with info.id, info.lineage and info.text set to md.
A join_failed control includes the public error. Drop frames for old
memberships. Leave with the same address and nonce.

Binary frames start with a lib0 var-string `@<membership>/<room>`,
followed by a y-protocols message. Chunked frames use
`+<zero-based sequence>/<total bytes>/@<membership>/<room>`. Each chunk
names the total and the same membership. Keep each message or chunk
below 1 MiB. There is one in-flight reassembly per socket, with 15 seconds
between chunks and a 64 MiB wire cap. A message exceeding the item's
content or state limits can still be refused inside that cap.

After joined, apply the host's sync step 2 to an empty Y.Doc or to the
persisted Y.Doc of the same lineage. Bind the editor to Y.Text md.
Use y-protocols sync step 1, step 2 and update, and awareness tag 1
with an owned client ID. Awareness carries the authenticated account
ID and an optional cursor with Yjs relative anchor and head. The browser
resolves the account display details and the anchors. Clear presence on
leave. Apply remote text after IME composition, preserve the original
transaction origins, and use a Y.UndoManager that tracks only the local editor origin.

Use tag 100 with a length-prefixed varuint check ID. The host returns
tag 101 with the latest check it can cover. A preview from another writer
is still pending until its durable commit. The client must correlate
that receipt with submitted updates in the same membership. A missing
dependency requests a fresh step 2 and holds checks. A permanent refusal
uses tag 102 with a length-prefixed JSON error-code string. Temporary
failure uses 103 before leaving. Keep unreceipted local and sibling-tab
edits across reconnect. A join refusal with error.code "conflict" and
error.details.reason "lineage_changed" calls for recovering only the
local diff from the last confirmed base. Retain the
recovery intent when it cannot be applied safely. A transport rejoin
or editor state alone must never clear the pending marker.

## Item access

The sharing adapter supplies a process-wide atomic authority revision.
A membership reuses its View only while the revision remains the same
even value it observed before and after a successful check. An odd value
pauses delivery. The next packet on that membership revalidates View
after an epoch change, with no eager scan of other memberships. Join
checks View and writes check Edit. The post-commit eviction hook cuts
off queued frames for recipients who lost their last grant.

On a stable epoch with the sharing adapter, a production peer frame
uses one current session statement, zero item or grant statements, and
zero plugin activation statements. A View refresh adds three reads for
Notes. A changed plugin activation epoch adds its own indexed read.
The interim space-grant adapter provides no revision, so its stable path
uses a current space capability statement in addition to the session
statement. These are path counts, measured latency is still required.

The additional sharing qualification uses 32 rooms with 32 participants
each and one unrelated grant mutation per second. Start timing at each
keystroke and stop after the peer applies it. The p95 ceiling remains 10 ms.
The fixture must use the real matching sharing revision source.

## Qualification

The two-client protocol benchmark is the room mode of calternal-perf.
Build the normal and perf-timings release binaries on dev-01 and copy
them to both perf VMs. Invoke the room mode under /usr/local/bin/perf-lock.sh
with the instrumented release as the head and the plain release of the
same revision as its control. Use the 1,000, 10,000 and 100,000 note
fixtures, alternating warm and cold arms. The driver times the first edit
through peer apply and through the matching durable save receipt. It
correlates the same keystroke's request ID with the journal commit
and the separately subscribed SSE client. Each arm also runs four
writers with 25 edits each and verifies every marker appears once in
durable content. A missing sample or a head
arm over any hard ceiling fails qualification.

The protocol driver starts a save check immediately after its edit.
The browser gate must also cover the editor's real check scheduling,
IME, undo and offline persistence when the browser binding lands.
