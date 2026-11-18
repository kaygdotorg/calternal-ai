# Saved threads

The `calternal-orchestrator` manifest owns kind `threads` and its operations.
Its Rust product engine uses the public content and turn engines; it has no separate
file writer, journal, SQLite store, capability table or stream. Addresses are
`/threads/<uuid>` and message references append the message UUID.

## File format

Each conversation is one UTF-8 Markdown file at
`userdata-ssd/spaces/<space-uuid>/AI/Chats/<title>.md`. Content assigns filenames
and the `calternal-id` UUID, preserves immutable history, and refuses outside edits.
The file uses the format, historical headings and byte-count markers defined in
[Readable file format](#readable-file-format). No reader for the previous
development format is included.

The content index caches the envelope, derived list fields and the pending send
identity (account, request ID, fingerprint and creation time). Message text lives
in the body. The pending intent does not grant authority; admission uses the
current caller, current space capabilities and plugin activation.

## Operations and durability

Manifest input/output schemas, target fields, capabilities and risks are the
single source of operation policy. `threads.create` makes an empty file;
`threads.list` returns metadata; `threads.models` lists the enabled provider catalog;
`threads.get` returns the Markdown and revision. With a `message_id`, it checks
the same thread authority and returns one saved record from the current
revision, using the locator maintained by [search](search.md).
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

Send has two recoverable phases. The first records the message and a pending
intent through the content outbox, then publishes the message receipt. The
second holds the same space lock while it checks the current head, admits the
job with a stable key, and stores the admission receipt in that same jobs
transaction. The admission callback never re-enters content. Trash checks job state under the same content lock. Once its tombstone is
durable, protected authority cleanup runs under an item-aware admission check;
a failure is replayable before deletion is acknowledged. Removing authority before
the content commit could otherwise forget the saved mode after a failed delete.
A fresh delete of an existing tombstone can finish cleanup after receipt expiry. Lock order is content → jobs → auth; auth callbacks
never acquire content or jobs locks. This serializes only one space.

A crash between message placement and admission leaves one visible message.
Retry the identical `threads.send` with its original request ID: the message
receipt replays, admission finishes once, and the same job is scheduled. Until
that retry, another send is refused with the next step. A crash after job admission
replays its receipt. Job retention never permits a retained admission receipt to
start work twice.
After the operator's receipt-retention window, a request is new, as for every
other write. The default window is seven days. An expired pending intent no
longer blocks a new send; its saved message is preserved.

A warm successful send reads metadata and the current revision. Small
transcripts add a bounded read of at most 8 KiB for the response. The turn
admission reads only the current summary under the content slot. It uses
the existing receipt and request identity to fence deletion and starts one
durable job. Receipt replay retains the same response. An omitted access
mode adds the existing protected-authority read.

Live output stays on the job stream. The server seals text at session and
tool boundaries, at turn end, after three seconds of emitted output, or when
16 KiB has accumulated. The uncommitted text is bounded to 32 KiB. Each append
writes one complete newline-terminated Markdown record with a single file
write. A fragment can continue an assistant message without modifying any
earlier bytes. The Markdown envelope records the last non-append rewrite,
while the saved headings record append times. Full thread reads join those
fragments using the logical
message ID. Metadata-only checkpoints use a hidden, complete control record.

The content transaction records the new revision as a patch with its start,
length, preceding-range witness, record hash and resulting file hash. Existing
SQLite snapshots remain readable. Older paths can still return a complete
snapshot. A threaded send with more than 8 KiB already saved returns the new
message and current metadata with `partial: true`. It does not replay the
entire transcript in the send receipt.

A warm append reads up to 8 KiB of staged prefix when it needs document
framing, one bounded prepared-record row in SQLite, and one preceding
range of at most 4 KiB from the file. It appends at most 64 KiB in one write
and syncs the file once. The SQLite write stores the new patch bytes once
with the existing FULL content commit, FULL journal publication and
outbox retirement. The incremental hasher holds a fixed-size state and a
4 KiB tail. Ordinary checkpoint appends do not allocate a transcript-sized
buffer or rehash the file. Message indexing consumes only new fragments
when the existing projection is caught up. An initial or stale projection
rebuild reads the current full revision.

On Linux, the writer checks the named inode's device, inode number, length and
change/modification times. It compares the hash of the last committed range
before writing and holds a file write lease while placing the append. If an
outside descriptor prevents the lease, the append retries. If a new inode
replaces the path, placement refuses it. A cold or changed proof starts full
verification in the background. The space slot is released during that scan.
Append waits for a proof for the same named file. A same-length edit that
changes bytes before the tail is detected by the inode stamp and the full
verification on mismatch.

If a process or machine dies during the write, a direct Markdown reader can
see a torn final suffix until the space is next opened. Before the server
reads or appends, it verifies the committed prefix and uses the outbox to
finish a matching short suffix. A completed suffix is recognized and kept
exactly once. Earlier Markdown is never changed by append or recovery.
An unrelated edit is preserved and blocks placement. The server never serves
a torn suffix.

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
| Manual snooze changed | `threads.snooze` with `data.snoozed_until_ms` |
| Archived or unarchived | `threads.archive`, `threads.unarchive` |
| Person message or agent text checkpoint saved | `threads.send`, `threads.checkpoint` |
| A provider sign-in was attempted or completed | `threads.provider_sign_in` with `data.notice` |
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

### Read the end of a saved thread

Pass `tail: true` to `threads.get`, or use `calternal threads get <id> --tail`.
A large response contains at most 64 recent framed records and 128 KiB of
Markdown from the current indexed revision. If the latest legacy record is
larger by itself, the API returns its last 128 KiB, aligned to UTF-8, with
`partial_record: true`. A cold or lagging index returns a retryable response.
The exact-message read continues to open just the indexed record at the
requested ID.

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
`offset`, `markdown` and `messages`. Insert the returned UTF-8 bytes into the cached
Markdown body at `offset` and advance its revision. Exclude frontmatter when
applying the insertion. The insertion contains complete new records, with
the same fields as `thread.messages` in a full get. A session-only checkpoint
has a zero-byte addition and no messages. A checkpoint extending or finishing
an existing message changes its heading or byte count and has no insertion
delta. Refresh the full document for those revisions.

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

At launch, each attempt captures the provider ID, harness and model labels
from the live registry snapshot that selects its adapter. A later registry
update or removal cannot relabel saved output, including the final checkpoint.
Each changed save takes one live snapshot for the orchestrator's activation
check. A retry or resume selects its provider again for the new attempt.
Existing authority checks still govern whether an attempt can continue.


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

The terminal checkpoint updates the agent heading with worked time and, when
observed, thought time. The marker records both durations in milliseconds.
A process stop before the terminal checkpoint can leave final timing absent.
Reasoning text is available on the live stream after filtering and is omitted
from saved messages. Both read forms expose the same structured agent
provenance: provider plugin ID, model ID, requested reasoning level,
thread-scoped machine reference, worked duration and reasoning duration.

Send adds one indexed author display-name lookup. The catalog validates the
selection in memory. Each changed output checkpoint loads the existing body
once and reuses the indexed tail cursor, with no per-frame reads or flushes.
Timing uses the normal terminal checkpoint. When timing is the only change,
it adds one ordinary content write with its existing file and directory syncs,
three FULL commits and its standard capability, activation, receipt and head
checks. It adds no separate job flush.

### Tool steps and the saved output limit

Tool starts, output notices and completions carry the tool name and short
target or description. Claude Code uses the executable name for a Bash
command and a bounded path for a file operation. The adapter publishes tool
output as a byte count. The saved message records the step and status in
Markdown blockquotes. The adapter and host scrub the issued Calternal
credential before the step reaches a client or the file.

A saved thread holds at most 16 MiB of Markdown, including the host-owned
identity line. An overflowing checkpoint is rejected atomically. A saved turn
reports `thread_full` on the job stream and in `turn_reason.code` on reload.
Earlier checkpoints remain readable.
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

The content index avoids body reads for lists. Full file-only reconstruction,
long-output attachments, and independent limits for saved files and transport
documents remain open work.

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
| `provider_signed_out` | `provider`, the re-authentication `message` and the reported `sign_in_command` |
| `waiting_on_person` | Private `request_id` visible to the turn account |
| `retrying` | `attempt`, `next_attempt_ms`, and `rule` (`provider_backoff`, `api_availability`, or `subscription_reset`) |
| `failed` | `class` (`transient`, `usage_limit`, or `permanent`) and `reset_at_ms` when the provider reports it. |
| `thread_full` | Permanent storage-limit failure with a `message` explaining how to continue. |
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
calternal threads --token-file person.token get <thread-uuid> --tail
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

## Readable file format

The owner approved the heading layout and one file per thread. A conversation
lives at `AI/Chats/<title>.md`. The reader uses one format.

### Frontmatter and headings

The existing app settles these conventions:

> Frontmatter also holds `date:` (created) and `last edited:`, written as ISO 8601 with a timezone offset.

That is [Notes ADR 0025]. The same paragraph says the first H1 is the title
and the server synchronizes the `title:` property with it. [Notes ADR 0023]
says “Notes live as `Notes/<Title>.md`, with a `calternal-id` in frontmatter.”
[Money ADR 0057] says “Participating notes carry `format: calternal-money`.”
[Contacts ADR 0063] says “Frontmatter contains only `format: calternal-contact`,
`calternal-id`, and optional `tags` for groups or Google labels.”
The contact companion in ADRs 0063 and 0064 holds original vCard data. Threads
keep one Markdown file by the owner's later decision.

[Tasks ADR 0066] and [Calendar ADR 0067] delegate field formats to [ADR 0068],
which requires “Boolean means true/false only” and gives tags as “String lists;
tags omit `#`, retain `/` nesting; default `[]`.” Its task-specific identity
promotion does not rename the host-owned `calternal-id` used for this
transcript. The Money specification uses `archived: true` or `false` and
defaults to false. The old app's ordinary note fixture orders the host ID,
title, creation, edit and optional tags. Insert the format tag after the
host ID and the archive flag after the dates. Use bare UUID and offset-aware
timestamp values, JSON-quoted strings where YAML needs quoting, and a plain
Boolean. The normal writer omits an unset tags list and a false archive flag.
The H1 and the frontmatter title agree on every managed write.

Each message starts with an H2. Person and automation headings name their
historical author and time. Agent headings name the harness, serving provider,
model, reasoning, time and attempt duration. The service is the one actually
reported by the harness. A gateway may serve a model from another company.
Show `Unknown provider`, `Unknown model` or `Unknown effort` when those facts
are missing. Use `Provider default` only when no explicit level was requested.
When an explicit level's effective value is unreported, add `requested` to the
requested level.

```text
person     = "## " label sep time
automation = "## Automation " label sep time
agent      = "## " label " via " label [billing]
             sep label sep effort sep time sep elapsed [sep outcome] [sep continuation] [sep thought]
sep        = " · "
billing    = " (subscription)" | " (API)"
time       = YYYY-MM-DD SP HH:MM:SS SP sign HH ":" MM
elapsed    = integer " s" | integer " min " integer " s"
             | "In progress" | "Duration unavailable"
outcome    = "Failed" | "Interrupted" | "Cancelled"
continuation = "Resumed" | "Retry " positive-integer
thought    = "Thought for " decimal " s"
```

Each author uses their account's time zone, falling back to the submitting
client's zone. Agent attempts keep the initiating person's zone. A signed-in person sets their preferred IANA zone with
`account.set_time_zone`. For create, send and rename, clients supply
`client_time_zone` and may supply `client_offset_minutes`. The server first
tries the saved zone, then the device's named zone, then its numeric offset
and UTC. Turn attempts snapshot the chosen zone at send and resolve the
civil offset at each write, including across DST. Earlier headings keep their
stored offsets. The setting never appears in shared thread metadata. A thread
can show several offsets at once. Previously written offsets are historical.
Attempt durations use elapsed work excluding queue, person waits and backoff.
Message order follows committed records.

### Identity and exact body bytes

Put one HTML comment directly below the H2. A person or automation marker has
`id:<uuid> role:<person|automation> bytes:<count>`. An agent marker adds
`turn:<uuid>` and compact `meta:<json>` after the byte count. This comment saves
provider and model IDs, requested effort, a thread-scoped machine reference,
and worked and reasoning durations. The marker is outside the body. After the
marker and its blank line, read exactly the stated number of UTF-8 bytes,
then consume the writer's record separator before reading the next H2. A
matching heading or whole marker inside those bytes is ordinary authored
Markdown. The branch uses full refreshes for checkpoints that replace a
heading or update an existing body's byte count. A new record that only
extends the transcript can still use its existing insertion delta.

Display names are snapshots. Escape generated heading punctuation and the
literal separators inside labels. Body bytes, including punctuation, code
fences, blank lines and line endings, remain exact. An optional managed
comment after the H1 carries the private provider-session binding. Public
reads exclude that binding. Message count comes from records. Last activity
comes from the corresponding content revision and headings. Placement and
the existing revision journal guard the one file. An attempt's turn UUID
associates retries and resumes. Child threads have their own files and the
parent may link to authorized child output.

Tool statuses and explicit provider reasoning summaries are Markdown
blockquotes. A sign-in result is a single readable line in the document prefix
before the message records. A thread retains the latest 16 sign-in flows.
The same line changes from attempted to done on success, and a
`threads.provider_sign_in` journal event signals a full refresh. For example,
`> Sign-in was done for Anthropic at 2026-11-18 19:30:04 +05:30.` An
unsuccessful attempt uses `attempted` instead of `done`. The re-authentication
prompt reads `<Provider> needs you to sign in again`, for example
`Claude needs you to sign in again`. No link or credential from that flow
goes into the file.

### Failure cases

| Case | Handling |
| --- | --- |
| Hand edit removes or duplicates an ID | The existing external-edit guard preserves the file and blocks writes. A parser presented with an invalid managed snapshot refuses the ID or framing. It never mints a replacement ID. |
| A body includes an identical generated H2 or marker | The byte count puts the line inside the body. It has no structural meaning there. |
| Two devices append concurrently | The existing per-space revision guard admits one write. A stale caller retries against the new head with the existing receipt rules. |
| The file is renamed or moved by hand | The missing recorded path blocks writes. A move is not inferred from a title or body. The blocked state awaits explicit reconciliation. Restoring the path alone does not clear that state. |
| A title contains filesystem punctuation or collides | Keep the title as written. The inherited safe filename and NFC/case-fold collision allocator choose the path, including a numeric suffix when needed. |
| A turn reaches the file ceiling | Decline the overflowing checkpoint and retain the previous revision. The existing `thread_full` reason explains the next step. |
| The clock changes or a DST boundary is crossed | Use the saved offset with each civil timestamp and keep journal order. Monotonic duration remains independent of the clock change. |
| A person changes their name or is deleted | Name changes affect subsequent headings. Historical labels remain display text. The existing account-erasure policy governs protected account data and journal attribution. |
| A provider omits model or effort | Show the corresponding unknown label. Keep a known requested selection distinct from a reported effective value. |
| A message has only attachments | An empty text range is valid and keeps its message ID. Attachment links use the normal Markdown body once attachment addresses exist. |
| A thread is forked or a message is edited later | A fork has a new `calternal-id` and can retain source references. The byte-count framing supports a separately authorized format-preserving edit. Edit history behavior remains an open decision. |

### Snoozing the Inbox

Call `threads.snooze` with `space_id`, `id`, and a future Unix timestamp
`until_ms`. Use `null` to clear a manual snooze. The operation requires
`content.write` and returns the thread UUID and committed manual deadline as
`snoozed_until_ms`. A retry with the same request ID returns that decision.

The manual deadline is shared by all members. Content schema 4 enables it in
the durable revision metadata. The Markdown transcript uses its existing format.
The top-level `snoozed_until_ms` on list and detail responses is the latest
future deadline from the manual setting and any `subscription_reset` retry.
The retry entry in the feed carries its existing `turn_reason.next_attempt_ms`.
Clients re-evaluate the Inbox when the deadline passes. The server computes
expiry on reads, so no timer write, extra journal entry or cleanup is needed
after restart. Clearing a manual snooze leaves the subscription retry intact.

```sh
calternal threads snooze <thread-id> --until-ms <unix-ms>
calternal threads snooze <thread-id> --clear
```

### Automatic thread title

Omit `title` when creating a thread to start it as “New thread” and allow
automatic naming. A supplied title and any manual rename establish a manual
title, including when the title is “New thread”. Existing threads keep their
titles. After a completed turn leaves at least five messages, the server can
submit the first five texts to Claude
Haiku 5.5 (`claude-haiku-5-5`) at low effort. Each text contributes at most
3,000 UTF-8 bytes. The sandbox runs on the last turn's personal machine, uses
the existing Claude subscription, and receives no Calternal agent credential.
Codex automatic titles are disabled.

The title runs in the background with up to eight admitted tasks per server
and one active title sandbox per machine. A full queue drops the submission.
A foreground turn takes priority over a title lease. Each thread can start
one attempt and one retry. Each provider attempt costs one short turn. No
foreground request awaits title output. Saving a title uses `threads.rename`
and keeps the UUID and its stable address. The thread metadata records the
automatic source. The content revision guard preserves a manual rename made
as the title arrives.

Claude Code uses safe mode, no built-in tools, no slash commands and no session
persistence. The adapter checks for local managed policy before starting
Claude and requires a verified personal Pro or Max subscription. Organization
subscriptions can receive server-managed hooks and are skipped. Read
`providers[provider_id].title_blocked` on the caller's machine in
`agent.readiness` for `managed_policy` or `subscription_unverified` when
observed. The thread shows no error. Menu regeneration remains pending.

[Claude Code model configuration] [Claude Code managed settings]

A plan's compute entitlement is a sandbox size in CPUs and RAM plus hours
per month.

### Five example threads

#### 1. Claude Code with its own subscription

```markdown
---
calternal-id: 019a8c00-0000-7000-8000-000000000011
format: calternal-thread/v1
title: "Release checklist"
date: 2026-11-18T19:30:00+05:30
last edited: 2026-11-18T19:30:18+05:30
---

# Release checklist

> Sign-in was done for Anthropic at 2026-11-18 19:30:04 +05:30.

## Maya Chen · 2026-11-18 19:30:00 +05:30
<!-- id:019a8c00-0000-7000-8000-000000000100 role:person bytes:36 -->

Which release checks are still open?

## Claude Code via Anthropic (subscription) · Claude Sonnet 5.5 · High · 2026-11-18 19:30:06 +05:30 · 12 s · Thought for 4.0 s
<!-- id:019a8c00-0000-7000-8000-000000000101 role:agent bytes:161 turn:019a8c00-0000-7000-8000-000000000110 meta:{"provider_id":"calternal-claude-code","model_id":"claude-sonnet-5-5","reasoning_level":"high","machine_id":null,"duration_ms":12000,"reasoning_ms":4000} -->

The accessibility review is the only open check.

> Tool — Read release.md · completed

> Reasoning summary — Compared the checklist with the review status.
```

#### 2. Shared thread, gateway and model switch

```markdown
---
calternal-id: 019a8c00-0000-7000-8000-000000000012
format: calternal-thread/v1
title: "Launch note"
date: 2026-11-18T19:40:00+05:30
last edited: 2026-11-18T19:42:09+05:30
---

# Launch note

## Mira Patel · 2026-11-18 19:40:00 +05:30
<!-- id:019a8c00-0000-7000-8000-000000000120 role:person bytes:37 -->

Draft a sentence for the launch note.

## fx via Vercel AI Gateway (API) · Claude Sonnet 5.5 · Medium · 2026-11-18 19:40:01 +05:30 · 8 s
<!-- id:019a8c00-0000-7000-8000-000000000121 role:agent bytes:58 turn:019a8c00-0000-7000-8000-000000000131 meta:{"provider_id":"calternal-fx","model_id":"claude-sonnet-5-5","reasoning_level":"medium","machine_id":null,"duration_ms":8000,"reasoning_ms":null} -->

The release makes shared project updates easier to follow.

## Jules Rivera · 2026-11-18 09:12:00 -05:00
<!-- id:019a8c00-0000-7000-8000-000000000122 role:person bytes:65 -->

Use this structure for the draft:

## Changes

- A short summary.

## Codex via OpenAI · GPT-5.5 · High · 2026-11-18 09:12:01 -05:00 · 8 s
<!-- id:019a8c00-0000-7000-8000-000000000123 role:agent bytes:49 turn:019a8c00-0000-7000-8000-000000000133 meta:{"provider_id":"calternal-codex","model_id":"gpt-5.5","reasoning_level":"high","machine_id":null,"duration_ms":8000,"reasoning_ms":null} -->

Shared projects now show recent updates together.
```

The `## Changes` line belongs to Jules’s body. Its bytes are unchanged.

#### 3. Parent thread links to work delegated to a subagent

```markdown
---
calternal-id: 019a8c00-0000-7000-8000-000000000013
format: calternal-thread/v1
title: "Compare onboarding notes"
date: 2026-11-18T19:50:00+05:30
last edited: 2026-11-18T19:50:21+05:30
---

# Compare onboarding notes

## Maya Chen · 2026-11-18 19:50:00 +05:30
<!-- id:019a8c00-0000-7000-8000-000000000140 role:person bytes:49 -->

Compare the setup steps with the short reference.

## Claude Code via Anthropic (subscription) · Claude Opus 5.5 · High · 2026-11-18 19:50:01 +05:30 · 20 s
<!-- id:019a8c00-0000-7000-8000-000000000141 role:agent bytes:114 turn:019a8c00-0000-7000-8000-000000000150 meta:{"provider_id":"calternal-claude-code","model_id":"claude-opus-5-5","reasoning_level":"high","machine_id":null,"duration_ms":20000,"reasoning_ms":null} -->

The reference is missing the account setup step. See [Reference review](<Reference review.md>) for the comparison.
```

#### 4. Child thread with an interrupted attempt and explicit resume

```markdown
---
calternal-id: 019a8c00-0000-7000-8000-000000000014
format: calternal-thread/v1
title: "Reference review"
date: 2026-11-18T19:50:02+05:30
last edited: 2026-11-18T19:50:19+05:30
archived: true
---

# Reference review

## fx via Vercel AI Gateway (API) · Claude Haiku 5.5 · Low · 2026-11-18 19:50:02 +05:30 · 5 s · Interrupted
<!-- id:019a8c00-0000-7000-8000-000000000160 role:agent bytes:53 turn:019a8c00-0000-7000-8000-000000000170 meta:{"provider_id":"calternal-fx","model_id":"claude-haiku-5-5","reasoning_level":"low","machine_id":null,"duration_ms":5000,"reasoning_ms":null} -->

I found the reference and saved the first comparison.

## fx via Vercel AI Gateway (API) · Claude Haiku 5.5 · Low · 2026-11-18 19:50:12 +05:30 · 7 s · Resumed
<!-- id:019a8c00-0000-7000-8000-000000000161 role:agent bytes:77 turn:019a8c00-0000-7000-8000-000000000170 meta:{"provider_id":"calternal-fx","model_id":"claude-haiku-5-5","reasoning_level":"low","machine_id":null,"duration_ms":7000,"reasoning_ms":null} -->

The setup note includes account setup. The reference starts at project setup.
```

The two attempts share a turn UUID. The placements are kept private.

#### 5. Automation, a failed attempt and retry

```markdown
---
calternal-id: 019a8c00-0000-7000-8000-000000000015
format: calternal-thread/v1
title: "Weekly release review"
date: 2026-11-18T20:30:00+05:30
last edited: 2026-11-18T20:30:41+05:30
---

# Weekly release review

## Automation Weekly release review · 2026-11-18 20:30:00 +05:30
<!-- id:019a8c00-0000-7000-8000-000000000180 role:automation bytes:43 -->

Summarize the release checks due this week.

## Claude Code via Anthropic (subscription) · Claude Sonnet 5.5 · Provider default · 2026-11-18 20:30:01 +05:30 · 2 s · Failed
<!-- id:019a8c00-0000-7000-8000-000000000181 role:agent bytes:38 turn:019a8c00-0000-7000-8000-000000000190 meta:{"provider_id":"calternal-claude-code","model_id":"claude-sonnet-5-5","reasoning_level":null,"machine_id":null,"duration_ms":2000,"reasoning_ms":null} -->

Anthropic was temporarily unavailable.

## Claude Code via Anthropic (subscription) · Claude Sonnet 5.5 · Provider default · 2026-11-18 20:30:33 +05:30 · 8 s · Retry 2
<!-- id:019a8c00-0000-7000-8000-000000000182 role:agent bytes:29 turn:019a8c00-0000-7000-8000-000000000190 meta:{"provider_id":"calternal-claude-code","model_id":"claude-sonnet-5-5","reasoning_level":null,"machine_id":null,"duration_ms":8000,"reasoning_ms":null} -->

Two checks are due on Friday.
```

[Notes ADR 0023]: https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0023-notes-are-flat-files-with-stable-ids.md
[Notes ADR 0025]: https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0025-title-properties-and-frontmatter.md
[Money ADR 0057]: https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0057-money-files.md
[Contacts ADR 0063]: https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0063-contact-notes-keep-original-vcard-lines.md
[Tasks ADR 0066]: https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0066-tasks.md
[Calendar ADR 0067]: https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0067-calendar.md
[ADR 0068]: https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0068-frontmatter-and-lines.md
[Claude Code model configuration]: https://code.claude.com/docs/en/model-config
[Claude Code managed settings]: https://code.claude.com/docs/en/managed-settings
