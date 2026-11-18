# Manual provider turns

Step one runs Claude Code from `calternal agent check|run|stop|resume`; it does not
create a conversation file. Supply a space UUID and a fresh thread UUID to check
or run. All actions are operations on `/api/v1/operations`; the CLI, a person and
an agent use that same path. The CLI watches the existing jobs SSE stream;
`agent.events` remains a bounded explicit snapshot operation. JSON streaming is NDJSON, including errors and the terminal
job record. Human output streams text and labelled tool steps.

```
calternal agent --base-url https://cal.example.org --token-file person.token \
  check --space-id <uuid> --thread-id <uuid>
calternal agent --base-url https://cal.example.org --token-file person.token \
  run --space-id <uuid> --thread-id <uuid> --prompt 'Inspect this workspace'
calternal agent --token-file person.token stop <job-id>
calternal agent --token-file person.token resume <job-id>
```

The enrolled machine's image needs Node and the provider launcher on PATH; the
person installs the official CLI and runs `claude auth login` inside that account's
sandbox home. The machine injects `CALTERNAL_URL` and a thread-scoped credential
for the Calternal CLI. Remote sandboxes use the server's HTTPS origin: a loopback
URL names the sandbox itself. HTTP is accepted only for direct loopback previews.
The server never receives the provider login.

`check` and `run` accept `--provider <plugin-id>`; omitting it uses the server
default. An omitted `--model` uses that provider manifest's default model.

One thread has at most one active or interrupted job. Admission records the prompt
before returning, reuses the same job on an identical request ID, and refuses a
changed input or concurrent turn. Manual admission uses the same bounded
(account, space, request ID) content receipts as other writes, behind a small
port that thread storage will later implement. Each space's jobs SQLite file
holds the turn and checkpoints. Token hashes, scopes, modes, approvals and
discard authorizations live in auth.db and commit through its recovery barrier. Text is live output, not one
file write per delta. Checkpoints store session identity, lease, last sequence and
a bounded 16 KiB text tail after session discovery, each tool start or finish, at turn end
and at least every three seconds of emitted output. Each such checkpoint is one
FULL transaction. Live buffers hold at most 256 chunks / 1 MiB per job. Completed
buffers last 30 seconds, with at most 64 retained completed streams.
Durable final jobs last seven days (at most 1024 final jobs per space); at most
64 active jobs fit in a space. Agent authority is capped at 1024 records per
account. The inbox retains up to 32 request and grant rows per thread and 256
per account. Starting another turn rotates the token, cancels superseded
pending requests, and carries forward eligible grants. Archiving or deleting a
thread removes its saved modes, credentials, and grants for every account.

Machine exec already supplies numbered, acknowledged chunks and back-pressure.
An admitted run remains queued while it reserves a lease and waits for execution
capacity. The first visible running state and its selected machine checkpoint
commit together. A resumed run can expose the previous checkpoint while queued.
An adapter crash or incomplete result ends outcome-unknown. Machine silence fences
the lease and makes the turn interrupted; no machine automatically receives it.
Only an explicit Resume starts interrupted work. Stop, completion and interruption
end the token, remove the container, and retain the mounted workspace and persistent
account home. Container names and directories include the account UUID, preventing a
caller reusing a thread UUID from adopting another account's mounts. Persistent
provider homes are intentionally retained until the machine owner removes them;
the provider owns the files, and Calternal does not inspect or clean login files.

Working areas follow the accepted orchestration retention rule: keep them until
all changes are preserved or the person explicitly discards them. The disposable
container layer is removed on release. Persist work in the mounted `/work` directory
and provider state in the mounted account home. Those directories have no timed
expiry. A person runs `calternal agent ... discard <job-id>` after stopping the
latest turn. The operation reports pending until every retained target machine
confirms removal; the persistent account home remains. A failed result requires
inspection and `discard <job-id> --retry`. Offline, never-sent requests survive
restart. An in-flight deletion lost with a session becomes failed rather than
being replayed over possibly newer work. Another active or newer turn refuses
an old discard. A machine owner can stop its role and run
`calternal machine discard --account-id <uuid> --thread-id <uuid> [--dir DIR]`
for copies older than job/lease history. Auto removes Podman artifacts when the
engine is available; `--backend direct|podman` selects explicitly. A machine-directory
lock prevents this local command from overlapping a live role.

Deletion metadata has at most 128 unfinished requests and at most 1152 total rows
per machine; completed rows are pruned after seven days or down to 1024 during
cleanup activity. There are at most 64 target machines per request. Automatic
cleanup after every change is preserved, and disk accounting against the person's
plan, are named follow-ups. Neither is replaced by a timed workspace expiry.

Each machine reserves at most `--max-sandboxes` slots across provider turns and
plugin workers, including starts and unconfirmed removals. A provider turn
removes its container when it ends. A lease with an idle provider container
parks it on the first successful machine poll after one lease TTL without an
exec (90 seconds by default). A later exec rebuilds the container over the same
mounts on that machine. Plugin workers keep their existing 60-second idle limit.
Machine shutdown and revocation close local containers. An unreachable server
also triggers local removal after a lease TTL, when the machine detects the
expiry. On startup the role removes orphan containers owned by its locked
sandbox directory before accepting work. The sweep preserves provider files.

Permission mode is independent of execution isolation. Cloud/pool machines always
use a sandbox. An account-owned machine defaults to sandboxed execution; its owner
can choose direct execution with `machine enroll --direct` or
`calternal machine set <id> --direct true --token-file person.token`. A direct
backend runs as the machine user in the workspace directory, with no filesystem,
network or resource isolation. The server refuses direct sessions in the pool or
without the account owner's stored consent. Changing execution policy ends the
machine session and its leases before new work can start.

| Mode | Scope and review |
| --- | --- |
| Least access | Thread, workspace and originating app prefix; outside scopes ask |
| Approve for me (default) | Person's current space access; read/reversible proceed, irreversible asks, external asks per scope, widening asks for each action |
| Full access | Person's current space access; read, reversible, irreversible and external operations proceed without approval |

Every operation rechecks person capabilities, token scope, the admitting sign-in
and declared risk. Revocation or expiry of that sign-in stops thread credentials
and execution. In Least access and Approve for me, irreversible/external effects
require an exact action grant; an app grant or originating app never substitutes
for it. Approve for me also exposes the irreversible-only reviewer hook; the
current rules-only reviewer asks. Full access allows these effects without
approval. Access widening is never authorized by mode alone; it keeps the existing
exact-action approval path. Least access also asks outside its originating app
or approved scopes.
One grant shape contains an app prefix, read/write permission and an optional
action hash. Approvals created by denials use that hash in every mode. Sensitive
action approvals conservatively bind canonical operation input; an
irreversible or widening action also binds its request ID. Reuse that ID after
approval. An external scope covers identical canonical input. App grants name a
prefix and read/write. An irreversible or widening action only allows once approval. This-thread
approval is available for reads, reversible actions and an exact external
scope. See [person requests](requests.md).
A thread token cannot approve, change mode, or reissue itself. An in-flight mode
change interrupts the job. Explicit Resume delivers a replacement to a fresh
process. Tokens expire after ten minutes or turn end.
Only hashes persist. Authority uses the existing account directory handle and
worker; no new installation-wide lock, worker or connection is added. Issuance
holds its space's jobs lock across the authority commit to fence stop/resume;
auth callbacks never acquire job or content locks. Worker callbacks recheck that hash, expiry and active turn,
and least-access content calls remain inside the granted kind scope.

`access.request` creates a bounded pending approval. An agent can call it with
`{"app":"notes"}` and an optional `write` flag. Its thread credential supplies
the job ID. An explicit ID must match that credential. A person supplies
`job_id`. Denied sensitive operations
create an approval and return its ID. The recipient lists it through
`calternal requests list` and approves with `calternal requests approve ID`
and the chosen lifetime, using an installation session with a fresh passkey
proof. The shipped reviewer uses rules and asks for irreversible actions.

| Change | Credential behavior |
| --- | --- |
| Approve once or this thread | The requesting job must be waiting on that request. Its old token is revoked before it is readmitted. |
| New turn | Issuance replaces any prior token for this account and thread. |
| In-flight mode change | The job is interrupted and its old token revoked. Explicit Resume issues a replacement. |
| Resume or wake after a decision | A fresh token binds the new run. |
| Stop, finish, or interrupt | The old run cannot use its credential. Stop and finish close pending requests. |
| Archive or delete thread | Thread authority and grants are erased. Pending requests are recorded as cancelled. |
| Once consumption | The protected claim authorizes one execution. An identical retry may replay a saved response while otherwise authorized. The live token remains in place. |
| Deny or answer | The parked job resumes with a fresh run credential. |
| Leave, removal, or loss of space read access | The membership transaction erases the former member's saved authority and requests. Active native jobs are interrupted. |
| Role change that retains space read access | The current token is checked against the new capabilities on each operation. |
| Sign-out | Installation liveness checks refuse the old credential. |

Core instructions live in `plugins/calternal-orchestrator/skills/core.md`. The
provider appends core, the generated one-line operation index, then the space's
`<userdata-ssd>/spaces/<space-id>/AI/AGENTS.md` using the documented
system-prompt append channel. The file is read through descriptor-relative fsio,
with a 16 KiB cap and a visible truncation notice. Missing means no extra text.
Instructions are context and never grant access. Personal instructions remain a
one-line seam until accounts have userdata.

Automatic provider recovery is separate from machine interruption. Explicit
transient/usage-limit errors with no pending tool effect receive five retries,
after waits of 30, 60, 120, 240 and 480 seconds. If retry five fails, API billing
checks availability every three hours. After the short retries, a subscription limit with a reported reset
snoozes until that reset. Auth failures, refusals, invalid
configuration, adapter crashes, unknown outcomes, user stop and machine silence
never enter this scheduler. Retried official sessions receive a continuation
message, not a replay of the original task. A `retry_waiting` job has durable
`checkpoint.retry` fields: retry count, reason, billing and next attempt in Unix
milliseconds. It holds no provider process or execution permit. The idle
container can be removed while its same-machine lease remains fenced by the
ordinary machine protocol. A silent waiting machine
interrupts the job instead of moving it. Restart restores the wait deadline.

Turning off the applicable recovery setting during a wait parks the turn as
interrupted; it does not perform one more automatic retry. Placement queues hold
no shared execution slot. Only placed readers await the existing fair execution
budget, while continuing cancellation and lease checks.

The orchestrator manifest declares typed default booleans for provider-error
retry and subscription-limit snoozing; both default on. `agent.settings` reads or
updates account defaults in that account's private appdata SQLite file. A run's
`retry_settings` overrides them, and `agent.settings` with a job ID updates that
thread's recovery settings. Account defaults affect later turns. No credential is
part of these settings.

Saved conversation admission now uses `threads.send`; see [threads](threads.md).
Manual-turn controls retain their shared receipt adapter. The CLI already follows the job
stream; its SSE decoder is shared with space live subscriptions. A Codex adapter
reuses the machine, job, access, instruction and retry paths.

Protected discard intent is separate from placement delivery metadata. The owner
and exact lease authorization is committed in auth.db before queuing a command;
every delivery rechecks it. At most 1024 current discard authorizations per
account are retained, one per thread. Delivery/acknowledgement rows in machines.db
are operational state, not grants: they cannot authorize a delete by themselves.
Local machine-owner deletion remains a filesystem administration command guarded
against a running machine role; it grants no Calternal access.

Provider output is scrubbed against the issued Calternal credential before event
publication or checkpoint folding. A bounded pending prefix handles a credential
split across text deltas; any extra flush keeps ordered event numbering and the
4 KiB text bound. Checkpoints are scrubbed again before durable saves. Provider
login handling and the choice of token transport remain outside this change.
Owner decision, 2026-11-18: a permission mode requested with `agent.run` or
`threads.send` applies directly to that turn, wider or narrower, and becomes its
saved thread mode. Choosing it on a send needs no additional passkey check.
The old credential is rotated on a mode change; thread credentials cannot choose
a mode or reissue themselves. Explicit in-flight `agent.mode` retains its existing
fresh-person check and explicit-resume flow. Source apps must match an enabled registered
plugin prefix/kind or a built-in operation group.

Turns waiting on a question or approval release their machine lease and resume
only on the recorded machine after the recipient resolves the request. See
[person requests](requests.md).

## Machine and provider readiness

`agent.readiness` takes `space_id` and an optional saved `thread_id`. It
requires a person with `plugins.use` in the space. Unknown and inaccessible
spaces or threads return the same not-found response. A member cannot read
another member's personal machines, even when both can read the same thread.
The caller's active, same-machine pin from a person request limits the
thread-scoped snapshot.

The result contains `machines`, `pool` and `check_ttl_ms`. A personal row
has the enrolled machine UUID and name, `enrolled`, `connected`,
`last_seen_ms` (null when this server has seen no connection), `execution`
(`direct` or `sandboxed`), `available` (placement capacity), and
`providers` keyed by provider ID. An entitled account receives anonymous pool
options with the same capacity and provider fields. Pool rows omit host
identifiers, names and timestamps. Without pool entitlement the array is empty.
Each provider has `enabled` for the space, `check` (null until observed or
after expiry), and `ready` when enabled, available and signed in. An observed
check contains only `installed`, `signed_in` and `sign_in_command`.
An unavailable provider reports the first unmet condition in `blocked`:
`disabled`, `machine_offline`, `machine_busy`, `check_required`,
`check_stale`, `runtime_unavailable` or `sign_in_required`. The provider's
official program owns the login.

Use a cached read with `calternal agent readiness --space-id SPACE_UUID`.
After enrollment, use `machines.verify` with the exact `machine_id` and a
`space_id` where you can use plugins. The machine's account owner can verify
it. A server administrator can verify a pool machine, even without an
execution grant. A pool execution grant does not give this verification access.
The optional `provider_id` uses the installation's default provider when omitted.

Verification runs a single Check job through the machine channel using the
same provider probe and cache as `agent.check`. The HTTP result contains
`sandbox_started` and a `provider` object with `installed`, `signed_in`, and
`sign_in_command`. The provider is `null` if the machine could not report
a check, including when a sandbox could not start. The result carries no job
ID or logs. Placement and probe each have a six-second wait. A retry with the
same request ID returns the same check.

Run it with `calternal machine verify MACHINE_UUID --space-id SPACE_UUID
--token-file PERSON_TOKEN`. Add `--json` for the same fields in the CLI envelope.

Refresh a provider with `calternal agent check --space-id SPACE_UUID
--machine-id MACHINE_UUID --provider PROVIDER_ID`. `--pool` checks an
available entitled pool machine. A Check job is private to its admitting
account. The scratch workspace is released after the check and the sandbox
home is retained. Both adapters report the official program's status during
regular turns as well.

The cache is in memory, keyed by account and provider on the selected
machine. It holds at most 128 observations per machine for five minutes each.
The oldest observation is evicted at capacity. Expiry or eviction returns
the state to unknown. Restart clears the cache. Changes made outside Calternal
become visible after the next provider check.

Subscribe with `GET /api/v1/account/live?readiness=true`. The private
`{"type":"readiness_changed"}` frame asks the account to reload readiness
when its machines connect or drop, capacity changes, or its provider
observations change or expire. This coalesced frame has no bookmark or machine
payload. Read a snapshot after reconnect. Pool machine notifications go only
to current pool-granted subscribers, and provider observation notifications go
only to the account checked.

The warm read takes the existing space capability lookup, a single indexed
space-local SQL statement for activations and any pin, and one owner-indexed
in-memory registry snapshot. No body reads, flushes or machine traffic. A cold
space opens its existing indexes. A check uses the ordinary fenced lease and
job receipt. Cache publication and invalidation add no durable writes.

## Shared-space execution design

A space's Owner or Manager configures its execution machine using the existing
`space.settings` capability (ADR 0058). `space.read` permits reading the setting.
Turn admission still requires the caller's existing capabilities and retains the
caller's identity for the turn, content permissions, and agent token. The space
Owner is the execution sponsor. The open server can apply its operator-set limits
to that sponsor. The private plans service owns the plan-hours meter.

Reuse the machine registry's `machines.db`, fenced leases, enrollment and
revocation checks, and provider sign-in scratch flow. Store one binding per space
UUID through the shared migration runner. Bind it to the current space Owner and
their enrolled sandbox machine. Resolve and recheck that binding before starting
work. If a configured target stops being eligible, refuse the shared route. An
explicit personal choice uses the caller's machine and login for that turn.
Personal and unconfigured spaces keep their existing per-account execution.

A shared sign-in uses a machine-local credential directory scoped to the space.
Only a current `space.settings` holder can write there through the existing
sign-in flow. The server holds no provider credentials. Shared execution must
keep the credential directory outside tool reach. The initial shared adapter
uses the existing tool-free Claude CLI mode, on a sandboxed machine. Personal
execution keeps its normal tools. The settings response carries the notice
“Personal subscriptions are licensed for use by one person.”

Call `space.machine.get` with `space_id` to read the setting and the license
notice. Call `space.machine.set` with `space_id` and `machine_id` to select a
machine, or send `machine_id: null` to clear it. Only people with
`space.settings` see the selected machine ID. Start the existing
`provider.sign_in.start` flow with `shared: true`, `space_id`, and
`provider_id: "calternal-claude-code"`. Its browser challenge and answer stay
private to the account that started the flow. Both `threads.send` and
`agent.run` accept `personal_machine: true` for a single turn.

Shared login requires a current Podman machine role and a provider image with
the dedicated shared Claude entry point. Older roles can still serve personal
turns. Shared turns start a new provider session with an ephemeral home and no
executable tools, hooks, or MCP servers. The machine keeps the shared login
under the enrolling Owner's space directory until that Owner removes it or
erases the account.
