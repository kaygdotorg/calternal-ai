# Plugin API v2 · revision 3

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
Mail has a data layer and fixture UI. Real mailbox access waits for a reviewed
mail engine, credential custody, bounded ingestion and a new-mail trigger.

## 1. Package and install

[Manifest v2](../contract/plugin-manifest-v2.schema.json) has apps with pinned
chrome/content templates, kinds with metadata/index declarations and an
optional live-room profile, operations with input/output schemas, capability,
risk, idempotency and optional item target, jobs/waits, schedule eligibility,
settings, secrets and typed credential routes. Before wake, resolve
`target: {kind, field}` from the required top-level string in the input.
Palette commands reference operations and typed inputs.
Turns is the one named engine an app can select. Provider contributions
pin a Turns adapter descriptor. Claude Code and Codex use the existing
[provider protocol](../contract/provider-v1.schema.json) and
[events schema](../contract/provider-events-v1.schema.json). The host chooses
and launches the adapter inside the agent sandbox, outside the plugin wall
and without machine-control authority. Provider-only and static packages
can omit a broker worker. Operations, jobs, schedule eligibility and
credential routes require one.

Reject unknown or inconsistent declarations before launch. Check unique IDs,
package operation namespace, references, schema compilation offline, pinned
assets and digests, indexed field types, and cross-reference constraints.
Each operation lists its host methods once. Derive the install summary by
union and constrain each call by that operation’s capability and risk.
Derive the mutation flag from risk. Reads use `read_only` and cannot mutate
user data. A job may checkpoint its own run as part of the durable job
protocol. Receipt-backed mutations retry with the same request ID. Never
automatically retry a non-idempotent external action.

Install shows kinds, methods, capabilities, risks, settings/secrets,
providers and outbound disclosures. Gallery installs take one tap. Source
installs warn. Pin the digest. Members see packages, grants and destinations. Check and probe agent-authored code
in the space without broker authority, then enable the exact digest and
grant the Owner confirmed. Verify updates before fencing/draining the old
revision. Permission widening needs the Owner. Resuming an old job under a
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
| Awake | Reuse processes between calls. Per-call execution begins after wake and has its own deadline (Q1). Work needing longer is a durable job outside the interactive budget. A package crash retires that process and its tickets. Other packages continue. |
| Draining → going to sleep | Stop admissions to the current epoch, drain within the placement bound, cancel/fence the remainder and retain committed engine effects and checkpoints. Close I/O, stop with acknowledged teardown or durable placement revocation, then sleep. A racing wake coalesces after shutdown. |

Nothing in memory is required to survive sleep. First release proves clean
boot only. A later snapshot restore must consume one host-side,
single-use runtime epoch bound to space, wall image, supervisor and package
digests. Restore cannot reuse inherited tickets, handles, descriptors,
credits or random state. Reseed from host entropy before processing work.
Restart a process if retained authority or identifier state cannot be
fenced. Host/runtime crash invalidates the old epoch. Recover uncertain
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

The outer header is `{v:1, epoch, seq, package, digest, invocation, message}`.
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
Wire and page ordinals only order data. The ticket is the invocation
reference. Receipts use the request ID within actor/space. Subeffect keys
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
   package and live authority on every callback and before delivery. Q2
   settles ticket lifetime. Terminal completion, cancel and revocation fence
   the reference. Persist principal, scope and delegation. Mint a new ticket
   when work becomes runnable.

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
Accept the canonical OnCalendar subset for daily, weekdays, weekly or
monthly at a single local time, such as `Mon..Fri *-*-* 09:00:00`. No shell or
arbitrary timer expression. Reuse the scheduler’s civil-time gap/fold rules.
Receipt identity is enrollment plus the scheduled instant. Run with the
creator’s authority and selected eligible machine or sandbox and their
configured permission mode (Approve for me by default). On machine outage,
catch up once that day in the enrollment’s time zone when the machine returns. Record a visible skip on a later day. Revoking the enrolled session or losing
access pauses runs pending fresh authorization. Removal fences admitted
occurrences as well as future schedules.

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

Issue a handle with `request_secret_handle` for an operation-declared
credential route. Redeem through `credential_call {route, handle, input}`.
The manifest pins a named HTTPS destination, exact host/port and secret,
and a registered host profile. Operations reference permitted routes.
The profile fixes method, path, input, credential injection and output.
The host compiles those schemas and rejects profile/manifest mismatches
before enable. No profile means the route stays disabled. Bind redemption
to the same live actor, invocation, grant revision, runtime epoch, secret
version, route, profile and endpoint. Reject mismatches before DNS. The
worker receives the projected typed result or a safe refusal. It cannot
choose headers, override endpoint or read the credential-bearing response.
Credentialed routes refuse redirects. Generic proxy requests accept no
handles or credentials.

For egress, require exact approved hostnames before DNS, pin checked
addresses and TLS identity, and recheck authority, hostname, address and TLS
on each allowed redirect. Approval explains URL disclosure including names,
paths and queries. Media stays anonymous. See Q6–Q7 for approvers/defaults and
Q9 for the Owner’s earlier TCP decision.

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
| Proxy · `content_prepare`, `proxy_request`, `credential_call` | Authorized sealed content revision → opaque asset refs. Declared HTTPS destination and bounded typed request → bounded response. Unapproved destination is `403`, oversize `413`, quota `429`. Anonymous media keeps the existing 10 MB, three redirects and 16 hosts/minute fixture ceilings. Typed call returns the profile output or `403` without contacting the endpoint. |
| Turns · `turn_start`, `turn_status`, `turn_cancel`, `turn_follow_up`, `turn_answer`, `turn_select_machine` | Thread ref to start and Turn ref for run controls, with typed payload → admitted turn, bounded status, queue state or protected-request receipt. A person can select their own eligible enrolled machine and login for their own turn. The host resolves eligibility and approval. A plugin cannot select arbitrary placement. Host drafts resolve to checked message text and attachment refs at submit. Transcript pages and append are client-to-host. |
| Schedules · `schedule_create`, `schedule_remove` | Declared eligible operation, pinned target/payload, time zone and calendar or enrollment ID → durable enrollment or removal receipt. One indexed, bounded write. Resume requires the enrollment and live authority. |

The Turns engine owns the `threads` kind, with indexed status. The item
engine exposes an indexed, host-maintained `trashed` boolean for lifecycle
views. Plugins cannot set it as metadata or claim host-owned kinds. An
authorized built-in lifecycle action (Trash, Rename, Restore or Undo) runs
directly against the host operation catalogue and keeps the runtime asleep.
Rename status and protected Undo are host-owned, including when initiated
through an interface action. A renderer gets only authorized status and a
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

Job admission persists operation, target and grant. Revalidate at claim,
terminal park and resume, including any child and descendants. The runner
validates a terminal `parked` result before atomically saving checkpoint and
wait through the existing jobs engine. There is no post-park acknowledgment.
Inbox answer/approval, typed signed wait delivery, deadline and cancel race
through the existing winning-signal transaction and request receipts. The
losers have no effect. Park uses no worker once the run ends. Retain existing
wait bounds and state visibility. Repeated runner execution can occur after a
crash. Committed effects replay by receipt.

## 4. Host obligations for descriptions

The native and web renderer lanes own `contract/description-v2.schema.json`
and `docs/description-contract.md` on `lane/description-contract`. Integrate
that shared schema before freezing descriptions in either format. Until it
lands, the manifest’s command-input reference points to the revision 2
draft for validation only. Retarget that reference at integration. The
[chrome baseline](https://git.kayg.org/kayg/calternal-ai/src/commit/05aac6a4f173b336eb556fa3e6b7db9d31f4fba1/contract/chrome-v1.schema.json)
and [catalogue](../packages/genui/src/catalog-data.json) are renderer inputs.

- Pin data-free bundled templates by (package digest, app, description ID).
  Serve them as bounded assets independently of the broker frame cap.
  Render from host cache and engines with zero wakes, including cache miss
  and public reads. Keep filled results scoped to the viewer and discard
  them on session change or revocation.
- Before querying, authorize package/template, audience, kind, collection,
  predicate fields and operands, sort, search corpus and output fields.
  Apply row authorization before computing count, rank or page order.
  Deny the query as a unit if a field or operation is outside that
  projection. Use host-owned projections for guests, including counts,
  eligible fields, statuses and safe errors. Only explicitly shared engine
  data can populate them. Guest-visible writes require a host sharing or
  disclosure check. Bind cursors to checked query and audience.
- Resolve actions against the authorized rendered template, action, row
  and target. Ignore client-supplied operation names or mutable selection
  changes after confirmation. For mutations or disclosures require the
  host’s trusted preview of material arguments, including target, fields
  disclosing data and destination. Bind confirmation and request ID to
  the exact normalized input. Reauthorize at execution. Built-in item
  actions stay on the host and share the exact engine fences.
- Resolve protected component routes and check their refs under the same
  audience and action policy. Transcript delivery is client-to-host.
  Use byte-bounded durable pages and committed cursors instead of the
  ephemeral job ring. Batch append, rate-limit delivery, reauthorize each
  page and cancel the old load when selection changes. A slow reader
  resumes from an authorized cursor or receives a safe reload.

## 5. Sharing and aggregation

Members trust the Owner's installed plugins with data available to authorized
calls in that space. A warm worker can remember an Owner value and return
it to a Reader acting on their own ticket. Host-filled bindings protect
engine query results, and the installed-plugin trust rule still applies to
what a worker knew or previously wrote. Run all guest logic in the home
space charging its quota. Guest outputs and public links are host-rendered
and grant-checked, with free-form worker output excluded. A public
link serves a saved host view and never wakes.

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

Private package database, description patches and base revisions, viewer
projections, worker-published feeds, room hook acknowledgments, standalone
inbound dispatch, direct sockets, brokered TCP pending Q9, private-network and open-internet proxy grants,
guest Prompt execution, snapshots, and gallery listing fields are outside
the first contract. Live room sync is an engine facility. Gallery listing
policy stays in the submissions repo. These cuts do not retire existing
features from the current API.

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

| Case | Assertion |
| --- | --- |
| Wire, quotas and control | Invalid UTF-8, duplicates, deep or oversized JSON, unknown fields, forged package/epoch, frame replay and wrong direction have no effect. Saturate one package and one space while another makes progress and controls arrive or the stalled carrier fences by its watchdog. |
| Independent permission matrix | Exercise Owner, Manager, Editor, Reader, foreign account, guest View/Comment and revoked ticket across methods and deliveries. Match expected capability/target outcomes from independent fixtures. A compromised supervisor reaches neither a second space nor machine control. |
| Commit and recovery | Race role/session/plugin revocation with effect commit and egress dispatch. Crash before and after save/park and reconnect with the same receipt. Race answer/deadline/cancel and assert one winning effect. Reject stale handles and changed agent-authored digest. |
| Scoped render and egress | Seed a warm worker with privileged data and assert the guest sees only host-bound fields. Check counts/windows, empty/load/error, selection coalescing and no wake on a cold read. Refuse disallowed URL/DNS/redirect without connection or foreign media. Verify audit redaction. |
| Lifecycle and migration | Coalesce wake, bound drain, replace after fault, verify clean boot and exhausted admission. Verify upgrade, remove, active job fences and real/fake parity. Test that parked jobs and signed wait signals survive restart. |

## 8. Build order and gates

### Step 0 — Spikes before freeze

Exercise the real engines and enrolled carrier on disposable fixtures.
The time limits below are spike gates measured after readiness on the
documented fixture, separate from the product limits in Q1/Q8. Record
wall, payload, rows visited, reads, commits and flushes as applicable.
Run timings that claim a performance result on the designated perf VM.

| Spike | Injections and pass condition |
| --- | --- |
| **One enrolled channel** | Run two real package processes multiplexed over **one** enrolled channel for Space A and a channel for Space B. Stall A's real write path mid-frame by blocking the carrier receiver until buffers fill. Also run a deliberately slow callback. With a writable carrier, a control frame and the other package’s callback finish within 500 ms while the slow callback is held for 2 s. With the carrier physically stalled, the host fences cancel/revocation within 500 ms and closes the channel within 2 s. Space B makes progress within 2 s throughout. On reconnect reject prior epoch frames/tickets and accept an authorized receipt replay once. Verify the remote stops by lease expiry with no delivered cancel. |
| **Cold indexed page** | Seed 100,000 rows with mixed visibility. Request the same 80-row window at offsets 0, 10,000 and 90,000 under a stable sort. Record query plans and rows visited for both window and count. Page work grows with the page and index lookup, never with the offset. Any indexed count cost is reported separately and cannot repeat on adjacent windows at the same authority revision. Each window completes within 500 ms and wakes nothing. Revoke between query and delivery and assert no count/rows leak. Replay the same admitted action once. |
| **Atomic large save** | Write an escape-heavy 2 MiB body through bounded stages. Reconstruct byte-for-byte with revision-pinned range reads, even after the live head advances. Race two begins, lose a chunk ACK, replace a chunk with different bytes, cancel mid-stage and crash with an orphan. One reservation per receipt and no partial edit. Race each of revocation, cancellation, epoch loss, enrollment removal and run replacement immediately before and after the recovery-visible commit. Inject crash after content commit before journal publication and after publication before reply. Before commit yields no revision or success receipt. After commit recovers one revision, one journal event and a correctly scoped replay. Save plus reconstruction completes within 5 s on the fixture, and restart reclaims orphan reservations before serving. No worker/hook blocks the commit path. |
| **Durable transcript** | Append batches from a real turn, stall the reader, cut the stream after commit before its ACK, then reconnect from the last committed cursor. Backfill starts within 2 s and yields contiguous committed events without duplicating the submission. Revoke before delivering a page and assert no subsequent bytes or bookmark are disclosed. Slow delivery cannot monopolize the writer. |

If a gate misses, record the measured failure and change the mechanism or
reduce the first surface before freeze. A fixture using separate per-package
transport or an in-memory broker is not evidence for the carrier spike.

### Step 1 — Freeze exercised schemas only

Freeze only models and fields exercised by Step 0. Candidate slices are the
space carrier, the target and item range/save receipt path, the cold page
query and transcript cursor. Generate matching Rust-backed schemas and TS
types. Exercise a renderer-owned description fixture before freezing those
nodes. Mark other manifest/engine declarations draft and leave them out of
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

Deliver the versioned guide rewrite and compile each example. Run the public
guide and scaffold with a fresh small-model author, first with the fake
broker and then through installed real fixtures. Pass when built-in verifier
cases all pass, the author uses only public documentation/SDK input, there
are zero failed commands, and a second verifier run passes unchanged.
Exercise a bound page, typed parked question with answer and deadline, and
an intentional refusal/retry with an actionable expected response.
Before freeze, paper-walk a list/form/reminder, Notes and Threads against
both formats with the shared renderer schema, and walk Mail’s list, sealed body, credential and
new-mail paths. Identify the real-mail dependencies that remain deferred.

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
  linked-user View/Comment and revocation, no worker wake for built-in
  lifecycle actions, and recover a committed save after rollback.
- **Threads.** After Notes passes, open an indexed thread list, submit a
  composer draft with an attachment on the person’s eligible machine,
  reconnect at a committed transcript cursor, queue a follow-up, act on
  a protected approval, and stop. Require exactly one submission per
  receipt, correct machine/login attribution, no lost committed transcript,
  and equivalent rollback/reopen behavior.

Rollback is a release-pair exercise. Show that a committed change remains
readable exactly once after rollback or refuse the downgrade before it
can discard the change. Mail still waits for a later engine contract.

## Open questions for the Owner

- **Q1 Interactive deadline — OPEN.** Recommend a three-second execution budget after a distinct bounded wake allowance, subject to the carrier spike.
- **Q2 Ticket lifetime — OPEN.** Recommend at most 60 seconds or until cancel, terminal or deadline, with renewal only through live durable job admission.
- **Q3 Guest access to a future package store — OPEN.** Recommend granting none by default and revisiting only with an explicit item-bound profile.
- **Q4 Gallery age ratings and screenshots — OPEN.** Recommend submissions-repo policy with optional age rating and one representative screenshot for visual apps.
- **Q5 Private package database — OPEN for a later contract.** Keep the decided broker-only first release. Revisit only for a use case that existing engines cannot cover.
- **Q6 Destination approver — OPEN where the decisions overlap.** Preserve the earlier Manager-approved named envelope with personal hosts inside it and the later Owner approval for plugin-wide changes. Recommend confirming the split and who can widen an envelope.
- **Q7 Media default — OPEN (#80).** Recommend outbound disclosure at install and following #80’s recommended default on for a Manager-installed package once its destination policy is approved.
- **Q8 Plan mapping — OPEN pending measurement.** Recommend choosing plan ceilings from measured wake, memory and saturation rather than embedding tiers in the contract.

- **Q9 Brokered TCP — OPEN for sequencing.** The Owner decided on generic TCP
  to an approved host and port. Keep that decision on record. Recommend a
  later dedicated broker profile with a pinned endpoint, byte and time limits,
  and revocation proof. Direct runtime sockets remain forbidden.

## Second-round review responses

The [revision 2 response table](https://git.kayg.org/kayg/calternal-ai/src/commit/29028acf25b2efc99b98194b777187c23bd8fa90/docs/plugin-api-v2.md#review-responses)
records the first round. **Renderer dependency** marks work assigned to the
native and web lanes. Part A specifies the host obligations in §4.

| Review item | Response and section |
| --- | --- |
| Sol #1/#4 · Bind/Invoke placement, row action and ID | Renderer dependency. §4 requires action resolution against the rendered template and row. |
| Sol #3 · Guest fields, statuses and errors | §4 defines host-owned projections and supported disclosures without worker results/errors. |
| Sol #7 · Rename status | §3 adds the scoped status read and host-owned Undo. |
| Sol #12 · Cancellation, epoch loss, schedule removal, run replacement | §3 orders every invalidation against durable commit. §8 races both sides. |
| Sol #14 · Descendant registration and rights | §5 assigns edge creation to engines, intersects effects at each hop and denies guest jobs. |
| Sol #17 · Typed credential redemption | §§1, 3 and manifest add operation-declared routes with endpoint and host profile. |
| Sol #24/#26 · Actual stalled writer and interrupted saves | §8 uses one shared enrolled channel and stalls its physical writer. Reconstruction, interrupted staging and a second space are tested. |
| Sol #30 · Inner discriminator and misleading examples | §7 names the exact inner discriminator and gates a rewritten v2 guide. |
| Sol new · Query side channels | §4 authorizes predicate, sort, search and output before work including count or rank. §8 tests unauthorized disclosure. |
| Sol new · Confused deputy | §4 ties invocation to rendered target and requires a host preview and confirmation bound to normalized input. Renderer dependency for presentation. |
| Sol new · Recovery-visible commit | §3 locates the authoritative transaction and specifies both outcomes. §§3, 5 fence descendants. |
| Sol new · Concurrent staging | §3 uses the receipt namespace, checked input, live invocation and bounded cleanup. |
| Sol gates · Steps 0–3 | §8 adds fault injections, time limits and offset work, limits freeze to exercised schemas, and defines wall, recovery, author and app passes. |
| Opus #1 · Row template, collection ID, chrome placement | Renderer dependency. §4 authorizes resolved collections and rows. |
| Opus #4 · Forms, row scope and action IDs | Renderer dependency. §4 binds the resolved action and checked input at admission. |
| Opus #5 · Composer input and transcript refs | §3 specifies checked host drafts and separate thread/turn refs. Component events are a renderer dependency. |
| Opus #7 · Purge, restore and Undo | §3 adds confirmed purge/Empty Trash, separates restore modes and gives the interface authorized status and Undo. |
| Opus #9 · Calendar, missed runs and notifications | §3 reuses canonical calendar syntax, creator authority, same-day catch-up and receipt-backed notifications. |
| Opus #29 · Template SDK | §7 adds `defineView` and `defineChrome` against the exercised shared schema. |
| Opus #30 · Version labels and guide | §§1, 7 and manifest collapse author versioning. §8 checks rewritten examples. |
| Opus new · TCP decision | Q9 records the Owner’s decision and proposes a later brokered profile. |
| Opus new · Viewer/route filter sources | Renderer dependency. §4 requires authorization of resolved operands. |
| Opus new · Threads and Trash queries | §3 uses Turns-owned `threads` and host-indexed `trashed`. |
| Opus new · Room-compatible metadata save | §3 checks the room-backed head and preserves body/checkpoint. §8 tests concurrent editing. |
| Opus new · Built-ins keep the worker asleep | §§3–4 specify host-catalogue dispatch. §8 verifies no wake. |
| Opus new · Approval models | §3 reuses the protected Inbox approval record, action and receipt for jobs, Turns and host confirmations. |
| Opus new · Duplicate method declarations | §1 and manifest use only operation-level methods. Install unions them. |
| Opus new · Ambiguous Bind scope | Renderer dependency for binding ID and row scope. §4 requires host resolution. |
| Opus paper exercise · Apps, cold open, Hello World and Mail | §§1, 4, 7–8 hide digests/protocol labels, simplify risk and versions, require cold reads and list/form/reminder/Notes/Threads/Mail walkthroughs. Shared Bind/Invoke schema is a renderer dependency. |
