---
title: "Threads"
---

<!-- Generated from: docs/threads.md, docs/orchestrator-proposal.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

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

The orchestrator is default-enabled for a fresh space. With the default server
plugin path, the native manifest is available from the binary if the working
directory lacks `plugins/calternal-orchestrator/manifest.json`. Deployments
using `--plugin-root` provide their own trusted plugin tree. An authorized
fresh space lists zero threads before any conversation is created.

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
