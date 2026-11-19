---
title: "Threads"
---

<!-- Generated from: docs/threads.md, docs/orchestrator-proposal.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

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

A saved thread holds at most 2 MiB of Markdown, including the host-owned
identity line. An overflowing checkpoint is rejected atomically. A saved turn
reports `thread_full` on the job stream and in `turn_reason.code` on reload.
Earlier checkpoints remain readable.
The failure message explains that the latest checkpoint was not saved and that
the person can start a new thread. The turn does not automatically retry.

The accepted design keeps long tool output in separate, linked storage.
The thread format has no artifact address, link grammar, or lifetime yet.
Until those are defined, the preview keeps only the tool status and byte count.

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

The [Skills and Automations operations](/docs/user/automations/) cover portable skill bundles and scheduled turn admission.

## Snoozing the Inbox

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

## Automatic thread title

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



A plan's compute entitlement is a sandbox size in CPUs and RAM plus hours
per month.

## Accepted design

- The plugin's ID is `calternal-orchestrator`. Every official plugin is `calternal-<name>`.
- Child threads are visible in the sidebar under their parent and archive automatically
  when they finish. A child's file records the thread and the message that spawned it
  (`spawned-by:` in frontmatter), permanently, so an archived child can always be traced
  to its origin and a parent can list every child it ever started.
- A request can carry a list of questions: it is a form described with the generated-UI
  components and drawn by the app. Approvals and secret entry stay fixed, trusted forms.
- The CLI is non-interactive; there is no TUI. The web UI is built at the same time as the
  orchestrator so there is a surface to plug it into and test.
- Offline sending is queued after all: a message sent while offline is a visible pending
  submission on the account that can be edited or cancelled, and is submitted on
  reconnect with its request ID, re-checking access first. (Restores the 2026-11-16
  decision.)
- Work does not move between machines by itself, on the pool or anywhere. If a machine
  goes silent, the turn shows as interrupted and the person is told; Resume starts it
  again from its last checkpoint, on that machine if it is back or on another one.
  (Restores the 2026-11-16 decision; the automatic re-placement built in the machine
  role becomes resume-on-request.)
- Cals come after plain threads.
- One file per thread; long tool output is kept outside the file and linked. Long threads
  are the UI's speed test.
- On a person's own machine they choose per machine: sandboxed, or direct access. On
  macOS the sandbox is the system's own process sandbox, the one Claude Code and Codex
  already use. The app offers, in a friendly way, to add the person's machine so agents
  can work on it.
- A space's permissions apply to every plugin in it, the orchestrator included. A thread
  in a shared space is visible to members who can read the space.
- A coding subagent gets its own worktree.
- Agent sandboxes have outbound network access (they must reach the provider and install
  packages); a space can restrict it to an allow-list. Plugin workers have none.

- When a turn fails with a provider or network error it retries automatically: 5 attempts,
  waiting 30, 60, 120, 240 and 480 seconds. After that, on API-key billing it checks every
  3 hours whether the provider is back; on a subscription it snoozes the thread until the
  limit's reset time and resumes by itself. Both are on by default and configurable.
- The right side is a flat sidebar like the left, not a card.
- Message text is not streamed to the screen by default; the finished message appears at
  once while steps and a working state show progress. Streaming is a setting.
