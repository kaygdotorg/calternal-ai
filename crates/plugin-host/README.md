# Plugin host library

`ManifestV1` is data read before code execution. `Registry` validates schemas and
publishes declaration/policy rows for a trusted grant surface. `WorkerSession`
provides worker-v2 admission, callback authorization, cancellation, ordered job
chunks and bounded queues. This crate mounts no routes and starts no processes.

Wire frames are UTF-8 newline-delimited JSON, max 256 KiB excluding newline.
Every frame has `protocol_version: 2`, `invocation_id`, and a tagged `message`.
The host sends `invoke`, `host_reply`, `cancel`; workers send `host_call`, `chunk`,
`complete`, `failed`. Unknown fields/versions/directions are errors. Binary or
unterminated frames are not accepted by the example transport. Bound the input
buffer **before** JSON decoding. Preserve FIFO order; pause reads when a queue is
full and retry the same frame after draining. Full queues never advance stream
sequence or execute broker effects. There are 32 concurrent invocations, 32
queued messages per direction, 128 callbacks per invocation, and 4096 lifetime
invocations before worker rotation. IDs cannot be reused within a session.

A process adapter owns exactly one long-lived worker for each (plugin, space),
pins its code/config version, supervises exit and deadlines, and drains both
queues. It compiles the per-operation input/output schemas once per session;
validation is synchronous and must run off an async event loop when necessary.
Schema HTTP/file retrieval is disabled. Schema evaluation complexity depends on
the schema; admission/compilation and runtime execution require external CPU and
memory limits before hostile plugins can be admitted.

Every callback echoes the original account/space/grant reference; equality does
not authorize it. The trusted `HostBroker` checks live session/installation,
membership, plugin activation and the operation’s closed required capability on
every call. It must enforce attenuated method/target grants as well. It must check again in the transaction/effect boundary, and provide durable
receipts for writes. Secret requests accept `{ "name": "declared-name" }` and return only
`{ "handle": "opaque-host-issued-id" }`; other response shapes are redacted. `start_job`
returns `{ "job_id": "host-issued-id" }`; only jobs admitted for this invocation
may emit chunks, whose sequence starts at zero across that invocation. These are
ephemeral chunks; durable job admission/checkpoints/replay belong to a later host
adapter. `next_delivery` rechecks current authority before returning queued data.
`next_outgoing` also rechecks current authority before releasing an invocation or
callback reply to the transport. Adapters must deliver immediately after draining,
and keep their final storage/network checks atomic with protected effects.

Cancellation invalidates local invocation state and drops queued results before
queuing `cancel`. If other invocations still fill the outgoing queue, `cancel`
returns back pressure **after** local invalidation: the supervisor must stop the
worker because the cancellation message could not be sent. Late effects/output
remain refused, including in this failure case. The adapter must kill an unresponsive worker after its deadline.
Cancelled writes can already have committed: reconcile receipts rather than
assuming rollback. Broker calls are synchronous and must complete or time out
before cancellation can be processed; an asynchronous process adapter must move
blocking work off its control loop.

The sandbox must deny ambient filesystem/network/process access and credentials,
provide only framed IPC, and enforce CPU/memory/time ceilings. This protocol and
Node permission flags do not establish OS isolation. No arbitrary plugin
installation, server wiring, trusted grant UI, durable scheduler, webhook route,
theme sealing or Collection provisioning side effect is implemented here.
Each kind declares `storage: { "folder": "Notes" }` or `storage: { "hidden": true }`,
plus `claimed_text: true` (userdata-ssd) or `false` (userdata-hdd). Claimed text is
Markdown (`extension` absent or `md`); any other kind must declare `extension`, 1 to 16
lowercase ASCII letters or digits. An operation may declare `target: { kind, field }`;
`field` must be a required string property of its input schema. Visible folders
must be NFC single components, at most 255 UTF-8 bytes, without separators,
control characters, leading dots, surrounding whitespace or reserved `Trash`.
Inherited fsio NFC/lowercase comparison rejects two plugins claiming the same
visible folder, including case-only differences. Hidden placement is
`.calternal/<plugin-id>/<kind>/` within the space userdata tree. appdata holds
host databases/state; these declarations cannot redirect a kind there.

Collection declaration IDs are templates, never public Collection UUIDs: the
host allocates a fresh UUIDv7 on enable. Component IDs are references to the
separately versioned component contract; no component renderer is included.
Schedules and theme paths are declarations only, not activation validation.

Notes Insights now speaks worker v2 and requests `read_item`; it no longer accepts
snapshots over the preview's worker-v1 stdin-to-EOF transport. The unchanged server
must keep that integration disabled until its separate adapter, trusted bundle
pin and manifest loading are updated. Existing v0 core/SDK types are retained
for that server until its lane removes them; no compatibility negotiation is
provided by this new library.

Module costs: registration probes indexes by contributed ID and allocates only new keys;
session operations use bounded maps/queues and compiled schemas. No library
operation takes a lock, performs filesystem I/O, fsyncs, starts a process or opens
a network connection. Each accepted callback performs one broker operation;
its adapter defines transaction/fsync/round-trip costs. SDK send promises carry
transport back pressure and pending callback/handler maps are bounded.
