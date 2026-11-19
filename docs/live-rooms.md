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

One server owns the installation's locked roots for its lifetime.
A second server cannot open those roots concurrently. After a crash the
next owner replays committed publication intents and loads the saved
checkpoints. A peer can see a preview before commit, so the browser keeps
its pending changes until it receives the matching durable check receipt.

A client can reconnect after an hour with a full update from its persisted
replica. It can integrate with the current room if the lineage and causal
dependencies still match. Trash, a missing or replaced file, and a write
from a previous release can prevent that merge. Keep the local edit
available for recovery or manual copy in those cases.

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
and the lineage of any persisted local copy. Accept `joined` only when
`room` and `membership` match the pending join. Require `info.lineage`,
verify `info.id` against the requested item UUID, and require
`info.text === "md"` before binding. A `join_failed` control includes
the public error. If authority stays in transition, the code is
`rate_limited` and `details.retry_after_ms` gives the delay before a fresh join.
Drop frames for old memberships. Leave with the same
address and nonce.

Binary frames start with a lib0 var-string `@<membership>/<room>`,
followed by a y-protocols message. Chunked frames use
`+<zero-based sequence>/<total bytes>/@<membership>/<room>`. Each chunk
names the total and the same membership. The server sends this
continuation form for outbound payloads over 512 KiB, with at most
512 KiB of payload per frame. Reassemble in order to the declared byte
count before decoding, even for the initial sync step 2. Use one
in-flight reassembly per socket, a 15-second gap limit and a 64 MiB wire
cap in either direction. Each inbound WebSocket message, including its
header, fits within 1 MiB. The item also has smaller content and state
limits.

After joined, apply the host's sync step 2 to an empty Y.Doc or to the
persisted Y.Doc of the same lineage. Bind the editor to Y.Text md.
Use y-protocols sync step 1, step 2 and update, and awareness tag 1
with one client ID bound by the host to that membership. An awareness
object uses `account_id` and `cursor`. A cursor object contains `anchor`
and `head` as Yjs relative positions. The cursor may be null. The server
stamps `account_id` from the authenticated participant, replacing any
client-supplied identity. The browser resolves display details and the
anchors. Clear presence on
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
or editor state alone must never clear the pending marker. Each reconnect
checks current access. If the account lost View while disconnected, the
new join returns `join_failed` with `error.code: "not_found"`.

When an established membership loses View, the server sends a JSON text
control with `type: "left"`, the `room` and `membership`, and
`reason: "access_revoked"`. Explicit eviction sends it immediately. A
lazy authority refresh sends it before the next affected delivery or
write. Discard that membership's incoming queue
and stop retrying it. The socket can still serve other memberships.
The Notes binding calls `sharing.rescue` with its own unreceipted changed
blocks. If rescue fails, retain the local recovery intent. A generic
`left` without this reason or a dropped connection follows ordinary
reconnect handling.

## Item access

The sharing adapter supplies a process-wide atomic authority revision.
A membership reuses its View only while the revision remains the same
even value it observed before and after a successful check. An odd value
pauses delivery. The next packet on that membership revalidates View
after an epoch change, with no eager scan of other memberships. Join
checks View and writes check Edit. The post-commit eviction hook cuts
off queued frames for recipients who lost View. A surviving grant or
space role can still authorize the account.

Once the revoke commits, the host stops admitting edits from that
membership and approving room frames for its transport. Edit admission uses a stable
authority revision. An edit already admitted can finish for peers who
still have View. If eviction reaches the writer before its preview enters
fanout, the edit is aborted. Each frame needs a current View decision
after its WebSocket is writable, even when the update was accepted and
queued earlier. The host submits an approved frame in the same poll.
A handoff approved before commit can finish, and bytes already in
WebSocket or operating-system buffers can arrive after commit.

On a stable epoch a production peer frame uses one current session
statement, zero item or grant statements, and zero plugin activation
statements. A View refresh adds three reads for Notes. A changed plugin
activation epoch adds its own indexed read. Every write checks Edit,
and it refreshes View first if its membership's proof expired.
These are path counts, measured latency is still required.

For the additional sharing qualification, pass
`--rooms-only --room-access-storm`. The fixture opens 32 rooms with 32
participants each and changes a grant in a separate space once per second.
Each of five one-second windows has one grant mutation and one edit per
room. The grant mutator runs independently of the writer. Each of the 31
observers is timed through peer apply for 4,960 samples per arm.
Participants reuse the authenticated fixture session. The p95 ceiling is
10 ms. The candidate must use the real sharing revision source.

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
