# Plugin API v2 · revision 14.14 draft

The first contract covers host-drawn apps, generic items, durable jobs and
generic conversation and execution engines. Freeze after the §8 spikes. Fixture bounds are test settings. See the
[Owner’s interface, runtime and sharing decisions](open-questions.md#recently-decided-2026-11-17-now-in-the-decision-records).

This draft is a first-release delta on revision 13.18 at `0466f80c`.
The draft also works through the renderer findings against the sealed revision 13.
[Description §5.1](description-contract.md#51-host-surfaces-and-navigation-on-the-wire)
and its catalogue entries define the host wire for rendered components.
A route operand is a checked navigation hint. Only a fresh host context
confers activation authority. Row-image reads go to the originating view
host using its bearer and current view lease. A refused or expired image
is absent and uses the existing fallback. The renderer formats RFC 3339
timestamps locally using the device time zone and locale.
See the [sealed generic engines proposal][generic-engines] and
[independent runner walkthrough][runner-walk]. This draft applies the
[2026-11-20 owner decisions][morning-decisions]. The remaining questions
and qualification gates are listed below. Existing provider behavior stays in the
current engine while the new path is gated.

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
mail ingestion and credential custody.

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
Declare the property named by `title_field` directly in the kind’s
`schema.properties`. A root `$ref` replaces that declaration under Draft 7.
After following any property `$ref` chain within the embedded
metadata document, the property schema declares `"type": "string"` or
`"type": ["string"]`. Patterns, combinators, conditionals and literals can
constrain values but cannot establish the declaration. Install refuses a
title with no such declaration.

For indexed fields, the canonical declaration is the explicit catalogue
`FieldType` in the kind’s `indexed_fields` or in the host catalogue for a
host field. Extractor destinations, set-valued fields and operands, and
sort keys use that declaration. The metadata schema and stored values
cannot establish an index type.

A kind’s optional `host_create` declaration opts its schema-valid item
defaults into the generic host create (§4.4). The optional checked
collection placement belongs to the same host transaction. At install, resolve any
`host_create.after` option to the same package’s operation or the job’s
`run_operation`. Require `target: {kind, field}` for the created kind and
a closed input object declaring exactly that one required property as the
catalogue’s `ItemRef` reference. Resolve it through the host registry and
ignore schema annotations. After commit, the host binds the created ref
to `target.field` as the sole input. This follow-up has its own dispatch and
authority and cannot delay the host operation or its Undo.

The host owns the qualified list and search projections of item text and
declared indexed fields. At install, reject declarations that reuse any
name in the catalogue’s `generic_item_fields`. The host index builds the list projections and the
background search worker publishes the search projections. A plugin’s jobs
produce derived outputs and facts. The plugin writes facts it wants people
to see onto items as declared fields.

Any package kind can opt into `conversation: true`. The orchestrator
declares `threads`, and independent runners may declare their own kinds.
A conversation gets a canonical Markdown file, bounded transcript,
authorized message corpus, scoped parent edge and checked
`active_execution` projection. The Inbox supplies viewer-scoped
`waiting_on`. None of these grants package or viewer access. The host
transcript engine is the sole conversation body writer. Conversation
kinds cannot declare a live editor room. See §3 for the corpus fence.
Provider contributions pin an adapter, image digest and model options.
IDs are open and unique across the installation, including bundled
contributions. Refuse a duplicate at install. Before profile lookup, pin
the resolved contribution's package identity, manifest digest and adapter
digest. Use only a profile bound to that contribution. A replacement
requires fresh resolution and eligibility checks.
A distinctive provider mark requires a
verified publisher that owns the canonical mark name. Its binding comes
from the existing trusted signed gallery entry or an operator checksum
pin that explicitly binds the approved artifact, publisher and mark names.
The host checks the installed contribution against that binding when it
projects the symbol. Use the generic symbol even on a fresh or offline host
when verified ownership is missing. The catalogue’s `provider_marks` rule
applies to every provider, including our own.
Claude Code and Codex use the existing
[provider protocol](../contract/provider-v1.schema.json) and
[events schema](../contract/provider-events-v1.schema.json). The host chooses
and launches the adapter inside the agent sandbox, outside the plugin wall
and without machine-control authority. Provider-only and static packages
can omit a broker worker. Operations, jobs, schedule eligibility and
credential routes require one.

The bundled Claude Code and Codex provider contributions each ship their
signed gallery entry with their distribution package. Verify the entry
through the same gallery publisher key and package digest checks used for
community contributions. Use the verified entry when showing the provider
mark. A bad signature, different publisher or mismatched package digest
drops the verified mark.

A kind with `claimed_text: true` may declare `searchable: true` to opt into
the host’s background search projection and the claimed-text plans in
`query.search`. Omission or false leaves the kind out of search. Notes
uses this same manifest declaration. The host checks the installed kind’s
declaration, the requester’s Bind reads and live scope and field authority at query
time. A foreign-kind search uses the owning installed package’s kind
declaration and the requesting package’s `reads`. The host owns the
separate scoped conversation-message projection. Apply corpus, field,
package and viewer checks to transcript, search, quote and count.

Reject unknown or inconsistent declarations before launch. Reserve the prefixes in the pinned [host catalogue](../contract/host-catalogue-v2.json#/reserved_operation_prefixes). Check unique IDs,
package operation namespace, references, schema compilation offline, pinned
assets and digests, indexed field types, and cross-reference constraints.
Each operation lists its host methods once. Derive the install summary by
union and constrain each call by that operation’s capability and risk.
Derive the mutation flag from risk. Read operations declare `risk: "read"`
and `idempotency: "read_only"` and cannot mutate user data. A job may checkpoint its own run as part of the durable job
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

Hosted spaces use per-space microVM isolation. The space has three VM
kinds with separate lifecycles.

- The small plugin VM serves taps and light jobs, with each enabled package
  in its own process with OS permissions, read-only code and resource accounting.
- The larger plugin VM starts on demand for heavy jobs. The manifest job
  declares `placement: "heavy"`. It uses the same plugin image and sandbox
  rules with more memory. A job without that declaration uses the small VM.
- The agent VM starts on demand and contains only that space’s agents as
  rootless containers. Heavy work may also use a machine the person adds.
  That machine may index a shared space only with the space’s explicit
  decision and the machine owner’s agreement.

Plugin code and agents never share a VM. No userdata tree is mounted into
any VM. Plugins and agents reach items only through checked broker calls
under their own scoped authority. Official and community packages use the
same API and limits. Self-hosting with KVM uses the same VM arrangement.
Without KVM, plugins run in sandboxed processes and agents run in rootless
Podman containers. Label both as sharing the machine kernel.

The trusted plugin supervisor can impersonate packages using tickets it can
see in its space. It gets a revocable, single-space lease. Machine enrollment,
placement authority, the host broker and provider logins stay outside the
plugin wall. A plugin cannot open another process’s files or pipes, read
host databases or credentials, select a space root or use raw network sockets.

| Placement state | Observable transition |
| --- | --- |
| Asleep | Serve authorized engine reads and saved templates. An admitted logic call, runnable job, due schedule or agent start wakes its assigned placement if asleep. A signed wait webhook wakes only via a winning signal that makes a job runnable. Cache misses, page reads, public links and rejected requests do not wake. |
| Waking | Coalesce concurrent admissions into one wake and lease per placement: small plugin VM, heavy plugin VM or agent VM. A tap and a due heavy job in a sleeping space wake two placements. Reserve capacity first. Recheck queued authority before first dispatch. Late startup after cancellation discards the old work. |
| Awake | Reuse processes between calls. After the separate wake allowance, the host publishes a result or `pending` at the interactive cutoff below. A pending call keeps its original ticket, request receipt and status ref. It may continue until that ticket expires. Long work uses a durable job with progress. A package crash retires that process and its tickets. Other packages continue. |
| Draining → going to sleep | Stop admissions to the current epoch, drain within the placement bound, cancel/fence the remainder and retain committed engine effects and checkpoints. Close I/O, stop with acknowledged teardown or durable placement revocation, then sleep. A racing wake coalesces after shutdown. |

Nothing in memory is required to survive sleep. First release proves clean
boot only. A later snapshot restore must consume one host-side,
single-use runtime epoch bound to space, wall image, supervisor and package
digests. Restore cannot reuse inherited tickets, handles, descriptors,
credits or random state. On every plugin VM wake, before plugin code runs, initialize the guest kernel
and in-process random generators from fresh host entropy. The no-KVM path
starts fresh processes with fresh entropy. On snapshot wake explicitly reseed both layers, including every
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

Wall measurements qualify the tested configuration. Measure all selected
placements with the full workload before freeze.

**Interactive cutoff.** Start the response clock at dispatch after wake
and admission. If the invocation is still running, return a host-owned
`status_ref` in the pending reply and record a bounded diagnostic.
The host publishes `pending` within three seconds of dispatch on every run. On the phone link, the caller receives it within 250 ms of host publication on every run. Do not launch a second invocation on retry. The
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

Each awake plugin runtime placement has one bidirectional authenticated
channel over the enrolled machine carrier. Handshake agrees one major and credits, binds lease to space
and starts a new epoch. A trusted endpoint stamps the package and pinned
digest. Use strict UTF-8 NDJSON, one object per LF, with a 256 KiB encoded
object cap (fixture). Charge capacity before parsing. Bound the line buffer
and reject invalid UTF-8, duplicate keys at any level, more than 32 nested
containers, unknown fields and wrong-direction messages. A malformed child
frame fences that process. Outer corruption or disconnect closes the channel.
Check raw bytes before a parser that collapses keys.

The outer header is `{v:1, epoch, seq, package, digest, ticket, message}`.
The host-to-worker `HostWorkerInvokeFrame` carries an `invoke` message
with operation, checked input and deadline. Claims and replays of a job
declaring `execution_intent: true` must include the host-stamped
`message.job_context` (`ExecutionJobContext`). Every other invocation
omits it. Refuse wrong-direction invokes or supplied context on worker
input. `call` carries
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
4. **Attempt by execution lease.** An agent gets a distinct principal and
   lease for the exact attempt, separate from the package invocation
   ticket and the person’s own session. When the sender asks the agent to find things,
   that request grants bounded space search and listing in the thread’s
   space under the active permission mode. Full access and the default
   Approve for me both allow those reads without a second prompt. The host
   binds the request to the authenticated sender and checks that person’s
   current space, scope, item, corpus and field rights before querying and
   at delivery under the transcript audience rule below. Apply the
   existing operation bounds and access records.
   Counts, ranking and excerpts use the same authorized projection.
   Other broker effects require their checked refs and existing ceilings.
   Raw enumeration, bulk exports and delegated tickets cannot widen the
   grant. Child execution keeps the existing ancestor ceilings. Recheck
   the exact attempt, runtime fence, live session or scheduled principal,
   grant, package generation
   and placement on renewal, broker effects and egress. Revocation fences
   effect and delivery immediately and stops renewal.

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

**Ending attempt authority.** The catalogue's
[`attempt_authority_end` table](../contract/host-catalogue-v2.json#/attempt_authority_end)
defines the durable fence and post-commit work for each action. In
confirmed archive the exact admitted-attempt fence and recovery work
commit in the FULL archive transaction before the reply. Undo leaves the
fence in force. Before dispatch or after a crash, the local committed
fence already blocks renewal, queued tools, effects and egress, including
descendant effects. Remote Stop and verified teardown consume the
deduplicated recovery work after commit. Release quota after verified
teardown. Snooze changes the actor's deadline only and schedules no Stop.

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
The worker cannot choose the recipient or supply feed text. A notification
may carry the host-checked enrollment label, status and authorized link.
Omit projected fields, body excerpts and worker payloads.

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
| Queries · `items_list`, `items_search` | Kind/optional collection, typed indexed metadata predicates, declared sort under the [catalogue’s sort rules](../contract/host-catalogue-v2.json#/query), cursor/window and field set → authorized page with cursor and count/offset where windowed. Unindexed filter/sort is `400`. Use engine-side query and bounded row/byte budget. Apply authority before count. No whole-kind materialization. |
| Rooms / sharing · `sharing_check` | Item ref plus requested action → permission for the live grant. A room is an attribute of the kind, opened client-to-host using its own bounded protocol. Room membership and events keep fencing the item grant. No plugin hook acknowledgment. |
| Jobs · `start_job`, `checkpoint_job`, `job_status`, `cancel_job` | Declared job, payload and resolved target/grant → durable job ID. Checkpoint only the owning current run. Status/cancel uses a scoped job ID with opaque `404` for foreign jobs. Bounded checkpoints, live progress and park count per space/package. |
| Settings / secrets · `read_setting`, `set_setting`, `request_secret_handle` | Declared key or secret purpose → effective value/version or opaque handle. Validate type, compare revision on writes and bind personal values to the actor. One bounded engine operation. Missing authorization is `403` or opaque `404` as appropriate. |
| Proxy · `proxy_request`, `credential_call` | Approved install line and bounded HTTPS request → bounded response. The host-only sealed content route exposes authorized opaque image refs to the renderer. Unapproved destination is `403`, oversize `413`, quota `429`. Anonymous media keeps the existing 10 MB, three redirects and 16 hosts/minute fixture ceilings. Typed call returns the profile output or `403` without contacting the endpoint. |
| Execution · `execution_start`, `execution_status`, `execution_input`, `execution_spawn_child`, `execution_stop` | The catalogue types each call. The host issues the admitted intent, resolves provider and placement, and binds every effect to the exact attempt under live authority. Start, input, child and Stop use checked receipts. |
| Schedules · `schedule_create`, `schedule_remove` | Declared eligible operation, pinned target/payload, time zone and calendar or enrollment ID → durable enrollment or removal receipt. One indexed, bounded write. Resume requires the enrollment and live authority. |

The package owns its conversation kind. The canonical transcript engine
alone writes its framed body, including role and provenance. Generic create
can initialize an empty conversation. Generic metadata changes preserve the
body. Refuse body replacement through save, staged upload, history restore,
generic body rewrites and live rooms. Raw body delivery, including editor
open, pinned ranges, revision material and export, checks live
message-corpus authority at read and egress. A title-only viewer learns
neither body bytes nor body-dependent revision data or bookmarks. The item engine exposes an indexed, host-maintained `trashed` boolean for lifecycle
views. Plugins cannot set it as metadata or claim host-owned kinds. The [host catalogue](../contract/host-catalogue-v2.json) registers typed
direct engine dispatch. Direct built-ins do not wake a worker in the
activation exchange. Protected Resume schedules the owning composer
job after the durable commit.
`host.item.patch-metadata` uses the same room-checked save path.
Rename status and protected Undo are host-owned. A renderer gets only authorized status and a
host-minted Undo action reference.

#### Generic execution, transcript input and protected requests

**Execution intents.** Prepared Send and protected Resume are the two
origins of an execution intent. Apply the
[catalogue’s `execution_intents` rules](../contract/host-catalogue-v2.json#/execution_intents)
to either origin. Under the conversation lifecycle fence, close admission
for archived or deleted heads and a cutover-fenced writer. Reserve the
same quota for either origin.
At most one unresolved intent or active attempt occupies a conversation,
and an intent admits at most one attempt. An authorized replay returns
the original result. Stop and protected archive pin the exact intent or
attempt and recheck the starter or live `PluginsManage` and the relevant
item right. A committed pending-intent refusal survives Undo. Snooze
and its Undo leave execution state and retry policy alone.

Prepared Send commits its message with the intent, Starting, receipt and
owning job under that fence before any wake. The
same-package Send declaration targets its own conversation kind and
names the replayable job to enqueue. The job declares
`execution_intent: true`. Its run operation targets the same kind and
accepts exactly its required target `ItemRef` in a closed input object.
At install, resolve these declarations and reject aliases from generic
dispatch, scheduling or create follow-up into the host composer.
Only the host composer admits its execution-intent job. At claim and
replay the host injects
`message.job_context.intent` outside operation input, bound to the exact
durable intent and owning job receipt. `execution_start` accepts only
`request_id`. Under the lifecycle fence the host resolves the intent and
owning receipt from the authenticated job context and rejects missing,
foreign or retired context. Replay needs no intent echo. Under the shared
rule it returns the admitted result even with a new request ID.
The host validates the final material, provider options and eligibility
against installed declarations. When `steer_live` is set and the prepared
attempt is still live, Send forwards checked text directly to
`execution_input` without waking the plugin. The exact attempt is part
of the prepared proof. A replacement requires a fresh preview. The host
can issue Stop during Starting for either intent origin. Its context
uses the shared exact-target rule and the attempt-authority table. See §4.4 for protected archive and Undo.

**Resume after interruption or cutover.** Follow the sealed proposal
§§A.3, A.5 and C.1. A proved saved provider session may be continued by
explicit person action. Restart reattachment still requires proof of
the same live execution. The host composer issues the catalogue's
`host.execution.resume` action with a view context and accepts no
interaction inputs. Resolve shows the checked conversation, provider,
machine, original profile and current permission material. Confirm the
challenge with `ResumeConfirmation`. The host checks live membership,
edit, corpus and execution-start rights and the saved binding's exact
account, contribution, conversation, enrolled machine, profile and
generation. The current login cannot substitute for the historical
profile. Keep native session data off the wire. Recheck current
placement, the qualified provider sandbox and effect ceilings. Reconcile
uncertain effects before admission.

Apply the shared execution-intent rules and use the existing
`ExecutionJobContext` and `execution_start` path for the fresh continuation. Append no message,
replay no old effect or approval, and return `pending` with the Starting
status ref. An exact authorized replay returns the original receipt.
Resume has no Undo. The eligible person can Stop the exact intent or
attempt. On stale proof or changed material, require a new preview.
Use the catalogue's typed refusal for busy, unreconciled, over-limit
and unavailable sessions. For a retired historical binding retain only
a protected retirement marker for a proved owner. Show that owner
“Previous session unavailable. Start a new session.” through the same
protected action's refusal. An unproved owner gets no context or notice.
Authority loss and foreign targets return opaque not-found with no
effect. Send can start a separately approved fresh session.

A replayable job may park on `execution` under the owning job and attempt.
Check the durable transition cursor and install the wait atomically.
Only host-verified phase, protected request, checked child request or
terminal transitions wake it, once per receipt. Tokens do not wake the
plugin VM. An authenticated attempt sends `ExecutionAttemptChildRequest`
over its attempt channel with a receipt ID, a target conversation kind and
up to 32 distinct admitted refs. The host derives the exact parent and
actor from that channel. Under the tree reservation fence check the
exact live parent, actor, eligible target and ancestor ceilings. An exact
receipt replay returns the same live checked ref. Changed input under that
receipt is refused. Reserve before minting: one pending request per attempt,
eight attempts including reservations per tree and depth at most three,
as bounded in the catalogue. Commit the reservation, immutable request and
`child_requested` transition together before ACK. At capacity return
`pending_limit` with no receipt, record, transition or wake.

Only the owning parent job can consume the ref using `execution_spawn_child`.
Recheck authority and convert the reservation to an admitted slot with the
generic child item and parent edge in one transaction. Child rights are the
intersection of the request, live caller and every ancestor's admitted refs
and effect ceilings. A retryable refusal keeps the reservation. Release it
exactly once on consumption, terminal refusal, expiry (five minutes from
mint) or fencing. The durable deadline and fence retire queued transitions,
including after restart. A consumed or retired mint receipt cannot allocate
a new slot within receipt retention. Replay a committed spawn by its own receipt.

The placement runtime enforces the heartbeat/renewal freeze and verified
teardown cutoffs in the sealed proposal. Keep quota until verified
teardown. Reattach requires exact execution proof, safe provider transport
and fresh host renewal. Failed proof, continuity, safe transport or renewal
durably retires the exact attempt under the catalogue’s attempt-authority
table, even if the placement lease is valid. Unproven placements stay
disabled. Reconcile uncertain effects before explicit Resume.

**Native provider logins.** In the person’s own sandbox, the verified
provider program may read and maintain its own login and directory.
Its tools run as that person and can read the same files. Claude Code
uses `.claude` and Codex uses `.codex`. These logins have no broker
requirement. Bind the login to the selected profile, provider contribution
and eligible placement. Keep it out of other accounts, packages, shared
mounts, host-readable output, portable snapshots and backups. For a
shared space, the configured shared login runs with tools off until
qualification proves a boundary that protects the login from tools
and other runs ([owner decision 74](open-questions.md)). A run with a
shared transcript cannot enable tools by selecting a personal native login
or changing permission mode. The host’s scoped discovery path still
follows the transcript audience rule. Preserve the gateway and tool-effect
fences. Ordinary package workers keep the brokered secret routes above. Automatic
account switching is deferred. Automatically qualify an installed digest and
placement with the same host-operated conformance suite for every
publisher. An unqualified personal-machine harness may report coarse
host-verified lifecycle state. It gets no trusted committed transcript
or approval surface.

The authenticated attempt sends the catalogue’s bounded common
`ExecutionDisplayFrame` subset of the provider envelope. The host validates
refs and stamps provenance. Scrub before publishing. Persist scrub carry
with the consumed checkpoint and keep private protocol frames, sessions
and credentials out of public records. Machine ingestion ACK follows
contiguous spool commit. Publication ACK follows the canonical file
boundary and receipt. Rebuild pages and search from the plain length-framed
Markdown records. The canonical file envelope keeps a host-owned
`transcript_attachment_bindings` frontmatter map from stable record
UUID to ordered checked attachment identities and immutable revisions.
Charge the map against the existing conversation file cap and retain
attachment revisions under ordinary content retention. Commit it with the record and the receipt. Use the existing frontmatter
and length-framed message format. Attachments add no message frame
type and do not alter the message bytes. Only the checked writer can
populate this key. Rebuild the projection from the committed envelope.
Strip the internal map from raw body reads and public exports. Ordinary
backups retain checked bindings and content under normal item custody.
A rollback reader must understand the binding map or refuse downgrade
before switching writers. Only commit bindings eligible for the entire
transcript audience at publication, and recheck current corpus and
attachment authority independently at delivery. Use one audience for
persisted content.

**Agent discovery and transcript audience (interim, owner asked
2026-11-20).** If only the sender can read the transcript, search and
listing use the sender’s full scoped live read rights. If anyone else
can read it, every row, field and content input to discovery must be
readable by both the sender and the whole transcript audience. Apply
that intersection before matching, ranking, counting or returning
material to the attempt, including snippets. The bound applies in both
permission modes. Recheck at discovery delivery and fence pending
derived output if the transcript audience widens or its source grants
cease to cover that audience during the attempt. Suppress the affected
output before any delivery or commit. Resume that work only in a fresh
attempt whose context excludes the ineligible material.
See the [catalogue’s transcript audience rule](../contract/host-catalogue-v2.json#/execution_attempt_authority/discovery).

Token events are transient. Apply the sealed proposal’s spool and
foreground budgets.

A protected Inbox answer is bound to the live process, exact request and
checked effect. For every provider, including Codex, a native approval is
actionable only when the provider marks it approvable and the gateway can
enforce that exact effect inside the attempt sandbox. A native
`can_approve: false`, missing approvability, or unproved containment yields
dismissal only. A card offers Approve with at least one lifetime and Deny
without lifetimes. The card uses the same named-material checker as an activation preview.
Show only the exact host-checked effect and its checked named inputs. Derive
decisions and lifetimes from the offered actions when validating and
rendering the card. The material list cannot supply them a second time. Resolve rechecks the live actor, request, approvability
and the exact lifetime offered in the context. External effects use a
separate protected broker action. A one-use decision gates the actual
gateway dispatch of the checked effect. Replay cannot dispatch it again.

#### Declarative extraction and retention

A package may declare bounded frontmatter, first-block or inline-tag
selectors in `extractors`. The host reads only declared content of the
same committed claimed-text body. Each selector’s output is a declared
`body_derived` field of one of the selector’s types in catalogue
`query.extractors`. Frontmatter emits Text, TextSet, Boolean, Number or
Timestamp. It cannot mint a host reference. Outputs and their counts
inherit the intersection of input audiences. No worker wake, regex or callback. A first-block
summary skips the title when declared and feeds the existing mini preview.
Inline tags take the manifest’s marker, hierarchy separator and exclusions.
For a shared `TextSet` output, collect all selector values under their
declared grammars, union them, and expand every nonempty parent with the
common inline separator, including parents of frontmatter entries. Use the [catalogue’s TextSet merge](../contract/host-catalogue-v2.json#/query/extractors/merge) for deduplication and ordering. Use a
flat union when no inline selector supplies a separator. Require one
separator per output and a single producer for any scalar output.
Charge the element, set and index-work limits to the merged result and
publish it with the source commit. Notes preserves the shipped scanner's
spelling and case. The host has no Notes defaults. The existing body_excerpt declaration is
the shorthand for the checked preview text selector. Notes declares its shipped grammar and
uses its package for tag rename and link rewrite. Reject `host_create.defaults`
for every extractor output, including `body_excerpt`, and for every declared
device image slot. On generic create run the host extractors on the empty
source in the FULL transaction. Follow the [catalogue’s extraction
initialization](../contract/host-catalogue-v2.json#/query/extractors/initialization). Project an
authorized empty set as `[]` and omit absent scalars, including an empty
summary or excerpt. A new image slot has no ref. Validate the resulting
state with the allowed package defaults before commit.

A `TextSet` filter uses `contains` with one nonempty Text element within
the set’s element limit. Use a literal or a checked route that resolves to
Text. A viewer or parent ref cannot supply a member. A window may project
a bounded `TextSet` array only for a declared and projected `TextSet` field.
The host
deduplicates values and counts each authorized item once. Do not sort by
a TextSet. Charge the catalogue’s total key budget on saves, sharing,
membership and Undo. Refuse excess work with `index_work_limit` under
the write fence. Required revoke fences immediately and supersedes the
affected projection while bounded cleanup runs. Qualify peak save and
scope work before opening the new indexed plan.

Generic item lifecycle applies declared Trash retention under the
installation policy, with bounded host cleanup and the same purge and
revocation fences. Notes declares its existing 30-day default. Retention
never depends on the space being awake.

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
as the source for kinds, fields, built-ins and shared wire shapes. The
host and SDK exercise the shared positive and negative fixtures. The
Python conformance tool has its own versioned test and fix path (§7). No template supplies trusted authority or operation semantics.

### 4.1 Catalogue and qualified queries

The catalogue’s `query.row_images` read checks live item and image-field
authority on every fetch. A body-derived image also requires live body
authority. Refusals return the same opaque 404 as an absent image. The host
marks body derivation from the source, including device snapshots of the body
and job output over it. Image retention follows the source revision, with
purge on revision erasure or item purge.

The [catalogue’s `Timestamp`](../contract/host-catalogue-v2.json) uses RFC 3339 with seconds 00 to 59 only, and the host records any leap second as second 59 with the fraction clamped.

The catalogue owns generic `id`, `title`, `updated`, `trashed` and
`trashed_at` fields for every item kind. `collections` and
`requests` are host-owned. Notes declares its own `notes` kind and
`pinned` metadata through the same manifest API a community app uses.
Kind declarations name an icon, a string title field and indexed fields
using the catalogue’s type and audience vocabularies. The host derives
generic fields from the engine record. Install checks the generic title
source before any projection exists. It must be ordinary Text metadata declared as in §1.
Reject extractor outputs, including `body_excerpt`, other body-derived
fields, host-owned aliases and an indexed source with narrower delivery
than the generic title. Use the same provenance for title search and
offline title display.
The manifest’s `reads` names every kind a package’s Bind may access,
including its own. The SDK fills the package’s own kinds into the emitted
list. Authors write `reads` only for another app’s kinds, including host
kinds. Check the emitted list before authorizing the viewer and field graph. A package or a field declaration cannot confer authority. A scoped row ID is the host-issued row
reference. Item rows use the kind’s row reference. Rows of a repeated
section use a `LayoutEntryRef`. A request row resolves only to an
authorized `RequestRef`. `waiting_on` and `pending_for` support the
viewer’s own equality operand only. A conversation’s `active_execution`
is the nullable `ExecutionRef` for the exact admitted attempt, available only
to an eligible execution actor on a ready member reply. The parent edge
names an `ItemRef`.
Before admission the host may issue a Stop context for any pending
execution intent. That intent has no active execution ref.

Compile one dependency graph per view, including chrome and content.
Check unique in-scope IDs, declared kinds and types, and all binding edges
before opening subscriptions. Reject cycles, ambiguous multi-row operands,
out-of-scope action inputs and references to undeclared binds or actions.
At query execution and delivery authorize the package and viewer, kind,
collection, each field of every input or output, each filter/sort,
corpus and any upstream Bind field. Check source provenance before any
projection, including a header or row title, secondary text, search excerpt,
symbol, image fallback and offline display. Body authority is separate from
a title grant. Recheck it before ranking on body fields or delivering their
values. An audience designation is a necessary check. The actual item,
source and grant still constrain the result. Cross-bind query operands are unsupported, including a foreign Bind’s
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
Apply the [catalogue’s sort rules](../contract/host-catalogue-v2.json#/query). Use half-open
`[from,to)`. The host may prefetch within the row and byte cap and returns
one catalogue `WindowReply`. Its `WindowResult` arm gives the Bind, state,
`window_ref`, result version, stable rows and the offset where applicable. Both schemas
cite the catalogue’s only request envelope,
`{type:"window", request: WindowRequestPayload}`. An absent `anchor` means
`start`. With `anchor:"end"`, `[from,to)` counts backward from the last
authorized row, with zero at the last row. Use this form for End and
near-end jumps only on non-search Binds with effective `exact` count
authority. A ready or empty End result carries `count.mode: exact`.
The offset identifies the returned row nearest the end when there are
rows. During catch-up, the active request receives `loading` with
`anchor:"end"`, without `offset` or `count`. Follow the [catalogue’s result ordering and correlation rules](../contract/host-catalogue-v2.json#/$defs/WindowResult).
Refuse an end anchor with `WindowPlanError.reason: end_unavailable` for
search Binds or when effective count mode is `estimate` or `none`.
Search End pages forward. The catalogue’s `query.window` sets the
window span. For a per-parent Bind require its edge-scoped
`parent` ref. Omit it at the root.
Count is an exact/estimate object or omitted, as allowed
by audience and plan. The chrome/content pair shares one Bind list.
`viewer_self` permits counts over the actor’s own authorized projection
only. Turn controls require current turn-specific authority. The host projects
`snoozed_until` as null after expiry and invalidates the actor’s Inbox Bind
without waking the runtime. Window locators belong to the checked query
and result version. Ordered collection windows use a scoped covering
index over collection, audience, sort key and stable ID, with a maintained
count per authorized scope for non-search list plans. Search counts
use the match-count policy in `query.search`.
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
item would exceed it. For the scalar-order path, replay touches at most
two covering-index keys per affected scope for one item save. Budget
TextSet replay against `query.text_set.max_total_index_keys`. Keep the
bounded key preflight at the authoritative mutation fence.

The first contract qualifies these plan families.

| Plan family | Required index |
| --- | --- |
| Stable item lookup | Kind and stable item ID, with a scoped authority lookup. |
| Ordered or filtered kind lists, including Inbox, Trash and self-only rows | Ordered indexes scoped to the authorized kind or viewer projection, with the supported filter and sort keys. |
| Manual collection + mutable ACL + sort, including the main Notes list | The scoped covering membership index, with current ACL applied before windowing and a maintained count per scope. Reading and sorting all members is disallowed even at offset zero. |
| Child and section windows | Parent-scoped ordered indexes for authorized child rows and personal section placements. Conversation child windows require an audience-scoped covering seek keyed by parent and declared sort order with stable child ID as the last key. Apply current authority before reading rows or the per-parent count. The kind-wide order index with a residual parent predicate does not qualify. |
| Claimed-text and conversation search | Ready, authority-versioned indexes for the scopes, shapes and depth in the [catalogue’s `query.search`](../contract/host-catalogue-v2.json). |
| Filtered counts | Non-search lists use a maintained count for the supported authorized scope or an audience-certified member estimate. Search counts qualified matches under `query.search.count`. |

The search Bind uses the [catalogue’s search profiles](../contract/host-catalogue-v2.json#/query/search) for matching, field filters,
deduplication and result order. Search covers all authorized searchable
fields by default. Both conversation tiers share the same posting,
expansion and row-work budgets.

The catalogue’s `query.search` defines the shared depth, scopes, tokenizer
and supported shapes. Settle pins the authorized scope before the first
window. Per-parent search requires a checked parent and a qualified
parent-scoped plan, including the collection dimension when declared.
Changing a collection or parent invalidates old generations, selections
and contexts for that Bind. An unsupported combination cannot fall back
to a kind-wide scan. Serve later windows while both work budgets in
`query.live_limits` hold.
If an otherwise qualified query exhausts either budget, return a
`refine_query` window containing only bounded authorized partial rows
available for the same query and published generation. Omit count and
action contexts. A broad prefix starts no expansion. It can retain only
already-issued rows for that same query, audience and generation.

Return `422 unsupported_query_plan` if the plan or index cannot satisfy
the requested scope, filters, sort, window or count within the work
budget. The typed `WindowPlanError` gives the reason. Never serve the
request with a table scan or a sort of the full candidate set.

After skip_limit the renderer pages from the nearer permitted end (start for search Binds or when the view lacks exact-count authority), keeping the original jump target as its goal. After end_unavailable it pages forward from the last loaded row. For a nonempty result set, forward paging ends on a short `ready` window, including a zero-row `ready` window at a page boundary. Keep the rows already shown. Only `state: "empty"` shows the Bind’s empty message. It means the whole result set is empty and is valid only when the requested `from` is zero. On `refine_query`, stop any pending End or forward-paging navigation, keep the rows already shown, and show the refine message. Do not retry the same window. Every other reason shows the Bind error state.

The first contract includes nested child rows and user-made sections (Q11).
`children: {bind}` uses a `source.parent: true` Bind windowed per expanded
parent, with depth at most three and no cycles. The declared edge is the
mandatory constraint. On each window request the host validates the
`WindowParentRef` issued for that child Bind in the parent row, reauthorizes
its entire ancestry and scopes rows and counts to that edge. A
`{parent: true}` operand is an optional extra predicate.
`Section.repeat: {over, rows}` requires `over` to read `layout_sections`
(`id`, `title`, `position`) and `rows` to use the declared per-parent edge.
The loader checks both and rejects any Bind used for section rows and as
a children target. Section rows present `LayoutEntryRef`. Child rows
present their kind’s row reference. Only live space members may read
their own layout, scoped to space and app. Layout membership grants no item access.

### 4.2 Live work, cache and host components

The save commits the authoritative item, receipt and replayable
publication intent before acknowledging. The window engine updates
ordered list indexes just after save through bounded background replay.
One publication commits the scoped covering keys, row snapshots, exact
counts and durable source epoch and replay watermark together. Repeat
replay idempotently. The existing search worker publishes its scoped index
and term statistics after the same authoritative commit. Acknowledgement
does not wait for either projection.

Use the [catalogue’s publication rules](../contract/host-catalogue-v2.json#/query/list_publication)
for list readiness. Check the visible projection’s source and authority
at planning and delivery. While it is behind or cannot be verified after
recovery, return `loading` without rows, a count, contexts, a cache record
or an end distance. Invalidate superseded window results and selections.
Only a ready or empty verified current generation can supply an exact
count. A hidden-only edit leaves visible state, rows, order, count and
result version unchanged. Revoke immediately at the authority fence.
Invalidate affected results and withhold them during replay. Deny scopes
the viewer lost. Power-cut and backlog qualification in Step 2 gates
enablement.

Set aggregate limits across the chrome/content pair and all tabs.
The catalogue sets 16 binds, 8 live subscriptions and 2 pending windows
per open view, 16 subscriptions per actor and 128 per space. Bound queued
bytes and query starts at all three levels. Per window cap materialized
rows at 4,096. Separately cap entries skipped by an index-only jump from
the nearer end at 250,000. Non-search lists read maintained counts
for the authorized scope. Claimed-text search can count qualified matches with exact-count
authority within the separate count-work budget. Threads search uses count
mode `none`. See `query.search.count` for qualified counts and
`query.live_limits` for count work and starts.
It emits up to four coalesced pushes/sec per bind. Authorize before spending work,
coalesce invalidations before evaluating and share a plan only within an
identical audience/authority scope. Supersede obsolete requests, return a
safe throttled state on exhaustion and preserve fair space progress.
For search Binds and views without exact count authority, End pages
forward from the last loaded row, or from the start if none is loaded.
For a nonempty result set, forward paging ends on a short `ready` window, including a zero-row `ready` window at a page boundary. Keep the rows already shown. Only `state: "empty"` shows the Bind’s empty message. It means the whole result set is empty and is valid only when the requested `from` is zero.
On `refine_query`, stop any pending End or forward-paging navigation, keep the rows already shown, and show the refine message. Do not retry the same window.
Use explicit `anchor:"start"` to continue past the absolute
jump ceiling. The host checks that `from` is exactly after an issued
ready start window for the same active view, Bind, parent and result
version, and seeks from its boundary key. These continuations carry no
count or cache record. With mode `none`, show no scroll extent implying a
total. Check effective count policy on every live delivery and send an end
distance only with exact count authority.

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
and current `cache_partition`. Replace the composer action map and its
contexts together, with omission meaning an empty map. Each context binds the
exact component, purpose and action ID as well as its lease and grant.
A composer token cannot be reused by an authored, shell or row action,
including actions in windows carried by a renewal. Re-evaluate Stop
against the exact unresolved execution intent or current attempt. Repaint only exact `retained_results`
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

The host builds the record from titles and approved offline display fields
only, excluding all body-derived parts as specified by
`MemberRowCacheRecord`. For Notes the record keeps the date as the
authorized `subtitle_timestamp`. The device formats it at draw time using
the pinned template. The snippet appears after a fresh authorized reply.

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

For the 600 ms phone cold-launch gate in §8, use the same connection
flight with and without the member row record. Use TLS resumption and pipeline authentication, `view.open` and the first windows in one application flight. The host handles them in that order.

The catalogue gives typed host component routes. `transcript.page`
takes a checked conversation ItemRef and a viewer-bound record cursor.
Return at most 128 records and 128 KiB. Reopen the boundary record on
authorized reconnect, replace it by UUID, and continue in record order.
Refresh the bounded host sign-in notices in the same reply. An empty
corpus has `committed_cursor: null` and no older cursor. Catch up without
an after cursor until the first committed record supplies a boundary.
A clipped record supplies an opaque body continuation for
`transcript.body`, bound to the viewer, lease, conversation, exact record
revision and next UTF-8 offset. Each page returns at most 16 KiB of whole
characters from that pin and an optional next ref. Recheck live corpus and
field authority at read and egress, including quotes and continuations.
An absent, expired, purged or revoked ref returns the same opaque 404,
with zero bytes and no continuation. Page and follow share
`CommittedTranscriptRecord.attachments`. The catalogue bounds this
projection to 16 distinct checked attachments per record and 16 MiB
of retained bytes per attachment. A reply carries an opaque ref,
checked media type and byte length only. The metadata counts toward
the 128 KiB reply cap. Refuse over-limit material before committing it.
An attachment its viewer can no longer read has no descriptor,
placeholder or count. Follow replaces the record's entire attachment
list and fences stale output when authority changes. Invalidate the
view lease if it cannot refresh the checked record on that subscription. A title-only or
revoked corpus viewer receives no records or cursor. Fetch a projected
ref with `transcript.attachment` under the same view and lease. Recheck
the exact immutable revision and both grants before emitting headers
or bytes and fence the transfer on revocation. These attachments are
a structured record field, separate from rendered Markdown.

The composer uses `composer.options` for checked installed
choices. Templates do not submit a second target or the protected Send
operation. See §4.3 for the cold prepared exchange.

Row images use the existing `RowImageIcon` and read route. Notes declares
a portrait aspect and the existing snippet fallback. An editing device
captures the saved revision, paints locally, and obtains the same slot’s
session-bound upload grant after the save ACK. Use the active editable
`item-editor` component to call `row_images.prepare` with the exact
committed state digest, MIME type, byte length and upload digest. The
request component must match the envelope and checked editor target.
Send the capture bytes to the upload route using the returned grant. Keep
separate bounds for upload and delivery. Upload accepts at most 512 KiB
and a 256 × 384 decoded still image, with two simultaneous decodes, 64
pending grants and 8 MiB of reserved staging per space. The grant and
staging expire after five minutes. Reclaim expired and orphan bytes and
reservations within another 30 seconds, including a sweep before new
admission after restart. Validate session, edit authority and exact source
at ingest and publication. Normalize and strip metadata, stamp authenticated
uploader provenance and make the bytes durable before committing the
pointer. Pixels supplied by an editor can be misleading. Attribution
identifies their source, and the next eligible capture replaces them.
On the authorized ready member row, `image_attribution[field]` identifies
the exact asset's uploader and host-projected label when the viewer can
see that identity. Use the older asset's uploader while it is retained.
Guest and public replies, caches and non-ready rows omit attribution.
The first valid image wins the revision and exact replay by that session
returns its receipt. The host serves the smaller normalized variant,
at most 64 KiB and 128 px on its longest edge. Keep the last authorized
image marked stale and fetchable until replacement. Delivery rechecks
current item, source, field and body authority even on a conditional
read. Unretained or unauthorized refs return opaque 404. Use the miniature
when no authorized image can be delivered. Keep image refs, decoded
images and body-derived row parts out of persisted records.

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
   Refuse catalogue operations with `available: false` at resolve and dispatch.
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
   condition. Saved authority requires a challenge for idle archive unless
   it consists only of thread grants retired in the archive’s FULL thread
   content transaction and restored with the thread head by Undo (§4.4).
   Retiring grants with head-only Undo requires a challenge.
   For `host.item.create`, a checked route or literal collection may be
   part of the same shortcut. Creating an empty item exposes no existing
   material, so no challenge applies even in a shared collection.
   For `host.collection.move`, the destination’s effective audience is
   the only challenge predicate. The chosen destination comes from a checked
   ChoiceRef. For other shortcut mutations the only material may be the
   drawn target or submitting form fields. Irreversible,
   external or access-widening effects, bulk sets, declared material
   inputs and any destination, recipient, audience or permission require
   `confirm:challenge`, including untouched prefills, defaults and autofill.
   Apply these rules outside the create and collection move rules above. The host also
   challenges unproven material for other operations. A plugin operation always
   uses separate resolve and dispatch. The manifest’s risk declaration
   grants no shortcut. On that ordinary path the host omits `confirm`
   when no challenge is required.
3. **Confirm and dispatch.** For a challenge, resolve returns the material
   required by the catalogue's [confirmation table](../contract/host-catalogue-v2.json#/confirmation/material_by_kind).
   Select the row by the checked operation and surface. Derive applicable
   entries from live checked inputs and effects, including manifest
   material inputs. Check table coverage, uniqueness and required material. Refuse a missing, empty or partial preview.
   Resolve authored operations in the originating manifest. Applicable
   material keys appear as `input:<key>` and retain their declared labels
   and checked values. Host keys are separate. A captured confirmation
   must use the originating activation ID, resolution ref and challenge.
   Apply the matching confirmation schema. Only prepared Send supplies
   `final_text`.
   The host issues a one-use proof after the authorized protected control
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
Manifest commands and mentions identify the bounded preflight parsing
rules. The host resolves them before preparing the proof. The package
must treat `final_text` as message content only. Any parser-derived
change to recipients, routing, permissions or other prepared material
(including a mention or slash command) invalidates the proof and requires
a fresh resolve and preview. Check live context and the exact prepared
material before confirming and dispatching in that exchange. The proof’s material fingerprint
excludes text only for this authenticated host composer Send. The durable
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
collections, and rechecks the destination’s effective audience under the
commit fence. Require a challenge only if that audience contains anyone
other than the actor. A newly shared destination needs a fresh challenge.
For an actor-only audience, one exchange after choice submits the move
when the engine commits it and Undo together. Otherwise resolve and
dispatch separately without a challenge for that destination. Leave
unrelated memberships intact. Personal layout uses `host.layout.move_to`,
`host.layout.move_up`, `host.layout.move_down` and
`host.layout.section.create`, `host.layout.section.rename`,
`host.layout.section.remove`. Resolve the entry and section in the actor’s
own layout against its context-pinned revision and check live space
membership and item and parent access. For Threads row actions
`{context:"item"}` yields the underlying checked conversation ItemRef.
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
| `host.item.create` | Under one FULL content transaction’s fence, check live creation authority, optional collection-membership edit authority and the normal item and membership quotas. Then commit the empty item with its permitted defaults and the host’s empty-source extraction, requested membership and Undo together. An omitted route parameter skips placement. A supplied ref that is absent, foreign, unreadable or not editable returns the same opaque `collection_unavailable` outcome. Quota refusal returns `create_limit_reached` with no item, membership or Undo committed. Undo rechecks live authority and the item head, its membership and sharing revisions and the introduced membership revision, if any. Refuse on any change. On success remove the item and its introduced membership together. Return the created ref with no runtime wake or challenge, even in a shared collection. Any declared follow-up receives only the created ref under §1 after commit. |
| `host.item.trash`, `host.item.restore` | The FULL content transaction records the item revision, recoverable placement and Undo. Full swipe covers single-item Trash and restore from Trash. Trash paths with separate lifecycle cleanup are ineligible. |
| `host.item.rename` | The FULL content transaction records the new head, placement and Undo for a single-item rename without deferred link rewrites. The current Notes job commits inbound-link changes in later transactions, so that path remains ineligible. |
| `host.collection.move` | Challenge only if the destination’s effective audience contains anyone other than the actor. Otherwise the FULL content transaction can commit the complete move and Undo in one exchange. A two-collection move is ineligible for the shortcut until both membership changes commit together. |
| `host.layout.move_to`, `host.layout.move_up`, `host.layout.move_down` | The per-person layout revision transaction records the complete placement or ordering edit and Undo for the actor, space and app. Move to still previews its destination. |
| `host.layout.section.create` | The per-person layout revision transaction creates the section and commits Undo that removes it. |
| `host.layout.section.rename` | The same layout transaction commits the title change and its prior title for Undo. |
| `host.layout.section.remove` | One per-person layout revision transaction retires the section and its placement generation and records Undo naming that generation. Retired generations are excluded from reads. Undo rechecks live membership and the post-effect layout revision and reactivates the exact generation in one transaction before its expiry. Reactivation grants no item access. Reclaim expired retired generations in bounded background batches. Removal and Undo perform no per-placement work. |
| `host.item.archive` | Idle archive uses the shared execution-intent predicate in §3. Saved authority requires a challenge unless it consists only of thread grants that the Step 2 writer retires in the same FULL thread content transaction as archive and that Undo restores together with the thread head. Retiring grants with head-only Undo requires a challenge. Check shared intent admission and archive eligibility under the same commit fence. The FULL thread content transaction records archive and its unarchive Undo together. The current writer commits the head without Undo and enters a separate protected authority transaction, so the shortcut stays ineligible until the Step 2 writer passes. After it passes, eligible idle archive takes one full-swipe exchange without a sheet. For an active attempt the host says “The running execution will be stopped.” The confirmed path applies the pinned-attempt fence or durable intent refusal in the archive transaction under the authority-ending table in §3. |
| `host.item.unarchive` | The FULL thread content transaction records the new head and Undo. A full swipe may unarchive an eligible thread. |
| `host.item.snooze` | Ineligible. Today’s writer stores one shared manual deadline. The actor-scoped operation stays unavailable until the Step 2 actor-keyed writer commits the actor’s deadline, replay intent and Undo together. Apply §3’s shared snooze rule. |
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
link serves a saved host view and never wakes. Item grants confer no personal-layout access. Layout reads and writes
require live membership. Shared spaces have no private per-person items.
Personal items live in the person’s own space and may point to a shared
item. The reference grants no access to that item.

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
The private scratch database and note thumbnail storage remain open design
items. See Owner decisions and open questions.

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

The first contract qualifies claimed-text and conversation search plans in
`query.search`. Exact distinct-thread search counts remain unqualified.

**Known limit for claimed-text CJK search.** The first contract supports whole unspaced
runs and their leading prefixes. Implicit word segmentation and interior
substring matching return `unsupported_query_plan`. A later search
generation can qualify a segmenting or trigram tokenizer.
[Unicode qualification measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/eac3eecf571c85811c1f6724e257ad19fb40aa00/docs/measurements/plugin-api-v2-step0.md#unicode-qualification).

## 7. Author SDK and conformance

Conformance is defined by this text, the description contract, the host
catalogue and the schemas. Review and seal them together. Schema
validation asserts formats, including calendar validity for date-time.
The host and SDK also apply the semantic rules below. The
[Python conformance tool](../contract/README.md#plugin-v2-conformance-tool)
and its modules and fixtures are versioned, tested and corrected
separately. Tool reports provide evidence. Derive cross-reply checks
from the catalogue’s authority, capture and confirmation tables where
possible. Tool fixes follow the tool’s release path independently of
the contract seal.

A shortcut's literal route value, including one set by its shell action, must
be an `Id` that the renderer can send in `EventRoute.row`.

| Rule class | Enforcement |
| --- | --- |
| JSON shape, local value constraints and formats | JSON Schemas with format assertion. Includes scalar rows, swipe position, parent operand shape, header operand shape and content depth. |
| Cross-reference resolution | Resolve against the accepted description and installed manifest declarations. Resolve Binds, parent edges, kind owners, installed operations and schema refs. |
| Catalogue eligibility | Apply the catalogue. Check host operations, full-swipe eligibility, reserved fields and searchable kind declarations. |
| Audience and scope | Apply the specified audience, scope and live authority rules. Check reads, per-parent consumers, contextual operands and cache opt-in. The host checks live authority. |
| Operand and projection types | Check against the declared types. Use the resolved Bind, field, catalogue entry or manifest signature. |
| Fixture origins | The conformance tool uses fixture origins as evidence. Resolve the originating description and Bind and compare with the reply. |
| Cross-value and tree-wide invariants | Apply the semantic bounds. Check window span, returned rows against exact count, duplicate action IDs and the aggregate node cap. |

`defineApp`, `defineKind`, `defineOperation` and declarative package
metadata generate a checked manifest, method inventory and immutable
asset/digest index. `defineChrome` and `defineView` author templates against
the renderer-owned shared schema when exercised. Typed `ctx` methods derive
from shared contract models.

For a first list, `defineView` supplies the standard Bind window and count
defaults and the shell’s empty, loading and error messages. It defaults the
row title to the generated `title` field. Authors may declare a leading
row icon. `defineChrome` uses the same Bind and title defaults for sidebar sections.
The SDK defaults `title_field` to a declared string `title`, adds the
package’s own kinds to `reads`, and emits `network: "none"` and an empty
`operations` list when omitted. Window and count defaults follow the
description schema and catalogue. A first list takes about eight author
concepts: package, app, kind, placement, item schema, Bind, list and sidebar
section. Add explicit author choices for actions, fields and presentation as
the app needs them.

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
small-model trial for the Reading List paper walk in description §9.1, a durable wait and refusal/retry.

**Gallery submission gate.** The gallery listing requires an icon,
description, screenshots, age rating, and a plain statement of what the
plugin connects to and why. Keep listing metadata in the submissions
repository. Private and agent-authored plugins need none of it. The
plugin guide skill and SDK must generate the listing from a checked
package, capture screenshots from fixtures and check completeness.
Before enabling submissions, a public-guide-only author must produce a
Reading List trial plugin and complete listing from the scaffold. The validator must accept
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

**Entry evidence.** Paper-walk Reading List (list, two-field Add with
created/then, Mark read full swipe, search and a one-date reminder), Notes (created ref, room edit, Move to collection,
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
at 150 ms RTT for exchange counts, the keyboard trial and cold launches. Measure indexed
engine latency without the added link delay. Protocol safety deadlines
and zero-effect assertions must hold on every repetition.

| Spike | Injections and pass condition |
| --- | --- |
| **One enrolled channel** | Two real package processes on one enrolled channel in Space A, plus Space B. Block A’s carrier receiver until its actual writer stalls mid-frame. Hold a slow callback for two seconds. With a writable carrier, control and the other callback complete p95 ≤500 ms. While stalled, local fencing p95 ≤500 ms, close p95 ≤2 s and Space B progress p95 ≤2 s. No old-lease effect after local fencing. Remote stops within 5 s of its last authenticated renewal on every run. Reconnect rejects the old epoch and authorized replay commits once. |
| **Cold indexed page** | 100,000 mixed-visibility rows. During grant/content churn test manual collection + ACL + sort from offset 0 through its last full authorized range and last visible row (at most 10,000 members, so the last full 200-row range starts at most at offset 9,800). Use a dense dynamic collection for offsets 10,000 and 90,000. Exercise search under the scoped search gate. In the cold Notes page arm, seed authorized thumbnails and record first row paint, visible thumbnail completion, image GET count and transferred bytes separately from indexed page latency. Offset 0 p95 ≤100 ms, deep windows p95 ≤500 ms. Seed enough authorized rows for every requested range and assert the exact authorized rows in order. Empty or refused replies fail. Meter materialized rows and skipped covering-index entries separately against their bounds. In a larger authorized scope, also request a range whose nearer-end jump exceeds the skip cap. Assert `skip_limit` followed by paging that returns the same authorized rows. Test End on a scope beyond the absolute offset ceiling using `anchor:"end"` and assert the last authorized rows. For stable-ID lookup assert one bounded indexed lookup and scoped authority check for a present, missing and unauthorized ID. For standalone exact counts assert one maintained-count lookup and the exact authorized total after grant churn. Exercise scalar-order saves at the 32-effective-scope cap and assert at most two covering-index keys touched per scope. Separately replace multiple indexed tags in one scope so replay touches more than two keys. Meter TextSet replay against `query.text_set.max_total_index_keys`, compare rows and counts with the live-authority oracle, and refuse an over-budget ordinary save before commit. Refuse a grant or membership that would create scope 33 with the typed limit error. Require save ACK p95 ≤40 ms with replay under load. Report save-to-ready lag separately and apply the Step 2 power-cut and backlog gate. Reject table scans and full candidate sorts, coalesce fan-out, refuse both counts and rows on revocation and wake no worker. |
| **Scoped search** | On locked perf-01 exercise the claimed-text and conversation shapes in their qualified scopes through the depth in `query.search`. Include a searchable community kind alongside Notes. For conversation search compare both tiers against the catalogue order. Seed title-only, message-only and dual matches, including tied message timestamps. Run two queries that reverse the order of a message-only pair and verify exact order, one row per conversation and the same order across page boundaries. Beyond that depth, assert ready windows with the exact authorized rows and order while both budgets hold. Exhaust each budget separately and assert a `refine_query` window with bounded provisional rows and no count. For Notes check the qualified Unicode cases, refusal before the mark-aware Devanagari rebuild, and CJK segmentation and substring refusal. Meter postings, materialized-row work and count work separately against `query.live_limits`. A broad prefix must start no expansion. Settle two parents of one Bind, then change the collection within one parent. Replay a window and both selection forms from the superseded scope with the old and current generations. No old rows, counts or context tokens. Repeat for a forged, foreign and revoked parent and both child-edge types. For a valid unqualified scope the first window returns `unsupported_plan` with zero scanned candidates. For an enabled scope compare rows and counts with a live-authority oracle. During publication lag and stale authority, assert `loading` with no rows, counts or activation contexts. Race revoke against delivery and assert zero stale results. For a reader with a ready search, edit a hidden row and a hidden field. Check before, during and after publication that the visible state, rows, ranks, count and result version stay unchanged. Measure foreground save and save-to-search lag separately. Keep maintenance out of the save transaction and meet the append gate. |
| **Atomic large save** | Stage an escape-heavy 2 MiB body and reconstruct it by revision-pinned ranges after the head moves, p95 ≤5 s. The 60 s range pin is invalidated by purge or revoke. Race duplicate begin, lost chunk ACK, conflicting bytes, cancel and orphan cleanup. Race revoke, epoch loss, enrollment removal and run replacement on both sides of commit and inject crashes before publication and before reply. No partial edit or success receipt before commit. After commit recover one revision and receipt. |
| **Durable transcript** | Stall the reader, cut after commit before ACK and restart. First screenful drawn p95 ≤150 ms on a warm authorized reconnect, with stable record IDs/order, cursor and durable receipts on replay. Reopen the boundary record and refresh sign-in notices in the same authorized reply. Revoked reconnect reveals neither rows nor bookmarks. An independent control request completes p95 ≤500 ms with the transcript reader stalled. |
| **Host + renderer authority** | On Svelte Chromium check both formats, a real protected approval and a live Bind. Forge row/route/Bind/context, untouched prefills, bulk IDs, ChoiceRefs, cached data and built-in surfaces. Inject duplicate and stale activations, revocation and unrelated churn. Zero unauthorized effects or rendered bytes, one commit on replay, and unrelated-churn action p95 ≤2 s from dispatch. Race revocation at egress, job claim/park and approval resolve before freezing those routes. Issue and fetch a body-derived thumbnail ref while all grants are live. Revoke only body access, preserving item access, image-field access and the view lease. Fetch that same ref and require the same opaque 404 as an absent image, with zero image bytes and no worker wake. For each §4.4 atomic-Undo case exercised in Step 0, crash before and after the named commit and during publication: recover neither effect nor Undo before commit, and both after it. Concurrently edit a dependency, then require Undo to refuse without another effect. Run idle-archive recovery in Step 2 on its writer. Exercise ineligible variants and prove the shortcut and full swipe cannot bypass their checks. Delay or spoof lease replies and verify only current, exact retained windows repaint. Test unverified and conflicting provider mark claims, including a fresh or offline host without verified ownership data. Only a verified publisher with a matching ownership binding may project a distinctive mark. Run the projection sweep with a title-only guest and an authorized body reader. Revoke only body access and verify titles still work. Assert no protected header, row title, secondary text, snippet, cache part, notification text, symbol, image ref or bytes. Check sorts, predicates, count and rank against the title-only oracle after edits to hidden bodies or restricted fields. Check eligible member projections and title-only search still return their expected rows. |
| **Host create with collection placement** | With the plugin asleep, create a default empty item in an authorized shared collection in one exchange without a challenge. Read the authorized row back with each configured extractor type. Require an empty set, omitted absent scalars and no image ref. Then save a valid false and zero and require both values in the projection. Crash before and after the FULL commit. Before commit recover no item, membership or Undo. After commit recover all three and replay the same receipt. Undo removes the item and its introduced membership together. Compare refusals for absent, foreign, unreadable and non-editable collection refs and require identical opaque outcomes. After resolve, delete the collection or revoke the actor’s membership edit access and require the same outcome on dispatch. Fill the item quota and membership quota separately, then require the typed limit outcome without an item, membership or Undo. Race two creates for the last membership slot and require exactly one commit. After a successful create, add another membership or a share grant without editing the head or introduced membership, and verify Undo refusal in each case. Assert zero runtime wakes throughout on a kind without a follow-up. |
| **Phone activation exchanges** | In the same Chromium phone link fixture, from action intent to host outcome, an eligible swipe and prepared Send each use one exchange. An ordinary plugin operation without a challenge uses two. On the Step 2 archive writer, an idle archive full swipe takes one exchange with no sheet and returns unarchive Undo. An active archive shows the challenge and the host-written stop material. Also exercise the cleanup cases. Require the bounds on every run and report p95 exchange counts and elapsed time. Alter prepared material, including via parsed final text, or drop the Send reply. Require a fresh preview for changed material and replay the exact final text once. |
| **Keyboard windowing** | 5,000 rows with seeded authorized thumbnails and 30 keys/sec on the phone fixture. During repeat, assert zero new image GETs and cancellation of in-flight image fetches. After settling, fetch only current refs for the visible rows. Record image request counts and bytes. At most four window requests in any second and zero selection/navigation events during repeat on every run. Report p95 request rate. After the first real rows paint, capture each viewport frame during the repeat burst. Require zero skeleton-row frames during the steady 30 Hz segment on every run. Same-version prefetch applies while keys repeat; an activation waits for the context of its drawn target. Delay a context for A until B is drawn and verify the activation cannot use A. |
| **Nested rows and user sections** | On real content and conversation engines, expand two parents and page their children independently. Check depth, ancestry, revocation and a forged parent ref. Create, rename and remove personal sections and Move to, Move up and Move down. Undo each committed edit, verify placement and order, refuse stale Undo, and prove a second member’s layout is unaffected. |
| **Persisted member rows** | Use an authenticated eligible root Bind. Verify device ciphertext and account/space key separation, reject expired or forged records and erase on known revocation, sign-out and account switch. Paint eligible rows on an unlocked launch, then confirm or correct them against the host. For a cached Notes timestamp, advance the device clock across a relative-date boundary and verify that the label updates before a fresh window. Check that unconfirmed rows have no executable actions and that guest and public windows leave no record. |
| **Phone cold launch** | Restart the client with an authorized account and a cold connection, with and without an eligible member row record. Start at launch intent and time first paint and first confirmed or corrected rows on the phone link. Require confirmed rows p95 ≤600 ms in both cases. Use TLS resumption and pipeline authentication, `view.open` and the first windows in one application flight. The host handles them in that order. The initial frame uses the local template and unconfirmed rows with a record, or the loading state without one. Confirm no action can run before fresh host context. |
| **Snapshot randomness** | Independently restore the same snapshot twice and sample the guest kernel and each restored process’s crypto RNG, including Node. Require different bytes for both restores at both layers and require fresh kernel and process randomness before any plugin code. Refuse admission if reseed or fresh process reinitialization cannot establish that. Include reseed time in measured wake. |
| **Interactive pending** | Force work past three plugin seconds, including a cold wake with the separate measured/fixture allowance. The host publishes `pending` within three seconds of dispatch on every run. On the phone link, the caller receives it within 250 ms of host publication on every run. Record both timestamps, one diagnostic and one status ref. Poll while it finishes and retry after a lost reply. Never launch again or allow effects after ticket expiry. |

Manual-collection measurements at offsets 10,000 and 90,000 were
out-of-range controls with zero returned rows. The dense dynamic collection
supplies the deep-offset measurements. The manual collection’s last full
authorized page and last visible row measured 0.158 ms and 0.101 ms p95.
In the historical Step 0 fixture, the combined save measured 31.299 ms
p95 at 32 effective scopes. The current publication decision follows
[the window-engine comparison][window-replay-measurements].
[Step 0 measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/355ab4203901f0efa89c8a6e818285cc6a9b0d8f/docs/measurements/plugin-api-v2-step0.md#cold-indexed-page).

Synchronous FTS maintenance missed the append gate at 66.8 ms p95. With
the existing background worker feeding the scoped fixture corpora, search
publication lag measured about 26 ms p95 idle and 54 ms p95 under 516
competing saves. [Scoped search measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/cb2064607e3a5c6e43b003cb408d619723c59dba/docs/measurements/plugin-api-v2-step0.md#follow-up--bounded-notes-search).

All qualified windows in the Unicode probe measured below 1.9 ms p95.
[Unicode qualification measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/eac3eecf571c85811c1f6724e257ad19fb40aa00/docs/measurements/plugin-api-v2-step0.md#unicode-qualification).

The [all-field search measurements](measurements/plugin-api-v2-step0.md#follow-up--search-across-all-searchable-fields) qualify the default search path at the catalogue’s shared depth. The earlier separate-field probes measured 5.893 ms p95 for Notes and 1.414 ms p95 for Threads. The Threads probe measured
append-to-scoped-publication lag at 46.847 ms p95 idle and 468.095 ms p95
under 1,916 competing appends. [Threads search and shared depth measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/9a78a43b1cda6a2074df9d1471752f3db5a2a0bb/docs/measurements/plugin-api-v2-step0.md#follow-up--threads-title-and-message-search).

If a gate misses, record the measured failure and change the mechanism or
reduce the first surface before freeze. A fixture using separate per-package
transport or an in-memory broker is not evidence for the carrier spike.

### Step 1 — Freeze exercised schemas only

Freeze only models and fields exercised by Step 0. Candidate slices are the
space carrier, the target and item range/save receipt path, the cold page
query and transcript cursor. Generate matching Rust-backed schemas and TS
types. Freeze descriptions, Invoke and each built-in only after its real
host/renderer authorization case passes. The description freeze also
requires the SwiftUI renderer to draw every valid fixture. Shape fixtures and the author
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
broker and then through installed real fixtures. The trial passes when the examples satisfy the pinned contract, the
author uses only public documentation and SDK input, and every command
succeeds. Keep both conformance tool runs as evidence.
Exercise the Reading List paper walk with its form, full swipe, search and
created/then, plus a typed parked question with answer and deadline and an
intentional refusal/retry with an actionable response.

### Step 2 — Integrate placement and engines

Before enabling the generic execution and preview routes, run these
paired controls on the actual engines. Contract fixtures check what can
be expressed on the wire. The runtime gates check what a host actually
authorizes, commits and delivers.

| Fixture | Required result |
| --- | --- |
| Attempt discovery and renewal | In Full access and Approve for me, ask the agent to list and search. Use the same sender and candidates in a sender-only transcript and a shared transcript. The sender-only run receives a sender-readable private match. The shared run never sees it in a row, count, rank, snippet or output, and both runs see the common match. Hidden-only edits leave the shared results unchanged. Check access records and no second prompt. Widen the transcript audience and revoke source authority during discovery and after a result reaches the attempt but before output delivery or commit. Require zero output from the affected context. Try foreign scope, forged tickets, raw bulk export and delegated discovery. Replace the attempt before renewal and require zero old-attempt effects. Repeat the existing child reservation, ancestor ceiling and replay checks. |
| Native provider login | Run verified Claude Code and Codex in the sender’s eligible personal sandbox. Verify the provider and its tools can access its own login, and another account and neighboring package cannot. In a shared space, request tool use under both permission modes and assert zero tool processes in the shared login sandbox. A personal profile must not re-enable tools for a shared transcript. Recover the same permitted profile on explicit Resume. Check that no login data enters transcript output, snapshots, backups or diagnostics. |
| Cold composer job and Stop | For each intent origin, pause after the durable intent and before claim. Stop using the pending-intent context, restart and admit the job. Require the refusal receipt, zero starts and no effect on a later intent. In the control arm, replay the claimed job with the same and then a new request ID. Require the same admitted result and one start in total. Echoing an intent in the request or calling with missing, foreign or retired host context starts zero runs. |
| Conversation body | A canonical append can be paged and, with corpus rights, its exact bytes can be read through a pinned range. A same-kind package save, staged replacement, history-body restore or room write cannot publish a forged role, provenance or bytes. A title-only viewer gets zero corpus bytes, lengths or bookmarks via transcript, editor, range or export. Revoke after opening a pin and before delivery to cover both readers. |
| Native approval and lifetime | For each provider offer Approve only on the approvable, contained arm. With native approvability false or containment unproved, forge an approval and require zero dispatches. Use a valid context with an unoffered lifetime and require zero dispatches. An exact offered one-use decision dispatches once, including on replay. |
| Read from an empty or clipped transcript | Open and follow a new conversation before its first append. Require no fabricated cursor, then one committed UUID and a usable cursor after append. Read a clipped multilingual body across pages and compare exact UTF-8 bytes with the pinned record. A copied or revoked continuation returns the same opaque refusal as an absent ref. |
| Device preview | Prepare a valid 100 KiB capture through the active editor and serve the normalized variant. On a distinct authorized member's row verify the exact host-stamped uploader ref and label. Verify the previous uploader remains on the stale asset and changes only on replacement. A guest or a member without uploader identity visibility receives no attribution. Refuse oversize, copied or late uploads. After head advance, the current authorized viewer fetches the same retained previous image, marked stale. Revoke only body authority and repeat the same fetch, including a conditional request. Require the same 404 as an absent ref, zero bytes and no 304 or wake. Cut before byte durability and before pointer commit. Require the prior authorized image or fallback in both arms and reclaim orphans and reservations by the staging deadline. |
| Merged Notes tags | Compare the declared frontmatter and inline selectors with the existing tag scanner. With frontmatter `tags: [work/q4, work]` and body `#personal #work/q4`, require exactly `personal`, `work` and `work/q4`, including one count per authorized item. Run with both selector orders. Over-budget merged expansion must refuse the save before publication. |
| Search settle race | Pin a collection at settle and verify an allowed window uses that exact scope. Send a window using another valid collection with the same generation and require a refusal. Settle the second collection with a fresh generation. Send two different texts to one Bind. Delay the first acknowledgement and a window carrying a previously unseen older result version until the second result renders. Require zero changes to rows, counts or navigation from superseded acknowledgements, results or plan refusals. Deliver the newer request before the older one in a second arm and require the newer search to remain current. |

Pass these end-to-end flows on the selected wall before app migration.

- **Space lifecycle.** Start each trial with all placements asleep. Admit a tap and due light
  work concurrently and assert one small-plugin wake. Admit concurrent due
  heavy jobs and assert one heavy-plugin wake. Repeat with the tap and a due
  heavy job in the same sleeping space and assert two wakes, one per
  placement. Include concurrent agent starts and assert one separate agent-VM
  wake. An asleep page and public/share read never wake. A package
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

- **Attempt authority after commit.** For every row in the catalogue’s
  attempt-authority table, cut after its durable commit but before reply
  and before remote dispatch. Hold the actual carrier Stop and keep the
  old process reachable for probes. Check renewal, broker effects,
  queued tool dispatch and egress, including a descendant, before and
  after recovery without releasing the Stop. For archive also commit
  Undo before releasing Stop and repeat the probes. Require refusal on
  the losing authority, no effect on an unrelated attempt and
  recovery of only the deduplicated cleanup for the exact old IDs.
  In the failed reattachment arm keep a valid placement lease, reject
  fresh continuity, then cut after committing interruption. On restart
  refuse the old proof and any new renewal of that attempt. Verify
  unchanged committed records before a fresh explicit Resume.
  Snooze is the qualified no-stop control. Cutover catches stale old
  writer claims while recovering prior authorized cleanup and adds no
  new Stop. The paper traces check the contract. The real host gate
  must prove the fences and recovery on the writer.

- **List index background replay.** The window engine lane qualifies this
  path before enablement. Power-cut before the save commit, after the
  durable ACK but before replay, during publication, and after publish
  before delivery. Recover exactly one committed item and receipt.
  Require loading with no count or contexts until the current epoch and
  generation are verified, including after loss of the projection WAL.
  Under a sustained peak and a paused-worker backlog, require bounded
  replay work and storage, drain to the live-authority oracle without
  duplicate keys or counts, and meet the 40 ms p95 save gate. Measure
  lag and backlog drain separately. For an eligible End request during
  catch-up, correlate the loading reply with the exact active request.
  Check its end anchor and absence of offset and count, then retry and
  verify a current exact-count result. Race revocation before replay,
  publication and delivery. Require unchanged results for unaffected
  viewers, zero stale results for affected viewers and normal ready counts
  after replay. Require
  zero plugin wakes.

- **Two-collection move writer.** Commit removal from the source,
  addition to the destination and Undo in one FULL content transaction.
  Verify crash recovery and restoration of both memberships on Undo.
  Enable the one-exchange shortcut for an eligible private destination
  only after this writer passes.

- **Idle-archive atomic writer with Undo.** In one FULL thread content
  transaction, check the idle condition under the commit fence and record
  archive and its unarchive Undo together. Before enabling idle archive,
  crash before and after the commit and during publication. Recover
  neither effect nor Undo before commit and both after it. On a recovered
  archive, refuse either origin of new execution intent and verify that
  unarchive Undo restores the head without starting an attempt. If the writer retires grants,
  also verify retirement and full restoration of those grants with the
  corresponding head transitions. Run each ordering for prepared Send and
  protected Resume, including a proved historical binding for Resume:

  1. Commit the intent before attempting idle archive. Then repeat
     with the idle archive attempt started first and paused before its
     fenced check until the intent commits. In both cases the committed,
     unresolved intent blocks sheet-free archive.
  2. Commit an execution intent and pause before job admission. Commit a
     confirmed archive through the challenged path with that matching
     saved intent. The FULL archive and Undo commit also durably refuses
     that exact intent with a plain outcome. Crash before and after that
     commit. After recovery, run unarchive Undo before resuming admission.
     Admission and replay must return the durable refusal with zero starts,
     even though the head is live again. A fresh intent of the same origin is the positive control.
  3. Commit archive first, then try to commit the intent. Require an
     opaque refusal with zero new intents, starts and wakes. If a preview
     was already resolved, use it to prove the commit recheck.

  Replay of a committed admission cannot start a new job. Race Send
  against Resume for the last conversation slot and the last quota slot.
  Require one admitted intent, one attempt and no second reservation or
  job. Race a newly active attempt, saved authority or pending approval against dispatch and
  require the protected path if cleanup is needed. Fix the existing dev
  defect in [thread admission](../crates/server/src/turns/thread_admission.rs):
  fresh admission checks deletion and intent identity and currently
  accepts an archived head with a matching unresolved intent. Refuse
  stale Undo. Rerun the Step 0
  phone activation spike on this writer before enabling idle full swipe.

  Measure how often otherwise-idle threads carry saved thread grants.
  The Step 2 lane uses that result to choose one of two paths. Either
  retire the grants in the same FULL transaction as archive and have
  Undo restore them with the thread head in one FULL transaction, or
  retain the challenge for those threads and record the measured
  frequency as a parity exception for the Owner.
- **Actor-keyed snooze writer.** In one FULL thread content transaction,
  commit the actor-keyed deadline, replay intent and Undo. Crash on both
  sides, verify all three commit together, and check the published index
  through the list replay gate. Exercise distinct deadlines
  for two actors on the same thread, changing and clearing each deadline,
  Undo and stale Undo refusal. In both pending-intent arms assert that
  snooze and its Undo leave the intent, capacity and execution retry
  state unchanged. Preserve subscription retry state. Enable
  actor-scoped snooze only after this gate passes.

### Step 3 — Notes first, then Threads

Replay these against shipped versions and the replacement using identical
fixtures, with final revisions, visible authority and durable outcomes equal.

- **Notes.** Cold open a collection and search. Edit one 2 MiB note and
  update metadata while a second client edits through Yjs. Reconnect an
  offline draft, retain body and history, rename and Undo, Trash, restore
  from Trash, and confirm Empty Trash as a Manager. Verify linked-user
  View and revocation, the host Share sheet and bounded
  bulk Trash/share and Choice-from-Bind Move, no worker wake for built-in
  lifecycle actions, and recover a committed save after rollback. Restore
  a revision the editor already holds in `EditorHead` through
  [`host.item.restore`](../contract/host-catalogue-v2.json#/operations/host.item.restore)
  with its checked `source_revision`, current head, live Editor authority,
  existing FULL commit fence and Undo. History browsing and selecting a
  revision from its listing are gated below.
- **Threads.** Ship only after Notes, both Step 2 writer gates and the
  sealed proposal’s [custody and cutover gates][generic-engines] pass.
  Halt new old-engine intents and attempts. Drain unresolved intents or
  verify their existing durable refusals. Require zero active attempts
  and zero unresolved execution intents of either origin before switching
  writers. Preserve their receipts and refusal records. See the [cutover wire audit](reviews/plugin-api-v2-14.8-cutover-wire.md)
  for the wire inventory. Migrate a legacy session into protected bindings
  only with authenticated historical proof of its exact account, provider,
  conversation, machine and profile. Retire Resume for an unproved binding
  and show “Previous session unavailable. Start a new session.” to its
  authorized owner. Keep its thread readable. For proved bindings, resolve
  and confirm protected Resume. Assert a fresh intent on the saved profile,
  no appended message, one execution on replay and zero effects after
  revocation, stale proof or a competing Send or Resume. Exercise busy,
  over-limit and unreconciled refusals. Remove private session copies from ordinary files,
  metadata, history and journal, and quarantine unsanitized inputs under
  protected retention. Prove an ordinary backup restores complete public
  history with stable IDs, order and canonical text and no private session
  records in the backup or restored inputs. Full rendered equality is
  blocked on historical attribution below. Verify projection rebuild and
  rollback from post-cutover writes before switching writers. Open an
  indexed thread list, submit a composer draft on the person’s eligible
  machine, reconnect at a committed transcript cursor, act on a protected
  approval, archive and unarchive, qualify the checked actor-scoped
  snooze timestamp and Stop. Require exactly one submission per receipt,
  no lost committed transcript and equivalent rollback/reopen behavior.
  On the original and rebuilt
  page and follow, assert equal attachment order, media types and lengths
  under equal authority. Reject an over-bound record. Revoke only one
  attachment before delivery while keeping corpus access. Require the
  same record and cursor with no descriptor or placeholder for that
  attachment, and opaque 404 on the old download ref. Exercise host-owned
  provider/model/reasoning/permission/machine controls and authorized
  options. Use already checked attachment refs to exercise prepared Send
  and committed attachment reads. New attachment upload is gated below.

**Existing host protocols.** The Notes editor uses the
[live room protocol](live-rooms.md#browser-binding) for Yjs deltas, receipts
and reconnect from a persisted replica. The host's account
[`drafts.*` and `pending.*` operations](account-input.md#operations)
handle saved drafts and revision-checked submission. The
[room and content save engines](live-rooms.md#merge-and-durability) handle
merge, revisions and checkpoints. These paths bypass the plugin wire. Step 3
still tests the end-to-end behavior and platform persistence.

**OPEN cutover requirements.** Keep each affected workflow's cutover
disabled until a later numbered revision pins its missing contract.
Passing the covered cases enables qualification only. Keep the shipped
path where available and keep the Notes-first release order. Any proposal
to defer a workflow past app cutover requires an explicit Owner parity
decision and a numbered revision. The
[catalogue open list](../contract/host-catalogue-v2.json#/open_cutover_requirements)
records the same blockers.

| OPEN | Missing contract | Cutover requirement blocked |
| --- | --- | --- |
| `renderer_history` | A portable renderer request and reply for browsing authorized revisions and picking one from history. The [existing `notes.history` operation](../contract/operations.json) is a parity reference and §3 covers engine reads. | Browse Notes history and select a revision from the listing. |
| `linked_user_comments` | A portable checked comment submission and returned comment record for linked users. | Linked-user Comment, including durable creation, delivery and revocation. |
| `historical_transcript_attribution` | Committed-record fields for historical headings and author, time, machine and login attribution. | Equal rendering and attribution after history restore, projection rebuild and rollback. |
| `attachment_upload` | A portable, checked composer upload contract that produces a Draft AttachmentRef. The [sealed proposal §A.1][generic-engines-composer] consumes checked refs. [§B.4][generic-engines-uploads] defers the new general file-upload routes. | Submit a draft with a new attachment on an eligible machine. |
| `mention_lookup` | A portable, scoped lookup and selection contract for composer mentions. The [sealed proposal §A.1][generic-engines-composer] covers checked mention material. [§C.3][generic-engines-revision] leaves lookup transport outside the revision scope. | Choose an authorized recipient and submit that mention under prepared Send. |
| `queued_follow_up` | Portable requests and record state for queuing, viewing, editing, cancelling and submitting follow-ups. | Queue and manage a follow-up independently of direct Send or steer. |
| `snooze_presets` | The host policy and portable presentation for the offered snooze presets. | Pass the shipped preset snooze workflow using the actor-scoped writer. |
| `snooze_custom_picker` | The custom snooze picker policy, including supported date/time choices, time zone and defaults. | Pass the shipped custom date/time snooze workflow using the actor-scoped writer. |
| `collection_creation` | A portable renderer action for creating a host-owned collection, including in the bulk workflow. Broker `collection_create` remains an engine call. | Create a collection from Notes or as part of placing bulk-selected items. |

Run executable catalogue fixtures for both apps, including nested child
rows and user-made sections. Drag is an Owner-approved parity exception
(Q11), covered by Move to and Move up/down. General-purpose selectors,
repeaters and conditional fields wait for the first primitives. A shipped
workflow needs parity or an explicit Owner exception before replacement.

**Remove first-party bridges at cutover.** Delete these after the
orchestrator and Notes qualify as ordinary packages.

| Bridge | What to delete |
| --- | --- |
| Namespace assignment | Delete `crates/server/plugin-namespace-owners.json`, `bundled_namespace_owners()`, `threads_owner()` and the distribution override in `check_operation_namespace` that grants the orchestrator the `threads` prefix. Remove the startup expectation that this hard-coded owner is in the distribution. |
| Bundled identity | Delete inheritance of native privileges on operator-approved replacement under `calternal-orchestrator` or `calternal-notes`. Remove package-ID checks that select native registration, routing or authority. |
| Native Threads | Delete the native `threads.*` dispatch branch and its authorization and lookup aliases. Delete special claim and completion routing in plugin jobs and the owner lookup in thread automation, enablement, operations, persistence, turn admission, readiness and retry. |
| Native Notes | Delete native Notes create, get, update, rename, delete, restore and discovery dispatch and its Notes-specific kind fallback. Retire the legacy tag, Trash, history and rename-job dispatch as their generic item, editor and query contracts qualify. |

Installed manifests provide ordinary operation, kind and job ownership.
Host-native operations reserve `host.`. Keep generic collision checks,
verified distribution installation and on-disk migrations.

Rollback is a release-pair exercise. Show that a committed change remains
readable exactly once after rollback or refuse the downgrade before it
can discard the change. Mail still waits for a later engine contract.

## Owner decisions and open questions

See the [recorded answers](https://git.kayg.org/kayg/calternal-ai/src/commit/3ab27a6bfdff921e11846e22675f9520d12962ae/docs/open-questions.md#recently-decided-2026-11-17-now-in-the-decision-records).

- **Q1 Interactive deadline — DECIDED.** Host publication has three seconds from dispatch after the separate wake allowance. Section 2 specifies the phone-link delivery allowance. Slower work is a job with progress.
- **Q2 Tickets and schedules — DECIDED.** 60 seconds per ticket, fresh at each run under the creator’s current rights. Stop on lost rights or departure, notify Managers, and allow scoped Manager takeover of the schedule (§3).
- **Q4 Gallery listings — DECIDED.** Icon, description, screenshots, age rating and plain connection disclosures are required for gallery submissions. The manifest stays small. See §7.
- **Q5 Storage parity — DECIDED.** Host engines serve official and community plugins equally. The host table store is dropped. See the open scratch database design below.
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

- **OPEN — Start the row’s leave animation on `ok`.** Decide whether to
  start leaving before the next window omits the row.
- **OPEN — Derived data.** Section 1 assigns list and search projections
  to the host and derived outputs and facts to plugin jobs. The host
  stores and serves derived outputs under item authority and invalidates
  them on item change. Note thumbnails are snapshots taken by the editing
  device and uploaded through the derived store. Until a current thumbnail
  is available, the renderer draws a simple document miniature from the
  authorised snippet, or without text if the snippet is unavailable.
  The session-bound upload and stale-keep rules follow the
  catalogue’s `query.row_images`.
- **OPEN — Offline note snippets.** Note snippets appear only after the host
  answers at launch, and the Owner may choose to let them persist.
- **OPEN — Private scratch database.** A package may keep a private disk
  per package and space for rebuildable scratch data. It is never the only
  copy and people never read it directly. The package writes what it wants
  people to see onto items. Scratch storage design remains open.
- **OPEN — Orchestrator composition.** Move agent runs out of the server
  into the orchestrator plugin. It composes a durable stream, running a
  container on a machine with given rights, and an approval request.
  The sender-scoped discovery and native provider login decisions
  supersede the interim rules in the sealed proposal’s §§A.2 and C,
  subject to the interim whole-transcript discovery rule and the existing
  tools-off rule for shared space logins. Runtime qualification and the
  Step 3 parity gates still apply. The [credential probe][provider-probe]
  records the constraint that led to the native login decision.
  Scratch activation and Photos-scale I/O and metadata stay outside
  this revision.

### Revision 14.14 draft questions and implementation seams

- The canonical writer and who owns a cross-runner list remain owner
  questions in [the sealed proposal’s §C][generic-engines]. The interim
  choices are the generic host Markdown writer and package-owned
  conversation kinds. Body-derived fields stay out of offline rows.
- Provider eligibility requires profile, supervisor and transport
  qualification under the native login rule in §3.
- The paper walkthrough exercises declarative shapes. The runtime still
  needs cold execution-intent and child transaction proofs, park race and
  restart qualification, and archive/snooze writers. For child admission, pause the parent job and
  race two fresh requests for the last slot. Require one ACK, one
  `pending_limit` and exactly one committed transition. Repeat with the
  parent queue full, aggregate tree full and depth exhausted. Replay a
  live receipt at capacity, then send changed input for that ID. Exercise
  consume, retryable refusal, terminal refusal, expiry over restart and a
  consume/fence race. Read back queue, reservation, child and wake counts
  at each boundary and require exactly one release and no stale admission.
  Do not enable an
  atomic-Undo shortcut before its gate.
- The [renderer gap report][renderer-gaps] and the Step 3 open list
  track the remaining portable transports. The
  [morning decision](morning-decisions.md#notes-and-orchestrator-cutover-issue-78)
  records the sequencing question.

See the [review dispositions](reviews/plugin-api-v2-review-dispositions.md)
for the revision 4 through 6 finding maps.

[generic-engines]: https://git.kayg.org/kayg/calternal-ai/src/commit/5abca642/docs/proposals/plugin-api-v2-generic-engines.md
[generic-engines-composer]: https://git.kayg.org/kayg/calternal-ai/src/commit/5abca642/docs/proposals/plugin-api-v2-generic-engines.md#a1-admission-send-and-lifecycle
[generic-engines-uploads]: https://git.kayg.org/kayg/calternal-ai/src/commit/5abca642/docs/proposals/plugin-api-v2-generic-engines.md#b4-photos-scale-follow-up
[generic-engines-revision]: https://git.kayg.org/kayg/calternal-ai/src/commit/5abca642/docs/proposals/plugin-api-v2-generic-engines.md#c3-numbered-contract-revision
[runner-walk]: https://git.kayg.org/kayg/calternal-ai/src/commit/ad6fc02e/docs/proposals/independent-runner-walk-2026-11-20.md
[provider-probe]: https://git.kayg.org/kayg/calternal-ai/src/commit/b3964ef3/docs/measurements/provider-credential-brokering-2026-11-20.md
[renderer-gaps]: https://git.kayg.org/kayg/calternal-ai/src/commit/880f524b918e2bb2850cafaaa58c66a40f3a208a/docs/reviews/v2-renderer-contract-gaps-2026-11-20.md
[morning-decisions]: https://git.kayg.org/kayg/calternal-ai/src/commit/821df5e21ef3da6897f7c8c39d1ce73145857851/docs/morning-2026-11-20.md#questions-for-you
[window-replay-measurements]: https://git.kayg.org/kayg/calternal-ai/src/commit/a96553bc314a4c4c6f695047b1ed6171e3b3bdfb/docs/reviews/window-engine-2026-11-19.md#proposed-revision-14--journal-backed-window-generations
