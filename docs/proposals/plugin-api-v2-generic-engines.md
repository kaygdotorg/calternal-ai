# Plugin API v2 generic engines

Proposal for sequential review. The host owns cold reads, security and
durability. Official and community packages compose the same engines.
The changes below require a numbered revision of the sealed contract.
[Owner decisions][owners] · [Sealed plan][sealed] · [Step 0 measurements][measurements]

## A. Execution without a server Turns engine

### A.1 Admission, Send and lifecycle

The manifest declares bounded composer commands and mention targets. The
host composer parses them before preflight and resolves checked attachments,
quotes, recipients and execution material. Provider contributions declare
models, reasoning options and their constraints. The host validates those
options and current placement and profile eligibility. The final-text
receipt covers the exact text and prepared inputs. Treat final text only
as content. A command or mention that changes prepared material requires
a fresh preview before Send. The package declares the send policy and a
bounded job intent. The host commits the message and intent with
the expected live head, reserves quota, and admits by receipt under the same
lifecycle fence. The cold commit projects Starting immediately, before worker
prioritization or machine placement. A scoped exclusive key serializes runs
per thread. Cold Stop fences the intent or the exact admitted attempt.
An unresolved intent blocks sheet-free archive. Confirmed archive durably
refuses that intent, including on replay after unarchive.
Preserve archive and Undo ordering without a plugin callback. Claim, park,
checkpoint and cancel reuse generic jobs. Reservations, parked decisions and
durable timers survive restart and use no worker while parked. Issue fresh
invocation authority at claim or resume. [Sealed §§4.3–4.4 and §8][sealed] ·
[Current admission][admission]

### A.2 Placement, agent authority and credentials

The host starts, inspects, controls, stops and reconciles executions using an
approved image digest, entry point and eligible placement. The actor supplies
authority and the sponsor supplies capacity. Admission reserves execution,
memory, spool and workspace budgets. The package can request an eligible
machine or placement class. The host binds the exact placement to the attempt.
The space has a small plugin VM, an on-demand heavy plugin VM and an on-demand
agent VM for rootless containers. Use the decided self-hosted wall where KVM
is unavailable. Userdata stays behind the broker in every placement.

Replace the operation-prefix scope in `turns/access.rs` with a distinct agent
principal per attempt. Grant only host-issued item refs, purpose-specific
operations and bounded effects. The interim refs-only rule (Q1) refuses
list, search, count, enumeration and bulk reads in all spaces. This removes
the person's space discovery that ships today in Full Access and
Approve-for-me, including discovery through the agent's CLI. Agent
principals cannot enqueue or delegate work that obtains discovery rights. Only independently admitted plugin jobs
can acquire indexer capabilities. A scratch disk or provider profile is never
a route to userdata. [Current Turns access][access]

Execution leases are separate from sealed 60-second invocation tickets.
Renew only against the live session or scheduled principal, grants, package
generation, eligible placement and exact attempt. Recheck authority at broker
effects and output egress. Revocation fences both immediately and stops
renewal. At most 5 s after the last authenticated renewal, a machine-local
deadline freezes the container and all its tool cgroups. Block its network
at the local boundary, including queued egress, so neither tools nor
transport can make progress. Kill at the 10-minute recovery deadline or
immediately on an authenticated host stop order. Thaw only after fresh
same-execution proof and renewal under current authority. Verify the
freezer and network fence on every enabled placement. Discard only under the
exact workspace lease. Retain an explicitly preserved workspace under its
quota and normal cleanup policy.

Pin a provider profile to its authorized account or space and a qualified
harness digest. Existing personal provider homes stay private to that harness.
Shared credential-isolated runs use the dedicated harness and separate tool
sandbox. A workload image gets neither provider-home mounts nor credentials.
The secret broker redeems opaque handles under current principal, intent,
attempt and destination. The pinned harness owns any machine-local login,
provider session or brokered provider connection. Enforce the installed
network declaration at the container and broker boundaries, including
`network: "none"`. Provider transport requires an eligible declared route.
Any package may qualify a harness through the same automated conformance
suite. Pin the tested digest and allow only the tested profile, sandbox
and network combinations. A labelled unqualified harness may run only
on the person's own enrolled machine with a personal profile. It gets
neither a shared/hosted credential profile nor the qualified host display
and approval path.
[Manual turns][turns] · [Provider protocol][provider] ·
[Claude Code][claude] · [Codex][codex]

`execution_input` sends bounded, receipt-bound steer input to the exact
live attempt. `execution_spawn_child` checks the same parent attempt,
actor, remaining budget and target eligibility. Its item refs, operations
and network rights are subsets of the parent's. The host commits the
parent edge for every conversation kind under the admission fence.
Revoke or expiry fences descendant authority. Protected answers use the
Inbox route.

### A.3 Canonical conversation files and same-execution reattach

Persist machine, boot, execution and attempt identifiers and the fence before
dispatch. Replace the in-memory ACK and unknown-exec adoption in
`machines/sessions.rs::output`. ACK only a contiguous private spool position
after durable commit. Accept restart reattachment only with fresh authenticated
proof of the same machine boot, surviving container (frozen or executing),
execution, attempt and current fence, plus continuity from the committed
position. Renew before thawing. If proof or renewal fails, fence the old
attempt, preserve the committed transcript, and require explicit
Resume by a currently authorized person. A saved provider session is useful
for Resume and provides no live-execution proof. Reconcile an uncertain
external effect before retry. The package can use durable timers to retry
a provider failure whose effect is known safe. [Machine output path][output]

The adapter inside the agent container emits the common display envelope
based on `provider-events-v1`. The host validates it, checks quote and
attachment refs and scrubs issued credentials across chunks. The host
writes the sanitized records to a self-contained, generic Markdown
conversation file (interim, Q3). This file is the canonical message and
transcript record. Use the content journal to stage the file tail and atomically
record the committed byte boundary, stable event IDs and the consumed-input
cursor by receipt. Recover a staged tail once or discard it before retry. The durable file and receipt precede
public delivery. Pages, event logs and the message search index are
rebuildable projections with stable IDs from the file. Cursor-only
checkpoints cover input with no display event. Reclaim spooled frames
through the committed cursor.

The orchestrator parks on `execution` wait for phase change, protected
approval request or terminal state. A durable receipt delivers the
transition once to a bounded continuation. Tokens, cold pages and
ordinary progress do not wake a plugin VM. Private provider frames,
session IDs and checkpoints stay in attempt-scoped custody outside
display, shared logs, search, exports and ordinary backups. Extensions
can stay private to adapters. Every provider uses the same qualified
public envelope. [Provider event schema][provider-events]

Cold page, follow and resume check the current audience at egress and release
the reader before waiting on transport. Measure first with a 16 MiB or 4,096
unconsumed-record cap per attempt and 128 MiB per space. Apply credit backpressure and stop as interrupted after 30 s of stalled input or drain on a live link.
During restart recovery the separate frozen-attempt deadline applies. Retire
unconsumed private frames after a 10-minute recovery window. Bound display
retention independently by age, events and bytes under the item retention
policy. Reject admission without a finite configured allowance. [Transcript spike][measurements]

### A.4 Protected requests

Use the host Inbox. A live eligible person resolves a checked request bound
to its actor, current attempt, target, expiry and verified action digest.
Recheck these at decision and at one-time consumption. A durable broker action
can wake one parked job via receipt. A native request is tied to the exact
live process and native request ID. Exit, replacement or revocation invalidates
it. Codex's current `can_approve: false` requests and Claude denial summaries
allow dismissal only. An actionable approval requires executable material the
host can verify. Guest Prompt stays disabled pending the sealed proof.
[Provider protocol][provider] · [Sealed §5][sealed]

### A.5 Ownership and cutover

| Current paths | Transfer |
| --- | --- |
| Turns admission, launch, lease, control and progress | Host retains lifecycle and receipt fences, admission, stop, cleanup, paging, egress authority and durable machine/stream transport. Packages own prompt construction, adapter calls, phase labels and retry policy. |
| Turns phases, retry, tool steps, instructions and thread presentation | The orchestrator owns provider/model option policy, title and instruction assembly, tool-step presentation and automation inputs. The host retains the common conversation file format and its cold decoder under Q3. Use declared inputs and committed projections. |
| Notes tags, links and live saves | Notes declares extractors and owns link-rewrite policy. The host runs the generic extractors and keeps room concurrency, save and lifecycle/Undo commits and checked index publication. |
| Turns access, redact, verify and readiness | Host keeps principal and effect enforcement, placement verification and confidential custody. Adapters own provider-specific probes, login behavior and readiness interpretation. |

First walk a minimal independent runner through the declared catalogue,
then qualify the shared engines with it and both shipped adapters.
Integrate Notes on generic item and derived-file surfaces. For Threads,
stop new old-engine admissions and wait until there are zero active
attempts. Leave saved sessions resumable and preserve their bindings by
account, provider, conversation, machine and profile. Preserve workspace
custody, grants, pending intents and receipts. Leave canonical thread files
untouched. Rebuild projections from those files and diff every thread
rendered through both paths, including text, order, tool/approval display
and attachment refs. Switch the single writer only on equality. Verify
that both releases can read new canonical commits exactly once before
enabling these writes. Abort a failing cutover before switching. A rollback
rebuilds projections from the same files and reopens under the existing
compatibility gate. [Sealed §8][sealed]

### A.6 Conversation capability and sealed replacements

The orchestrator declares `threads` as its own package kind with
`conversation: true` (interim, Q6). Any kind may opt in. The capability
gets a durable transcript, the message corpus and an authority-checked
`active_execution` ref. Inbox decisions project `waiting_on` under
`viewer_self`. The item lifecycle supplies archived state and actor-scoped
snooze, plus generic parent edges with scoped child windows. These are
host engines any qualifying kind can use.

| Sealed surface | Replacement |
| --- | --- |
| Reserved `turns.`, manifest `engine: turns` and Turns-owned `threads` and the `turns.*` supersession entry | Drop the prefix and manifest discriminator. Use package-owned conversation kinds, pinned provider contributions and generic execution. |
| `turns.create`, `turns.start`, `turn_start` | Manifest-enabled generic cold create, host-prepared composer dispatch to the package's declared Send, and `execution_start` for the admitted job. |
| `turns.archive`, `turns.unarchive`, `turns.snooze` | `host.item.archive`, `host.item.unarchive`, `host.item.snooze` under the item lifecycle fence with the existing Undo eligibility. |
| `host.turn.stop`, `turn.stop` wording, `turn_cancel`, `turn_status`, `turns.follow_up`, `turn_follow_up`, `turn_answer` | `host.execution.stop` and `execution_stop`, `execution_status`, `execution_input` and protected Inbox response. Add `execution_spawn_child` and the `execution` wait. |
| `ThreadRef`, `TurnRef`, `eligible_turn_actor`, `active_turn` | Checked kind `ItemRef` and `ExecutionRef`, `eligible_execution_actor`, and `active_execution`. Apply to layout entries, context operands, composer and stop. An item grant conveys no execution control. |
| Turns-owned `provider`, `state`, `waiting_on`, `snoozed_until`, `archived`, `parent`; `ProviderId.known_values` | Declared provider and state fields, host Inbox/lifecycle/parent projections. `ProviderId` is open and resolves installed contribution IDs and marks. Remove catalogue-specific known-value seeds. |
| `query.search.turns`, `TurnsSearchFilter`, Turns message index, `TurnTranscript`, `transcript.page`, `TranscriptPageRequest.thread` and composer `thread` operands | Conversation title/message corpus using scoped search qualification and a generic field filter. Rename `TurnTranscript` to `ConversationTranscript`. Keep `transcript.page` and `composer.options` with checked conversation item operands and common committed events. |

## B. Derived files and private scratch

The host serves lists, counts, search and sharing from authorized items while
the space sleeps. A manifest may opt into cold empty-item creation from
declared defaults. Further initialization consumes its committed change.
Private per-package scratch replaces the proposed first-release host table
store.

### B.1 Device snapshots, cold previews and extractors

Declare each derived slot's kind, producer, media type and quota. Any
editor whose local state equals the committed note revision may obtain a
fresh grant bound to their authenticated session, item, revision and slot.
The host checks the committed state digest, including the exact Yjs
checkpoint for a room. The editing device paints its own capture
immediately as a local preview. Capture and upload do not delay save ACK.
After ACK, stage the still image, validate type and dimensions, re-encode,
strip metadata and make the file durable. At publication recheck session,
current edit rights and exact head. The first valid capture wins for
that revision. Replay the same receipt only with the same digest and
session. A copied grant, different bytes or a late capture for an old
head cannot replace it.

A member with edit rights can submit misleading pixels for their note.
The host verifies revision binding and attribution, and the pixels remain
editor-supplied. Attribute the image to its uploader. Keep the last
authorized image marked stale until the next valid image commits. Recheck
both current item authority and the old revision/dependency grants at
each asset delivery or cache revalidation. Purge, redaction or audience
loss can remove the stale image immediately. A revoked image never
returns bytes or a `304`. Guest and public images are session-only.

One declarative, bounded extractor facility runs inside the generic save
and indexing path. Manifests choose typed frontmatter keys for indexed
fields or tag sets and first-block selection for summaries. For Notes,
skip the title block and cap the summary at three blocks and 256 text
bytes. Compute it from the committed structured body on every writer
path, including rooms, device, agent and plugin edits. A cold create
supplies empty values. No worker wake, app-specific host decoder or
background job is needed for these list fields.

Extend `RowTemplate` and the sealed §2.4 row rule with a bound portrait
image and second line (date and snippet for Notes). If there is no
authorized image, use the latest authorized summary for a miniature.
An old item without that projection or an empty note uses a blank
miniature. In revision 13.2 only permitted non-body display fields may
enter `MemberRowCacheRecord`. Body snippets and block summaries arrive
after fresh authorization. Restoring them offline is an owner option in
§C. Read and miss paths wake zero VMs. [Description contract][description] ·
[Revision 13.2 member cache][rev132]

### B.2 Original files, bulk reads and derived media

Use a generic resumable device upload for original file items on the HDD
tier, available while the space sleeps. A checked begin reserves space and
binds a session, item or creation target, expected head, length and digest.
Idempotent chunk receipts advance only the durable contiguous offset.
Resume rechecks live authority and the upload identity. Commit verifies
the digest and head, makes file placement and item publication recoverable,
and records one receipt. Cancel, expiry or revoke fences later commits.
For spikes, use 4 MiB chunks, a declared file ceiling of at least 200 MiB,
four concurrent uploads and at most 1 GiB staged per space. Bound abandoned
staging with a one-hour expiry and meter actual bytes separately from
metadata batches.

An admitted indexer in the heavy VM opens a bounded stream of item refs
through the broker. Authorize once per file open, pin its revision, and
carry bounded chunks on the existing credit-controlled space channel.
Use at most four concurrent file streams and 4 MiB in-flight credit per
stream for initial qualification. Check the live file/consent fence at
egress. Revoke, purge or cancel cuts the stream and releases the pin. No
userdata mount or repeated 64 KiB range RPC is needed. Rate-limit bulk I/O
ahead of the worker so foreground reads retain their budget.
The locked perf-01 rusqlite probe completed 30,600 reads with one authority
lookup per file. Four streams delivered 549–558 MB/s on the 4 MiB and
200 MiB corpora. That is fixture evidence for the initial stream limit,
with no deployment throughput guarantee. The Photos spike also measures
real import, derived work and foreground interference.
@thread:thr_tth3axchm4#msg=1857 · [Runtime measurements issue][bulk-read]

Photo thumbnails may provisionally come from the device at import or
from a plugin job (Q5), using the same derived slot and authority checks.
Only a current, committed original is eligible to publish a thumbnail.

The existing change outbox drives a durable subscriber by receipt. Coalesce work to the
latest desired revision and recheck dependencies and producer rights at
publication. Save ACK waits for neither job delivery nor asset work.
Keep pointers, indexes and bounded hot previews in SSD appdata, with bulk
media under the host-managed HDD tier. Support the decided one-disk
installation. Back up canonical files and items under their ordinary policy.
Treat missing staged or evicted derivatives as misses and reclaim in bounded
batches. Purge and revoke deny delivery before cleanup.

For qualification, cap a derived preview at 512 KiB encoded and 256 × 384
decoded pixels. Allow at most two active decodes, 64 pending grants and 8 MiB
aggregate staging per space. Reserve before ingest, expire grants and
abandoned staging within five minutes, and refuse new staging at the cap.
Other asset types also require finite declared and host-capped resource
limits. Cross a crash cut only after durable bytes and a fenced pointer
commit. Eviction switches off the pointer before reclaiming bytes.

### B.3 Private scratch isolation and custody

Provision one size-capped appdata disk per package and space. Declared
background jobs in the heavy plugin VM alone can use it. Bind the exclusive mount
lease to space UUID, installed package and producer generation, source scope
and exact job attempt. Run each lease in a dedicated OS identity and mount
namespace with no shared memory or handles with interactive workers. On
job exit or revoke, fence the lease, terminate the old process and detach
before reuse. After a crash or restore, prove the old attachment fenced.
Scratch and scratch-job memory stay outside both backup and VM snapshots.
Agent VMs and personally enrolled machines have no scratch mount. Guests or
item-share activations cannot invoke or indirectly enqueue scratch work.

Enforce the size quota at the volume, including auxiliary files. Fence jobs
and wipe the disk on uninstall. Disk exhaustion stops scratch work without
blocking canonical writes. Rebuild only from brokered files and committed
items. Originals, user edits and durable job checkpoints stay in ordinary
host custody. A package must recover when conformance deletes its disk.
Reuse scratch only under the same live source scope and rebuild after
grant shrink. The interim indexer principal is the job creator (Q2), with
sealed rechecks, stop and Manager takeover. Expensive item-local outputs
such as embeddings are package-private derived files keyed by item revision,
producer/model digest and output name. They are outside backups and display
routes. A scratch rebuild reloads still-authorized current artifacts and
recomputes misses. Revoke, purge and source changes fence access before
cleanup. Approved personal-machine jobs can deposit those artifacts
through the same fenced broker upload and never mount the scratch disk.
Pin recognition model weights as package assets or retrieve them from an
approved declared destination by digest. Qualify the heavy-VM CPU path
without assuming a GPU. Renderers, item shares, guests,
other packages and agents have no disk, row or query access. They receive
only ordinary authorized item fields and derived files.

### B.4 Publication audiences, machine consent and similarity

Publish source-local fields by default. A photo's recognized labels go onto
that photo and are filtered by ordinary Binds. Multi-item outputs require
host-checked source refs, expected revisions and a destination audience
contained in every input audience. Apply that check to identifiers, scores,
order, counts and even the existence of a result. Dependency change or
revocation fences stale joins, indexes, versions and counts before delivery.
“Photos like this” runs as a background ML job in the heavy plugin VM.
Restrict candidates to the requester's authorized source scope before
ranking. The first-release interim result (Q4) is a requester-bound host
job status payload of up to 50 refs, with receipt retention (default
seven days). The host rechecks source and candidate refs, grant and
revision fences on every delivery. Any changed input invalidates the
result as a whole. It creates no item and makes no cross-space write. Sealed installed-package trust still
applies to deliberate copying by a trusted package. Checked provenance
protects the declared path and cannot prove the origin of arbitrary output.
[Sealed §5][sealed]

The heavy plugin VM is the default for shared originals. A personal
machine can index them after both the space's explicit decision and the
machine owner's agreement to the named machine, package, purpose and
source scope. Consent states that the operator may retain delivered bytes.
Check both revisions at assignment, on each bounded broker delivery
and at publication. Withdrawal fences streams and queued work immediately,
including on an unexpired lease. Enable this placement only after it passes
the consent-revocation spike.

### B.5 Bulk item metadata commits

Extend the content engine with a real per-space batch transaction. A loop
of today's single-item FULL commits cannot meet this contract. Stage files
if needed using the current durability primitives. Check expected heads,
source revisions, actor rights, producer generation and declared fields
at the fence. Coordinate with live rooms and refuse or defer contended
items. One FULL transaction commits the accepted heads, indexes, counts,
any derived-file pointers and the bounded outbox range and receipt.
All referenced derived bytes are durable before this commit. The batch
fingerprint covers pointer digests. Projection and recovery use the same
receipt for metadata and pointer updates. The receipt fingerprints normalized
input and records per-item committed, stale or refused outcomes. An exact
replay rechecks delivery authority and returns those outcomes without writes.
Recovery idempotently publishes the committed range, retires the outbox and
reclaims unreferenced staging. An output-only patch never requeues its
producer for the same source revision.

Add `TextSet` to `FieldType` and a typed `contains` predicate for
membership. Use normalized deduplicated elements, at most 16 per field
per item and 64 UTF-8 bytes each for qualification. Tags and recognized
labels use this path. Seek an authorized scope and one value in index
order. Qualify combined predicates before enabling them. Exact counts
count items once. Charge each element, each affected audience and sort
entry, and both removals and inserts to covering-index and batch
budgets. Refuse writes that would exceed those budgets.

Initial spike limits are at most 128 patches and 1 MiB per batch, one in
flight per job, two batch admissions per second across each space, 8 MiB
total queued or staged, and 8,192 index keys per batch within sealed scope
limits. Reserve
foreground writer capacity and yield between batches. Pause admission at a
one-second oldest unpublished age or the byte/work cap. Resume as workers
drain. Feed and search consume by commit cursor, retain the latest pending
revision per item, and coalesce only authorized changes into one notice
per affected Bind per 100 ms. Keep targeted revocations immediate. Bound
each consumer queue and recover it from the committed range.
[Sealed query and save gates][sealed]

## C. Owner questions and qualification

The defaults here are interim. They let the next revision be reviewed
without settling the questions.

1. **May an agent search or list a space?** A person might ask it to find
   and summarize their Lisbon notes without selecting them first. The
   answer changes the agent broker and the Full Access experience before
   freeze. Refs only limits prompt-injection exposure and discovery work,
   but removes space discovery from modes that ship today. Scoped search
   under current member rights is more convenient and adds disclosure
   and indexed-query budgets. Recommend opt-in per thread, allowed in
   personal spaces and off by default in shared spaces, with visible
   access records. Interim: refs only everywhere.
2. **Whose rights may an indexer use?** If the person who started a photo
   index leaves, the creator-bound job stops. Choose the principal now
   because it controls admission, eligibility and retained artifacts.
   Creator rights limit exposure but interrupt work, require takeover
   and can incur rebuild I/O. Installation rights keep coverage and
   throughput stable, with a standing read grant and broader exposure
   if the package errs. Recommend a revocable Owner-approved grant
   shown at install and ending on uninstall. Interim: sealed creator
   authority.
3. **Who writes a conversation's Markdown file?** Two runners might
   exchange conversations. The writer choice determines the cold-read
   contract and whether each run needs an awake plugin. The host can
   validate the common events and write one portable format, keeping
   cold reads and the plugin VM off the token path. It owns format
   maintenance and imposes a common display envelope. A package writer
   has more format freedom, needs its own trusted file-to-view boundary,
   and adds wake and processing costs unless it supplies an equally
   cold reader. Interim and recommendation: host writer with the common
   generic transcript format.
4. **How long should “photos like this” results be kept?** Ana runs it on
   a shared trip album and wants to reopen the matches next month. The
   answer determines whether the contract needs a cross-space write.
   A requester-bound seven-day status result has bounded storage and
   delivery checks, but expires. A personal item lasts and can be
   organized, at the cost of a new checked cross-space write,
   dependency invalidation, and retained index/storage work. Interim
   and recommendation: ephemeral status results under normal receipt
   retention.
5. **May a device make photo thumbnails during import?** A phone can
   send a small image with an original and show the first results before
   a heavy job reads the library. This determines which producers the
   upload grant permits. A device path adds bandwidth and untrusted
   pixels, with the same attribution and checks as note snapshots. A
   plugin-only path has producer-controlled pixels and follows the
   2026-11-19 jobs-only derived-data decision, with more host I/O and
   latency for large imports. Interim and recommendation for
   qualification: allow both through one slot, with plugin regeneration.
   The owner must approve that extension before enabling device imports
   of thumbnails.
6. **One Threads list for every runner, or separate lists?** For example,
   should an Aider runner's conversations appear beside Claude Code
   conversations? The answer fixes list ownership and discovery before
   catalogue freeze. A shared list makes browsing easier but requires
   a common schema, cross-kind audience checks and a qualified merged
   index. Separate lists give runners more control and natively scoped
   reads, with extra navigation and cross-kind Inbox/search work for
   the person. Interim and recommendation: the generic conversation
   capability is available to any kind. The orchestrator owns its
   Threads list, and providers can contribute to any eligible runner.

**Separate cache option for the owner.** Should a device retain a
body-derived snippet or miniature summary so Notes can paint it offline
before a recheck? A person reopening a notebook on a flight would see the
full row earlier, but a former member's device could retain body text
after revocation, and caches would grow. Keeping revision 13.2 gives
faster, smaller title-only cache reads and defers body previews to fresh
authority. Interim and recommendation: preserve revision 13.2. Qualify
any expansion as a separate owner-approved change.

### C.1 Qualification gates

Use sealed §8's machines, repetitions and instrumentation. New numbers
are proposed qualification targets. Start with the independent-runner
paper exercise, then qualify device snapshots and same-execution recovery.
Run the three Photos arms separately. First-invite parity can qualify
Threads and Notes before enabling Photos consumers.

| Spike | Pass condition that can falsify the proposal |
| --- | --- |
| Independent runner, inbox and cutover | From a package-owned conversation kind, use provider options, Send, stream, ask, steer, spawn child, Stop, archive/Undo and snooze through generic routes. Both shipped adapters and an independent adapter must pass the same digest-pinned conformance. Deny cross-attempt controls, delegated discovery, credential/network access and stale approvals. Run a tool call and a permission-changing composer command across preflight and replay. Cut over with zero active executions. Require exact old/new transcript render equality for every file, unchanged historical hashes, and exactly-once post-cutover rendering across rollback. |
| Sleeping-space Send | Over the sealed phone link, send an already prepared message to a sleeping space. Require one exchange and p95 ≤500 ms from user intent to the renderer painting durable Starting. Require p95 ≤8 s from intent to the admitted container being ready on a single wake and ≤15 s for the 20-way arm. Report first-token time separately and verify zero duplicate receipts or runs. |
| Same-execution recovery | Cut before private spool commit, after commit before ACK and after ACK before file commit. Restart with 30 s and 5 min downtime, stall the transcript reader and partition control. Freeze and block all agent/tool/network progress ≤5 s from last renewal. A matching attempt thaws only after fresh proof and renewal and resumes without lost or duplicate events. Invalid proof requires explicit Resume. Beyond the 10-minute recovery window the process is gone. Host Stop kills the exact attempt without waiting for expiry. Check zero effects or egress after revocation, bounded unconsumed spool, no plugin wake on tokens, and the sealed 150 ms first-screen and 500 ms independent-control p95 gates. |
| Device snapshot | On a sleeping space require save intent to durable ACK p95 ≤40 ms, local paint p95 ≤100 ms from capture availability, and ACK to current painted preview p95 ≤2 s on a second currently authorized viewer. Race a merged room edit and an agent edit with two editors' captures. Keep the previous authorized image marked stale until replacement. Reject out-of-head uploads, copied grants and changed digests. Cut around file durability, pointer commit and eviction. Enforce staging caps, zero revoked bytes or stale-authorization `304`, clean miniature fallback and zero preview-driven wakes. |
| Scratch delete and rebuild | Delete at rest and mid-job, fill the volume, restart the heavy VM, restore a runtime snapshot and follow warm privileged work with a guest activation. Require at most one live mount, zero leaked data or guest-triggered scratch work, exclusion from backup/snapshot and wipe on uninstall. Rebuild identical published item fields on a static fixture. Reload all still-current embedding artifacts without recomputing them. Inject a source edit and assert only its old artifact is unusable. |
| Photos originals and thumbnails | Import 10,000 mixed-visibility 4 MiB originals on an otherwise idle 200 Mbit/s fixture link, then repeat while driving taps. Require total import ≤35 min and identical byte digests, zero import wakes before derivative work, and first confirmed thumbnail ≤5 s after first original commit on the device arm and ≤15 s on the heavy-job arm. Resume after a chunk/ACK cut. Revoke an upload and each machine consent mid-stream; assert zero subsequent deliveries or commits under the revoked grant. Keep sealed foreground gates. |
| Photos labels | On the same corpus, publish checked labels and thumbnail pointers while driving taps, saves and reads. Compare label-filtered windows and exact counts to an independent visibility oracle, including an edit to a hidden item. Hit the write, key and queue caps, cut around batch commit and outbox retirement, and assert exact recovery and no unauthorized results. Require sealed foreground gates, steady-load publication p95 ≤2 s and oldest unpublished age ≤5 s. Record reads, commits, flushes and queue depth. |
| Photos similarity | On the same corpus run 1,000 qualifying requests against pinned heavy-VM CPU and model fixtures. Compare all ordered IDs and scores to an independent authorized oracle and require p95 ≤5 s on warmed embeddings. Insert or edit a hidden photo and verify identical eligible results. Revoke a source during ranking and before status delivery. Assert no stale or foreign refs, zero cross-space writes and no replay visibility after receipt expiry. |

### C.2 Numbered contract revision

Revise the manifest, description schema, catalogue, verifier and fixtures
together for the §A.6 replacements, extractor and contribution declarations,
three runtime roles, common execution/events, generic cold create, scratch,
resumable uploads and bulk reads, derived slots, image and second-line row
fields, `TextSet` and checked batch publication. Remove the planned
first-release table store. Carry forward the default all-field search
with optional field filter. Qualify every new route against the sealed
authority matrix before enabling it.
[Sealed plan][sealed] · [Combined-search measurement][combined-search]

## Simplification opportunities

Reuse the protected Inbox, item lifecycle fence and change cursor. Keep
ordinary labels source-local. Return similarity through existing job status.

## First review disposition

| Finding | Where addressed |
| --- | --- |
| 1. Agent enumeration | §A.2 per-attempt principal and capabilities |
| 2. Run lifetime | §§A.2, C machine deadline and partition gate |
| 3. Credentials and network | §§A.2, C pinned harness and adapter matrix |
| 4. Protected requests | §§A.4, C broker vs. live native requests |
| 5. Durable machine ACK | §§A.3, C identity and spool cuts |
| 6. Private spool and display | §§A.3, C custody, envelopes and bounds |
| 7. Send, archive and Stop | §§A.1, C lifecycle fence and receipts |
| 8. Cutover custody | §§A.5, C preserved bindings and release pair |
| 9. Server ownership | §A.5 host and package mapping |
| 10. Upload identity | §§B.1, C save-session grant and exact replay |
| 11. Cold miniature and media | §§B.1, C.2 row schema and egress |
| 12. Scratch isolation | §§B.3, C exclusive, isolated leases |
| 13. Publication audiences | §§B.4, C source checks and owner question |
| 14. Batch feasibility and flood | §§B.5, C transaction, caps and cuts |
| 15. Machine consent | §§B.4, C per-delivery and publication fences |
| 16. Bounds and qualification | §§B.2, C resource caps and observable gates |
| 17. Contract revision | §C.2 coordinated schema and conformance changes |

## Second review disposition

| Finding | Where addressed |
| --- | --- |
| 1. Restart window | §§A.2–A.3, C.1 freeze, fresh thaw and downtime arms |
| 2. Ordinary conversation kinds | §§A.6, C.2 generic capability and sealed-name replacements |
| 3. Canonical file | §§A.3, A.5, C (Q3) host file writer and projections |
| 4. Token path and wait | §§A.3, C.1 adapter output and execution wait |
| 5. Composer commands | §A.1 declared preflight parsing |
| 6. Agent discovery | §§A.2, C (Q1) labelled interim and parity cost |
| 7. Steer and child | §§A.2, A.6 exact parent and subset authority |
| 8. Harness qualification | §§A.2, C.1 shared suite and personal fallback |
| 9. Snapshot continuity | §§B.1, C.1 local paint, matching room state and stale image |
| 10. Declarative extraction | §§A.5, B.1 one cold-save facility |
| 11. Tags and labels | §§B.5, C.2 `TextSet`, `contains` and fan-out |
| 12. Original file I/O | §§B.2, B.5, C.1 resumable upload, bulk stream and atomic pointers |
| 13. Similarity custody | §§B.4, C (Q4) bounded status result |
| 14. Indexer authority and rebuild | §§B.3, C (Q2), C.1 creator authority and private artifacts |
| 15. Scratch placement | §§B.3, C.1 heavy VM only |
| 16. Spool caps | §A.3 unconsumed records |
| 17. Member row cache | §§B.1, C cache option and revision 13.2 restriction |
| 18. Reversible cutover | §§A.5, C.1 idle cutover and full transcript diff |
| 19. Spike pass conditions | §C.1 timed Send and distinct Photos arms, scratch and preview checks |

[owners]: https://git.kayg.org/kayg/calternal-ai/src/commit/85add29af840b225dd9a83cae2e5cd4ce3aecfc9/docs/open-questions.md
[sealed]: ../plugin-api-v2.md
[description]: ../description-contract.md
[measurements]: ../measurements/plugin-api-v2-step0.md
[combined-search]: ../measurements/plugin-api-v2-step0.md#follow-up--search-across-all-searchable-fields
[admission]: https://git.kayg.org/kayg/calternal-ai/src/commit/85add29af840b225dd9a83cae2e5cd4ce3aecfc9/crates/server/src/turns/admission.rs
[access]: ../../crates/server/src/turns/access.rs
[output]: ../../crates/server/src/machines/sessions.rs
[turns]: ../agent-turns.md
[provider]: ../provider-protocol.md
[claude]: ../../plugins/calternal-claude-code/README.md
[codex]: ../../plugins/calternal-codex/README.md
[provider-events]: ../../contract/provider-events-v1.schema.json
[rev132]: https://git.kayg.org/kayg/calternal-ai/src/commit/539c10b68f78625c16faad5a8762523b45bfd7b6/docs/plugin-api-v2.md#42-live-work-cache-and-host-components
[bulk-read]: https://git.kayg.org/kayg/calternal-ai/issues/77
