# Plugin API v2 · revision 10

The first contract covers host-drawn apps, generic items, durable jobs and
Turns. Freeze after the §8 spikes. Fixture bounds are test settings. See the
[Owner’s interface, runtime and sharing decisions](open-questions.md#recently-decided-2026-11-17-now-in-the-decision-records).

## Scope and supersession

| Earlier record | Provisions superseded for v2 |
| --- | --- |
| [Plugin API](plugin-api.md) | **Decided (2026-11-17)** app order, interface regions, manifest, placement, storage and network bullets. **Provider and risk contract (step 1)** operation declaration and activation. **Authoring and interface prototype** renderer/slot publication and worker activation. **Page a plugin’s own kinds** placement and cold calls. **Palette discovery** delivery of contributions. **Sealed plugin content** permissions and destination policy. **Durable job waits** and **Signed webhook delivery** invocation authority and wake rules. **Open** cloud sandbox question. |
| [Interface proposal](plugin-interface-proposal.md) | **What a plugin can contribute**, **App switcher entries**, **Sealed components**, **Stateful generated views**, **The shell decision**, **Plugin worker placement and network access** (including **Brokered network capabilities**), **Swift clients**, **Notes and Threads as ordinary plugins**, and **Can official plugins use the same contract?** as statements of the v2 interface, placement, dispatch and migration order. |
| [Plugin workers](plugin-workers.md) | Preamble through edited revision readiness and **Placement and unresolved contracts** as statements of the v2 runtime. **Development and tests** still describes the current container adapter. |

Today the server has [per-package workers](../crates/server/src/plugins/worker/transport.rs)
and [broker adapters](../crates/server/src/plugins/broker.rs), container
isolation for no-KVM, durable jobs/waits, revisions, receipts, diagnostics,
provider adapters, Notes/collection sharing, anonymous media and the trial
kit. Reuse them. Add the space supervisor, commit fences, bindings and SDK.
Mail has a data layer and fixture UI. Real mailbox access waits for reviewed
mail ingestion, credential custody and the host table engine.

## 1. Package and install

[Manifest v2](../contract/plugin-manifest-v2.schema.json) has apps with one pinned
AppDescription (shared Bind list, chrome and content), kinds with metadata/index declarations and an
optional live-room profile, operations with input/output schemas, capability,
risk, idempotency and optional item target, jobs/waits, schedule eligibility,
settings, secrets and typed credential routes. Before wake, resolve
`target: {kind, field}` from the required top-level item ref. A bulk
operation uses `{kind, selection_field}` with a checked host selection ref.
Operations name a title. The host supplies its own effect and target
labels and may require a preview in addition to
the operation’s labeled `material_inputs` keyed by input field. An operation that
declares `creates` can expose only a ref that the owning host engine
recorded in the committed receipt and the viewer may currently open.
Turns is the one named engine an app can select. Provider contributions
pin a Turns adapter descriptor. Claude Code and Codex use the existing
[provider protocol](../contract/provider-v1.schema.json) and
[events schema](../contract/provider-events-v1.schema.json). The host chooses
and launches the adapter inside the agent sandbox, outside the plugin wall
and without machine-control authority. Provider-only and static packages
can omit a broker worker. Operations, jobs, schedule eligibility and
credential routes require one.

Reject unknown or inconsistent declarations before launch. Reserve `host.` and `turns.` for the host catalogue. Check unique IDs,
package operation namespace, references, schema compilation offline, pinned
assets and digests, indexed field types, and cross-reference constraints.
Each operation lists its host methods once. Derive the install summary by
union and constrain each call by that operation’s capability and risk.
Derive the mutation flag from risk. Reads use `read_only` and cannot mutate
user data. A job may checkpoint its own run as part of the durable job
protocol. Receipt-backed mutations retry with the same request ID. Never
automatically retry a non-idempotent external action.

Install shows kinds, declared Bind reads, methods, capabilities, risks,
settings/secrets, providers and outbound disclosures. Gallery installs take one tap. Source
installs warn. Pin the digest. Members see packages, grants and destinations. Check and probe agent-authored code
in the space without broker authority, then enable the exact digest and
grant the Owner confirmed. Verify updates before fencing/draining the old
revision. Permission widening needs the Owner except for the network
installation approvals below. Resuming an old job under a
new digest requires compatible migration and current grants. Use the
[upgrade runner](upgrades.md). Disable/remove fences triggers, tickets and
scoped views, stops the package and preserves host-owned items and share
fallbacks.

## 2. Space runtime

One isolated long-lived runtime per space, one process per enabled package
inside it, each with its own OS permissions, read-only code and resource
accounting. Official and community packages use the same API and limits.
The trusted supervisor can impersonate any package using tickets it can see
in that space. This is the accepted blast radius. It gets a revocable,
single-space lease. Machine enrollment, placement authority, the host broker
and provider logins stay outside the wall. A plugin cannot open another
process’s files or pipes, read host databases or credentials, select a space
root or use raw network sockets.

| State | Observable transition |
| --- | --- |
| Asleep | Serve authorized engine reads and saved templates. An admitted logic call, runnable job or due schedule can wake. A signed wait webhook wakes only via a winning signal that makes a job runnable. Cache misses, page reads, public links and rejected requests do not wake. |
| Waking | Coalesce wakes into one placement lease. Reserve capacity first. Recheck queued authority before first dispatch. Late startup after cancellation discards the old work. |
| Awake | Reuse processes between calls. A tap gets a result or `pending` within three seconds of plugin time after a separate wake allowance. A pending call keeps its original ticket, request receipt and status ref. It may continue until that ticket expires. Long work uses a durable job with progress. A package crash retires that process and its tickets. Other packages continue. |
| Draining → going to sleep | Stop admissions to the current epoch, drain within the placement bound, cancel/fence the remainder and retain committed engine effects and checkpoints. Close I/O, stop with acknowledged teardown or durable placement revocation, then sleep. A racing wake coalesces after shutdown. |

Nothing in memory is required to survive sleep. First release proves clean
boot only. A later snapshot restore must consume one host-side,
single-use runtime epoch bound to space, wall image, supervisor and package
digests. Restore cannot reuse inherited tickets, handles, descriptors,
credits or random state. On every wake, before any plugin code runs,
initialize the guest kernel and in-process random generators from fresh host
entropy. On snapshot wake explicitly reseed both layers, including every
restored process’s crypto RNG. Reinitialize a process with fresh entropy
if it cannot reseed. Refuse admission unless kernel and process randomness
are fresh before plugin code runs. Count all of this inside the wake
allowance. The wall probe observed the same Node crypto bytes after independent restores even
with a kernel reseed. With both layers reseeded, its guest stage including
the sampling draws measured about 180 ms at p50 and 267 ms at p95.
[Snapshot randomness measurement](https://git.kayg.org/kayg/calternal-ai/src/commit/2562b95e8de9ce6749adeae4afd8cb7dc08353c2/docs/measurements/space-runtime-walls.md#snapshot-randomness).

Host/runtime crash invalidates the old epoch. Recover uncertain
work by receipt or checkpoint, and return a safe interruption for work
that cannot be replayed.

The Owner’s plan buys compute and apps and sets space quotas. Exact mapping
is pending. Bound and charge per space and package before decode (including
malformed frames), DNS, launch or snapshot I/O. Account for RSS, CPU,
processes, queued bytes, calls, traffic and parked work. Reserve teardown
capacity until confirmed. Bound shared queues and use fair dispatch so a
saturated space cannot starve a second. Suggested spike limits are 64 queued
and 32 active calls per space, 16 queued and 8 active per package, 128 callbacks
per invocation. Admissions beyond quota get `429` with a retry hint.

**Pending measurement — `thr_8t85rv4yx8`** is measuring hosted microVM wake time and
awake/idle/snapshot memory, agent containers inside the space with provider
login isolation, and self-hosted KVM/no-KVM shapes. Self-hosted operators
choose the wall with the weaker option labelled. Container tests prove the
selected fixture wall only.

**Interactive cutoff.** Start the three-second response clock after
wake and admission. On cutoff issue a host-owned `status_ref` and record
a bounded diagnostic. Do not launch a second invocation on retry. The
held invocation can still finish under its original authority until
the 60-second ticket expires. Cancel or expiry fences late effects and
status presents the last authorized outcome. Durable work needing more
time must checkpoint and continue as a job.

The base full-snapshot fixture uses a **three-second wake allowance**,
separate from plugin time. Its 256 MiB image answered at 269.12 ms p95
cached and 488.74 ms evicted (100 each). With 20 simultaneous wakes it
measured 922.41 ms and 2,518.85 ms p95 (60 each). These measurements are
from the wall lane on `calternal-perf-01-backup01` under
`/usr/local/bin/perf-lock.sh`. The hosted allowance awaits measurement
with the full workload, before freeze. [Wall A measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/2562b95e/docs/measurements/space-runtime-walls.md)

## 3. Channel, authority and effects

### Carrier and framing

One bidirectional authenticated channel per awake space over the enrolled
machine carrier. Handshake agrees one major and credits, binds lease to space
and starts a new epoch. A trusted endpoint stamps the package and pinned
digest. Use strict UTF-8 NDJSON, one object per LF, with a 256 KiB encoded
object cap (fixture). Charge capacity before parsing. Bound the line buffer
and reject invalid UTF-8, duplicate keys at any level, more than 32 nested
containers, unknown fields and wrong-direction messages. A malformed child
frame fences that process. Outer corruption or disconnect closes the channel.
Check raw bytes before a parser that collapses keys.

The outer header is `{v:1, epoch, seq, package, digest, ticket, message}`.
`invoke` carries operation, checked input and deadline. `call` carries
method, input and callback ID. `reply` correlates by callback ID and returns
typed data or a safe error. `chunk` is bounded ephemeral job progress.
`complete` is the only terminal worker result, including failure or park.
`cancel` and `ready` are control messages. Credit/back-pressure control
is independent of application callback completion.

Both directions give each package byte credits and a bounded queue. Debit
encoded bytes before enqueue and return credit on receiver release. Keep a
bounded reserved control queue/credit window for cancel, grants, credit and
fencing. Dispatch callbacks independently of reads and fairly across
packages. The writer assigns consecutive direction-local `seq` at dequeue.
Control overtakes unsent data only. A blocked physical write prevents all
later traffic on that stream. A host-local cancel or revocation must fence
effects immediately without sending a frame. A write watchdog closes an
unwritable channel and fences its lease locally. The remote endpoint stops
under lease expiry even if it never receives a cancel. Reconnect mints a new
epoch, drops queued frames/credits and rejects gaps, duplicates and old
frames. Replay is a new admission with the same durable request ID where
permitted. Wire and author payload limits are independent.

The two authority counters are **runtime epoch** for the lease and **grant
revision** for permissions, access and policy. Digests identify packages.
Wire and page ordinals only order data. The `ticket` is the ephemeral invocation reference (see §4.3). Receipts use the request ID within actor/space. Subeffect keys
derive from that request and its step or checkpoint position.

### Every call and delivery

1. **Space by channel.** Resolve resources in the lease-bound space. Request
   input cannot select a different space or machine-placement authority.
2. **Package by capability.** Resolve the stamped package and pinned digest.
   Check declared and approved operation/method/capability and its effect
   ceiling. Refuse wider risk, targets or egress.
3. **Person by ticket.** The host mints after admission and stores actor,
   session or delegation, package/digest, operation, target, allowed effects,
   approval, audience, epoch, grant revision and expiry. Compare the bound
   package and live authority on every callback and before delivery. Tickets
   expire after 60 seconds or earlier on terminal, cancel or revocation.
   Persist principal, scope and delegation. Mint a fresh ticket from the
   creator’s current rights for each job run or schedule occurrence.

**Commit and recovery.** Every effect must take the per-space authority
fence at the engine’s recovery-visible commit point. Check the live grant,
expiry, invocation cancellation, runtime epoch, enrollment and current job
run under that fence. The same fence orders their invalidation. This includes
membership/session changes, uninstall, cancel, epoch loss, schedule removal
and job-run replacement. Admission, callback serialization and a preflight
grant read cannot substitute for this check.

The item commit point is the FULL content transaction that records the new
revision **and** its replayable outbox intent with the request fingerprint
and result. The later journal publication and file placement complete it.
The jobs/schedules commit point is the durable transaction recording state
and its receipt or recoverable publication intent. Use the existing
engine and [outbox publication](../crates/content/src/publication.rs).
Fence until commit or rollback, release before follow-on publication, and
perform no remote work under the fence. Pause admission for a space during
unresolved authority recovery. For changes affecting multiple spaces,
mark each affected space fenced before publishing the authority change.
Reconcile that mark and committed receipts before allowing new effects.
A marker or journal flush that does not make recovery authoritative is
insufficient.

If invalidation wins first, roll back provisional work and leave no success
receipt or visible effect. If commit wins first, keep the effect and recover
publication exactly once even if the response was lost or the actor later
loses access. Recheck live access before replying or replaying that receipt.
Cleanup never deletes committed data. For external effects the fenced point
is admission to actual DNS/send/redirect dispatch, with a durable dispatch
claim for a mutation. Close affected streams on invalidation. Bytes already
dispatched cannot be recalled. When delivery is uncertain, report it as
such and replay only against an explicitly idempotent endpoint.

A schedule is a separate revocable enrollment of creator, package digest,
operation, target/payload, allowed effects, approval mode and IANA time zone.
Reuse [automation calendars](../crates/core/src/automation_schedule.rs).
The catalogue’s `ScheduleCreateInput` accepts a one-date `{type:"once", at}`
or recurring `{type:"recurring", expression}` calendar and an IANA time
zone. Recurring uses the canonical OnCalendar subset for daily, weekdays,
weekly or monthly at a single local time. Reject arbitrary timer or shell
expressions. Reuse the scheduler’s civil-time gap/fold rules.
Receipt identity is enrollment plus the scheduled instant. Run with the
creator’s authority and selected eligible machine or sandbox and their
configured permission mode (Approve for me by default). On machine outage,
catch up once that day in the enrollment’s time zone when the machine returns. Record a visible skip on a later day. Revoking the enrolled session or losing
access fences admitted work. When the creator leaves the shared space or
loses the necessary right, stop the schedule and notify the space’s current
Managers and Owner through host outcomes with no private payload. A Manager
or Owner may explicitly take over. Reauthorize operation, target, settings,
machine and account-configured servers under the new creator, and reuse
existing occurrence receipts so takeover never replays committed runs.
Removal and takeover fence the prior enrollment before admitting new work.

Set `notify_creator` on an enrollment to request a host notification on outcome. The
scheduler records a receipt-backed event with enrollment label, status and
an authorized link, then uses the host notification path. At most one per
occurrence. A Reminder can notify even when the operation returns no payload.
The worker cannot choose the recipient or supply arbitrary feed text.

Signed wait webhooks deliver a typed signal to one parked job. Verify the
enrolled route, signed correlation and replay identity, and revalidate the
job's target and grant before its existing winning-signal transaction.
The sender cannot select actor, operation or target. Standalone dispatch is
outside this contract.

**Network.** A plugin may contact only the public destinations its install
screen says it can use. Installation approves the listed lines. Show at
most these three, in plain language, and omit unused lines.

| Install line | Manifest field | Approval |
| --- | --- | --- |
| Fixed sites | `network.fixed_sites` lists exact HTTPS hosts | Manager or Owner approves at install. |
| Servers you add | `network.user_servers` opts in | The actor approves their own server by entering the exact host in account setup. |
| Any website | `network.any_website` opts in | Owner approval at install with a plain warning. |

An update that adds a line or expands the listed fixed sites asks again
under the applicable role. Unchanged lines retain consent. People can
remove their configured hosts independently of package installation.
Direct sockets and private-network access stay disabled. TCP comes later
as “Servers you add” with a port.

Declare `network: "none"` for the explicit **No network access** worker tier.
Install and gallery use the manifest schema’s `noNetworkBadge` predicate:
`network == "none"` and `content_fetch` absent. With content fetch, say “No plugin
network access. Loads remote images anonymously.” Adding a network line
removes the badge and asks for approval. Every package process has no ambient
socket route and has only its own supervisor endpoint. The host checks
that endpoint’s package identity/digest and current installed policy on
every broker egress entry, including delegated jobs and credential
routes. A package with `network: "none"` cannot obtain a worker proxy or credential
route, even in a space with a network-enabled package. Refuse before DNS,
connection or proxy-cache lookup. Cross-package IPC cannot borrow egress.

Render remote images only if the package declares `content_fetch: true`.
The declaration grants no worker egress. It withholds the No network badge
because rendering image URLs still contacts their destinations. If absent, load no remote images. If present, the
authorized host renderer may fetch through the anonymous proxy with one
per-person setting, default on. No worker broker method prepares or reads
these assets. The host media route exposes no fetched bytes or handles to
the worker and allows no credentials, active content or arbitrary headers.

Before DNS authorize the exact requested host under the selected approved
line or the anonymous-image path. Resolve through checked public addresses
and pin address and TLS host identity for dispatch. Recheck authority,
hostname, address and TLS on redirects. Refuse private addresses and unsafe
redirects. Explain URL disclosure in the trusted preview where material.

Issue a handle with `request_secret_handle` for an operation-declared
credential route. Redeem through `credential_call {route, handle, input}`.
The route names a fixed approved site or an account-setup server slot, a
host profile and a secret. Require the matching network line. A credential
route cannot attach a secret to the “Any website” permission. Resolve an
exact HTTPS endpoint for this actor before issuing the handle. The profile
fixes method, path, typed input, credential injection and projected output.
Check profile and manifest schemas before enable. Bind redemption to the
actor, ticket, grant revision, epoch, secret version, route and endpoint.
Refuse mismatches before DNS and disallow credentialed redirects. Generic
proxy calls accept no credentials. Do not expose credential-bearing
responses or secret bytes to the worker.

Refusals return 4xx, code, plain message and optional path or next step.
Foreign resources give opaque `404`. Audit records include time, space,
package/digest, actor/delegation, operation/method, safe target ID, request
ID, outcome, grant revision, cost and latency. Omit values, tokens, URLs
with data and output. Reserve bounded audit capacity before effects. Refuse when it is exhausted. Read
audits batch asynchronously with zero read-path flushes. Record crash gaps.
Fixture retention is 10,000 records or seven days, whichever comes first.

### Engine surface

Use shared IDs and engine errors. Results below are contracts for adapters,
not new storage APIs. Each listed read is bounded by indexed work or by byte
range. Every mutation is bounded by one engine-owned commit/admission plus
its receipt, with long work as a durable job.

| Engine / methods | Input → output, refusal and bound |
| --- | --- |
| Items · `item_head`, `item_range`, `item_history` | Kind/item ref → head metadata and opaque revision. Range adds revision, offset and maximum bytes → base64 bytes and next offset (fixture 64 KiB decoded, frame budget after encoding). History adds cursor/limit → authorized revision summaries. Unknown/foreign ref is `404`, obsolete range pin `409`. |
| Items · `item_save` | Create or item ref, expected head, and full body or metadata patch → committed revision. A metadata patch on a live kind checks the expected revision against the room-backed head under the same write fence as CRDT commits. Preserve current body and CRDT checkpoint. Return `409` with a fresh head when stale and avoid a full body upload. Large writes use the staged protocol below. |
| Items · `item_trash`, `item_restore` | Editor or higher. Trash records a tombstone. Restore from Trash uses the current trashed head and restores a live placement. Restore from history takes a source revision and expected live head and appends that version as a new revision. No history restore against a trashed item. Stale heads give `409`. |
| Items · `item_rename`, `item_rename_status`, `item_rename_undo` | Rename accepts ref, expected head and name and returns a durable action ID. Status uses that ID and checks live item authority. Completion can return a protected Undo action bound to the result and expected revisions. Undo is a fresh receipt-backed operation, with conflict if affected heads moved. Publish progress and actions through the host result route, with no worker payload. |
| Items · `item_purge`, `empty_trash`, `collection_create`, `collection_membership` | Purge and Empty Trash are Manager/Owner-only, irreversible and one confirmed durable job over a pinned Trash selection. Reauthorize per batch and resume by receipt. The 30-day policy runs separately. Collection creation and bounded membership changes commit once with expected revision. |
| Queries · `items_list`, `items_search` | Kind/optional collection, typed indexed metadata predicates, declared sort with stable ID tie-breaker, cursor/window and field set → authorized page with cursor and count/offset where windowed. Unindexed filter/sort is `400`. Use engine-side query and bounded row/byte budget. Apply authority before count. No whole-kind materialization. |
| Rooms / sharing · `sharing_check` | Item ref plus requested action → permission for the live grant. A room is an attribute of the kind, opened client-to-host using its own bounded protocol. Room membership and events keep fencing the item grant. No plugin hook acknowledgment. |
| Jobs · `start_job`, `checkpoint_job`, `job_status`, `cancel_job` | Declared job, payload and resolved target/grant → durable job ID. Checkpoint only the owning current run. Status/cancel uses a scoped job ID with opaque `404` for foreign jobs. Bounded checkpoints, live progress and park count per space/package. |
| Settings / secrets · `read_setting`, `set_setting`, `request_secret_handle` | Declared key or secret purpose → effective value/version or opaque handle. Validate type, compare revision on writes and bind personal values to the actor. One bounded engine operation. Missing authorization is `403` or opaque `404` as appropriate. |
| Proxy · `proxy_request`, `credential_call` | Approved install line and bounded HTTPS request → bounded response. The host-only sealed content route exposes authorized opaque image refs to the renderer. Unapproved destination is `403`, oversize `413`, quota `429`. Anonymous media keeps the existing 10 MB, three redirects and 16 hosts/minute fixture ceilings. Typed call returns the profile output or `403` without contacting the endpoint. |
| Turns · `turn_start`, `turn_status`, `turn_cancel`, `turn_follow_up`, `turn_answer` | Thread ref to start and Turn ref for run controls, with typed payload → admitted turn, bounded status, queue state or protected-request receipt. A person can select their own eligible enrolled machine and login for their own turn. The host resolves eligibility and approval. A plugin cannot select arbitrary placement. Host drafts resolve to checked message text and attachment refs at submit. Transcript pages and append are client-to-host. |
| Schedules · `schedule_create`, `schedule_remove` | Declared eligible operation, pinned target/payload, time zone and calendar or enrollment ID → durable enrollment or removal receipt. One indexed, bounded write. Resume requires the enrollment and live authority. |

The Turns engine owns the `threads` kind, with indexed status. The item
engine exposes an indexed, host-maintained `trashed` boolean for lifecycle
views. Plugins cannot set it as metadata or claim host-owned kinds. The [host catalogue](../contract/host-catalogue-v2.json) registers typed
direct engine dispatch. Its built-ins never start a plugin worker.
`host.item.patch-metadata` uses the same room-checked save path.
Rename status and protected Undo are host-owned. A renderer gets only authorized status and a
host-minted Undo action reference.

**Staged save.** Begin reserves total decoded bytes against the existing
receipt namespace (space, actor, request ID), checked operation fingerprint,
resolved target, expected head, total length and digest. Concurrent identical
begins return one handle and reservation. Changed input in the namespace
returns `409`. Each chunk carries a byte offset and bounded base64 data.
Matching chunks replay, conflicting overlaps fail. Bind the handle to the
live invocation and fence chunks and commit. Verify length and digest,
seal staged bytes immutably, then recheck seal, authority and head under
§3’s fence. Replay a committed receipt without a new stage. Fixture caps
are 2 MiB/upload, 64 KiB/chunk, two handles/actor, four handles and 8 MiB
per package, eight handles and 16 MiB per space. Reserve before writing or
return `429`. On invalidation retire handles and queue bounded cleanup.
An authorized retry can restage uncommitted input. Expire after 60 idle
seconds, reclaim within 30 seconds and sweep orphans before restart admission.

**One approval model.** Use the existing protected
[Inbox requests](../crates/core/src/requests.rs) `Approval` record and
its answer/approve/deny receipt rules. A job park and a Turn reference
the same request type and resolver. Approvals bind the material action,
target, grantee and lifetime. Irreversible lifecycle actions use one-use
confirmation. A stored approval cannot substitute for a live grant.

Background work has no overall interactive deadline or persistent ticket.
Each authorized run gets a fresh 60-second ticket. Longer work saves a
checkpoint and continues in a later authorized run when its ticket expires.
If its creator loses rights or leaves, stop the job, including parked
continuations, record the reason and preserve committed work. Schedule
takeover is a host action as described above and confers no job takeover.

Job admission persists operation, target and grant. Revalidate at claim,
terminal park and resume, including any child and descendants. The runner
validates a terminal `parked` result before atomically saving checkpoint and
wait through the existing jobs engine. There is no post-park acknowledgment.
Inbox answer/approval, typed signed wait delivery, deadline and cancel race
through the existing winning-signal transaction and request receipts. The
losers have no effect. Park uses no worker once the run ends. Retain existing
wait bounds and state visibility. Repeated runner execution can occur after a
crash. Committed effects replay by receipt.

## 4. Host catalogue, queries and activation

The [description contract](description-contract.md) and its
[shared schema](../contract/description-v2.schema.json) are reviewed in
this same tree. Both use the [host catalogue](../contract/host-catalogue-v2.json)
as the source for kinds, fields, built-ins and shared wire shapes. Validate
positive and negative fixtures with the same strict JSON parser and semantic
loader as the host before freeze. The paper verifier is only a draft check. No
template supplies trusted authority or operation semantics.

### 4.1 Catalogue and qualified queries

The catalogue owns generic `id`, `title`, `updated`, `trashed` and
`trashed_at` fields for every item kind. `threads`, `collections` and
`requests` are host-owned. Notes declares its own `notes` kind and
`pinned` metadata through the same manifest API a community app uses.
Kind declarations name an icon, a string title field and indexed fields
using the catalogue’s type and audience vocabularies. The host derives
generic fields from the engine record and its configured title field.
The manifest’s `reads` names every kind a package’s Bind may access,
including its own. The SDK fills the package’s own kinds into the emitted
list. Authors write `reads` only for another app’s kinds, including host
kinds. Check the emitted list before authorizing the viewer and field graph. A package or a field declaration cannot confer authority. A scoped row ID
identifies the kind’s reference. A request row resolves only to an
authorized `RequestRef`. `waiting_on` and `pending_for` support the
viewer’s own equality operand only. `threads.active_turn` resolves as a
Turn ref, never as the parent Thread ref.

Compile one dependency graph per view, including chrome and content.
Check unique in-scope IDs, declared kinds and types, and all binding edges
before opening subscriptions. Reject cycles, ambiguous multi-row operands,
out-of-scope action inputs and references to undeclared binds or actions.
At query execution and delivery authorize the package and viewer, kind,
collection, each field of every input or output, each filter/sort,
corpus and any upstream Bind field. Cross-bind query operands are unsupported, including a foreign Bind’s
selection as a filter or collection operand. Action/component edges may
refer to one authorized stable row under the same audience. A Choice prompt
may read up to 50 options from an independently authorized Bind. The host
issues opaque ChoiceRefs scoped to the prompt and versions. Resolve the
chosen row ID from the host snapshot. A raw client value conveys no target. The viewer
cannot turn an unreadable field into a predicate, sort key, string prefix,
search term, order, count or error observable. Do not evaluate a disallowed
plan. Search in the explicitly authorized member or shared corpus **before**
rank or pagination. Never search a private body for a guest with title-only
access. Count only the authorized projection. Guest count fallback is
always `none`, including in nested windows. `exact` requires an explicit
count-enabled shared root or child projection.

The catalogue defines typed predicates, count modes and window ceilings.
Equality operands use the field’s scalar type without coercion.
`in` accepts a checked homogeneous list (1–32 scalars), including a literal
list or a typed route/selection list if its source is authorized.
`exists` takes no operand and tests indexed non-null presence.
Ordering and `prefix` require the field’s declared type and index collation.
`all` and `any` have a combined 16-predicate cap and no nesting.
Two sort keys maximum plus stable ID as a tiebreaker. Use half-open
`[from,to)`. The host may prefetch within the row and byte cap and returns
one catalogue `WindowReply`. Its `WindowResult` arm gives the Bind, state,
`window_ref`, result version, actual offset and stable rows. Both schemas
cite the catalogue’s only request envelope,
`{type:"window", request: WindowRequestPayload}`. An absent `anchor` means
`start`. With `anchor:"end"`, `[from,to)` counts backward from the last
authorized row, with zero at the last row. Use this form for End and
near-end jumps only when the effective authorized count mode is `exact`
or `estimate`. Its reply includes the count and gives the end-relative
offset of the returned row nearest the end. Return rows in the Bind’s normal
order. When the effective count mode is `none`, refuse an end-relative
request with `WindowPlanError.reason: end_unavailable` before computing
or returning an end distance. Both forms require
`0 < to - from ≤ 200`. For a per-parent Bind require its edge-scoped
`parent` ref. Omit it at the root.
Count is an exact/estimate object or omitted, as allowed
by audience and plan. The chrome/content pair shares one Bind list.
`viewer_self` permits counts over the actor’s own authorized projection
only. Turn controls require current turn-specific authority. The host projects
`snoozed_until` as null after expiry and invalidates the actor’s Inbox Bind
without waking the runtime. Window locators belong to the checked query
and result version. Ordered collection windows use a scoped covering
index over collection, audience, sort key and stable ID, with a maintained
count per authorized scope. Exact counts use that maintained count.
An index-only jump may skip up to 250,000 entries from the nearer end,
independently of the 4,096-row materialization cap (§4.2). The absolute
jump ceiling stays 500,000. End-relative offsets are bounded by the
largest exactly representable integer on the wire. Scopes of up to
500,000 rows can jump anywhere. The measured slope projects about
11 ms at the skip cap. [Step 0 measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/355ab4203901f0efa89c8a6e818285cc6a9b0d8f/docs/measurements/plugin-api-v2-step0.md#covering-index-and-maintained-count).

Cap each item at 32 distinct effective (collection, audience) scopes
after deduplicating direct and inherited ACL paths. Check all affected
items before committing a grant or another scope-expanding change.
Refuse the entire grant, membership or other expanding change with
`422 effective_scope_limit_exceeded` and name the 32-scope limit if any
item would exceed it. A save touches at most two covering-index
keys per effective scope.

The first contract qualifies these plan families.

| Plan family | Required index |
| --- | --- |
| Stable item lookup | Kind and stable item ID, with a scoped authority lookup. |
| Ordered or filtered kind lists, including Inbox, Trash and self-only rows | Ordered indexes scoped to the authorized kind or viewer projection, with the supported filter and sort keys. |
| Manual collection + mutable ACL + sort, including the main Notes list | The scoped covering membership index, with current ACL applied before windowing and a maintained count per scope. Reading and sorting all members is disallowed even at offset zero. |
| Child and section windows | Parent-scoped ordered indexes for authorized child rows and personal section placements. |
| Search | Authorized member or shared corpus index with bounded rank and pagination. |
| Filtered counts | A maintained count for the supported authorized scope, or an audience-certified member estimate. |

Return `422 unsupported_query_plan` if the plan or index cannot satisfy
the requested scope, filters, sort, window or count within the work
budget. The typed `WindowPlanError` gives the reason. For `skip_limit` on an otherwise qualified jump, page from the nearer
known end. Use the start if the end is unknown or the effective count mode
is `none`. The host
can page by an adjacent window’s boundary key for the same view and result
version. For `end_unavailable`, use count-free End paging (§4.2). For
`unsupported_plan` or `materialization_limit`, or if paging is also
refused, show the Bind error state. Never serve the request with a
table scan or a sort of the full candidate set.

The first contract includes nested child rows and user-made sections (Q11).
`children: {bind}` uses a `source.parent: true` Bind windowed per expanded
parent, with depth at most three and no cycles. The declared edge is the
mandatory constraint. On each window request the host validates the
`WindowParentRef` issued for that child Bind in the parent row, reauthorizes
its entire ancestry and scopes rows and counts to that edge. A
`{parent: true}` operand is an optional extra predicate.
`Section.repeat: {over, rows}` requires `over` to read `layout_sections`
(`id`, `title`, `position`) and `rows` to use the declared per-parent edge.
The loader checks both. Only live space members may read their own layout,
scoped to space and app. Layout membership grants no item access.

### 4.2 Live work, cache and host components

Set aggregate limits across the chrome/content pair and all tabs.
The catalogue sets 16 binds, 8 live subscriptions and 2 pending windows
per open view, 16 subscriptions per actor and 128 per space. Bound queued
bytes and query starts at all three levels. Per window cap materialized
rows at 4,096. Separately cap entries skipped by an index-only jump from
the nearer end at 250,000. Exact counts read the maintained count for the
authorized scope. Meter count work independently and limit starts to
four/sec/space.
It emits up to four coalesced pushes/sec per bind. Authorize before spending work,
coalesce invalidations before evaluating and share a plan only within an
identical audience/authority scope. Supersede obsolete requests, return a
safe throttled state on exhaustion and preserve fair space progress.
In a count-free view End pages forward from the last loaded row, or from
the start if none is loaded, until the host returns a short terminal
window. Use an explicit `anchor:"start"` to continue past the absolute
jump ceiling. The host checks that `from` is exactly after an issued ready
start window for the same active view, Bind, parent and result version,
and seeks from its boundary key. These continuations carry no cache record.
A short window signals exhaustion only when no more authorized rows remain. Throttling or a work budget refusal does not signal End.
The renderer shows no scroll extent that implies a total. Check effective
count policy on every live delivery and never send an end distance to a
count-free view.

Keyboard viewport prefetch is bounded host I/O during repeated keys.
Apply prefetch replies for the result version already drawn at once.
Hold pushes that change the version while selection settles. Navigation
waits for settling. Selection-dependent activation follows §4.3. Prefetch
and pushes never wake a worker.

Keep filled live windows in bounded memory, partitioned by account,
session, space, audience and view. Hide them on lease expiry or disconnect.
The host rechecks current authority, grant revision and the exact window
versions it issued. `LeaseRenewRequest` carries `renewal_id`, `lease_ref`,
`cache_partition`, `grant_revision` and `result_versions` keyed by
`window_ref`. Accept `LeaseRenewResult` only from the authenticated host
session with matching outstanding `renewal_id`, current `request_lease_ref`
and current `cache_partition`. Repaint only exact `retained_results`
pairs of `window_ref` and `result_version`. Changed grants or versions
require fresh authorized windows. Ignore superseded replies and keep rows
hidden on timeout or refusal. Clear live rows on revocation, sign-out,
account or audience switch, session end or guest link closure.

Data-free templates may persist. The only persistable viewer projection is
the catalogue’s `MemberRowCacheRecord` (Q10). The host issues it only for
a live own-space member on an opted-in, start-anchored root window whose
Bind has invariant query inputs. A list source may name a literal
collection. Search text, references and route, form, selection or parent
operands in the source, filter or sort preclude persistence. Account-bound
`{viewer:person}` stays invariant. The schema and loader reject other
`persist` declarations. The authenticated `ViewOpenResult` and `WindowResult` carry
the view audience. Accept a record on write only from an own-space-member
reply matching the active view, including the lease partition, account,
space, template, Bind, window ref and offset. The window schema forbids
records in other audiences and in per-parent or end-anchored results.

On write and load verify the schema, host proof, account, space, template,
root Bind, offset, current opt-ins and `issued_at ≤ now < expires_at`.
Reject lifetimes past the allowed retention. On load show a record only
for the matching local own-space root display. Never serialize live
windows, guest or public rows, action credentials or activation contexts.

On an unlocked device paint eligible records at launch as unconfirmed,
then confirm or correct them with the host’s first window answer. Encrypt
with account- and space-bound keys and retain for at most 14 days or
`cache.maxAgeDays` if shorter. Erase on sign-out, account switch or known
revocation. Every action requires a fresh host context.

The catalogue gives typed host component routes. `transcript.page`
accepts a checked Thread ref, a durable committed cursor
and a bounded page budget (fixture at most 128 events and 128 KiB encoded).
The cursor identifies the thread and committed position and is checked
against the current audience. The host publishes ephemeral progress
separately from committed events, batches durable append, rate limits
delivery and ends with the committed cursor. Resume returns the next
committed page without gaps or duplicate stable event IDs. Expired bookmarks
require an authorized reload. Check before sending each page and on resume.
Changing selection cancels the old delivery. Composer input carries typed
text and host-checked attachment/mention refs. A quote is a thread and
opaque transcript passage ref, minted for an authorized durable range;
a selection or arbitrary source string cannot serve as a quote. Composer
options use `composer.options` and carry authorized selected/default
refs. Map `interaction.draft` and `interaction.options` into the host-owned
`turns.start`, with the Thread from the context. Validate each provider,
model, reasoning, permission and machine choice for the active actor and
Thread. The host supplies missing defaults, rejects stale choices and
checks attachment/mention/quote refs. The composer owns this admission.
The template does not construct a send operation or a second Thread ref.
Progress streaming is host-internal.

A sealed frame ref names a committed content revision and belongs to a
space and audience. Verify it on first use and each asset request. Expire
fixture refs after five minutes or earlier on grant/session loss or frame
removal. `sealed_frame.open` serves HTML on a sandboxed host route with no scripts, forms,
popups, same-origin privileges, top navigation or direct network.
The anonymous media proxy serves approved opaque images only when declared and enabled (§3).
Only the trusted wrapper can call `sealed_frame.resize` or
`sealed_frame.open_link` with an opaque, checked link ID.
The host re-resolves links, authorizes internal routes and uses its own
safe external link affordance after checking scheme and disclosure.
Never accept a URL/navigation target or data sent by the frame as authority.

### 4.3 Activation protocol

| Identity | Issuer and lifetime |
| --- | --- |
| `action_id` | Author’s action name, unique within the template activation scope. Stable across equivalent template revisions. |
| `activation_id` | Renderer-generated per deliberate activation, retained for retries of that activation only. It grants nothing. |
| `request_id` | Host-owned durable action receipt identity, mapped from the activation in the actor/space namespace. Uses configured receipt retention (default seven days). |
| `ticket` | Host-issued ephemeral channel authority for one worker invocation. Ends on terminal, cancellation, fencing or expiry. Never reaches the device or persists. |

`view.open` returns a 15-second subscription lease and view-level
`activation_contexts` for header, form and composer. Each WindowResult
delivers row/action contexts for the stable rows it authorized. The opaque
token binds viewer/session, space/audience, template digest, action, route,
stable row and only that action’s dependency versions. It expires with the
subscription lease. Renew view and valid window contexts through
`view.renew`, along with unchanged grant/result versions. A newer grant or
material dependency invalidates affected contexts. Unrelated window churn
never retargets or stales a still-valid action. Bound context count (256 per
viewer and 4,096 per space) and encoded WindowResult bytes. Withhold actions
until their context is available. A selection-dependent action must use the
context for the stable row drawn at activation. Send any pending selection
as a `SettledSelectionRequest` on `selection.settled` and wait for a
matching `SelectionContextResult`, scoped to the lease, cache partition,
Bind and row. Ignore superseded contexts. The host resolves
only against the issued context, never its current selection.

1. **Resolve.** Accept action/activation IDs, the context and declared
   typed device interactions only. Reconstruct server-owned route, row,
   viewer, Bind values and action from host state and the context.
   Validate form/prompt slots and host draft refs. Bulk resolve may submit
   up to 50 stable IDs from the context-bound Bind. Reauthorize every ID
   against that snapshot and return a scoped `selection_ref` inside the
   resolution. A separate `selection.set` call is unnecessary for bulk
   activation. For a Choice prompt, fetch `ChoiceOptionsRequest` for the
   context and slot before showing the returned labels. Submit the picked
   ref in `interaction.choices`. Resolve that host-issued ChoiceRef against
   the checked prompt and Bind versions.
2. **Derive confirmation.** `confirm:none` permits the shortcut for host
   catalogue reads and for catalogue mutations with `atomic_undo: true`
   whose engine commits the complete effect and its Undo in one
   recovery-visible transaction and satisfies any `atomic_undo_when`
   condition. The mutation’s only material may be
   the drawn target or fields of the submitting form. Irreversible,
   external or access-widening effects, bulk sets, declared material
   inputs and any destination, recipient, audience or permission require
   `confirm:challenge`, including untouched prefills, defaults and autofill.
   The host also challenges unproven material. A plugin operation always
   uses separate resolve and dispatch. The manifest’s risk declaration
   grants no shortcut. On that ordinary path the host omits `confirm`
   when no challenge is required.
3. **Confirm and dispatch.** For a challenge, the host returns written
   material and a one-use proof only after the authorized protected control
   confirms the exact checked input. Check actor, scope, dependencies and
   fingerprint again at dispatch and consume the proof under the commit
   fence. Only the eligible catalogue shortcut
   `ResolveActivation{dispatch:true}` resolves and dispatches in one
   exchange. Otherwise return a resolution without running the operation.
   Dispatch returns an ActionOutcome and may include a checked created ref,
   status ref or Undo ref.

**One-exchange Send.** The host composer pre-resolves when material changes
(entered thread, approved provider/model/reasoning/permission/machine,
attachments, mentions or quote) and when its context needs renewal.
Preflight contains no final text. The host-bound composer preview presents
those fields in place. Explicit Send confirms the same material and supplies
only the final text in one `invoke_confirm` with `final_text`.
The host accepts `final_text` as message content only. Any parser-derived
change to recipients, routing, permissions or other prepared material
(including a mention or slash command) invalidates the proof and requires
a fresh resolve and preview. Check live context and the exact prepared
material before confirming and dispatching in that exchange. The proof’s material fingerprint
excludes text only for this authenticated host Turns Send route. The durable
receipt fingerprint includes the final text and all other input, so a
replay cannot change the message or duplicate a submission. The proof
confers no approval for a later tool or external disclosure.

A replay checks actor, space, package digest, action, activation and
normalized input fingerprint plus current delivery authority. Retry
mutations/disclosures only with the same receipt where the checked
operation allows `request_id`. Changed input conflicts. A lost reply after
commit does not require the old proof again. The saved receipt itself
requires current disclosure authority. An uncertain non-replayable effect
returns outcome unknown. A plugin-computed destination, recipient or
disclosed data outside the checked preview needs new protected approval
at actual broker dispatch. The commit fence remains the final check.

### 4.4 Protected built-ins

Use exact catalogue names and input/outcome definitions. Only the
designated host request card and live approver can resolve an approval.
Only the authorized holder of an eligible host-issued Undo ref can Undo,
with the original target and current revision checked again. Share uses
a host-owned audience sheet. A bulk selection ref is context-bound and
contains at most 50 eligible items in the fixture. `SelectionSetRequest`
and `SelectionSetResult` accept stable IDs from the checked Bind and return
a scoped `selection_ref` for authorization at dispatch. Purge and Empty Trash
use a trusted preview, Manager/Owner confirmation of a host-pinned set and per-batch authority.
Use the catalogue’s typed Undo, status, request-card resolve, Share and
kind-specific Trash-sheet routes. The host sends an authorized audience
list or pinned Trash selection/count before the protected confirmation.
Status polling uses the original action’s
audience. `showWhen` may use viewer roles for visibility; the host still
authorizes every operation. Metadata patch takes scalar field/value with
the rendered head from context. Full swipe requires the first action in
the swipe set, catalogue `full_swipe: true`, the applicable eligibility
conditions and effective `confirm:none`. Bulk and protected actions stay
on their designated surfaces. Move to collection resolves the chosen
CollectionRef from the option Bind, authorizes every affected item and both
collections, previews the chosen collection and exact set, and applies one
bounded membership change. Leave unrelated
memberships intact. Personal layout uses `host.layout.move_to`,
`host.layout.move_up`, `host.layout.move_down` and
`host.layout.section.create`, `host.layout.section.rename`,
`host.layout.section.remove`. Resolve the entry and section in the actor’s
own layout against its context-pinned revision and check live space
membership and item and parent access. For Threads row actions
`{context:"item"}` yields the underlying ThreadRef.
`{context:"placement"}` yields the LayoutEntryRef in a repeated section.
Move to accepts the thread to add an unplaced item or the placement to move
it. It resolves the destination ChoiceRef for the protected preview.
Removing a section changes only the personal layout.

For every eligible case below, commit the complete effect and the protected
Undo ref with its inverse data in the named transaction. Publish the ref
only after commit. Undo rechecks live authority and the post-effect
revisions of all affected resources and refuses if any dependency moved.

| Catalogue operation | Commit boundary and eligibility |
| --- | --- |
| `host.item.trash`, `host.item.restore` | The FULL content transaction records the item revision, recoverable placement and Undo. Full swipe covers single-item Trash and restore from Trash. Trash paths with separate lifecycle cleanup are ineligible. |
| `host.item.rename` | The FULL content transaction records the new head, placement and Undo for a single-item rename without deferred link rewrites. The current Notes job commits inbound-link changes in later transactions, so that path remains ineligible. |
| `host.collection.move` | A single-collection membership edit and Undo fit in one FULL content transaction. A two-collection move is ineligible because the existing membership writer commits one collection at a time. Destination and bulk previews still apply. |
| `host.layout.move_to`, `host.layout.move_up`, `host.layout.move_down` | The per-person layout revision transaction records the complete placement or ordering edit and Undo for the actor, space and app. Move to still previews its destination. |
| `host.layout.section.create` | The per-person layout revision transaction creates the section and commits Undo that removes it. |
| `host.layout.section.rename` | The same layout transaction commits the title change and its prior title for Undo. |
| `host.layout.section.remove` | The same layout transaction commits removal and retains the section, placements and order for Undo. |
| `turns.archive` | The current writer commits the thread head without Undo, then checks for and cancels a non-final turn, runs a separate protected transaction to cancel pending approvals and remove saved authority, and invalidates affected feeds. An idle full swipe requires proving under the commit fence that there is no active turn or authority or approval to retire, skipping cleanup and committing the archived head and unarchive Undo in one FULL thread content transaction. Until then the shortcut is ineligible. For an active turn the host challenge says “The running turn will be stopped.” Authority or approval cleanup also requires a challenge. |
| `turns.unarchive` | The FULL thread content transaction records the new head and Undo. A full swipe may unarchive an eligible thread. |
| `turns.snooze` | Ineligible. Today’s writer stores one shared manual deadline. Expose the actor-scoped operation only once the actor-keyed deadline, its index entry and Undo commit together. Subscription retry state is unaffected. |
| `host.item.patch-metadata` | The room-checked FULL content transaction records the changed head and Undo for a single boolean toggle. Full swipe covers this case. |

On success, Invoke `then:{route}` may navigate using a
host-minted created ref, reauthorized at open.

## 5. Sharing and aggregation

Members trust the Owner's installed plugins with data available to authorized
calls in that space. A warm worker can remember an Owner value and return
it to a Reader acting on their own ticket. Host-filled bindings protect
engine query results, and the installed-plugin trust rule still applies to
what a worker knew or previously wrote. Run all guest logic in the home
space charging its quota. Guest outputs and public links are host-rendered
and grant-checked, with free-form worker output excluded. A public
link serves a saved host view and never wakes. Item grants confer no
personal-layout access. Layout reads and writes require live membership.

Guest tickets cover the home item and its host-recorded children. Only a
host engine can create a child edge while committing a typed parent–child
effect under live authority. Creating an edge to an existing item requires
a member with authority on both items and the relationship mutation.
A guest may add a Comment via the host engine, which creates the narrow
child in that same transaction. Workers and payload links cannot register
edges. Resolve graph edges in the home space. At each hop intersect parent
scope and permitted effects, live grant, and child-specific rights and
operation. Apply the intersection to rooms, tools, media and every resolved
reference. A revoked edge or grant cuts all descendants. Deny guest job
admission until inherited authority is proven across the full durable
path. Explicit Prompt remains a product grant, off by default with a warning
and Owner approvals, and ships disabled until the tool/machine path is
proven.
“Continue in my space” copies the permitted item and attachments under the
receiving space's rights and quota. If the package is missing, offer
authorized install or refuse with a plain reason. Exclude credentials,
grants, approvals and run state.

“Shared with me”, Inbox, search and notifications are host features. Derive
their feeds from authorized item journals, job parks and the scheduler’s
creator outcome events. Workers cannot
publish arbitrary feed summaries. Revalidate on delivery and remove
revoked entries and media from subsequent views.

## 6. Deliberate cuts

Every package uses the same brokered data engines, official or community.
No package owns a database. Build a host-owned table engine, open to every
plugin through the broker, when an app needs bulk records or aggregates.
Mail is the expected first consumer.

Description patches, persisted viewer projections except `MemberRowCacheRecord`
under §4.2, worker-published feeds, room hook acknowledgments, standalone
inbound dispatch, direct sockets, brokered TCP and private-network access,
guest Prompt execution, snapshots, and gallery listing fields are outside
the first contract. Live room sync is an engine facility. Gallery listing
policy stays in the submissions repo. These cuts do not retire existing
features from the current API. The first renderer contract omits author
confirmation and preview overrides, pending overrides, result-display
policy, general viewer operands except person, index-based row sources,
sidebar cards, appearance/scheme/accent/title image and manifest commands.
Use host policy for trusted operations and results.

## 7. Author SDK and conformance

`defineApp`, `defineKind`, `defineOperation` and declarative package
metadata generate a checked manifest, method inventory and immutable
asset/digest index. `defineChrome` and `defineView` author templates against
the renderer-owned shared schema when exercised. Typed `ctx` methods derive
from shared contract models.
A retry keeps the same request ID and typed target. Authors never pass
`space_id` to broker methods or see tickets, epochs, leases, seq, callback
IDs or renderer keys. Host-mediated client operations may still select an
authorized space. Authors choose the manifest API version. Templates
negotiate renderer features without a manifest version field per asset.
The private fixture adapter speaks `protocol_version: 2`, `host_call`.
The space channel uses `v: 1`, `call`. The existing provider handshake keeps
its independent version. Keep all three protocol labels out of author
examples. Rewrite the public guide for Plugin API v2, removing broker
`space_id` inputs and teaching the typed routes, templates and refusals.
Validate the guide examples against the generated SDK and real fixture.

An in-process fake broker runs the same schema, declaration, target, quota
and receipt rules, with deterministic clock, actors, revocation and scripted
effects/failures. It needs no machine, credentials or VM and must pass a
shared real/fake fixture suite. An undeclared method or kind returns
`403 not_declared`, an exact manifest field path and the change to make.
Foreign resources keep their opaque response. Authorized package authors
get bounded development diagnostics with the actual reason and no foreign
values. Ship a scaffold, check/test diagnostics and public-guide-only
small-model trial for a bound view, a durable wait and refusal/retry.

**Gallery submission gate.** The gallery listing requires an icon,
description, screenshots, age rating, and a plain statement of what the
plugin connects to and why. Keep listing metadata in the submissions
repository. Private and agent-authored plugins need none of it. The
plugin guide skill and SDK must generate the listing from a checked
package, capture screenshots from fixtures and check completeness.
Before enabling submissions, a public-guide-only author must produce a
package and complete listing from the scaffold. The validator must accept
it, reject an omitted required field and flag a stale connection statement
or missing screenshot. It derives the No network access badge using
`noNetworkBadge` on the pinned manifest (§3). This preparation and check require no publishing.

| Case | Assertion |
| --- | --- |
| Wire, quotas and control | Invalid UTF-8, duplicates, deep or oversized JSON, unknown fields, forged package/epoch, frame replay and wrong direction have no effect. Saturate one package and one space while another makes progress and controls arrive or the stalled carrier fences by its watchdog. |
| Independent permission matrix | Exercise Owner, Manager, Editor, Reader, foreign account, guest View/Comment and revoked ticket across methods and deliveries. Match expected capability/target outcomes from independent fixtures. A compromised supervisor reaches neither a second space nor machine control. |
| Commit and recovery | Race role/session/plugin revocation with effect commit and egress dispatch. Crash before and after save/park and reconnect with the same receipt. Race answer/deadline/cancel and assert one winning effect. Reject stale handles and changed agent-authored digest. |
| Scoped render and egress | Seed a warm worker with privileged data and assert the guest sees only host-bound fields. Check counts/windows, empty/load/error, selection coalescing and no wake on a cold read. Refuse disallowed URL/DNS/redirect without connection or foreign media. Test Manager approval of fixed sites, Owner-only Any website, actor-only account servers, added-line reapproval and No network badge/filter changes. A No network package in the same space as an enabled neighbor must make zero DNS, connection or proxy-cache requests via raw sockets, broker calls, credentials, jobs or forged package routing. Test anonymous images with and without `content_fetch` and the viewer setting. The worker gets no bytes in any case. Verify audit redaction. |
| Lifecycle and migration | Coalesce wake, bound drain, replace after fault, verify clean boot and exhausted admission. Verify upgrade, remove, active job fences and real/fake parity. Test that parked jobs and signed wait signals survive restart. |

## 8. Build order and gates

### Step 0 — Spikes before freeze

**Entry evidence.** Paper-walk Reading List (list, Add, mark-read and a
one-date private reminder), Notes (created ref, room edit, Move to collection,
bulk Trash and Share), Threads (create, archive, snooze, composer, follow-up,
protected approval), and Mail (list, sealed body, credentials and new
arrival). Check Reading List, Notes and Threads executable chrome/content
fixtures with their fixture manifests using the catalogue. Include malformed
fixtures, declared reads, one shared Bind list, and every host surface used.
The carrier, cold page and atomic save spikes are independently cleared.
The combined host/renderer spike starts after its paper paths verify.

Use disposable real engines and the **Svelte web renderer in Playwright
Chromium** for the combined browser/phone fixture. For each timed condition
run **100 measured repetitions** and report **p95** on
`calternal-perf-01-backup01` under `/usr/local/bin/perf-lock.sh`, using
binaries built on a dev VM. Record Chromium version, wall, payload, query
plans, rows visited, reads, commits and flushes. Use an emulated phone link
at 150 ms RTT for exchange counts and the keyboard trial. Measure indexed
engine latency without the added link delay. Protocol safety deadlines
and zero-effect assertions must hold on every repetition.

| Spike | Injections and pass condition |
| --- | --- |
| **One enrolled channel** | Two real package processes on one enrolled channel in Space A, plus Space B. Block A’s carrier receiver until its actual writer stalls mid-frame. Hold a slow callback for two seconds. With a writable carrier, control and the other callback complete p95 ≤500 ms. While stalled, local fencing p95 ≤500 ms, close p95 ≤2 s and Space B progress p95 ≤2 s. No old-lease effect after local fencing. Remote stops within 5 s of its last authenticated renewal on every run. Reconnect rejects the old epoch and authorized replay commits once. |
| **Cold indexed page** | 100,000 mixed-visibility rows. During grant/content churn test manual collection + ACL + sort from offset 0 through its last full authorized range and last visible row (at most 10,000 members, so the last full 200-row range starts at most at offset 9,800). Use a dense dynamic collection for offsets 10,000 and 90,000. Exercise qualified windowed search within its authorized corpus. Offset 0 p95 ≤100 ms, deep windows p95 ≤500 ms. Seed enough authorized rows for every requested range and assert the exact authorized rows in order. Empty or refused replies fail. Meter materialized rows and skipped covering-index entries separately against their bounds. In a larger authorized scope, also request a range whose nearer-end jump exceeds the skip cap. Assert `skip_limit` followed by paging that returns the same authorized rows. Test End on a scope beyond the absolute offset ceiling using `anchor:"end"` and assert the last authorized rows. For stable-ID lookup assert one bounded indexed lookup and scoped authority check for a present, missing and unauthorized ID. For standalone exact counts assert one maintained-count lookup and the exact authorized total after grant churn. Exercise saves at the 32-effective-scope cap and assert at most two covering-index keys touched per scope. Refuse a grant or membership that would create scope 33 with the typed limit error. Measure the existing write and index maintenance together on the save path and require p95 ≤40 ms. The measured two-collection sort change adds about 9 ms p95 in isolation. Reject table scans and full candidate sorts, coalesce fan-out, refuse both counts and rows on revocation and wake no worker. |
| **Atomic large save** | Stage an escape-heavy 2 MiB body and reconstruct it by revision-pinned ranges after the head moves, p95 ≤5 s. The 60 s range pin is invalidated by purge or revoke. Race duplicate begin, lost chunk ACK, conflicting bytes, cancel and orphan cleanup. Race revoke, epoch loss, enrollment removal and run replacement on both sides of commit and inject crashes before publication and before reply. No partial edit or success receipt before commit. After commit recover one revision and receipt. |
| **Durable transcript** | Stall the reader, cut after commit before ACK and restart. First screenful drawn p95 ≤150 ms on a warm authorized reconnect, with the durable cursor, stable event and receipt sequences unchanged. Revoked reconnect reveals neither rows nor bookmarks. An independent control request completes p95 ≤500 ms with the transcript reader stalled. |
| **Host + renderer authority** | On Svelte Chromium check both formats, a real protected approval and a live Bind. Forge row/route/Bind/context, untouched prefills, bulk IDs, ChoiceRefs, cached data and built-in surfaces. Inject duplicate and stale activations, revocation and unrelated churn. Zero unauthorized effects or rendered bytes, one commit on replay, and unrelated-churn action p95 ≤2 s from dispatch. Race revocation at egress, job claim/park and approval resolve before freezing those routes. For each §4.4 atomic-Undo case, crash before and after the named commit and during publication: recover neither effect nor Undo before commit, and both after it. Concurrently edit a dependency, then require Undo to refuse without another effect. Exercise ineligible variants and prove the shortcut and full swipe cannot bypass their checks. Delay or spoof lease replies and verify only current, exact retained windows repaint. |
| **Phone activation exchanges** | In the same Chromium phone link fixture, from action intent to host outcome, an eligible swipe and prepared Send each use one exchange. An ordinary plugin operation without a challenge uses two. Require the bounds on every run and report p95 exchange counts and elapsed time. Alter prepared material, including via parsed final text, or drop the Send reply. Require a fresh preview for changed material and replay the exact final text once. |
| **Keyboard windowing** | 5,000 rows with 30 keys/sec on the phone fixture. At most four window requests in any second and zero selection/navigation events during repeat on every run. Report p95 request rate. No skeleton while continuous scrolling is steady. Same-version prefetch applies while keys repeat; an activation waits for the context of its drawn target. Delay a context for A until B is drawn and verify the activation cannot use A. |
| **Snapshot randomness** | Independently restore the same snapshot twice and sample the guest kernel and each restored process’s crypto RNG, including Node. Require different bytes for both restores at both layers and require fresh kernel and process randomness before any plugin code. Refuse admission if reseed or fresh process reinitialization cannot establish that. Include reseed time in measured wake. |
| **Interactive pending** | Force work past three plugin seconds, including a cold wake with the separate measured/fixture allowance. Check caller-visible pending p95 ≤3 s from admission, one diagnostic and one status ref. Poll while it finishes and retry after a lost reply. Never launch again or allow effects after ticket expiry. |

Manual-collection measurements at offsets 10,000 and 90,000 were
out-of-range controls with zero returned rows. The dense dynamic collection
supplies the deep-offset measurements. The manual collection’s last full
authorized page and last visible row measured 0.158 ms and 0.101 ms p95.
The combined save measured 31.299 ms p95 at 32 effective scopes.
[Step 0 measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/355ab4203901f0efa89c8a6e818285cc6a9b0d8f/docs/measurements/plugin-api-v2-step0.md#cold-indexed-page).

If a gate misses, record the measured failure and change the mechanism or
reduce the first surface before freeze. A fixture using separate per-package
transport or an in-memory broker is not evidence for the carrier spike.

### Step 1 — Freeze exercised schemas only

Freeze only models and fields exercised by Step 0. Candidate slices are the
space carrier, the target and item range/save receipt path, the cold page
query and transcript cursor. Generate matching Rust-backed schemas and TS
types. Freeze descriptions, Invoke and each built-in only after its real
host/renderer authorization case passes. Shape fixtures and the author
trial alone are insufficient. Mark other manifest/engine declarations draft and leave them out of
the stable SDK. Reuse the container wall and durable jobs for a thin test
adapter. Existing job/wait contracts remain the regression baseline.

Run an independent role/method/target matrix and both race orderings from
Step 0 against the real fixture for each frozen slice. Negative cases must
produce zero durable effect and zero unauthorized data. Include malformed
frames, other-space and machine-control denial, blocked-channel fencing,
attempts to widen scope, replay around a lost reply, query side channels
and stale action denial for exercised descriptions. Run fake/real parity
for those same slices and the reused jobs/waits path, including one winning
signal and zero repeated effects after restart. Extra §7 cases gate their
own later surfaces.

The [current checked-in guide](../plugins/calternal-plugin-guide/reference.md) is v1 and is excluded from v2 fixtures and
trials. Deliver the versioned v2 rewrite and compile each example. Run the public
guide and scaffold with a fresh small-model author, first with the fake
broker and then through installed real fixtures. Pass when built-in verifier
cases all pass, the author uses only public documentation/SDK input, there
are zero failed commands, and a second verifier run passes unchanged.
Exercise a bound page, typed parked question with answer and deadline, and
an intentional refusal/retry with an actionable expected response.
### Step 2 — Integrate placement and engines

Pass these end-to-end flows on the selected wall before app migration.

- **Space lifecycle.** Concurrent admitted logic and due work coalesce to
  one wake. An asleep page and public/share read never wake. A package
  crash leaves its neighbor alive. Disconnect fences the old epoch and
  clean restart resumes only receipt/checkpoint-safe work. Stop confirms
  physical teardown or durable revocation.
- **Guest and media.** View and Comment see the granted root/children
  only. Revoke mid-room, query, media request and action. No later
  disclosure or commit. Retain a durable commit ordered first.
  Prove host-only guest projection and zero restricted egress. Prompt and
  guest job admission stay disabled until separately proven.
- **Install/update/remove and recovery.** Verify checked immutable bytes,
  refuse unapproved widening, and fence the old revision across an in-flight
  write and parked wait. On release-pair failure restore a compatible
  package plus its engine data and receipts, with no duplicate signal or
  write. Complete the remaining §7 cases for surfaces being enabled.
  Snapshot enablement awaits wall measurements and its own recovery proof.

### Step 3 — Notes first, then Threads

Replay these against shipped versions and the replacement using identical
fixtures, with final revisions, visible authority and durable outcomes equal.

- **Notes.** Cold open a collection and search. Edit one 2 MiB note and
  update metadata while a second client edits through Yjs. Reconnect an
  offline draft, retain body and history, rename and Undo, Trash, restore
  from Trash and history, and confirm Empty Trash as a Manager. Verify
  linked-user View/Comment and revocation, the host Share sheet and bounded
  bulk Trash/share/collection creation and Choice-from-Bind Move, no worker wake for built-in
  lifecycle actions, and recover a committed save after rollback.
- **Threads.** After Notes passes, open an indexed thread list, submit a
  composer draft with an attachment on the person’s eligible machine,
  reconnect at a committed transcript cursor, queue a follow-up, act on
  a protected approval, archive and unarchive, snooze, and stop. Require exactly one submission per
  receipt, correct machine/login attribution, no lost committed transcript,
  and equivalent rollback/reopen behavior. Exercise host-owned
  provider/model/reasoning/permission/machine controls and authorized
  options, checked attachments and snooze presets plus the shipped custom
  date/time picker unless the Owner approves a parity exception.

Run executable catalogue fixtures for both apps, including nested child
rows and user-made sections. Drag is an Owner-approved parity exception
(Q11), covered by Move to and Move up/down. General-purpose selectors,
repeaters and conditional fields wait for the first primitives. A shipped
workflow needs parity or an explicit Owner exception before replacement.

Rollback is a release-pair exercise. Show that a committed change remains
readable exactly once after rollback or refuse the downgrade before it
can discard the change. Mail still waits for a later engine contract.

## Owner decisions and open questions

See the [recorded answers](https://git.kayg.org/kayg/calternal-ai/src/commit/3ab27a6bfdff921e11846e22675f9520d12962ae/docs/open-questions.md#recently-decided-2026-11-17-now-in-the-decision-records).

- **Q1 Interactive deadline — DECIDED.** A tap has three seconds of plugin time after a separate short wake allowance. Slower work is a job with progress.
- **Q2 Tickets and schedules — DECIDED.** 60 seconds per ticket, fresh at each run under the creator’s current rights. Stop on lost rights or departure, notify Managers, and allow scoped Manager takeover of the schedule (§3).
- **Q3 Guest access to a future host-owned table store — OPEN.** Recommend granting none by default and revisiting only with an explicit item-bound profile.
- **Q4 Gallery listings — DECIDED.** Icon, description, screenshots, age rating and plain connection disclosures are required for gallery submissions. The manifest stays small. See §7.
- **Q5 Storage parity — DECIDED.** No plugin database. Host engines serve official and community plugins equally. Build the table engine when needed, expected for Mail (§6).
- **Q6, Q7, Q9 Network — DECIDED.** Install lines and actor approval follow §3. No network access is explicit. Declared content fetch loads anonymous images through the host, default on per person. No private network in the first release. TCP follows later under “Servers you add”.
- **Q8 Plan mapping — OPEN pending measurement.** Choose plan ceilings from measured wake, memory and saturation.
- **Q10 Own-space member row persistence — DECIDED.** Paint eligible
  encrypted rows at launch and then confirm or correct them (§4.2). Sol
  recommended fresh host authorization before paint. The Owner accepted
  that a removed member may still see cached titles until the device learns
  of the revocation or the rows expire.
- **Q11 Nested child rows and user-made sections — DECIDED.** Both enter the
  first contract. Drag waits, with Move to and Move up/down as the recorded
  parity exception (§§4.1, 4.4).

See the [review dispositions](reviews/plugin-api-v2-review-dispositions.md)
for the revision 4 through 6 finding maps.
