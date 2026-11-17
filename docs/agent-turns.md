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
a bounded 16 KiB text tail after session discovery, every tool finish, at turn end
and at least every three seconds of emitted output. Each such checkpoint is one
FULL transaction. Live buffers hold at most 256 chunks / 1 MiB per job. Completed
completed buffers last 30 seconds, with at most 64 retained completed streams.
Durable final jobs last seven days (at most 1024 final jobs per space); at most
64 active jobs fit in a space. Agent authority is capped at 1024 records per
account, with 32 approvals per turn. The current mode remains until superseded or
the account or space is deleted, so later turns cannot recover a broader initial
mode. Starting another turn replaces the token/grants row and removes old approvals;
thread deletion will be wired through the thread-storage lane.

Machine exec already supplies numbered, acknowledged chunks and back-pressure.
An adapter crash or incomplete result ends outcome-unknown. Machine silence fences
the lease and makes the turn interrupted; no machine automatically receives it.
Only an explicit Resume starts interrupted work. Stop, completion and interruption
end the token, stop processes and retain workspace files and the persistent account
home. Container names and directories include the account UUID, preventing a
caller reusing a thread UUID from adopting another account's mounts. Persistent
provider homes are intentionally retained until the machine owner removes them;
the provider owns the files, and Calternal does not inspect or clean login files.

Working areas follow the accepted orchestration retention rule: keep them until
all changes are preserved or the person explicitly discards them. This lane keeps
released workspaces and stopped containers until explicit deletion, with no timed
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
| Full access | Person's current space access, everywhere; widening still asks |

Every operation rechecks person capabilities, token scope and declared risk.
Sensitive action approvals conservatively bind canonical operation input; an
irreversible or widening action also binds its request ID. Reuse that ID after
approval. An external scope covers identical canonical input. App grants name a
prefix and read/write. Scope approval currently lasts this turn; once-only and
always-in-space grants need a later settings implementation. This turn's grants
already use protected recovery.
An old token cannot approve, change mode, or reissue itself. Token changes revoke
the old credential immediately and pause the turn; explicit Resume delivers its
replacement to a fresh process. Tokens expire after ten minutes or turn end.
Only hashes persist. Authority uses the existing account directory handle and
worker; no new installation-wide lock, worker or connection is added. Issuance
holds its space's jobs lock across the authority commit to fence stop/resume;
auth callbacks never acquire job or content locks. Worker callbacks recheck that hash, expiry and active turn,
and least-access content calls remain inside the granted kind scope.

`access.request` creates a bounded pending approval. Denied sensitive operations
also create an approval and return its ID in the error details. `agent.events`
shows requests. The person signs in again with a passkey, then runs
`calternal agent ... approve <job-id> <approval-id>`; development bearers and
agent credentials cannot approve. There is no approval UI in this lane. The
reviewer trait permits future advice only for irreversible actions; the shipped
rules-only implementation asks the person. No model reviewer is selected.

Core instructions live in `plugins/calternal-orchestrator/skills/core.md`. The
provider appends core, the generated one-line operation index, then the space's
`<userdata-ssd>/spaces/<space-id>/AI/Instructions.md` using the documented
system-prompt append channel. The file is read through descriptor-relative fsio,
with a 16 KiB cap and a visible truncation notice. Missing means no extra text.
Instructions are context and never grant access. Personal instructions remain a
one-line seam until accounts have userdata.

Automatic provider recovery is separate from machine interruption. Explicit
transient/usage-limit errors with no pending tool effect receive five retries,
after waits of 30, 60, 120, 240 and 480 seconds. If retry five fails, API billing
checks availability every three hours. A subscription limit with a reported reset
snoozes until that reset; no reset is guessed. Auth failures, refusals, invalid
configuration, adapter crashes, unknown outcomes, user stop and machine silence
never enter this scheduler. Retried official sessions receive a continuation
message, not a replay of the original task. A `retry_waiting` job has durable
`checkpoint.retry` fields: retry count, reason, billing and next attempt in Unix
milliseconds. It holds no provider process or execution permit; its same-machine
lease remains fenced by the ordinary machine protocol. A silent waiting machine
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

Simplification opportunities: replace the small manual-turn admission port with
`threads.send` when the file-storage lane lands. The CLI already follows the job
stream; its SSE decoder is shared with space live subscriptions. A Codex adapter
reuses the machine, job, access, instruction and retry paths.

Protected discard intent is separate from placement delivery metadata. The owner
and exact lease authorization is committed in auth.db before queuing a command;
every delivery rechecks it. At most 1024 current discard authorizations per
account are retained, one per thread. Delivery/acknowledgement rows in machines.db
are operational state, not grants: they cannot authorize a delete by themselves.
Local machine-owner deletion remains a filesystem administration command guarded
against a running machine role; it grants no Calternal access.
