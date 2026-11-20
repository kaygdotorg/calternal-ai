# Running code, machines and agents · design section · 2026-11-21

Status: one section of the primitives-only design (thesis primitives 4 and 7). It
tests the thesis, specifies the primitive and lists the owner's questions. It decides
nothing that is the owner's. Lane: `lane/design-run`. Tracking: Forgejo #107.

Inputs: the thesis (`docs/proposals/primitives-only-thesis-2026-11-21.md`, dev), the
host draft (`docs/proposals/primitives-only-host-2026-11-21.md`, `lane/v2-host-base`,
P7 Work, P8 Sandboxes, P9 Sync, P10 Calls), the morning record
(`docs/morning-2026-11-20.md`: restart, 24 hours, harness placement, alarms), revision
14 §2 "Space runtime" and §8 "Running execution and update gate", the paused slices 1, 3
and 4 and their final reports, and the measurements
(`docs/measurements/space-runtime-walls.md`, `plugin-workers-in-sandboxes.md`). The
plugin-split measurement (#103) has posted no results yet; this section does not
depend on its numbers, and question 6 waits for them.

## 0. The idea in five lines

1. There is one kind of thing that runs: **work**. A row in one per-space table says
   what should run, where, for whom, until when, and its state.
2. Work runs in a **box**: a plugin process in the space, a hosted sandbox, or a
   sandbox on a machine a person enrolled. The box has no network, no files and no
   rights except what its grant says.
3. Three things start work: a **call**, a **time**, a **change**.
4. A program on a machine talks to the host through **two append-only files** with
   one writer each: the **inbox** (host writes: inputs, call results, Stop) and the
   **log** (the machine writes: output, call requests, the end). Syncing a
   single-writer append-only file is "send what the other side lacks". That is the
   whole restart story.
5. The outside (network, secrets, notifications) is three host calls, not a primitive
   of its own.

## 1. The primitive

### 1.1 Work: one table, one row per unit

Per space, in the space's own SQLite file (as `jobs.db` is today), never
installation-wide.

| Column | Meaning |
| --- | --- |
| `id` | UUID of the work. |
| `request` | The caller's request ID. Unique per (plugin, request). A retry with the same request returns this row. |
| `plugin`, `version` | Who owns it, pinned at start. |
| `actor` | The person it acts for (or the space, for install-time watches). Authority is re-checked at every start and every call. |
| `target` | Either `op:<name>` (plugin code in the space) or `program:<name>` (a declared program in a box). |
| `key` | Optional concurrency key: at most one row per (plugin, key) is not ended. A second start with the same key is refused or replaces, as the caller says. |
| `state` | `waiting` → `due` → `running` → `stopping` → `ended`. |
| `at` | When it may start (an alarm). Null means now. |
| `place` | `space`, `hosted`, or a machine ID. Set when it starts. |
| `deadline` | Absolute time at which the host and the box both end it. |
| `input` | Small JSON (≤ 16 KiB). Large input goes in a file the work is granted. |
| `checkpoint` | Small JSON (≤ 64 KiB) for space work that resumes after a crash. |
| `end` | `completed`, `stopped`, `failed`, `interrupted`, `unreachable`, `refused`, with a short reason. |
| `result` | Small JSON (≤ 16 KiB), or a file reference. |
| `started`, `ended` | Times (UTC). |

This one table replaces revision 14's intents, held Sends, attempts, launch bindings,
controls ledger, the update fence, jobs, waits, alarms with grace and missed bits, and
the receipts table (a write call with a request ID is a row that is `ended` at once;
its `result` is the receipt).

**Bound and retention.** Rows that are not ended: at most 1,024 per space and 256 per
plugin (operator defaults). Running: at most 32 per space and 8 per plugin in the
space; programs in boxes have their own quota (§1.5). Ended rows are deleted after the
receipt retention (7 days by default, `CALTERNAL_RECEIPT_RETENTION_DAYS`) and beyond
the newest 1,024 per space. What a run did is kept in its log file, not in this table.

**Where it lives.** This table is neither userdata (a person's files) nor appdata
(rebuildable). It is host state, like people, grants and machine enrollments. The
thesis has no name for this third store; see §7 and question 5.

### 1.2 Operations

Costs are on the request path: SQLite reads (R), durable flushes (F, an fsync of WAL
or file), and host-to-box messages (M). A call that is refused costs at most one read.

| Operation | Inputs | Result | Failure | Cost |
| --- | --- | --- | --- | --- |
| `work.start` | `request`, `target`, `input`, optional `at`, `key`, `deadline`, `place` (`space` / `hosted` / `mine` / `ask`), for programs a `grant` (§1.6) | the row (`id`, `state`) | `quota`, `key_busy`, `not_allowed` (grant ∩ person), `no_place` (no eligible box), `bad_input` | 1 R (request lookup) + 1 F (insert). Programs: + 1 F to create inbox and log (one directory sync). Due-time change of the space: + 1 F on the host's next-due table (only when the earliest alarm moves). |
| `work.send` | `id`, `request`, `record` (≤ 64 KiB) | inbox sequence number | `ended` (the run is over: the caller decides what next), `not_program`, `quota` | 1 R + 1 F (inbox append) + 1 M (pushed on the live link if connected). |
| `work.stop` | `id`, `request` | `stopping` or `ended` | `not_allowed` | 1 R + 1 F (row to `stopping` and the inbox Stop record in one change). Effects of the run are fenced from this commit (§2.4). |
| `work.get` / `work.list` | `id`, or `key` / `state` filter, ≤ 100 rows a page | rows | — | 1 R (indexed). |
| `watch.start` | `request`, `target` (a file, a folder, or a saved query), `op`, optional `key` | the watch (a work row of target `op`, state `waiting`) | `quota`, `not_allowed` | 1 R + 1 F. |
| `net.fetch` | `url`, `method`, `headers`, `body` (≤ 1 MiB), optional `secret` use (§1.8) | status, headers, body (≤ 8 MiB) | `destination_not_declared`, `private_address`, `quota`, `timeout` | 0 R on the content store; 1 audit record (batched, not flushed per call). |
| `notify` | `to` (people), `item`, `phrase` (one of the plugin's declared phrases) | `sent` count | `not_reader` (dropped silently per recipient), `quota` | 1 R per recipient (read check) + 1 F (queued push). |

**Who may stop or send to a run:** its actor, the owning plugin (which applies its own
rule, for example "anyone in the chat may stop the agent"), and the space's managers
and owner. The SDK writes `target` as `op:` or `program:`.

There is no `alarm.*`, no `job.*`, no `turn.*`, no `steer` and no `attach`. An alarm
is `work.start` with `at`. A steer is `work.send`. Reattachment is ordinary sync.

### 1.3 The three triggers and their bounds

| Trigger | What starts | Bounds (operator defaults) | Delivery rule |
| --- | --- | --- | --- |
| **A call** | A plugin operation, from the interface, another plugin, a webhook, or a running program's call record | Per space 32 running and 64 queued calls; per plugin 8 and 16. One call ends within 30 s of wall time; longer work must be `work.start`. Calls made by one call: at most 128. | At most once per request ID while the receipt is kept. |
| **A time** | `work.start` with `at` | ≤ 1,024 waiting per space. `at` at most one year ahead. A plugin computes its own next time (no recurrence rules in the host, owner decision). | Delivered **once**, possibly late, with `scheduled` and `started` times. The plugin decides to run or skip (owner decision). If the host crashed while it ran, it is delivered again once with `retry: 1`; the SDK derives the request IDs of its writes from the work ID, so repeated writes are no-ops. |
| **A change** | `watch.start`, or a watch declared in the manifest (created at install) | ≤ 256 watches per plugin. At most one delivery in flight per watch; changes during it are merged into the next. At most one delivery per second per watch. A plugin's own writes never trigger its own watches (Chat's parts are separate plugins, so the notify part still sees the core's writes). A chain of change deliveries stops at depth 4 (the depth travels in the origin stamp). | The op receives `{target, from, to}` revisions and reads the difference itself. Only changes its actor may read now are announced. |

Every trigger also counts against a **wake budget** per space (for example 600 wakes an
hour). A sleeping space wakes only for: an admitted call, a due alarm, a change
delivery to a watch, or a program record that needs plugin code. Sync of a run's log,
reads of files and queries never wake plugin code.

### 1.4 Where plugin code runs (the box in the space)

Today and in revision 14: one microVM per space for plugins, one Node process per
plugin inside it, read-only code, no network, a seccomp profile without sockets. Wake
from a full snapshot about 0.2 s warm and 0.4 s cold (p50, one space); twenty
simultaneous wakes 0.84 s warm and 2.3 s cold; a resident space with five workers
about 127 MiB PSS on the process wall, and about 12 MiB per extra worker in the
50-worker measurement (`space-runtime-walls.md`, `plugin-workers-in-sandboxes.md`, perf-01).
A space stays resident for 15 minutes after last use (owner decision).

This design keeps that wall. What changes is when a process exists:

| Plugin shape | Process | Status |
| --- | --- | --- |
| Interface only (kinds, collections, descriptions, no operations) | none | Agreed by the owner to try; Reading List is this shape. |
| Has operations | starts on first call, stops after idle (proposed 60 s) | Agreed to try; being measured (#103). |
| Same publisher, opted in | one runtime, a separate heap (isolate) each | Agreed to try and measure; isolation loss stated in question 6. |

The work table makes the choice invisible to authors: a plugin never assumes its
process is alive between two triggers. State lives in files, in `checkpoint`, or in
the row's `input`.

### 1.5 Programs in boxes (long work and agents)

A plugin declares programs in its manifest. A harness is an ordinary plugin with one
program; nothing in the host is about agents.

| Field | Meaning |
| --- | --- |
| `image` | The program's root image by digest (an OCI image or a Nix closure). |
| `entry` | The plugin's own adapter code, which runs inside the box next to the program, reads the inbox and writes the log. |
| `network` | Destinations (host names, port 443) the box may reach. |
| `login` | A named login folder the program needs (`claude`), mounted at a stated path. |
| `workdir` | `none`, `scratch`, or `ask` (a folder the person picks on their own machine). |
| `limits` | Memory, CPU, processes, log size; never more than the operator's maximum. |

**Where it may run** (owner decision, 2026-11-20): an **approved** program (a reviewed
build pinned by digest by the operator; ours go through the same approval) may run
anywhere, including hosted sandboxes. Any other program runs only on a machine
enrolled by the person the run acts for; the install screen says it gets network
access and the person's logins. The check is one line at `work.start`:
`approved(plugin, image digest) or machine.owner == actor`. In a shared space, "the
space's subscription" means a machine and login the space owner provided; see §4
row 12 and question 2.

**The box.** On a machine: rootless Podman with no network device, a read-only
root, only the granted mounts, seccomp, and a local egress proxy that allows only the
declared destinations and refuses private addresses (slice 4 built and tested this).
Hosted: the space's agent microVM, the same container inside. The machine enforces the
deadline, Stop and the 24-hour rule itself, so they hold when the server is down.

**Quota per run** (operator defaults): deadline up to 7 days (default to be decided,
question 7), log up to 256 MiB, inbox up to 16 MiB, one record up to 1 MiB. At most
4 programs running per person per machine and 8 per space hosted.

### 1.6 The run's grant and its two files

At `work.start` the host computes the run's grant once and stores it with the row:
the plugin's declared rights ∩ the acting person's rights ∩ what the caller selected
(for Chat: the records whose authors consented). It always includes:

- append to its own **log**, read its own **inbox**;
- the calls the harness declared, each still checked per call at the call table
  (calls section) with the run as the caller.

Files, per run, in the harness plugin's folder (host draft question 4, option b):

```
Harnesses/claude-code/Runs/2026-11-21 1830 Plan the move 7f3a/
  inbox.md   written only by the host
  log.md     written only by the box (through the machine)
```

Each file is a record file (files section): a Markdown block per record, with a
sequence number and an origin stamp the host keeps where the plugin cannot forge it.
Record types are the harness plugin's own except the few the host and machine
themselves write or read: `stop` and `result` (inbox), `call` and `ended` (log), and
`applied` / `not_delivered` (log, the delivery receipt for an inbox record). Text
people read (inputs, replies) is Markdown; a program's raw events go in a fenced JSON
block inside the record, because they are machine output that is rarely read and has
no Markdown shape.

**Sync.** Each file has one writer, and the other side only reads, so there is nothing
to merge. On every connect, the machine says "log: I have through n; inbox: I have
through m", the host answers with what it has, and each side sends the missing
suffix. The host appends a batch at its expected position (compare-and-set on the
sequence), so a duplicate batch is a no-op. The machine keeps its whole log until the
run's files are deleted, so the host can always ask again. Cost: one flush per synced
batch on each side; the machine batches at most every 50 ms or 128 KiB.

### 1.7 One channel, one message format

Every exchange between host, space runtime, machine and box entry is a **call**:
`{id, name, input}` answered by `{id, ok | error}`. The caller's identity is the
channel's (a space worker's ticket, a machine's enrolled key, a run's grant), never a
field in the message.

| Link | Carries |
| --- | --- |
| Host ↔ space runtime (vsock pipe) | Calls into plugin operations and the plugin's calls back |
| Host ↔ machine (one authenticated link, live push when connected) | `sync.offer` / `sync.append` for run files, work assignments, and nothing else |
| Machine ↔ box entry (stdin / stdout) | Inbox records in, log records out; the entry's calls are `call` records in the log |

A machine is just another caller with a narrow grant per run. There is no separate
broker socket, egress socket or relay protocol.

### 1.8 The outside: network, secrets, notifications

| Call | Rule | Checked at |
| --- | --- | --- |
| `net.fetch` | Only destinations declared in the manifest and approved at install; HTTPS; DNS resolved and re-checked at connect and on each redirect; private and link-local addresses refused (`calternal_types::public_address`); per-plugin byte and request quota. Plugin code in the space has no other route. | The host's fetch path (`crates/server/src/netguard`, `plugins/network`). |
| Secrets | A plugin declares a secret name and the one destination it is for. A person or manager types the value in a host-drawn form. Plugin code never reads it; it asks `net.fetch` to put it into a named header for that destination only. Responses that contain the secret value are refused. | `crates/secrets`, at `net.fetch`. |
| `notify` | Recipients must be able to read `item` now. The push carries the item's title (from the index, as the recipient may see it) and one of the plugin's **declared phrases** (static strings in the manifest, reviewed at install, at most 8). No other text leaves the host towards a push service. Per plugin and recipient: 30 an hour. Mute, focus and lock-screen settings are the recipient's own, applied by the host and device. | `crates/server/src/web_push`. |
| Programs in boxes | Their network is the machine's local proxy over the declared destinations. They do not use `net.fetch`; they keep working while the server is down. | The machine (slice 4 `egress.rs`). |
| Logins | A login is a folder the person picks: on their machine a path they choose; hosted, a folder stored for that person, encrypted, never readable by space plugin code, mounted only into runs that person started with an approved program. | `work.start` (placement) and the machine's mount builder. |

## 2. The restart test: does "the output is a file" hold?

The owner's requirement (2026-11-20, reference bb 0.50.0): a running chat keeps
working when the server goes down and comes back; the agent keeps running; its
output arrives afterwards; messages and Stops sent around the restart are neither
lost nor applied to the wrong run; after 24 hours of continuous unreachability the
machine stops the run and keeps its log. bb 0.50.0 describes the same behaviour:
"bb now records which messages, steers and stops are still on their way to the agent
… A stop that arrives after a reconnect no longer interrupts a turn the agent started
in the meantime" ([changelog](https://getbb.app/changelog#0-50-0)).

Each case below says what happens in this design and what in the old design it
replaces.

### 2.1 Output during a restart

The box keeps writing `log.md` on the machine. Nothing on the machine waits for the
host. When the host is back, the machine's sync sends the suffix the host lacks.
Replaces: the output spool, the relay window and credit, `EntryAck`, the per-frame
digest chain, `EntryAttached`. **Holds**, with one condition: the host must have
flushed a batch before it tells the machine "I have through n" (otherwise a crash
loses an acknowledged batch; the machine still has it and resends, so even this only
costs a resend).

### 2.2 Which run is this? (the reattachment proof)

The machine connects with its enrolled key. For each run it reports, the host checks
one row: `place == this machine` and `state in (running, stopping)`. That is the whole
proof. A machine that rebooted knows its programs are gone (it starts them; it saw
them die) and writes `ended: interrupted` itself. A forged machine has no key. Another
machine fails the `place` check. **Holds.** What remains of the old proof: nothing
beyond the placement column, plus one rule for lost data:

- If the host holds more of a log than the machine (the machine was restored or its
  disk damaged), the host keeps its copy and the run ends `interrupted`.
- If the host was restored from a backup, its sequence numbers could be reused for
  different records, and a machine would skip them as "already have". So a restore
  starts a new **host epoch**; a machine that sees a new epoch ends its runs
  `interrupted` and syncs their logs as history. Every run in flight is lost on a
  restore, by design.

A run is never restarted by reconnecting. Continuing an interrupted run (for example
Claude Code's own session resume, as bb's "resume interrupted threads") is a new
`work.start` by the plugin, under current authority, with a link to the old run.

### 2.3 Send exactly once

The person's device keeps an outbox and retries with the same request ID until it gets
an answer. Then:

| Where it breaks | What happens |
| --- | --- |
| The server dies before the commit | The device retries after restart; the host commits once. |
| The server commits and dies before answering | The retry finds the row by request ID and returns the stored result. |
| The machine is unreachable | The inbox record waits on the host; the machine receives it on reconnect. |
| The run ends while the record is on its way | The machine writes `not_delivered: run ended` for it. The plugin sees that and starts a follow-up run (Chat's queued follow-up). Never applied to another run. |
| The machine applies it and crashes before the log syncs | The machine writes `applied n` to its log before it hands the input to the program. On restart it never feeds n again. |

So: **exactly once into the inbox, at most once into the program, and never silently
lost**: each inbox record ends with either `applied` or `not_delivered` in the log.
This is honest but it is not "exactly once into the program" in the case where the
machine daemon crashes between writing `applied` and handing the input over while the
program keeps running (the input is then lost, and shown as applied). The machine can
narrow this by writing `applied` after the program's own acknowledgement where the
program has one: Codex app-server's `turn/steer` returns the accepted turn
([app-server README](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md)),
and Claude Code re-emits each stdin message with `--replay-user-messages`
([CLI reference](https://code.claude.com/docs/en/cli-reference#cli-flags)). Question 1.

### 2.4 Stop targets the exact run

Stop is written to **that run's** inbox. A new run has a new inbox. A late or repeated
Stop can only ever reach the run it names. bb's bug ("a stop that arrives after a
reconnect interrupts a turn the agent started in the meantime") cannot be written in
this design.

At the Stop commit the host also **fences** the run: from then on it refuses every
`call` record from that run except log appends and `ended`. Output the program wrote
before it saw the Stop still syncs to the log (history), but its writes to the chat
are refused. When the machine sees the Stop it kills the box and writes `ended:
stopped`.

Not contained: while a machine is unreachable, a stopped program on it keeps running
and keeps reaching its declared destinations (for example the provider's API) until the
machine reconnects, or its deadline, or the 24-hour rule. On a person's own machine
this is the person's own program and login; on a hosted sandbox the host kills the box
directly, because it controls the agent microVM the box runs in.

### 2.5 The 24-hour rule

The machine measures time since its last successful sync exchange with the host,
on the boot clock (which counts suspend). At 24 hours it stops the run like a local
Stop, writes `ended: unreachable`, and keeps both files; they sync when the host
returns. Slice 4 built and tested this on a paused clock
(`crates/machine/src/runner/entry_outage_tests.rs` on `lane/v2-host-harness`).

The host mirrors it: a row whose machine has not synced for 24 hours plus a margin
for clock drift (proposed 1 hour) is marked `ended: unreachable` and fenced. Without
the mirror a lost laptop would hold a `running` row and its concurrency key forever.

Open: a laptop that sleeps for 30 hours wakes with the rule already past. Read
literally, the run stops at wake. Question 8.

### 2.6 Steering a running agent

`work.send` appends to the inbox; the live link pushes it within one round trip; the
harness's entry hands it to the program (stdin for Claude Code's stream input, a steer
request for Codex). Delivery and race with completion as §2.3.

### 2.7 Calls from a running program

The entry writes a `call` record to the log (`{seq, name, input}`). The host executes
it once, keyed by (run, seq), with the run as the caller, and appends the result as
an inbox record. During an outage the call simply waits; after reconnect it runs under
the authority of **now** (a consent withdrawn in the meantime applies). This replaces
slice 4's separate broker socket and the in-memory resend buffer (up to about 32 MiB
per run). Cost: one round trip and one flush on each side per call; a program that
makes many small calls should batch them.

### 2.8 Server update

Runs on machines and in hosted sandboxes do not depend on the server process, so an
update no longer has to wait for idle. The update drains only in-flight calls in the
space runtime (stop admitting, wait up to 10 s, restart); devices retry with their
request IDs. This deletes revision 14's updater fence, `UpdateGate` and the held-Send
table.

Two limits: a hosted sandbox lives in a microVM on the host, so it survives a restart
of the server process but not a reboot of the host machine; and the space runtime's
plugin processes restart with the server (their work resumes from `checkpoint`, or is
delivered again once with `retry: 1`). Question 3.

### 2.9 Verdict on the thesis's central claim

**Mostly true.** The spool, relay window, acknowledgements and the reattachment proof
do disappear. Three things do not, and the thesis should say so:

1. Output needs a twin: the **inbox** (inputs, call results, Stop) is the second file.
   "One file the machine appends to" is half the channel.
2. The files must be **single-writer and append-only with sequence numbers**; that,
   not "sync", is what makes delivery exact. A general two-way file sync with merges
   would bring the relay's problems back.
3. Two host-side rules stay: the **fence at Stop** (refuse the run's effects after the
   commit) and the **lost-machine mirror** of the 24-hour rule. Each is one check on
   one row.

## 3. What plugins can do and cannot do

**Plugins can:**

- Run code when someone uses them, at a time they choose, or when a file or a list
  changes. *Example: a reminder plugin wakes on Saturday at 09:00 to show "Read the
  article you saved".*
- Compute their own repeating schedule (every weekday, the last Friday of the month)
  and decide what to do with a late one. *Example: an automation that runs at most
  once a day skips yesterday's missed run and only runs today's.*
- Run a long program in a sandbox, on hosted machines if the program is approved, or
  on the person's own machine. *Example: a video plugin transcodes a file for an hour
  on the person's Mac.*
- Run an agent (Claude Code, Codex, opencode, any harness someone writes) that keeps
  working through a server update and a dropped Wi-Fi. *Example: a Claude run started
  on a laptop keeps working while the server updates, and its reply appears after.*
- Send more input to a running program, stop it, and see whether each input arrived.
  *Example: "also check the tests" reaches the running agent; if it had just
  finished, Chat starts a follow-up.*
- Call web services they declared, using a secret the person typed, without seeing the
  secret. *Example: a weather plugin calls one weather API with the person's key.*
- Send a push notification to people who can read the item. *Example: "New in Plan
  the move" on the phones of the chat's members.*

**Plugins cannot:**

- Run anything outside its box, or keep running once Stop reaches its machine, after
  its deadline, or after 24 hours without the server; and nothing it does after Stop
  is accepted by the host. *Example: a broken harness that ignores Stop is killed by
  the machine, not asked.*
- Reach any address they did not declare, or the local network. *Example: a plugin
  declared for `api.example.com` cannot reach `192.168.1.1`.*
- Read a secret, a login folder, or another program's files. *Example: a community
  harness cannot read the Claude login of another harness.*
- Run an unapproved program on hosted machines or on someone else's machine.
  *Example: a stranger's harness installed in a shared space runs only on the
  machine of the person who starts it.*
- Wake a space more often than its budget, or loop by reacting to its own writes.
  *Example: a plugin that rewrites a file on every change is not woken by its own
  write.*
- Put message text in a push. *Example: a push says "New in Plan the move", never the
  message.*
- Make the host retry or "catch up" missed times for it. *Example: an alarm due during
  a 2-hour outage arrives once, late, with both times.*

## 4. Security

| # | Attack | What stops it | Checked at |
| --- | --- | --- | --- |
| 1 | Plugin code reads or writes beyond its grant | No ambient authority in the box; every effect is a call checked against grant ∩ person ∩ target | Call table (calls section), once per call |
| 2 | A run keeps acting after Stop | Fence at Stop commit: the host refuses its call records; the machine kills the box | `work.stop` commit; machine on inbox Stop |
| 3 | A late or duplicate Stop hits a newer run | Stop lives in one run's inbox | Structural (address = run) |
| 4 | Replay or injection of output | Sequence numbers with compare-and-set append; machine key; `place` check | Host sync handler |
| 5 | A machine claims another machine's run | `place` column | Host sync handler |
| 5b | Host restored from backup reuses sequence numbers | New host epoch ends every run in flight | Machine, at connect |
| 6 | A program outlives a dead server | Machine-side deadline and 24-hour rule on the boot clock | Machine runner |
| 7 | A lost machine pins a `running` row forever | Host mirror: 24 h + margin → `ended: unreachable`, fenced | Host timer per space (next-due table) |
| 8 | Flood: endless alarms, watches, calls, wakes | Row quotas, per-plugin running caps, wake budget, change coalescing, depth 4 | `work.start`, `watch.start`, dispatcher |
| 9 | Change-trigger loop between two plugins | Own writes don't trigger own watches; depth counter in the origin stamp; wake budget | Dispatcher |
| 10 | Fill the disk through a log | Per-run log and inbox caps; space storage quota; record size cap | Host append; machine writer |
| 11 | Exfiltrate over the network | Declared destinations only, private addresses refused at resolve, connect and redirect | Host fetch path; machine egress proxy |
| 12 | A run in a shared space reveals others' consented words to the machine's owner | **Not stopped.** A machine runs the program and sees its whole log. If the space's machine belongs to the space owner, the owner can read runs other members started | Question 2 |
| 13 | Unapproved harness on own machine sends everything it got to its author | **Not stopped by design.** It gets network and logins (disclosed at install). Containment is only what it was given: the consented records and the person's own folders | Consent (authority section) |
| 14 | Steal a secret by echo (the declared destination reflects the header) | Secret bound to one destination; responses containing the secret refused. **Not sure it holds** for encoded echoes (base64, split across chunks) | `net.fetch` |
| 15 | Steal a login | Login folders mounted only into runs of the person who owns them, only for approved programs when hosted; never visible to space plugin code | Machine mount builder; `work.start` |
| 16 | Snapshot reuse after wake (same random numbers) | Reseed kernel and every process before plugin code runs (measured: Node needs its own reseed) | Space runtime wake |
| 17 | Push leaks content | Title from the index as the recipient may see it, plus a declared phrase only | `notify` |
| 18 | A space starves another | Per-space queues, fair dispatch, no installation-wide lock on the request path; the only cross-space structure is the next-due table, read by the timer, not by requests | Dispatcher |
| 19 | Copies of consented text outlive "delete for everyone" | **Not stopped.** The run's log (and the provider) saw it | Question 4 |
| 20 | Hosted sandboxes without KVM share the kernel | Labelled as such (revision 14 rule kept) | Operator setting |

Places I am not sure the design holds: rows 12, 13, 14 and 19 above; the depth limit of
4 for change chains (picked, not measured); the 50 ms machine batching (taken from the
8 ms slice-4 batch, not measured for host flush load); and whether a separate isolate per
plugin in one runtime (option 3 in §1.4) is a security boundary at all (V8 isolates
share a process; a V8 bug is an escape to the neighbour).

## 5. Was and is

| Topic | Dev today | Revision 14 | This design |
| --- | --- | --- | --- |
| Plugin code | Node worker per plugin; native Rust for Notes, Threads, Turns, automations | Same, per-space microVM, three VM kinds, 3 s interactive cutoff and `pending` | Same wall; no process for interface-only plugins; start on first call |
| Long work | `jobs.db`: jobs, waits, signals, streams in memory, checkpoints | Jobs + execution intents + attempts | One work row |
| Agents | Native Turns: admission, lease, control, retry, readiness, provider routes | Attempt, one successor per item, one contribution call per attempt, harness ABI | A plugin with a program; run = row + inbox + log |
| Output | Machine log, relayed by the native turn code | Spool, relay window, credit, per-frame digests, `EntryAttached`, ACK rules | The log file syncs |
| Steer and Stop | In-memory run controls | Controls ledger, exact Stop per attempt, outbox | Inbox records; fence at Stop |
| Restart | Native turn recovery, not qualified against the owner's requirement | Five-case gate, update fence, held Sends | Sync resumes; no fence |
| Alarms | Automation scheduler with recurrence rules (native) | One-shot alarm + grace + missed bit | `work.start` with `at`; delivered once late; plugin decides |
| Network | Broker egress allow-list, netguard | Same + separate egress socket for entries | `net.fetch` for space code; machine proxy for boxes |
| Secrets | Secret handles (`request_secret_handle`) | Secret handles + proxy calls | Declared secret used by `net.fetch` |
| Webhooks | Signed wait webhooks that make a parked job runnable (`plugins/signals/webhook.rs`) | Same | A call from outside to a declared operation; the host checks the signature with a declared secret |
| Push | Native web push for chat events | Fixed phrase for Threads | `notify` with declared phrases |
| Logins | Provider homes, shared broker, Claude and Codex routes | Per-harness login folders, personal vs space bindings, a sharing exception | A folder the person picks |

**Deleted:** native Turns (`crates/server/src/turns/*`), automation recurrence rules
(`automations/scheduling`), the native provider routes and `machine/sandbox/shared_broker*`
and their exceptions, revision 14's execution tables (5), the update gate, held Sends,
the contribution spool and its window (its token bucket survives), the entry relay wire and its ACK
rules, the separate entry broker socket, alarm grace and missed bits, the receipts table
as a separate table.

**Survives (adapted):**

| Code | Becomes |
| --- | --- |
| `crates/machine`: `enroll.rs`, `identity.rs`, `link.rs`, `connection.rs`, `ownership.rs` | Machine enrollment and the one authenticated link |
| `crates/machine/src/sandbox/podman*`, `worker-seccomp.json`, `microvm/` | The box on machines and hosted |
| `crates/machine/src/sandbox/provider_home.rs` | Login folders (renamed; no provider names) |
| `lane/v2-host-harness`: `sandbox/podman/entry.rs`, `sandbox/egress.rs`, `sandbox/entry_log.rs`, `runner/entry_outage_tests.rs`, `runner/entry_tail.rs` | Running the entry; the machine's egress proxy; the log writer (now writing the record file); the 24-hour and deadline tests |
| `calternal_types::public_address` (harness branch) | The one private-address list for both fetch paths |
| `lane/v2-host-execution`: request ID before acknowledgement, exact Stop, the storage halves of the five restart cases (`execution/recovery_tests.rs`) | The work table's writer and its tests, rewritten against one table |
| `lane/v2-host-registry`: `contribution_spool/rate.rs` | One generic token bucket for quotas |
| `crates/server/src/plugins/jobs*` (dev) | The work table's per-space SQLite file, quotas and retention (`RETAIN_FINAL`, `MAX_FINAL_PER_SPACE`) |
| `crates/server/src/netguard`, `plugins/network`, `crates/secrets`, `web_push` | `net.fetch`, secrets, `notify` |
| `crates/machine/src/runner/space_channel.rs`, `protocol/output.rs`, `plugin-host/src/entry*`, `server/src/machines/space_channel.rs` | **Thrown away**: replaced by file sync and call records |

## 6. Worked examples

### 6.1 Reading List (no running code)

Interface only: a kind and a collection; no process ever starts. If the author adds a
reminder, it is one call:

```ts
export const remind = defineOperation({
  input: { item: ItemRef, at: Time },
  run: (ctx, { item, at }) =>
    ctx.work.start({ op: "nudge", at, key: `nudge:${item}`, input: { item } }),
});

export const nudge = defineOperation({
  run: (ctx, { item }, when) =>
    // when = { scheduled, started }; the plugin decides about late runs
    when.started - when.scheduled < hours(12)
      ? ctx.notify({ to: [ctx.person], item, phrase: "reminder" })
      : null,
});
```

### 6.2 Notes

Notes needs no program. It uses a change trigger for its view snapshot (the thumbnail
shown while the plugin sleeps, owner decision) and an alarm for a daily note:

```yaml
watches:
  - target: { kind: notes }
    op: render_snapshot        # coalesced, at most once a second per watch
```

```ts
export const render_snapshot = defineOperation({
  run: async (ctx, { from, to }) => {
    for (const file of await ctx.files.changed(from, to))
      await ctx.files.replace(snapshotPath(file), await draw(file));
  },
});
```

Rename and link updates are files-section host operations; they need no running code.

### 6.3 Chat: an agent run that survives a server restart

Plugins: `calternal-chat` (the chat file), `calternal-chat-ai` (calling agents in,
consent), `calternal-chat-claude-code` (the harness, with one program).

Harness manifest:

```yaml
name: calternal-chat-claude-code
depends: [calternal-chat]
programs:
  claude:
    image: registry.example/claude-code@sha256:4be1…
    entry: dist/entry.js
    network: [api.anthropic.com]
    login: { name: claude, mount: /home/agent/.claude }
    workdir: ask
calls: [chat.append_reply, files.read, items.search]   # each checked per call
```

Call-in (in `calternal-chat-ai`, after consent; the selection is the records whose
authors said yes, or waited one minute):

```ts
const run = await ctx.work.start({
  request: send.request,               // the person's Send request ID
  program: "calternal-chat-claude-code/claude",
  place: "ask",                        // host draws "Run on"; secrets never reach the UI
  key: `chat:${chat.id}`,              // one run per chat at a time
  grant: { read: selection.records },  // replies go through chat.append_reply
  input: { chat: chat.id, prompt: selection.text },
});
```

Entry (runs in the box, next to `claude`):

```ts
for await (const rec of inbox()) {
  if (rec.type === "input") { await applied(rec); claude.stdin.write(rec.text); }
}
for await (const ev of claude.events()) {
  log.append({ type: "event", ev });                         // working log
  if (ev.type === "assistant_message")
    log.call("chat.append_reply", { chat, anchor: `${run}/${ev.id}`, text: ev.text });
}
log.end("completed");
```

Timeline with a restart:

| Time | Event | Effect |
| --- | --- | --- |
| 18:30:00 | Ana asks the agent in a chat with Lee; Lee consented "always" | `work.start`; row `running` on Ana's Mac; inbox and log created |
| 18:30:20 | Server update begins | Calls in the space drain ≤ 10 s; the Mac keeps running Claude and appending to `log.md` |
| 18:30:40 | Ana sends "also check the tests" | Her phone's outbox holds it |
| 18:31:10 | Server back | Mac syncs: the host appends log 41–97, executes calls 52 and 90 (`chat.append_reply`, idempotent by anchor). Phone retries Send: the host appends Ana's message to the chat and inbox record 2 |
| 18:31:11 | Mac receives inbox 2 | Writes `applied 2`, hands it to Claude |
| 18:32:00 | Ana presses Stop | Row `stopping`, inbox record 3 `stop`, fence. Mac kills the box and writes `ended: stopped` |
| 18:32:05 | Her phone's duplicate Stop arrives | Reaches the same, ended run: no effect |

Delete for everyone, quotes and consent are the files and authority sections' rules.
What this section adds: a record deleted for everyone stays in any run log that already
read it (question 4); a consent withdrawn while a run waits for a call result applies
to that call (§2.7).

## 7. Where the thesis is wrong or incomplete here

| Thesis says | Problem | Changed to |
| --- | --- | --- |
| "Output is appended to an ordinary file … a restart means sync resumes" | Half the channel; inputs, call results and Stop need the reverse direction | Two single-writer record files per run (§2.9) |
| "Work in progress is one table with a state column" | True, but it is not userdata and not rebuildable appdata; the thesis's two stores do not hold it | A third store, host state, beside people and grants (question 5) |
| "Stop ends the process" | It cannot while the machine is unreachable | Fence at the host at once; the process ends on reconnect, deadline or 24 h (§2.4) |
| Seven primitives, "the outside" separate | Network, secrets and push are three calls with a grant; nothing new | Primitive 7 merges into calls and grants |
| "A change" trigger | No bound or loop rule | Coalescing, own-write exclusion, depth 4, wake budget (§1.3) |
| Restart claim, implicitly for all runs | Hosted sandboxes die with a host reboot; space plugin calls die with the server | Stated limits (§2.8, question 3) |
| Silent on streaming | Token-by-token text is not a record; storing every delta bloats files | Records per finished message; live tokens only in the run log (part of question 4) |
| Silent on the grace decision | The evening record says "missed runs: a grace period, with one default"; the third set says "the server never decides" | Delivered once late; grace is the plugin's setting (question 7) |

## 8. Questions for the owner

**1. What does "exactly once" mean for a message to a running agent?**
*What:* a message you send to a running agent goes through your phone, the server and
the machine. This design stores it exactly once and never applies it twice, and every
message ends marked "arrived" or "did not arrive". *Why now:* it decides how much the
machine must prove, which is the heart of the restart design. *Example:* the machine's
helper crashes in the instant between writing "arrived" and handing the message to
Claude, while Claude keeps running: the message is shown as arrived but Claude never
saw it. *Options:* (a) accept that window (milliseconds, only on a helper crash) and
mark "arrived" after the program's own acknowledgement where the program has one
(Codex, Claude Code's echo): simple, honest in every common case. (b) Require the
program to acknowledge every input, and refuse harnesses whose programs cannot:
stronger, but excludes some community harnesses. *Recommendation:* (a); it gives the outcome bb 0.50.0
describes (a message that misses its run is kept, not lost or misapplied), and the
rare case is visible.

**2. Whose machine runs an agent in a shared space, and who may read the run's log?**
*What:* you decided that in a shared space the space's subscription is used. A
machine sees everything its programs see. *Why now:* it decides whether one member's
agent run, with other members' consented messages, can be read by the space owner.
*Example:* in the family space, Ana calls Claude into a chat with Lee; the run uses the
Mac the space owner (Kim) enrolled; Kim can read `log.md` on that Mac, including Lee's
messages. *Options:* (a) the space's machine, and the install and consent screens say
"runs on Kim's machine": simple, one subscription, Kim sees runs. (b) Space runs only
on hosted sandboxes (approved harnesses), never on a member's machine: nobody's machine
sees others' runs, but costs hosted compute. (c) Always the caller's own machine and
login, even in a shared space: private, but reverses your decision on subscriptions.
*Recommendation:* (b) for shared spaces, with (a) as a space owner's explicit setting
shown in every consent request. Unapproved harnesses still run only on the caller's
own machine (your placement decision), which is (c) for them.

**3. Should automatic updates wait for hosted runs, and may the host reboot under them?**
*What:* runs on people's machines survive any server update. Hosted sandboxes survive
a server restart but not a reboot of the host computer. *Why now:* it sets the
update policy and decides whether hosted runs need snapshotting. *Example:* a kernel
update needs a reboot while a hosted Codex run is halfway through a long refactor.
*Options:* (a) server updates never wait; reboots wait until no hosted run is active
(as bb waits for idle), up to a limit such as 24 hours, then proceed and mark runs
`interrupted` with a "resume" offer: simple. (b) Snapshot hosted sandboxes before a
reboot and resume them: no interruption, but snapshot of running agents is new work
and unmeasured. *Recommendation:* (a) now; (b) only if interruptions turn out to
matter.

**4. Run files: who reads them, how long are they kept, and does "delete for everyone"
reach them?**
*What:* each agent run keeps its inbox and working log as files (what it was told and
what it did, including the text of messages it read). *Why now:* these copies are the
one place the delete-for-everyone and consent rules cannot reach, and their size is
the largest storage cost of agents. *Example:* Lee's message is read by a run, then Lee
deletes it for everyone; the message is gone from the chat and quotes, but Ana's run
log still holds it (and the provider already received it). *Options:* (a) only the
person who started the run can read its files; kept 30 days unless they keep the run;
delete-for-everyone also blanks the text in run logs the host stores (not on the
machine, not at the provider): more work, closer to what people expect. (b) Same
reader and retention, no blanking, and the delete dialog says "agents that already
read it keep their copy": honest and simple. *Recommendation:* (b), plus a 30-day
default; also confirm that other people in the chat see the agent's finished messages
only, not its live token stream (which would otherwise need a growing record type).

**5. Is work state allowed to be a host database instead of files?**
*What:* you decided userdata is strictly files and appdata is rebuildable. The list of
running and scheduled work is neither: it cannot be rebuilt from files, and it is not
a person's content. *Why now:* it decides whether runs and alarms are files people can
see and edit. *Example:* "remind me Saturday" is one row; is it also a file
`Reminders/Saturday.md` the person could delete in Notes to cancel it? *Options:*
(a) a host table (like people and grants), bounded to 7 days after the end; what a
run did lives in its files: fast, one flush per change, nothing to keep in sync. (b)
Each work unit is a file with host-owned fields, and the table is an index: visible,
but every state change is a file write and plugins could edit fields the host relies
on. *Recommendation:* (a); a plugin that wants visible schedules (automations) keeps
its own file and sets alarms from it.

**6. How should plugins share memory in a space (after #103 measures)?**
*What:* each plugin with code is a separate process (about 12 MiB each on the
measured wall). Three cheaper shapes are being measured. *Why now:* Chat is several
plugins; the shape sets per-space memory. *Example:* Chat with composer, people, AI and
notify parts would be four processes. *Options:* (a) interface-only plugins run no
process and others start on first call and stop when idle: no loss of isolation,
first-call delay. (b) Also one runtime per publisher with a separate heap each: less
memory, but plugins of one publisher are no longer separated by the operating system,
only by the JavaScript engine. *Recommendation:* (a) by default; (b) only if the
numbers show (a) is not enough, and only for one publisher's own plugins, as you
said.

**7. Grace for missed alarms and the default run deadline.**
*What:* the record has "the server never decides" (third set) and "missed runs: a
grace period, with one default" (evening). A run also needs a default deadline
(machines accept up to 7 days; slice 4 asked). *Why now:* both are numbers authors
see. *Example:* a 07:00 daily summary when the server was down until 11:00. *Options:*
(a) the host delivers once late with both times; grace is a setting in the automations
plugin and an SDK helper with one default (proposed 1 hour); run deadline default 24
hours, maximum 7 days. (b) The host applies a grace and drops later alarms: one rule
for all plugins, but a policy in the host. *Recommendation:* (a), which reads both
records together.

**8. A laptop that sleeps longer than 24 hours.**
*What:* the machine stops a run after 24 hours without the server. A sleeping laptop
is also "without the server". *Why now:* it decides what people find when they open
their laptop after a weekend. *Example:* Friday 18:00 Ana closes the lid during a run;
Monday 09:00 she opens it. *Options:* (a) count sleep: the run stops at wake, its log
syncs, Chat offers "resume": strict, matches the rule literally. (b) Do not count
sleep (the program was not running either): the run continues on Monday if the
server answers within a minute of wake: friendlier, but a run can then live longer
than 24 hours of wall time. *Recommendation:* (a); the rule exists so that runs never
act long after anyone watched them.

## Simplification opportunities

- Merge thesis primitive 7 into calls and grants: `net.fetch`, secrets and `notify`
  are three calls.
- Merge the receipts table into the work table: a write call with a request ID is an
  ended row.
- One process path: the machine runs a program with an inbox and a log; hosted runs
  use the same machine daemon inside the agent VM, so there is one runner, not two.
- Delete the update fence entirely once runs live outside the server process.
- The harness lanes' `space_channel` relay, `EntryAttached` and the entry broker socket
  can be deleted without replacement; the log's `call` records cover them.
- `provider_home.rs` loses its provider names and becomes a generic login-folder mount.
- Space jobs and agent runs share the same row; the only difference is `target` and
  whether two files exist.
