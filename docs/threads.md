# Saved threads

The `calternal-orchestrator` manifest owns kind `threads` and its operations.
Its Rust product engine uses the public content and turn engines; it has no separate
file writer, journal, SQLite store, capability table or stream. Addresses are
`/threads/<uuid>` and message references append the message UUID.

## File format

Each conversation is one UTF-8 Markdown file at
`userdata-ssd/spaces/<space-uuid>/AI/Chats/<title>.md`. Content assigns filenames
and the `calternal-id` UUID, preserves immutable history, and refuses outside edits.
The small frontmatter uses these keys:

```markdown
---
calternal-id: 019a8c00-0000-7000-8000-000000000001
title: "Plan"
provider: "calternal-claude-code"
model: "claude-sonnet-5-5"
message-count: 2
mode: "approve_for_me"
parent-thread: "019a8c00-0000-7000-8000-000000000002"
spawned-by: "019a8c00-0000-7000-8000-000000000003"
created-at: "2026-11-17T12:00:00.000Z"
archived: false
deleted: false
updated-at: "2026-11-17T12:00:00.000Z"
last-activity-at: "2026-11-17T12:00:00.000Z"
---

<!-- message:019a8c00-0000-7000-8000-000000000004 role:person time:2026-11-17T12:00:00.000Z -->
Inspect this workspace.
<!-- /message -->

<!-- message:019a8c00-0000-7000-8000-000000000005 role:agent turn:019a8c00-0000-7000-8000-000000000005 provider:calternal-claude-code model:claude-sonnet-5-5 reasoning:high machine:72c16856719dbd545329e7c00d4445bd2ba045066533774b1315572ec50da38e time:2026-11-17T12:00:01.000Z -->
The workspace has one project.
<!-- turn-timing duration-ms:1200 reasoning-ms:400 queued-ms:10 sandbox-ms:100 tool-ms:200 -->
<!-- /message -->
```

Parent and spawn keys are optional; together they identify the parent conversation
and the message that spawned this one. A message marker carries a stable UUID and an RFC 3339 UTC time. A person
message is stamped on append. An agent message is stamped at its first saved
checkpoint and retains that time when text is appended.
The first attempt uses the job UUID as its message ID. A later attempt gets
its own message ID and the same `turn` UUID so provenance stays attached to the
output of its machine. Each opening agent marker holds the manifest provider
and model IDs, the requested reasoning level, and a thread-scoped reference to
its personal machine. The reference is stable for the account and thread and
keeps the real machine UUID private. A dash means provider-default effort or
an anonymous pool placement. The same
marker grammar allows missing attributes in earlier conversation files.
The rebuildable metadata stores the current output insertion offset, run and
checkpoint. Later chunks extend that attempt's message without scanning text.
Markers and message text are for display. Authority never comes from the Markdown. Frontmatter string values use
JSON-quoted YAML scalars so titles cannot inject keys. The host writes exactly
one `calternal-id` key. No migration or alternate reader is supported.

The rebuildable content metadata mirrors the envelope and holds the pending send
identity (account, request ID, fingerprint and creation time). Message text lives
in the body. The pending intent does not grant authority; admission uses the
current caller, current space capabilities and plugin activation.

## Operations and durability

Manifest input/output schemas, target fields, capabilities and risks are the
single source of operation policy. `threads.create` makes an empty file;
`threads.list` returns metadata; `threads.models` lists the enabled provider catalog;
`threads.get` returns the Markdown and revision;
`threads.send` commits a person's message before admitting the turn;
`threads.stop` cancels current work. `threads.rename` updates the title and filename
while preserving the UUID and messages. `threads.archive` hides it from the
ordinary sidebar and `threads.unarchive` returns it to that list. Both preserve
its address.
`threads.delete` moves a stopped file to its space's `.trash/<uuid>.md` and deletes
all accounts' saved thread modes and credentials through the auth recovery barrier.
Deleted IDs remain resolvable as deleted. Listing children by `parent_thread`
includes archived children and deleted child tombstones, preserving provenance. A thread credential cannot address another thread,
even with Full access, and no operation trusts a caller's space hint over its ID.

Send has two recoverable commits, rather than pretending content.db and jobs.db
share a SQLite transaction. The first records the message plus a pending intent
and request receipt through content's existing outbox. The second holds that
space's content lock while it checks the current head, admits the existing turn
job with a stable key and publishes a separate admission receipt. It never
re-enters content from a callback. Trash checks job state under the same content lock. Once its tombstone is
durable, protected authority cleanup runs under an item-aware admission check;
a failure is replayable before deletion is acknowledged. Removing authority before
the content commit could otherwise forget the saved mode after a failed delete.
A fresh delete of an existing tombstone can finish cleanup after receipt expiry. Lock order is content → jobs → auth; auth callbacks
never acquire content or jobs locks. This serializes only one space.

A crash between message placement and admission leaves one visible message.
Retry the identical `threads.send` with its original request ID: the message
receipt replays, admission finishes once, and the same job is scheduled. Until
that retry, another send is refused with the next step. A crash after job admission
uses its stable key; after admission receipt publication it uses that receipt.
Job retention never permits a retained admission receipt to start work twice.
After the operator's receipt-retention window, a request is new, as for every
other write. The default window is seven days. An expired pending intent no
longer blocks a new send; its saved message is preserved.

One warm successful send loads the current body in the handler and again
for admission. The write uses one indexed header and one current-file hash
check. If an earlier write is still pending, it first finishes that placement
and reloads the header. The handler, the content write and admission each
read their receipt once. A previous pending intent adds one admission receipt
read. Omitting a mode adds one indexed protected-authority read. Jobs admission
reads its request key, active thread and space count. Account settings and the
resulting job each have one read. Resolver, policy and activation reads use the
existing paths.

Content acknowledges after three FULL SQLite boundaries covering content,
journal and outbox retirement. The lifecycle worker or next write finishes
userdata placement with file and directory fsyncs and a NORMAL bookkeeping
update. Send adds one FULL job admission transaction and one FULL admission
journal transaction. The journal witness retains its durability work.

Live output stays on the existing job stream. Scrubbed text and tool statuses
are appended at session, tool-start, tool-finish and turn-end checkpoints, at
three seconds of emitted output, or when 16 KiB accumulates, whichever comes
first. A pending chunk is bounded to 32 KiB;
job checkpoints retain their existing bounded tail. Each nonempty output flush
loads the thread body once, then uses the content write's header read, receipt
lookup and userdata hash check. It adds the same three FULL content boundaries
and deferred placement, plus one FULL job checkpoint. A pending prior revision
may need placement first.
Checkpoint request IDs prevent duplicate text on replay; concurrent revisions
retry at most four times. A provider's session event can add one content write
before any text arrives. Other empty checkpoints add no content write.

Each thread retains one `provider-session` record in its Markdown envelope.
It contains a native session ID bound to the account that started it, the
provider, and the enrolled machine. It follows the thread's lifetime and
survives job pruning. It carries no credential. Admission snapshots the
current account's record from the already loaded thread. The next send uses
it only when placement selects that same provider and machine. A change in
either starts a fresh session and emits a `notice` on the job stream. The
current job checkpoint also carries the text in `session_notice` so it can
be shown after reconnect. A different account receives a fresh session
without a notice about the prior account's binding.

## Client reads and live updates

`threads.list` returns rows with `id`, `address`, `revision`, `turn_state`, `turn_reason` and
`metadata`. The file-derived metadata includes `provider`, `model` (null until
known), `mode`, `message_count`, `last_activity_at`, `archived` and an optional
`parent_thread`. Last activity is the creation time or the time of the latest
saved message text. Renames and archive changes update `updated_at`. The indexed
mode is the last recorded thread choice. Send still uses the caller's protected
mode when the input omits one.

A warm page executes **one indexed query and zero thread file reads**. It
seeks the list index by update time and item UUID, filters before the limit,
and looks up the latest job by index for each returned row and one lookahead.
It reads no message bytes and performs zero flushes. Pass `next_cursor` back
as `cursor` with the same space and filters. The cursor is scoped to the
current account, and `null` marks the last page. The jobs connection attaches the same space's content
database read-only, once per cached connection. A cold read also opens the
existing indexes. Static metadata is mirrored in Markdown and can be indexed
again from its envelope. Live state comes from the retained jobs index. Full
file-only reconstruction is a separate foundation feature.

`threads.get` returns `thread.turn_state` and the ordinary durable `job` view
when a turn is nonfinal. Follow that job ID on the existing job event stream
with the last job bookmark. When the stream resets, load its retained
checkpoint. The turn labels are `idle`, `queued`, `running`, `waiting`,
`interrupted` and `failed`. Admission and retry wait are queued, job waits are
waiting, and successful or canceled work returns to idle. Final failure remains
visible for the existing job retention period. The requests lane parks turns waiting on a person in this same durable
`waiting` state. Reading a thread exposes its job state without exposing private
request contents. The turn reason is also returned at `thread.turn_reason`.

Subscribe to the owning space on `/api/v1/live` and save its journal bookmark.
Refresh the list or open thread for an entry whose `item` is the thread UUID and
whose `kind` is one of these values.

| Change | Journal kind |
| --- | --- |
| Created | `threads.create` |
| Renamed | `threads.rename` |
| Archived or unarchived | `threads.archive`, `threads.unarchive` |
| Person message or agent text checkpoint saved | `threads.send`, `threads.checkpoint` |
| Turn state changed | `threads.turn_state_changed` |
| Deleted | `threads.delete` |

The turn-state entry carries `data.job_id`, `data.state` (the sidebar label) and
`data.job_state` (the underlying job lifecycle value) and `data.turn_reason`. The same committed journal
wakes both list and detail subscribers. The feed rechecks current space access and
item ownership on replay and delivery. Turn state uses a space-local outbox
written in the existing job transaction. One drain per cached space job connection reads at
most 64 entries per page, appends each through the existing FULL journal commit,
then retires the page in one jobs commit. The drain releases the jobs lock before
taking the content lock or awaiting the writer. Publication retries on subsequent
job transactions and recovery. Pending entries are eligible for pruning after
seven days. The journal's operator-configured receipt retention bounds deduplication.

### Read one checkpoint addition

Content journal entries identify the acting account in `author` and carry
its attribution in `data.actor`. That record has `account` and `via`.
`via` is null for a person and `{"Agent":{"thread":"THREAD_UUID"}}` for an
agent, including saved turn checkpoints. A plugin invocation carries
`{"Plugin":{"plugin":"PLUGIN_ID"}}`. The server derives this from admitted
authority. A retry or replay of an unpublished journal entry keeps the first
writer's attribution. Live entries from an erased author carry
`author_label: "a deleted account"` alongside the historical ID. Human
journal output uses the label.

A `threads.checkpoint` entry carries the target `data.revision` and
`data.append` with `base_revision`, `offset`, `start` and `length`.
Pass the entry's `data` as `since` to `threads.get` with the thread ID.
For example, the operation input is

```json
{"id":"019a8c00-0000-7000-8000-000000000001","since":{"revision":"v4.example","append":{"base_revision":"v3.example","offset":240,"start":588,"length":9}}}
```

The response contains `delta` with `id`, `base_revision`, `revision`,
`offset` and `markdown`. Insert the returned UTF-8 bytes into the cached
Markdown body at `offset` and advance its revision. Exclude frontmatter when
applying the insertion. The first checkpoint includes a new message's markers.
Later checkpoints can insert before its closing marker, which rules out a
message index or a whole-file suffix as the cursor. `start` and `length` are
the byte range in the target revision and are echoed from the journal entry.
Clients can copy `data` into `since` as before. The read uses the revision
and append range, and ignores the accompanying actor.
A session-only checkpoint has a zero-byte addition. A full get still returns
the document, metadata and active job.

Apply journal entries in order. Each pair describes one committed transition and
can be read after newer checkpoints arrive. If the client lacks the matching
base, receives a non-checkpoint document change, or gets a conflict, reload
with a full `threads.get`. Do the same when the journal resets. The same
space, item, capability and agent-thread checks apply to both read forms.

After ordinary authorization, the delta path performs one indexed lookup and
at most one bounded SQLite BLOB read. It performs zero userdata file reads,
job lookups and flushes. The checkpoint derives the range from its existing
body and write buffer. Publication reuses the outbox payload and existing
journal commit, with no additional reads or flushes per event.

### Models, phases and turn history

Call `threads.models` with `{"space_id":"SPACE_UUID"}` and the space's
`plugins.use` capability. Each response has a `providers` array.

```json
{"providers":[{"id":"calternal-claude-code","name":"Claude Code","default_model":"claude-sonnet-5-5","models":[{"id":"claude-sonnet-5-5","name":"Sonnet 5.5","reasoning_levels":["low","medium","high","xhigh","max"]}]}]}
```

Each provider supplies its own stable model IDs, display names and ordered effort
levels. The server validates and caches the trusted manifests until restart. A
read applies current space activation in one indexed query. It does not contact
a machine or use a person's login. Use `agent.readiness` for capacity and sign-in.
Provider versions, model releases, account entitlements and organization effort
caps can change separately from the installed manifest.

Send a model ID from that provider and optionally `reasoning` from that model.
Omit `model` for the provider's catalog default. Omit `reasoning` to retain
the provider's settings. Unknown choices are rejected before committing the
message. An explicit level records the requested choice. Claude Code does
not report whether organization settings capped the effective effort.

Job stream chunks with `data.type == "turn_phase"` have a `phase` and Unix
`at_ms`. Phases are `queued`, `starting_sandbox`, `running`, `thinking`,
`running_tool`, and `waiting_on_person`. The last means the host parked the
job for a trusted request. Its ID and content use the existing private request
path. The current phase is also included as `job.checkpoint.turn_phase` when
saved. Provider frames retain `{version, seq, event}`. A reasoning block emits
`reasoning_started`, zero or more `reasoning_delta` frames with `text`, and
`reasoning_finished` with monotonic `duration_ms`. Stream time measures
observed work and can include transport or back-pressure.

The terminal checkpoint adds one short `turn-timing` comment before the agent
message's closing marker, at most one per attempt. The worked duration excludes
queue time, person waits and retry delays. The marker also records observed
reasoning, queue, sandbox and tool durations in milliseconds. Abrupt stops may
leave a checkpoint without final timing. Claude exposes thinking as provider
text on the live stream. The file retains timing only. Both reads expose the
same opening and timing markers in Markdown. Apply checkpoint deltas in order
before parsing message markers. Ordinary assistant Markdown is untrusted display
content.

Send checks the loaded manifest, placement adds a fixed-size hash on its existing
blocking worker, and live phases reuse the job buffer. These add zero indexed reads and zero
flushes. Timing piggybacks on the terminal checkpoint with no added job flush.
If the terminal checkpoint otherwise had no Markdown to save, the timing
marker makes it one content checkpoint. That adds the usual capability check,
one activation read, one receipt lookup, and one content head read, followed by
the normal indexed write checks and three FULL commits
(content, journal and outbox retirement), plus the normal file and directory
syncs. No per-frame filesystem work is added.

### Tool steps and the saved output limit

Tool starts, output notices and completions carry the tool name and short
target or description. Claude Code uses the executable name for a Bash
command and a bounded path for a file operation. The adapter publishes tool
output as a byte count. The saved message records the step and status in
Markdown blockquotes. The adapter and host scrub the issued Calternal
credential before the step reaches a client or the file.

The existing content limit accepts at most 2 MiB of submitted Markdown,
before placement adds the host-owned identity line. Previously an oversized
checkpoint was rejected atomically and the turn ended with a generic
outcome-unknown state. A saved turn now fails with `thread_full` on the job
stream and `turn_reason.why` on reload. Earlier checkpoints remain readable.
The failure message explains that the latest checkpoint was not saved and that
the person can start a new thread. The turn does not automatically retry.

The accepted design keeps long tool output in separate, linked storage.
The thread format has no artifact address, link grammar, or lifetime yet.
Until those are defined, the preview keeps only the tool status and byte count.

## Bounds and remaining integration

Prompts are at most 16 KiB, titles 256 bytes, provider
IDs 80 bytes, models 128 bytes. Each list page returns at most 1,000 rows
(100 by default). Owned conversation files and
Trash have no automatic time expiry; receipts, jobs and streams retain their
existing bounds. Long tool output must be stored separately and linked, following
the accepted grill decision; this change does not invent an attachment format.

The list operation returns paged metadata. An addressable
standard collection awaits the foundation's accepted Markdown Views format.
Cross-turn transcript handoff remains turn engine integration work. Each
send admits a fresh job and can resume the native provider session.

The turn engine currently refuses a send while a turn is active or interrupted.
Live steering needs an incremental machine-input transport; the existing exec
input is delivered once. The provider protocol has `steer`, but the server does
not yet deliver later commands. This is an implementation gap against the
accepted steering behavior, not a new product decision. Interrupted work uses
`calternal agent resume <job-id>`.

The content index avoids body reads for lists. Tail/block indexing, full file-only
reconstruction, long-output attachments, and splitting the 2 MiB limit from the
transport document ceiling remain named work; no long-thread speed claim is made.

A mode requested with a send applies directly in either direction and is saved;
there is no additional passkey check for that choice. When omitted, the mode
comes from that account's protected saved mode, not another member's last choice.
Before its first turn, the creator's own `mode-for` initial setting applies;
other accounts start at Approve for me. Each new turn replaces the old credential. Thread credentials cannot select a mode. This follows the owner's
2026-11-18 decision; explicit in-flight mode changes keep their existing checks.

Use `turn_reason.code` to explain turn state. A running
turn has a null reason. Before placement the public label is queued. The
following reason payloads use Unix milliseconds for timestamps.

| Code | Fields and meaning |
| --- | --- |
| `idle`, `queued`, `completed`, `cancelled` | No turn, pending admission, completed work, or cancelled work |
| `no_machine`, `waiting_for_capacity` | Placement wait with `queue_position` |
| `machine_offline` | Placement wait with `queue_position` and `since_ms` (last contact when known) |
| `provider_signed_out` | `provider` and the reported `sign_in_command` |
| `waiting_on_person` | Private `request_id` visible to the turn account |
| `retrying` | `attempt`, `next_attempt_ms`, and `rule` (`provider_backoff`, `api_availability`, or `subscription_reset`) |
| `failed` | `class` (`transient`, `usage_limit`, or `permanent`) and `reset_at_ms` when the provider reports it. A storage-limit failure includes `why: "thread_full"` and its `message`. |
| `interrupted` | `resumable`, `why`, and the last machine contact `since_ms` when known. Uncertain outcomes are nonresumable. |

The account that submitted a turn receives its private machine and sign-in
reasons. Other space members receive generic queue, failure or interruption
reasons for those states. The journal applies the same projection on live
delivery and replay. See [machine and provider readiness](agent-turns.md#machine-and-provider-readiness)
for account-scoped availability and provider checks.

## CLI

```sh
calternal threads --token-file person.token create --space-id <uuid> --title Plan
calternal threads --token-file person.token list --space-id <uuid>
calternal threads --token-file person.token models --space-id <uuid>
calternal threads --token-file person.token list --space-id <uuid> --cursor <cursor>
calternal threads --token-file person.token get <thread-uuid>
calternal threads --token-file person.token get <thread-uuid> --since '<checkpoint journal data JSON>'
calternal threads --token-file person.token --request-id <stable-id> send <thread-uuid> --message 'Inspect this workspace' --model claude-sonnet-5-5 --reasoning high
calternal threads --token-file person.token stop <thread-uuid>
calternal threads --token-file person.token rename <thread-uuid> --title "New title"
calternal threads --token-file person.token archive <thread-uuid>
calternal threads --token-file person.token unarchive <thread-uuid>
calternal threads --token-file person.token delete <thread-uuid>
```

Every command accepts `--json`, uses the shared width-aware human renderer and
prints terminal-only hints. JSON is one document. Use the
ordinary jobs stream to follow the returned job. Offline submissions use the
existing account-owned `pending.queue` and `pending.submit` operations; reconnect
rechecks access before the ordinary thread operation runs.

A stop receipt includes the exact canceled job. HTTP acknowledgment releases that
job's machine lease using the existing turn cleanup; replay never selects a later
job. This adds one job refresh inside cancellation and one indexed job load for
lease cleanup, plus the existing machine release commit.

Native lease admission and its first checkpoint share the existing per-space job
lock: one indexed current-run check precedes machine admission. Cancellation
therefore either sees the saved lease ID or prevents admission. The callback
never re-enters jobs/content; publication failure releases the lease. This also
closes the stop race before the first lease checkpoint exists.

The orchestrator is default-enabled for a fresh space. With the default server
plugin path, the native manifest is available from the binary if the working
directory lacks `plugins/calternal-orchestrator/manifest.json`. Deployments
using `--plugin-root` provide their own trusted plugin tree. An authorized
fresh space lists zero threads before any conversation is created.
