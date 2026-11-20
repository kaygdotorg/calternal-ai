# Plugin API v2 · revision 14.31 draft

The first contract covers host-drawn apps, generic items, durable jobs and
generic file, view and execution engines. Section 8 gates runtime enablement. Fixture bounds are test settings. See the
[Owner’s interface, runtime and sharing decisions](open-questions.md#recently-decided-2026-11-17-now-in-the-decision-records).

This draft is a first-release delta on revision 13.18 at `0466f80c`.
The draft also works through the renderer findings against the sealed revision 13.
[Description §5.2](description-contract.md#52-host-surfaces-and-navigation-on-the-wire)
and its catalogue entries define the host wire for rendered components.
A route operand is a checked navigation hint. Only a fresh host context
confers activation authority. Row-image reads go to the originating view
host using its bearer and current view lease. A refused or expired image
is absent and uses the existing fallback. The renderer formats RFC 3339
timestamps locally using the device time zone and locale.

The [owner’s rescope decisions][decisions] move file formats, application
behavior and transcript presentation to packages. This draft specifies
the generic file, execution and authority boundaries. The sealed
[generic engines proposal][generic-engines] remains background for those
boundaries. The existing engine serves current clients until parity and
placement qualification pass.

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
risk, idempotency and optional item target, jobs/waits, alarm eligibility,
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


An app may declare typed, versioned extension points in its manifest.
Another package contributes through the same manifest mechanism for
harnesses, panels, row fields, actions and editors. The host resolves the
exact app, point, version and installed operation, checks both declarations,
pins both package generations and intersects the caller’s rights with the
approved interface. The host validates the resulting native description and
rechecks the requested item and data scope. No direct connection or implicit
cross-package grant follows from installation. Use checked extension choices
for a durable host-issued `HarnessRef` at execution-job admission. A fresh
renderer gets eligible contributions through the scoped
[`extension.choices` route](../contract/host-catalogue-v2.json#/component_routes/extension.choices).
A checked alarm can retain that ref across restarts. Resolve the same
pinned contribution under live authority at claim. Use the catalogue’s
[extension points](../contract/host-catalogue-v2.json#/extension_points).

A **harness** is a plugin that runs an agent program such as Claude Code
or Codex. Threads (`calternal-threads`) declares a harness point. Bundled
and community harnesses contribute through the same interface. Another app
can publish its own point. A contribution names its owner package and app,
point and interface version. **Threads owns the Markdown thread item.**
A harness keeps a separate working log in its own files. Threads may link
to that log by an authorized item locator and writes replies to its own
file. Share, Trash and archive target the thread item. They confer no
access to the working log.
Source authorization applies before rows or counts are computed.
A base indexed row is available while a contributor sleeps. Fetch optional
plugin fields only for visible rows, with the exact source revision and audience.

Kinds declare file format and version. Editors can contribute support for
an agreed Markdown format, and packages can declare their own formats.
The person picks a preferred editor when several support the format. Check
installation, file format and live edit authority. An editor that cannot
preserve unfamiliar blocks opens read-only. Selection grants no new rights.

A harness declaration pins an image digest and entry asset and discloses
login directories. Install shows image publisher, digest, placement,
mounted data scope, each login path and egress, under the same rules for
all publishers. Distinctive provider marks require a verified publisher
binding from a signed gallery entry or explicit operator checksum pin.
The bundled contributions ship signed entries and go through the same
verification. Use the generic fallback when the check fails. A mark
confers no execution or credential authority.

A kind with `claimed_text: true` and `searchable: true` opts into the
host’s background text index. The host checks installed declarations,
package reads and live field scope before querying. A kind may separately
declare plugin search profiles, including a background index job. See §4.1.

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
settings and secrets, contributions, alarm jobs and outbound disclosures. Gallery installs take one tap. Source
installs warn. Pin the digest. Members see packages, grants and destinations. Check and probe agent-authored code
in the space without broker authority, then enable the exact digest and
grant the Owner confirmed. Verify updates before fencing/draining the old
revision. Permission widening needs the Owner except for the network
installation approvals below. Resuming an old job under a
new digest requires compatible migration and current grants. Use the
[upgrade process](upgrades.md). Disable/remove fences triggers, tickets and
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
| Asleep | Serve authorized engine reads and saved templates. An admitted logic call, runnable job, due alarm or agent start wakes its assigned placement if asleep. A signed wait webhook wakes only via a winning signal that makes a job runnable. Cache misses, page reads, public links and rejected requests do not wake. |
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
placements with the full workload before enabling the canary.

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
with the full workload before enabling that placement. [Wall A measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/2562b95e/docs/measurements/space-runtime-walls.md)

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
declaring `execution: true` carry `message.job_context` under the
[catalogue framing rule](../contract/host-catalogue-v2.json#/$defs/HostWorkerInvokeFrame). Refuse wrong-direction invokes or supplied context on worker
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
   creator’s current rights for each job run or admitted alarm.
4. **Attempt by execution lease.** An agent gets a distinct principal and
   lease for the exact attempt, separate from the package invocation
   ticket and the person’s own session. When the sender asks the agent to find things,
   that request grants bounded space search and listing in the target item’s
   space under the active permission mode. Full access and the default
   Approve for me both allow those reads without a second prompt. The host
   binds the request to the authenticated sender and checks that person’s
   current space, scope, item, body and field rights before querying and
   at delivery under the file audience rule below. Apply the
   existing operation bounds and access records.
   Counts, ranking and excerpts use the same authorized projection.
   Other broker effects require their checked refs and existing ceilings.
   Raw enumeration, bulk exports and delegated tickets cannot widen the
   grant. Child execution keeps the existing ancestor ceilings. Recheck
   the exact attempt, runtime fence, live session or alarm principal,
   grant, package generation
   and placement on renewal, broker effects and egress. Revocation fences
   effect and delivery immediately and stops renewal.

**Commit and recovery.** Every effect must take the per-space authority
fence at the engine’s recovery-visible commit point. Check the live grant,
expiry, invocation cancellation, runtime epoch, enrollment and current job
run under that fence. The same fence orders their invalidation. This includes
membership/session changes, uninstall, cancel, epoch loss, alarm cancellation
and job-run replacement. Admission, callback serialization and a preflight
grant read cannot substitute for this check.

The item commit point is the FULL content transaction that records the new
revision **and** its replayable outbox intent with the request fingerprint
and result. The later journal publication and file placement complete it.
The job and alarm commit point is the durable transaction recording state
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
defines the durable fence and post-commit work for each action. Apply its
[retirement authority rule](../contract/host-catalogue-v2.json#/attempt_authority_end/retirement_authority)
to the entire affected set, including any queued successor, before committing
an actor-initiated retirement. In confirmed archive the exact fences and
recovery work commit in the FULL archive transaction before the reply. Undo leaves the
fence in force. Before dispatch or after a crash, the local committed
fence already blocks renewal, queued tools, effects and egress, including
descendant effects. Remote Stop and verified teardown consume the
deduplicated recovery work after commit. Release quota after verified
teardown.

If invalidation wins first, roll back provisional work and leave no success
receipt or visible effect. If commit wins first, keep the effect and recover
publication exactly once even if the response was lost or the actor later
loses access. Recheck live access before replying or replaying that receipt.
Cleanup never deletes committed data. For external effects the fenced point
is admission to actual DNS/send/redirect dispatch, with a durable dispatch
claim for a mutation. Close affected streams on invalidation. Bytes already
dispatched cannot be recalled. When delivery is uncertain, report it as
such and replay only against an explicitly idempotent endpoint.


An alarm wakes a declared replayable job at one absolute time. Pin the
creator, target, package generation and checked input. The catalogue
[alarms](../contract/host-catalogue-v2.json#/alarms) set the pending
counts and bytes. Commit consumption and job admission together. Replay
the admitted job by receipt without a second firing, including after
restart. An eligible missed alarm is delivered once, late, with
`scheduled_at` and `actual_at`. The plugin owns the schedule, decides
whether to run or skip from the person’s setting, and computes its next
alarm. Disable or uninstall retires outstanding alarms and fences
admitted work. A re-enabled package can arm fresh alarms.

Notifications use the authorized item title and a fixed host phrase.
Packages never supply push text containing body content.

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
| Items · `item_create`, `item_head`, `item_range`, `item_history` | Apply the [catalogue item schemas](../contract/host-catalogue-v2.json#/item_engine). Create returns a checked item, revision, portable locator and receipt. Range is pinned and bounded. History uses the existing cursor and authorized revision summaries. |
| Items · `item_save` | The owner commits body, declared metadata, including its indexed title, and an optional leaf rename in one receipt. The creation directory stays fixed. Use the typed `head_conflict` reply to reread and reapply an edit on a checked newer head. Metadata-only changes preserve the room checkpoint. Large writes use the typed staged protocol below. |
| Items · `item_trash`, `item_restore` | Editor or higher. Trash records a tombstone. Restore from Trash uses the current trashed head and restores a live placement. Restore from history takes a source revision and expected live head and appends that version as a new revision. No history restore against a trashed item. Stale heads give `409`. |
| Items · `item_rename`, `item_rename_status`, `item_rename_undo` | Rename accepts ref, expected head and name and returns a durable action ID. Status uses that ID and checks live item authority. Completion can return a protected Undo action bound to the result and expected revisions. Undo is a fresh receipt-backed operation, with conflict if affected heads moved. Publish progress and actions through the host result route, with no worker payload. |
| Items · `item_purge`, `empty_trash`, `collection_create`, `collection_membership` | Purge and Empty Trash are Manager/Owner-only, irreversible and one confirmed durable job over a pinned Trash selection. Reauthorize per batch and resume by receipt. Apply catalogue `trash` for retention. Collection creation and bounded membership changes commit once with expected revision. |
| Queries · `items_list`, `items_search` | Kind/optional collection, typed indexed metadata predicates, declared sort under the [catalogue’s sort rules](../contract/host-catalogue-v2.json#/query), cursor/window and field set → authorized page with cursor and count/offset where windowed. Unindexed filter/sort is `400`. Use engine-side query and bounded row/byte budget. Apply authority before count. No whole-kind materialization. |
| Rooms / sharing · `sharing_check` | Item ref plus requested action → permission for the live grant. A room is an attribute of the kind, opened client-to-host using its own bounded protocol. Room membership and events keep fencing the item grant. No plugin hook acknowledgment. |
| Jobs · `start_job`, `checkpoint_job`, `job_status`, `cancel_job` | Declared job, payload and resolved target/grant → durable job ID. Execution admission uses [`ExecutionJobStart`](../contract/host-catalogue-v2.json#/$defs/ExecutionJobStart), including its top-level `harness`. Checkpoint only the owning current run. Status/cancel uses a scoped job ID with opaque `404` for foreign jobs. An execution cancellation uses the same retirement authority as Stop. Bounded checkpoints, live progress and park count per space/package. |
| Settings / secrets · `read_setting`, `set_setting`, `request_secret_handle` | Declared key or secret purpose → effective value/version or opaque handle. Validate type, compare revision on writes and bind personal values to the actor. One bounded engine operation. Missing authorization is `403` or opaque `404` as appropriate. |
| Proxy · `proxy_request`, `credential_call` | Approved install line and bounded HTTPS request → bounded response. The host-only sealed content route exposes authorized opaque image refs to the renderer. Unapproved destination is `403`, oversize `413`, quota `429`. Anonymous media keeps the existing 10 MB, three redirects and 16 hosts/minute fixture ceilings. Typed call returns the profile output or `403` without contacting the endpoint. |
| Execution · `execution_start`, `execution_status`, `execution_input`, `execution_spawn_child`, `execution_stop` | The catalogue types each call. The host issues the admitted intent, resolves provider and placement, and binds every effect to the exact attempt under live authority. Start, input, child and Stop use checked receipts. |
| Alarms · `alarm_set`, `alarm_cancel` | Declared replayable alarm job, checked input and one UTC instant → durable receipt. Supply the typed `AlarmSet.harness` exactly for an execution alarm under the catalogue’s admission rule. The job's host metadata carries scheduled and actual time. |
| Files · `item_append` | Checked item, expected revision and bounded opaque bytes → durable revision, byte boundary and receipt. |
| Item views · `item_view_publish` | Declared snapshot slot, checked exact audience and source revision → retained native description and receipt. |


### Generic item files and execution

A kind’s owning package writes its files and defines their encoding. The
host maintains item identity, revisions, file placement and authority.
Apply the catalogue’s [file append rule](../contract/host-catalogue-v2.json#/file_append).
`item_append` checks the exact head, write authority and, when issued under an attempt, its live execution and
audience fence. Commit the bounded opaque append, revision, byte boundary
and receipt recoverably before ACK, using staged writes and the outbox.
Retry a lost ACK by receipt. Checked revision-pinned range reads work
while the space sleeps. Recheck at delivery and fence on revoke or purge.
A title-only grant supplies neither file body nor body-dependent revisions
or derived display. Attachments are separate authorized item or file refs
with their own retention and delivery checks.

The host admits a declared replayable execution job on the exact item
of a kind declaring `execution_target`
with the actor, package digest, admitted harness, placement, audience
generation, rights and quotas pinned. Apply the catalogue’s
[successor rule](../contract/host-catalogue-v2.json#/execution_intents/successor)
when an item already has a head job or active attempt. Admit the queued
follow-up through the checked job path. Pin the typed harness binding at
admission and recheck it at promotion and claim. The authenticated job
context supplies the target, pinned harness and admitted receipt. `execution_start`
checks the resolved harness and optional continuation under that fence.
An authorized retry returns the same admitted attempt. The owning job
opens the [generic contribution call](../contract/host-catalogue-v2.json#/extension_points/calls)
with the checked input and pinned contribution. Consume committed frames by
call and sequence. Use stable subeffect receipts to write owner output under
the exact attempt fence before successful job completion. The contributor
writes its own working files through checked host methods. The [entry
ABI](../contract/host-catalogue-v2.json#/extension_points/harness_abi) applies
to each verified placement. Stop can target
the exact pending job or admitted attempt using generic host control.
Protected approvals bind the checked effect to the actual dispatch.
The catalogue’s execution wait compares the transition cursor and parks
atomically. Wake the owning job for verified phase, approval, child
request or terminal changes. The harness owns provider options, phases
above that boundary, retries, commands, queued follow-ups and presentation.

An authenticated attempt can request a child reservation for a declared
kind and checked target refs. Under the tree fence pin the exact parent’s
harness and placement and check the actor and ancestors’ scope and effect
ceilings. Reserve before issuing a
checked child request; only the owning job can consume it. Apply the
catalogue's tree, expiry and replay bounds. Any child item and parent
edge commit through the generic item authority path. `execution_input`
delivers to the exact live attempt under its checked rights.

The placement runtime independently enforces freeze and verified teardown
deadlines. Keep quota until teardown is verified. Reattach across server
restart only with exact live execution proof, safe transport and fresh
renewal. Failed reattachment durably retires the attempt. The plugin may
offer continuation in a fresh job through an opaque placement-owned
binding issued by `execution_session_bind`. The host pins the item and
audience generation, actor, package and harness digest, profile and
machine. Do not expose native session IDs. Current authority, compatible
binding, and reconciliation of uncertain effects are required before
admission. Section 5 retires incompatible bindings before widening.

A harness declares its relative `login_directories` mount paths. The bundled harnesses declare `.claude` and `.codex` by the same
mechanism. Mount private backing directories for each harness by default.
Only the person’s explicit choice shares a backing directory between
selected harnesses. A matching path or name grants no access. Apply
the catalogue’s [login custody rule](../contract/host-catalogue-v2.json#/execution_attempt_authority/provider_login)
before mounting. In a personal sandbox the program and its tools may read
the approved login. Keep credentials out of other accounts, output and
portable backups. In a shared space the default is the separate space
binding approved by the Owner and configured by the Owner or a Manager.
A member may explicitly choose their own enrolled machine and approved
login for one run they start, through the catalogue’s
[placement choice](../contract/host-catalogue-v2.json#/execution_placement). That run stays in their isolated sandbox and only that
member’s explicitly selected runs can mount their login. Both choices
limit discovery to what the whole file audience can read and keep tools
off until isolation is proved (owner decision 74). Recheck the selected
profile and session at continuation as well as at admission.

The same host choice offers eligible machines in a personal space. Any
app can request it in a checked execution action using the description’s
choice prompt. The renderer requests and submits the host choice under
the catalogue’s exchange. The worker receives only the opaque ref and
forwards it as `ExecutionJobStart.placement`.
**Harness placement (owner decision, 2026-11-20).** A harness reviewed
once and approved for its pinned build can run on any eligible placement,
including hosted sandboxes. Bundled harnesses go through the same review.
A different build needs its own approval for hosted placement. Any other
harness may be installed and runs only on a machine the acting person
enrolled. The install screen discloses that limit, network access and
access to the person’s provider logins when granted. Check the exact
build and live placement authority at admission, reattachment and renewal.
A provider mark alone confers no placement rights.

**Agent discovery (interim, owner asked 2026-11-20).** A sender-only
item uses the sender’s live read rights. For a shared item use the
intersection of the sender and all file readers, before matching,
ranking, counting or returning content. Asking the agent grants bounded
discovery in Full access and Approve for me. Recheck live access at
delivery, record checked access, and suppress data before egress or
commit when authority changes. Section 5 applies to audience expansion.

The app owning the item chooses its record boundaries, native view and
stable message anchors. The renderer can open a locator plus anchor under
the same live item and revision checks as any other item page. Host job
and approval controls remain generic. A retained snapshot draws while
the item owner sleeps (§4.2).

**Threads participants.** People and agents are participant data in the
Threads file. Threads denies attempt discovery on its kind. A run gets
only the app's checked disclosure, and the harness has no direct writer
authority on the thread. Calling in an agent selects a message, a range
or the whole conversation. Apply the generic
[author-consent rule](../contract/host-catalogue-v2.json#/attempt_disclosure)
to every included author's material. Trusted authorship, recording and
revoking consent, groups and cross-space delivery are OPEN. Keep the
affected invocation path disabled until pinned. People discovery is
limited to one's group. There is no server-wide people discovery.
[Recorded Threads decisions][threads-decisions] For an agent called into a conversation between people,
use the caller's own enrolled machine and login outside a shared space
and the space binding in a shared space. Shared-space tool restrictions
still apply. Calling a person into an agent conversation follows the
existing audience fence. Any new invite or delivery mechanics remain open.

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

Item lifecycle uses the [space Trash policy](../contract/host-catalogue-v2.json#/trash)
with bounded host cleanup and the same purge and revocation fences.

**Staged save.** Call `item_save_begin`, `item_save_chunk` and `item_save_seal` from the [catalogue](../contract/host-catalogue-v2.json#/item_engine), then commit the sealed ref with `item_save`. Begin reserves total decoded bytes against the existing
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
its answer/approve/deny receipt rules. A parked job and an execution
use the same request type and resolver. Approvals bind the material action,
target, grantee and lifetime. Irreversible lifecycle actions use one-use
confirmation. A stored approval cannot substitute for a live grant.

Background work has no overall interactive deadline or persistent ticket.
Each authorized run gets a fresh 60-second ticket. Longer work saves a
checkpoint and continues in a later authorized run when its ticket expires.
If its creator loses rights or leaves, stop the job, including parked
continuations, record the reason and preserve committed work. Only live authority can admit another job.

Job admission persists operation, target and grant. Revalidate at claim,
terminal park and resume, including any child and descendants. The host
validates a terminal `parked` result before atomically saving checkpoint and
wait through the existing jobs engine. There is no post-park acknowledgment.
Inbox answer/approval, typed signed wait delivery, deadline and cancel race
through the existing winning-signal transaction and request receipts. The
losers have no effect. Park uses no worker once the run ends. Retain existing
wait bounds and state visibility. Repeated job execution can occur after a
crash. Committed effects replay by receipt.

## 4. Host catalogue, queries and activation

The [description contract](description-contract.md) and its
[shared schema](../contract/description-v2.schema.json) are reviewed in
this same tree. Both use the [host catalogue](../contract/host-catalogue-v2.json)
as the source for kinds, fields, built-ins and shared wire shapes. The host and SDK qualify against the corresponding positive and negative cases.
The conformance tool and its fixtures follow a separate release path (§7). No template supplies trusted authority or operation semantics.

### 4.1 Catalogue and qualified queries

The catalogue’s `query.row_images` read checks live item and image-field
authority on every fetch. A body-derived image also requires live body
authority. Refusals return the same opaque 404 as an absent image. The host
marks body derivation from the source, including device snapshots of the body
and job output over it. Image retention follows the source revision, with
purge on revision erasure or item purge.

The [catalogue’s `Timestamp`](../contract/host-catalogue-v2.json) uses RFC 3339 with seconds 00 to 59 only, and the host records any leap second as second 59 with the fraction clamped.

The catalogue owns generic `id`, `title`, `updated`, `archived`, `trashed` and
`trashed_at` fields for every item kind. `collections` and
`requests` are host-owned. Notes declares its own `notes` kind and
`pinned` metadata through the same manifest API a community app uses.
Eligible harness-associated entries can project the catalogue’s verified
`provider_icon`. Kind declarations name an icon, a string title field and indexed fields
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
authorized `RequestRef`. Filter `requests.pending_for` with equality to
the viewer’s own account only. An eligible item’s protected execution status identifies the nullable `ExecutionRef` for the exact admitted attempt, available only
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
body and any upstream Bind field. Check source provenance before any
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
plan. Check the explicitly authorized search fields **before**
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
host-issued `scope_ref`, `window_ref`, result version, stable rows and the offset where applicable. Both schemas
cite the catalogue’s only request envelope,
`{type:"window", request: WindowRequestPayload}`. An absent `anchor` means
`start`. With `anchor:"end"`, `[from,to)` counts backward from the last
authorized row, with zero at the last row. Use this form for End and
near-end jumps only on non-search Binds with effective `exact` count
authority. A ready or empty End result carries `count.mode: exact`.
The offset identifies the returned row nearest the end when there are
rows. On an eligible End request, `loading`, `throttled` and `error`
retain `anchor:"end"` and the active request scope without `offset` or
`count`. `refine_query` belongs to start-anchored search. Follow the
[catalogue’s result ordering and correlation rules](../contract/host-catalogue-v2.json#/$defs/WindowResult).
Refuse an end anchor with `WindowPlanError.reason: end_unavailable` for
search Binds or when effective count mode is `estimate` or `none`.
Search End pages forward. The catalogue’s `query.window` sets the
window span. For a per-parent Bind require its edge-scoped
`parent` ref. Omit it at the root.
Count is an exact/estimate object or omitted, as allowed
by audience and plan. The chrome/content pair shares one Bind list.
`viewer_self` permits counts over the actor’s own authorized projection
only. Window locators belong to the checked query
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
| Child and section windows | Parent-scoped ordered indexes for authorized child rows and personal section placements. Child windows require an audience-scoped covering seek keyed by parent and declared sort order with stable child ID as the last key. Apply current authority before reading rows or the per-parent count. The kind-wide order index with a residual parent predicate does not qualify. |
| Filtered counts | Non-search lists use a maintained count for the supported authorized scope or an audience-certified member estimate. Search counts qualified matches under `query.search.count`. |

The search Bind uses the [catalogue’s search profiles](../contract/host-catalogue-v2.json#/query/search) for matching, field filters,
deduplication and result order. Search covers all authorized searchable
fields by default within the qualified posting, expansion and row-work budgets.

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

Default ranking and paging always run in the host index. Resolve the
effective search path before settle from the authenticated view audience.
An explicit profile is available only to a live member view. Guest and
public views use default host search and its normal qualification, even
when the template names a profile. An eligible member profile can call its
package’s read operation independently. It can rerank checked
candidates or consult its own index built by the package’s background job.
Apply the catalogue’s [plugin search rule](../contract/host-catalogue-v2.json#/query/search/plugin)
to index inputs, matching, ranking and delivery. Use the explicit
cursor exchange. Profiles that cannot demonstrate pre-ranking authority
stay off. Profile failures never affect default search.

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
authority within the separate count-work budget. Explicit plugin profiles use their separate cursor and result shape. See `query.search.count` for qualified counts and
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
Hold newer window results that change the version while selection settles.
Apply scoped invalidations immediately under the catalogue’s
[signal table](../contract/host-catalogue-v2.json#/subscription_catch_up/signals). Navigation
waits for settling. Selection-dependent activation follows §4.3. Prefetch
and pushes never wake a worker. Live window, search and item-view
subscriptions and lease renewal use the catalogue’s
[catch-up rule](../contract/host-catalogue-v2.json#/subscription_catch_up).

Keep filled live windows in bounded memory, partitioned by account,
session, space, audience and view. Hide them on lease expiry or disconnect.
Use the [common result retention rule](../contract/host-catalogue-v2.json#/view_result_retention)
for host windows, plugin search pages and item-view pages. Send their
exact `window_ref` and `result_version` pairs and accept confirmation
only on the current authenticated renewal. Keep each confirmed unchanged
result and use its current scoped contexts. Recheck Stop against the
exact pending job or admitted attempt. Clear affected content on actual
invalidation, lease loss or disconnect. Refetch under fresh authority.

Data-free templates remain available for cold launch. Signed invariant-root
member row records follow the catalogue’s
[device reconciliation rule](../contract/host-catalogue-v2.json#/device_reconciliation).
Use the last completed inventory for disk prepaint and reconcile in
parallel when connected. Body-derived display fields may persist when
covered by an exact authorized offline download grant. Validate the
signed record against account, space, template, Bind, offset and current
opt-ins. Treat it as display only, with no context or reference usable
for a live action. Verify live authority before applying any new window.

An authenticated member may download the full readable content, including
bodies, attachments and authorized derived fields. Use the catalogue’s
[offline download](../contract/host-catalogue-v2.json#/offline_download)
manifest and reauthorized bounded ranges. One grant covers a checked
selection across all of its manifest pages. Encrypt bytes and derivatives
per account, device and space. Apply the same device reconciliation rule
for next-contact revocation and local sign-out, including search and keys.
There is no expiry by default.
A space may configure an offline expiry. A disconnected device with a
previous copy cannot be wiped remotely. Export starts with authorized files
and attachments. Live actions require fresh host authority.

**Item view snapshots.** A package writes a ready-made native description
for a declared item slot, under the catalogue’s
[view snapshot](../contract/host-catalogue-v2.json#/view_snapshots) rules.
Its authored content follows the same read bound as a write to the item
file: the package’s declared reads and the acting principal’s live grants.
The host validates the description and binds it to the source revision
and item audience. Both retained and live views follow the
[catalogue’s item-view rule](../contract/host-catalogue-v2.json#/view_snapshots/body_content).
The host checks shape and scope. The author controls what the text says.

Draw the retained newest page without waiting for a wake. When the revision
has moved, mark a still-authorized snapshot stale. When absent, show the
basic item shell and loading state. Only a run with live item-write
authority can replace the retained view, normally the run that wrote the
file. Check that authority at scope admission and commit.

A member reader may request a live view through the pinned read operation.
Apply the same item-view rule to the transient reply. Opening or refreshing
an item never replaces the retained view, even when the reader can edit.
An eligible member can request older pages through `item_view.open`
using the checked cursor. Follow the active live head for commit signals.
On change request the newest live page. On invalidation clear the
affected content under the item-view rule. Guest and public delivery
uses only the retained newest page. Revoke or purge drops that snapshot.

Item-view leaves draw literal text, code, a heading with optional authored
attribution, and checked images or attachments. They carry no markup,
host chrome, confirmation UI or action context. The
[catalogue’s asset route](../contract/host-catalogue-v2.json#/component_routes/item_view.asset)
reauthorizes both body and asset before delivering bytes. Notes’ row
images keep their own size and delivery limits.

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
when no authorized image can be delivered. Image refs and decoded row images remain live-only. Apply the
[offline projection rule](../contract/host-catalogue-v2.json#/offline_download)
to body-derived text.

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
`activation_contexts` for header and form. Each WindowResult
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
   Validate form and prompt slots. Bulk resolve may submit
   up to 50 stable IDs from the context-bound Bind. Reauthorize every ID
   against that snapshot and return a scoped `selection_ref` inside the
   resolution. A separate `selection.set` call is unnecessary for bulk
   activation. For a Choice prompt, fetch `ChoiceOptionsRequest` for the
   context and slot before showing the returned labels. Submit the picked
   ref in `interaction.choices`. Resolve that host-issued ChoiceRef against
   the checked prompt and option source. Use the catalogue’s placement exchange for a host-owned execution choice.
2. **Derive confirmation.** `confirm:none` permits the shortcut for host
   catalogue reads and for catalogue mutations with `atomic_undo: true`
   whose engine commits the complete effect and its Undo in one
   recovery-visible transaction and satisfies any `atomic_undo_when`
   condition. Archive uses its catalogue eligibility rule.
   For `host.item.create`, a checked route or literal collection may be
   part of the same shortcut. Creating an empty item exposes no existing
   material, so no challenge applies even in a shared collection.
   For `host.collection.move`, use the catalogue’s rule for the effective
   audience of the checked ChoiceRef destination. Apply the §5 challenge
   to any audience-widening effect, including an inverse. For other shortcut mutations the only material may be the
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
   material keys appear as `input:<key>` with labels and values checked
   under the catalogue’s confirmation table. Independent host keys are separate. A captured confirmation
   must use the originating activation ID, resolution ref and challenge.
   Apply the catalogue’s confirmation schema.
   The host issues a one-use proof after the authorized protected control
   confirms the exact checked input. Check actor, scope, dependencies and
   fingerprint again at dispatch and consume the proof under the commit
   fence. Only the eligible catalogue shortcut
   `ResolveActivation{dispatch:true}` resolves and dispatches in one
   exchange. Otherwise return a resolution without running the operation.
   Dispatch returns an ActionOutcome and may include a checked created ref,
   status ref or Undo ref. A package-created ref uses the catalogue’s
   [created convention](../contract/host-catalogue-v2.json#/created_outcome).

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
space Trash-sheet routes. The host sends an authorized audience
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
commit fence. Apply the catalogue's confirmation rule and §5 when new
file readers gain access. A newly shared destination needs a fresh
challenge. When eligible for the shortcut, one exchange after choice
submits the move and commits it with Undo. Leave
unrelated memberships intact. Personal layout uses `host.layout.move_to`,
`host.layout.move_up`, `host.layout.move_down` and
`host.layout.section.create`, `host.layout.section.rename`,
`host.layout.section.remove`. Resolve the entry and section in the actor’s
own layout against its context-pinned revision and check live space
membership and item and parent access. For item row actions
`{context:"item"}` yields the underlying checked ItemRef.
`{context:"placement"}` yields the LayoutEntryRef in a repeated section.
Move to accepts the item to add an unplaced item or the placement to move
it. It resolves the destination ChoiceRef for the protected preview.
Removing a section changes only the personal layout.

For every eligible case below, commit the complete effect and the protected
Undo ref with its inverse data in the named transaction. Publish the ref
only after commit. Undo rechecks live authority and the post-effect
revisions of all affected resources and refuses if any dependency moved.

| Catalogue operation | Commit boundary and eligibility |
| --- | --- |
| `host.item.create` | Under one FULL content transaction’s fence, check live creation authority, optional collection-membership edit authority and the normal item and membership quotas. Then commit the empty item with its permitted defaults and the host’s empty-source extraction, requested membership and Undo together. An omitted route parameter skips placement. A supplied ref that is absent, foreign, unreadable or not editable returns the same opaque `collection_unavailable` outcome. Quota refusal returns `create_limit_reached` with no item, membership or Undo committed. Undo rechecks live authority and the item head, its membership and sharing revisions and the introduced membership revision, if any. Refuse on any change. On success remove the item and its introduced membership together. Return the created ref with no runtime wake or challenge, even in a shared collection. Any declared follow-up receives only the created ref under §1 after commit. |
| `host.item.trash`, `host.item.restore` | The FULL content transaction records the item revision, recoverable placement and Undo. Full swipe covers eligible single-item Trash and restore from Trash. Apply §5 to restores that add file readers. Trash paths with separate lifecycle cleanup are ineligible. |
| `host.item.rename` | The FULL content transaction records the new head, placement and Undo for a single-item rename without deferred link rewrites. The current Notes job commits inbound-link changes in later transactions, so that path remains ineligible. |
| `host.collection.move` | Use the catalogue's confirmation rule for the checked destination. Apply §5 whenever the move adds file readers. On an eligible shortcut, the FULL content transaction commits the complete move and Undo in one exchange. A two-collection move is ineligible for the shortcut until both membership changes commit together. |
| `host.layout.move_to`, `host.layout.move_up`, `host.layout.move_down` | The per-person layout revision transaction records the complete placement or ordering edit and Undo for the actor, space and app. Move to still previews its destination. |
| `host.layout.section.create` | The per-person layout revision transaction creates the section and commits Undo that removes it. |
| `host.layout.section.rename` | The same layout transaction commits the title change and its prior title for Undo. |
| `host.layout.section.remove` | One per-person layout revision transaction retires the section and its placement generation and records Undo naming that generation. Retired generations are excluded from reads. Undo rechecks live membership and the post-effect layout revision and reactivates the exact generation in one transaction before its expiry. Reactivation grants no item access. Reclaim expired retired generations in bounded background batches. Removal and Undo perform no per-placement work. |
| `host.item.archive` | Use the catalogue’s checked idle predicate, qualification gate and execution fence. A protected archive binds the whole affected set and uses the catalogue’s retirement authority rule. Undo that adds file readers goes through §5. |
| `host.item.unarchive` | The FULL item content transaction commits the new head and Undo. Use the catalogue shortcut only when eligible. Apply §5 if the change adds file readers. |
| `host.item.patch-metadata` | The room-checked FULL content transaction records the changed head and Undo for a single boolean toggle. Full swipe covers this case. |

On success, Invoke `then:{route}` may navigate using a
host-minted created ref, reauthorized at open.

## 5. Sharing and aggregation


**Widening a protected item's audience.** Any grant adding a file reader,
including share, link, membership, invite, collection movement, restore
or Undo, applies the [one host widening action](../contract/host-catalogue-v2.json#/attempt_authority_end/actions/item_audience_widening).
The host records widening protection with the first personal-authority
write. That item fact persists across installation changes. The authorized
sharer confirms host-written material that discloses the
whole committed file, including personal discoveries. The server checks
the grant and committed boundary without interpreting the records.
Attachments retain their own grants.

On a widening inverse, the ordinary `undo` request resolves a protected
`ActivationResolution` without committing the inverse. The existing
confirm exchange dispatches the checked inverse under the same widening
fence. Before publishing any wider grant, durably fence affected personal
attempts and pending jobs, discard uncommitted output and retire saved
bindings incompatible with the new audience, including completed runs.
The placement must prevent access to retired session data. Later shared
runs can use the committed file and context cleared for the new audience.
Remote teardown follows the durable fence.

Members trust the Owner's installed plugins with data available to authorized
calls in that space. A warm worker can remember an Owner value and return
it to a Reader acting on their own ticket. Host-filled bindings protect
engine query results, and the installed-plugin trust rule still applies to
what a worker knew or previously wrote. Run all guest logic in the home
space charging its quota. Guest outputs and public links are host-rendered
and grant-checked. Guest reads never invoke a worker. An eligible retained
item snapshot is stored authored content and uses the file-read check. A public
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
their feeds from authorized item journals, job parks and the alarm job’s
creator outcome events. Workers cannot
publish arbitrary feed summaries. Revalidate on delivery and remove
revoked entries and media from subsequent views.


## 6. Deliberate cuts

The same generic engines serve bundled and community packages. A
package owns its transcript, composer, provider selection, mention
picker, snooze policy and calendar logic. The host owns
[space Trash](../contract/host-catalogue-v2.json#/trash). Private
scratch, Photos-scale I/O, direct sockets, brokered TCP and arbitrary
guest runtime calls are outside this revision. The versioned v1 engine
stays in service through application cutover.

Default text search uses the qualified host plan. CJK supports whole
unspaced runs and leading prefixes. Implicit segmentation and substring
matching require separate qualification. A plugin may choose a separate
index and ranking under §4.1.

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

The SDK generates the author context and typed broker inputs and replies
from the pinned catalogue. The host supplies the stable request identity,
actor, authored time and checked job and alarm metadata. Use deterministic
subeffect keys under that request identity. Authority, cancellation and
live timestamps remain host-owned. A retry keeps the same request ID and
typed target. Authors never pass
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

### Step 0 — Qualification evidence

**Entry evidence.** Paper-walk Reading List (list, two-field Add with
created/then, Mark read full swipe, search and a one-date reminder), Notes (created ref, room edit, Move to collection,
bulk Trash and Share), Threads (contributed harness, sleeping view,
job admission and protected approval), and Mail (list, sealed body, credentials and new
arrival). Check revised Reading List, Notes and Threads examples against the
pinned schemas. Include malformed
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
| **Scoped search** | Check default ranking and explicit plugin search independently. Verify scoped rank, bounded postings, unchanged default behavior with a stalled plugin, per-result authority and revoke before delivery. |
| **Atomic large save** | Stage an escape-heavy 2 MiB body and reconstruct it by revision-pinned ranges after the head moves, p95 ≤5 s. The 60 s range pin is invalidated by purge or revoke. Race duplicate begin, lost chunk ACK, conflicting bytes, cancel and orphan cleanup. Race revoke, epoch loss, enrollment removal and run replacement on both sides of commit and inject crashes before publication and before reply. No partial edit or success receipt before commit. After commit recover one revision and receipt. |
| **Durable file append** | Stall a reader, cut after append commit before ACK and restart. Verify the exact pinned byte boundary, one committed append, a replayed receipt and zero bytes on revoked reconnect. Control requests remain independent. |
| **Host + renderer authority** | On Svelte Chromium check both formats, a real protected approval and a live Bind. Forge row/route/Bind/context, untouched prefills, bulk IDs, ChoiceRefs, cached data and built-in surfaces. Inject duplicate and stale activations, revocation and unrelated churn. Zero unauthorized effects or rendered bytes, one commit on replay, and unrelated-churn action p95 ≤2 s from dispatch. Race revocation at egress, job claim/park and approval resolve before freezing those routes. Issue and fetch a body-derived thumbnail ref while all grants are live. Revoke only body access, preserving item access, image-field access and the view lease. Fetch that same ref and require the same opaque 404 as an absent image, with zero image bytes and no worker wake. For each §4.4 atomic-Undo case exercised in Step 0, crash before and after the named commit and during publication: recover neither effect nor Undo before commit, and both after it. Concurrently edit a dependency, then require Undo to refuse without another effect. Run idle-archive recovery in Step 2 on its writer. Exercise ineligible variants and prove the shortcut and full swipe cannot bypass their checks. Delay or spoof lease replies and verify only current, exact retained windows repaint. Test unverified and conflicting provider mark claims, including a fresh or offline host without verified ownership data. Only a verified publisher with a matching ownership binding may project a distinctive mark. Run the projection sweep with a title-only guest and an authorized body reader. Revoke only body access and verify titles still work. Assert no protected header, row title, secondary text, snippet, cache part, notification text, symbol, image ref or bytes. Check sorts, predicates, count and rank against the title-only oracle after edits to hidden bodies or restricted fields. Check eligible member projections and title-only search still return their expected rows. |
| **Host create with collection placement** | With the plugin asleep, create a default empty item in an authorized shared collection in one exchange without a challenge. Read the authorized row back with each configured extractor type. Require an empty set, omitted absent scalars and no image ref. Then save a valid false and zero and require both values in the projection. Crash before and after the FULL commit. Before commit recover no item, membership or Undo. After commit recover all three and replay the same receipt. Undo removes the item and its introduced membership together. Compare refusals for absent, foreign, unreadable and non-editable collection refs and require identical opaque outcomes. After resolve, delete the collection or revoke the actor’s membership edit access and require the same outcome on dispatch. Fill the item quota and membership quota separately, then require the typed limit outcome without an item, membership or Undo. Race two creates for the last membership slot and require exactly one commit. After a successful create, add another membership or a share grant without editing the head or introduced membership, and verify Undo refusal in each case. Assert zero runtime wakes throughout on a kind without a follow-up. |
| **Phone activation exchanges** | In the same Chromium phone link fixture measure eligible host swipe, ordinary plugin resolve/dispatch and protected challenge separately. Check stale contexts, lost replies and exact replay. A widening Undo returns a disclosure preview before confirm dispatch. |
| **Keyboard windowing** | 5,000 rows with seeded authorized thumbnails and 30 keys/sec on the phone fixture. During repeat, assert zero new image GETs and cancellation of in-flight image fetches. After settling, fetch only current refs for the visible rows. Record image request counts and bytes. At most four window requests in any second and zero selection/navigation events during repeat on every run. Report p95 request rate. After the first real rows paint, capture each viewport frame during the repeat burst. Require zero skeleton-row frames during the steady 30 Hz segment on every run. Same-version prefetch applies while keys repeat; an activation waits for the context of its drawn target. Delay a context for A until B is drawn and verify the activation cannot use A. |
| **Nested rows and user sections** | On the real item and layout engines, expand two parents and page their children independently. Check depth, ancestry, revocation and a forged parent ref. Create, rename and remove personal sections and Move to, Move up and Move down. Undo each committed edit, verify placement and order, refuse stale Undo, and prove a second member’s layout is unaffected. |
| **Persisted member rows** | Use an authenticated eligible root Bind. Verify device ciphertext and account/space key separation, reject expired or forged records and erase on known revocation, sign-out and account switch. On unlocked launch paint eligible rows from the last completed reconciliation in the first frame. Start reconciliation in parallel on authenticated reconnect, erase omissions as each current page arrives and verify the displayed rows are corrected without waiting for a new window. For a cached Notes timestamp, advance the device clock across a relative-date boundary and verify that the label updates before a fresh window. Check that unconfirmed rows have no executable actions and that guest and public windows leave no record. |
| **Phone cold launch** | Restart the client with an authorized account and a cold connection, with and without an eligible member row record. Start at launch intent and time first paint and first confirmed or corrected rows on the phone link. Require confirmed rows p95 ≤600 ms in both cases. Use TLS resumption and pipeline authentication, `view.open` and the first windows in one application flight. The host handles them in that order. Start paged `device.reconcile` in parallel on authentication. Delay reconciliation beyond first paint. The initial frame uses the local template and unconfirmed rows left by the last completed reconciliation, or loading without records. In the cached arm, omit one held scope or offline grant from the reconciliation response. Require its affected records to be erased and disappear when reconciliation completes, with unrelated records preserved. Confirm no action can run before fresh host context. |
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

The [all-field search measurements](measurements/plugin-api-v2-step0.md#follow-up--search-across-all-searchable-fields) record the earlier host-managed search paths at the shared depth. Those samples do not qualify explicit plugin profiles. The earlier separate-field probes measured 5.893 ms p95 for Notes and 1.414 ms p95 for Threads. The Threads probe measured
append-to-scoped-publication lag at 46.847 ms p95 idle and 468.095 ms p95
under 1,916 competing appends. [Threads search and shared depth measurements](https://git.kayg.org/kayg/calternal-ai/src/commit/9a78a43b1cda6a2074df9d1471752f3db5a2a0bb/docs/measurements/plugin-api-v2-step0.md#follow-up--threads-title-and-message-search).

If a gate misses, record the measured failure and change the mechanism or
reduce the first runtime slice before enabling it. A fixture using separate per-package
transport or an in-memory broker is not evidence for the carrier spike.

### Step 1 — Qualify reviewed schemas

Review the text, catalogue and schemas independently of the conformance
tool’s release. Enable only exercised runtime slices. First qualify the
space carrier, checked targets, file ranges and save receipts, and cold
indexed windows. Generate matching Rust-backed schemas and TypeScript
types. Qualify real host/renderer authorization before enabling Invoke
and each built-in. Check the same native description shapes in SwiftUI.
Reuse the container wall and durable jobs for the thin test adapter.

Run an independent role/method/target matrix and both race orderings from
Step 0 against the real fixture for each enabled slice. Negative cases must
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

- **Append and cold view.** Race duplicate and conflicting append requests,
  stale head, machine fencing and restart before and after commit. Verify a
  cold revision range without waking. Open the sleeping item with a valid
  snapshot, then advance the head, change audience, purge and revoke.
  Check the publishing run’s declared reads and live principal. Refuse
  access through an undeclared read and verify delivery only to eligible
  file readers. Reject a snapshot carrying chrome or confirmation UI.
  Require stale marking only under current read authority. Race write
  revocation against publication. Verify a reader refresh changes only
  that reader’s live view, including when the reader also has edit rights.
  Revoke body access with title access intact before admission and during
  delivery, for retained, stale and live views. Require zero body content.
  Page beyond one native page and follow new committed output. Revoke
  an asset grant during a bounded range read. Confirm no further bytes.
  Renew an unchanged retained page and an unchanged live page. Require
  neither blanking nor a plugin call. For item-view follow, live windows,
  default and explicit search and renewal, commit an authorized change
  just before subscription registration, between registration and the
  baseline read, and after the read before first delivery. Require the
  current result or its scoped invalidation, with no missed update or
  version regression. Repeat with revocation and a hidden-only edit.
  On one lease and Bind, open two collection scopes with distinct routes.
  Delay the first scope's invalidation until the second is drawn. Require
  that only the named scope clears. For opted-in invariant scopes, cache
  several offsets, invalidate one scope without a reason or revoke notice
  and restart offline. Require every offset of that scope absent with
  unrelated records intact. Race a buffered cache write and a restart
  while applying the invalidation.
  Invalidate an affected view or
  cut the lease or connection and require immediate clearing. Reopen
  under fresh authority. Meter wake separately from first paint.
- **Execution and widening.** Prove freeze, exact reattachment and teardown
  under transport and server loss. Verify one admitted attempt on replay.
  Attempt widening through direct share, membership, collection and Undo.
  Confirm whole committed content. Repeat after removing the kind's
  execution declaration and after uninstall while retaining the written
  file. Crash before dispatch and after commit.
  Check that queued output and retired session files cannot be delivered
  or reopened from a widened item. Verify generic approval dispatch binds
  to the checked effect. Test the approved build on an eligible hosted
  placement. Refuse an unapproved build there or on another person’s
  enrolled machine, and admit it on the actor’s own eligible enrollment.
  Recheck a changed build and a revoked enrollment. Apply the same
  checks to community and bundled harnesses. Try another harness’s
  login path without and with explicit sharing. Start a shared-space run
  as another member. Verify the default space binding and an explicit
  override to that member’s own enrolled machine and login. Reject
  another member’s login, a forged override and a revoked enrollment.
  Require tools off and audience-limited discovery with both bindings.
  Use the same declared prompt in a personal space with two eligible
  machines. Fetch, select and dispatch through the renderer exchanges.
  Verify the worker receives only the ref, that another activation or
  harness cannot use it, and that choosing a revoked option admits no job.
  Refuse stale builds and revoked bindings. Queue a successor as a
  different starter. Refuse archive by the head starter
  alone and refuse dispatch after a successor or authority change. Check
  that Stop of the head alone preserves the successor. Replay the queued
  receipt, fill the queue, revoke before promotion and require at most one
  running attempt. Give `start_job` and a restarted alarm conflicting
  refs in job input. Recheck the original binding on promotion and claim. Require the exact typed harness binding and refuse
  admission after that binding is revoked.
- **Lists and search.** Before enabling background list replay, cut power on
  both sides of the save and publication commits. Introduce a backlog.
  Require bounded recovery to live-authority rows and counts, loading without
  a count while behind, correct End replies, zero worker wakes and save ACK
  p95 ≤40 ms on the perf VM. Apply the two-key scalar-order cap and the
  separate TextSet budget. Prove the default query remains independent of
  custom ranking and plugin indexes in a slow or denied plugin arm.
  Reject profiles whose unreadable field can influence a match, ordering
  or continuation. Exercise default and explicit settle, a cursor chain
  ending without rows and unchanged renewal without a plugin call.
  For a plugin-search invalidation require the exact lease, Bind and
  settle generation to clear only that chain and its scoped selections
  and contexts, with no worker call. Deliver stalled old pages and a
  signal for a superseded generation. Neither may alter the fresh chain.
  Restart the invalidated chain only through a fresh settle. Reject its
  old cursors, selections and action contexts.
  Settle the same template as a member, guest and public viewer. Require
  zero plugin calls for guest or public, including a forged profile cursor.
- **Generic idle archive.** Use an ordinary declared kind and the real
  item writer. Under the lifecycle fence check idle eligibility. Crash
  before and after the FULL transaction that commits the archived head,
  any eligible grant retirement and exact Undo together. Verify neither
  effect nor Undo before commit and both exactly once after it. Race
  admission, approval and grant changes and refuse the shortcut when
  the predicate changes. Verify protected Undo when restoration adds
  readers, stale Undo refusal and zero worker wakes on an eligible
  first full swipe. Enable the shortcut only after this gate passes.
- **Alarm, offline and contribution.** Kill the server before a due alarm
  and after consume-before-reply. Require one late job with scheduled and
  actual time, and zero admissions after disable. Download a body and its
  attachments and cache a body-derived row part. Close all views, revoke
  the offline grant and change row authority in one scope while disconnected.
  Reconnect to a different app. Require the last completed cache in the
  first frame and erase omissions when reconciliation answers, including
  body-derived parts whose enclosing scope survives. Restart offline and
  verify surviving records still paint while erased data stays absent.
  Reconcile more than 256 valid grants in pages and retain all of them.
  Download more than one page of readable items with one grant, and test
  revocation of a part on a later page. Delay a reply, retire that attempt
  and retry with a fresh ID on the same connection. Replay the old reply
  and require zero changes to the inventory or display. Check offline
  sign-out and configured expiry. Install two editors for one Markdown
  format and two independent harness contributions. Verify explicit editor
  choice and mixed authorized rows, bounded optional field wake work and no
  effect of an uninstalled or disabled contribution.

### Step 3 — Application cutover

Build in order: Threads, Notes, Mail, Files, Photos. Before using
the generic path, Threads supplies historical headings and
attribution, attachments and queued follow-ups. Verify old and new
rendering from retained files, restart recovery and rollback. Notes
requires the history browser, linked-user comments and collection
creation. Portable upload, history browsing, comments and collection
creation remain open where the generic contract lacks those shapes.
Hold each affected cutover until its required behavior is pinned and
qualified. Mention picker and snooze presets or custom date may follow
as plugin features. Photos product questions wait for that plugin.

The [Threads spike][threads-spike] records the app and harness fixture.
Qualify the real contribution path with separate packages. Test a wrong
package, pinned-image mismatch, out-of-order output, lost output ACK,
Stop between output and append, and replay of the owning job. Require
zero unauthorized writes, no repeated provider start and one append per
accepted publication receipt.

The Threads cutover requires measured product wake p95 at or below
500 ms for both the plugin and agent placements on the perf VM.
Measure the actual cold Send path through its acknowledgement and
first agent output. Report both user-visible intervals separately.
Send acknowledgement waits for the plugin wake. First output
also waits for agent placement. Treat the two wake measurements as
cutover gates.

Before switching readers, durably record the widening-protection flag
on every retained legacy thread under the catalogue’s
[item-record rule](../contract/host-catalogue-v2.json#/attempt_authority_end/actions/item_audience_widening/item_record).
Check the entire retained inventory, including restorable files. A legacy
thread with no saved session still needs the whole-file disclosure when
shared. Verify the flag on restart and preserve the check across rollback.

Drain active attempts before switching writers. Remove native Turns
and Notes dispatch and authority aliases, the `threads` namespace owner
map and lookup, bundled-ID exceptions and native owner routing in startup,
jobs and turn admission. Installed manifests provide ordinary ownership.
Keep the generic collision checks, trusted install and additive migrations.
Delete the transitional `open_cutover_requirements` and
`application_cutover_requirements` catalogue tables after cutover.


## Owner decisions and open questions

The [recorded decisions][decisions] govern file custody, the Threads
composition, full offline content, search, one-shot alarms, retention
and identical bundled and community eligibility. The
[placement decision][placement-decision] sets hosted eligibility for
reviewed builds and user-enrolled placement for other builds. The
live-authority discovery rule above remains interim.

Application cutover gates are listed in §8 and the catalogue. Pin the
generic upload and linked-user comment shapes, history browsing and
collection creation before the affected cutover. Old-message attribution
and queued follow-ups are owned by the app and harness contract.

[generic-engines]: https://git.kayg.org/kayg/calternal-ai/src/commit/5abca642/docs/proposals/plugin-api-v2-generic-engines.md
[window-replay-measurements]: https://git.kayg.org/kayg/calternal-ai/src/commit/a96553bc314a4c4c6f695047b1ed6171e3b3bdfb/docs/reviews/window-engine-2026-11-19.md#proposed-revision-14--journal-backed-window-generations

[decisions]: https://git.kayg.org/kayg/calternal-ai/src/commit/18e467b6/docs/morning-2026-11-20.md#third-set-of-answers-owner-2026-11-20

[placement-decision]: https://git.kayg.org/kayg/calternal-ai/src/commit/a4154906/docs/morning-2026-11-20.md#third-set-of-answers-owner-2026-11-20

[threads-decisions]: https://git.kayg.org/kayg/calternal-ai/src/commit/d3d37a4cffa4ad4faff053a255caab70b3fee8b7/docs/morning-2026-11-20.md#threads-with-people-first-answers-owner-2026-11-20
[threads-spike]: https://git.kayg.org/kayg/calternal-ai/src/commit/c3c70b73/docs/reviews/threads-plugin-spike-2026-11-20.md
