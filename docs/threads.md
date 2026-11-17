# Saved threads

The `calternal-orchestrator` manifest owns kind `threads` and its nine operations.
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
model: "sonnet"
message-count: 1
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
```

Parent and spawn keys are optional; together they identify the parent conversation
and the message that spawned this one. A message marker carries a stable UUID and an RFC 3339 UTC time. A person
message is stamped on append. An agent message is stamped at its first saved
checkpoint and retains that time when text is appended.
An agent response uses its job UUID as a stable message ID across checkpoints.
The rebuildable metadata stores the current output insertion offset, run and
checkpoint; subsequent chunks extend that same message without scanning text.
Markers organize presentation only: message text is untrusted and never parsed
for access, roles, provenance or instructions. Frontmatter string values use
JSON-quoted YAML scalars so titles cannot inject keys. The host writes exactly
one `calternal-id` key. No migration or alternate reader is supported.

The rebuildable content metadata mirrors the envelope and holds the pending send
identity (account, request ID, fingerprint and creation time). Message text lives
in the body. The pending intent does not grant authority; admission uses the
current caller, current space capabilities and plugin activation.

## Operations and durability

Manifest input/output schemas, target fields, capabilities and risks are the
single source of operation policy. `threads.create` makes an empty file;
`threads.list` returns metadata; `threads.get` returns the Markdown and revision;
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

One warm successful send loads the current thread head three times before its
admission read: the handler, content's revision guard and outbox publication;
placement loads it once, and admission loads it once (five bounded head loads,
including one body split by the thread handler). It performs one current-file
hash check, plus guarded placement's checks, and one jobs thread lookup. The
handler reads the message receipt once, content's write checks it once, and
admission reads its receipt once; a previous pending intent adds one admission
receipt read. Omitting a mode adds one indexed protected-authority read.
Jobs admission reads its request key, active thread and space count;
account settings and the resulting job each have one read. Resolver/policy and
activation reads are additional and use the existing paths.

A content write commits one FULL content transaction, one FULL journal
transaction, one FULL outbox-retirement statement and one FULL placement-state
statement, plus atomic file and directory fsyncs. Send adds one FULL job
admission transaction and one FULL admission journal transaction: six SQLite
commit boundaries, one file placement. The inherited journal witness has its
own durability work. No new global lock, connection or thread is introduced.
This is an operation count, not a timing or performance-gate claim.

Live output stays on the existing job stream. Scrubbed text is appended at session,
tool-finish and turn-end checkpoints, at three seconds of emitted output, or when
16 KiB accumulates, whichever comes first. A pending chunk is bounded to 32 KiB;
job checkpoints retain their existing bounded tail. Each nonempty output flush
loads the thread once, then uses the same content write's reads and four SQLite
commit boundaries plus file placement; the job checkpoint adds one FULL commit.
Checkpoint request IDs prevent duplicate text on replay; concurrent revisions
retry at most four times. Empty checkpoints add no content write.

## Client reads and live updates

`threads.list` returns rows with `id`, `address`, `revision`, `turn_state` and
`metadata`. The file-derived metadata includes `provider`, `model` (null until
known), `mode`, `message_count`, `last_activity_at`, `archived` and an optional
`parent_thread`. Last activity is the creation time or the time of the latest
saved message text. Renames and archive changes update `updated_at`. The indexed
mode is the last recorded thread choice. Send still uses the caller's protected
mode when the input omits one.

A warm list executes **one index query and zero thread file reads** for N threads.
The bounded content index scan joins the space's current turn index with one
indexed seek per candidate. It reads no message bytes or job checkpoint payloads
and has zero flushes. The jobs connection attaches the same space's content
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
request contents.

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
`data.job_state` (the underlying job lifecycle value). The same committed journal
wakes both list and detail subscribers. The feed rechecks current space access and
item ownership on replay and delivery. Turn state uses a space-local outbox
written in the existing job transaction. One drain per cached space job connection reads at
most 64 entries per page, appends each through the existing FULL journal commit,
then retires the page in one jobs commit. The drain releases the jobs lock before
taking the content lock or awaiting the writer. Publication retries on subsequent
job transactions and recovery. Pending entries are eligible for pruning after
seven days. The journal's operator-configured receipt retention bounds deduplication.

## Bounds and remaining integration

The existing content ceiling is 2 MiB per thread file; growth fails visibly and
preserves the saved head. Prompts are at most 16 KiB, titles 256 bytes, provider
IDs 80 bytes, models 128 bytes. List is one bounded page of at most 1,000 content
rows (100 by default); filters run within that page. Owned conversation files and
Trash have no automatic time expiry; receipts, jobs and streams retain their
existing bounds. Long tool output must be stored separately and linked, following
the accepted grill decision; this change does not invent an attachment format.

The list operation currently returns a bounded metadata page; an addressable
standard collection awaits the foundation's accepted Markdown Views format.
Cross-turn provider-session continuation and transcript handoff also remain turn
engine integration work; each send currently admits a fresh job.

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

## CLI

```sh
calternal threads --token-file person.token create --space-id <uuid> --title Plan
calternal threads --token-file person.token list --space-id <uuid>
calternal threads --token-file person.token get <thread-uuid>
calternal threads --token-file person.token --request-id <stable-id> send <thread-uuid> --message 'Inspect this workspace'
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
