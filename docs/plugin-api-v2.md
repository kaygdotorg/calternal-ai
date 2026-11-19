# Plugin API v2 · revision 2

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
settings, secret declarations, and proxy destinations. A target declares
`{kind, field}`. The host resolves the top-level input field as an item ID
before wake. Commands in the palette reference operations and typed inputs.
Turns is the one named engine an app can select. Provider contributions
declare a Turns adapter and reference its executable descriptor. Claude Code and Codex remain provider plugins under the existing
[provider protocol](../contract/provider-v1.schema.json) and
[events schema](../contract/provider-events-v1.schema.json). The host chooses
and launches the adapter inside the existing agent sandbox, outside the plugin wall. The adapter does not hold machine-control authority. Provider-only or
description-only packages may omit a broker worker. Operations, jobs,
schedule eligibility and host calls require it.

Reject unknown or inconsistent declarations before launch. Check unique IDs,
package operation namespace, references, schema compilation offline, pinned
assets and digests, indexed field types, and cross-reference constraints.
An operation may request only package-declared host methods, and each method
must also fit its operation capability and risk. Read-only operations
cannot mutate user data. A job may checkpoint its own run as part of the
durable job protocol. Irreversible operations follow protected approval.
A retry of a receipt-backed mutation carries the same request ID. A
non-idempotent external action is never retried automatically.

Install shows kinds, methods, capabilities, risks, settings/secrets,
providers and outbound disclosures. Gallery installs are available in one tap. Git/source installs warn. Pin the immutable digest. Members see installed
packages, permissions and destinations. Check and probe agent-authored code
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
fencing. Dispatch callbacks independently of the reader and fairly across
packages. The single writer assigns consecutive direction-local
`seq` at dequeue, where control can overtake unsent data only.
Reject gaps, duplicates and wrong epoch before effects. Reconnect mints a
new epoch and discards queued frames and credits. Replay is a new admission
with the same durable request ID where permitted. The wire caps frame and
queue size independently of author payload limits.

The **runtime epoch** fences the live lease and the **grant revision** fences the space's current permissions, installation
access and protected policy. These are the two authority counters. Package digests identify bytes. Wire and page
ordinals order messages and contents only. The ticket *is* the opaque
invocation reference. The durable receipt is keyed by request ID in its
existing actor/space namespace. A broker subeffect derives a stable key from
that request and its declared step or job checkpoint position.

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
   the reference. Durable state stores principal, scope and delegation. Runnable work receives a fresh ticket.

Each engine serializes grant-revision changes with commit. Check the current
revision, effect rights and target under the same authority fence held
through commit. A revocation ordered first refuses the effect and records no
success receipt. A commit ordered first is durable and its receipt replays
under current disclosure rights. A preflight authorization check is
insufficient. Fence egress at DNS/send and redirect dispatch and close
affected streams on revocation. Already dispatched bytes cannot be recalled.
The receiving renderer rechecks the viewer before delivering cached data or
a stream page.

For an admitted schedule, persist a separate revocable enrollment pinned to
creator, package digest, operation, target, permitted effects, approval mode
and IANA time zone. Remove is receipt-backed and fences unclaimed and
in-flight occurrences. The occurrence ID is enrollment plus canonical
scheduled instant, including the fold decision. Retries use its receipt.
Sign-out/revocation of the enrolled session pauses execution and requires
fresh authorization. Wait webhooks use only a signed, scoped correlation
for a particular parked job, validate typed payload and replay identity,
and cannot choose another actor, operation or target. Standalone routes
are outside the first contract.

Secret handles redeem only for the live actor, invocation, grant revision,
runtime epoch, secret version, exact endpoint and typed operation. The host
injects credentials and strips credential-bearing responses. Generic proxy
requests accept no credentials. Require exact approved hostnames before
DNS, pin checked addresses and TLS identity, and recheck authority,
hostname, address and TLS on redirects. Approval explains URL disclosure
including names, paths and query strings. Media stays anonymous. See Q6–Q7
for the default and approver.

Refusals return 4xx, code, plain message and optional path or next step.
Foreign resources give opaque `404`. Audit records include time, space,
package/digest, actor/delegation, operation/method, safe target ID, request
ID, outcome, grant revision, cost and latency. Omit values, tokens, URLs
with data and output. Reserve bounded audit capacity before effects. Read
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
| Items · `item_save` | Kind/item or create intent, expected head and structured metadata plus inline content or staged upload → committed item/revision. Large saves use begin → chunk → commit within this method. Begin reserves length/digest and issues a staged handle. Chunks carry byte offsets and bounded base64 bytes. Duplicate bytes replay and differing bytes conflict. Commit binds the same request, target and expected head, verifies length/digest, and publishes once atomically. Exceeding kind/document bounds gives `413` and wrong head `409`. Fixture proves existing 2 MiB text limit, even for escape-heavy data. Abandoned staging expires under quota. |
| Items · `item_trash`, `item_restore`, `item_rename`, `collection_create`, `collection_membership` | Item/collection refs, expected revision and requested change → head or durable action ID/status. Rename returns a durable action ID for status and an authorized, receipt-backed undo reference accepted by the rename method. Restore and undo are revision-checked. Membership is one bounded add/remove batch. No cascade outside an admitted job. |
| Queries · `items_list`, `items_search` | Kind/optional collection, typed indexed metadata predicates, declared sort with stable ID tie-breaker, cursor/window and field set → authorized page with cursor and count/offset where windowed. Unindexed filter/sort is `400`. Use engine-side query and bounded row/byte budget. Apply authority before count. No whole-kind materialization. |
| Rooms / sharing · `sharing_check` | Item ref plus requested action → permission for the live grant. A room is an attribute of the kind, opened client-to-host using its own bounded protocol. Room membership and events keep fencing the item grant. No plugin hook acknowledgment. |
| Jobs · `start_job`, `checkpoint_job`, `job_status`, `cancel_job` | Declared job, payload and resolved target/grant → durable job ID. Checkpoint only the owning current run. Status/cancel uses a scoped job ID with opaque `404` for foreign jobs. Bounded checkpoints, live progress and park count per space/package. |
| Settings / secrets · `read_setting`, `set_setting`, `request_secret_handle` | Declared key or secret purpose → effective value/version or opaque handle. Validate type, compare revision on writes and bind personal values to the actor. One bounded engine operation. Missing authorization is `403` or opaque `404` as appropriate. |
| Proxy · `content_prepare`, `proxy_request` | Authorized sealed content revision → opaque asset refs. Declared HTTPS destination and bounded typed request → bounded response. Unapproved destination is `403`, oversize `413`, quota `429`. Anonymous media keeps the existing 10 MB, three redirects and 16 hosts/minute fixture ceilings. Credential use follows the typed-only redemption rule above. |
| Turns · `turn_start`, `turn_status`, `turn_cancel`, `turn_follow_up`, `turn_answer`, `turn_select_machine` | Own turn/thread ref and typed operation payload → admitted turn, bounded status, queue state or protected-request receipt. A person can select their own enrolled machine and login for their own turn. The host resolves eligibility and approval. A plugin cannot choose arbitrary placement or act as the selected machine. Transcript pages and live append are client-to-host. |
| Schedules · `schedule_create`, `schedule_remove` | Declared eligible operation, pinned target/payload, time zone and calendar or enrollment ID → durable enrollment or removal receipt. One indexed, bounded write. Resume requires the enrollment and live authority. |

Job admission persists operation, target and grant. Revalidate at claim,
terminal park and resume, including any child and descendants. The runner
validates a terminal `parked` result before atomically saving checkpoint and
wait through the existing jobs engine. There is no post-park acknowledgment.
Inbox answer/approval, typed signed wait delivery, deadline and cancel race
through the existing winning-signal transaction and request receipts. The
losers have no effect. Park uses no worker once the run ends. Retain existing
wait bounds and state visibility. Repeated runner execution can occur after a
crash. Committed effects replay by receipt.

## 4. Descriptions and host components

An app ships data-free chrome and content templates. Static labels and
view structure may be bundled. Viewer records, results and credentials may
only appear in host-filled bindings. Select the published whole template by
(package digest, app, description ID). No patch/base-revision protocol or
viewer-keyed projection cache. The bundled template loads without waking
the runtime. Templates are bounded host-served assets outside the broker envelope.
Publishing one requires a new immutable package revision. Cache templates without viewer data. First release keeps
filled windows in authorized session memory only and drops them on sign-out,
revocation and viewer switch. An offline client can draw the template and
host fallback states.

The [chrome v1 draft](https://git.kayg.org/kayg/calternal-ai/src/commit/05aac6a4f173b336eb556fa3e6b7db9d31f4fba1/contract/chrome-v1.schema.json)
supplies existing native row and action conventions. Advertise supported
chrome versions and negotiate v2 for bindings. Keep v1 renderable during migration. A client returns a host compatibility reason and supported versions for an unsupported
required version. Content uses the [GenUI envelope](../contract/v0.schema.json#/definitions/GenUiView),
the [component catalogue](../packages/genui/src/catalog-data.json) and its
[expression budgets](../packages/genui/src/budgets.ts). The shared draft
[description extensions schema](../contract/plugin-description-v2.schema.json)
defines additions for both formats. The native and web lanes own the
integrations into their existing schema/catalogue. A plugin draws chrome
through chrome declarations. Apple renders with native components and the
web renders the same contract in Svelte. The ported block editor and sealed
HTML may use an isolated content-only web view.

- **Bind** is one node with closed `view` of item/list/search, kind,
  optional collection, declared fields, typed indexed predicates/sort and a
  bounded window size. Item and search arguments come from host route,
  settled selection or interaction slots. No filter expressions. Render
  empty, loading and error slots or host defaults. Chrome rows map to stable
  IDs, single-line title, leading icon, trailing marks and host-rendered
  swipe/menu actions. Keep existing action role, tone and safe-first
  full-swipe rules. Host queries use the viewer's rights even for the count.
  Return count, offset and rows for a requested `[from,to)` index window
  (fixture cap of 80 rows plus a byte cap). Host supplies stable IDs and
  a window revision. Replace the window on change, skip stale or identical
  updates, and keep selection local until it settles. The host debounces
  search (250 ms fixture), cancels stale queries and coalesces invalidations.
  Keyboard and reduced-motion behavior come from [native feel](native-feel.md).
- **Invoke** in content and a typed invoke action on a chrome row/control
  reference a declared operation and input. Evaluate input expressions under
  the existing GenUI budget, including route, selection and typed prompt
  text. The host admits an ordinary operation, owns the request ID and
  correlates a pending indicator, result or plain error with the event ID.
  The host displays pending and suppresses activation while keeping focus. Retry and double tap reuse the ID while pending for receipt-backed
  operations. The tree cannot read the result directly. Bindings refresh
  after committed writes. Destructive confirmation is host-owned.
  Events carry a rendered revision and stable target IDs. Coalesced
  updates can skip intermediate states. Actions can be delivered more than
  once. A successful effect commits once within receipt retention.

[Component configs and events](../contract/plugin-description-v2.schema.json)
use host-issued scoped refs and protected routes. The tree cannot read raw
event data. See the [host catalogue](ui-host-catalogue.md).

| Host component | Config and owned events |
| --- | --- |
| Item editor | Item ref. Owns load/save, revisions and the kind's live-room attachment. |
| Transcript | Turn ref. Owns selection-cancelled loads, windowed scrolling with anchoring, first-screen render marker and durable append stream. Plugin layout cannot wrap or virtualize its internals. |
| Composer | Item/turn target and typed send operation. Host owns drafts, send/stop/attach/mention events and request state. |
| Approval/request card | Request ref. Host owns answer/approve/deny and keeps the card accessible outside clipping or overlays. |
| Sealed HTML | Opaque frame ID. Host owns sandbox attributes, load-remote-media policy and bounded height reports, with no top navigation or plugin-defined message channel. |

Transcript reconnect uses a durable committed cursor and byte-bounded
pages, independent of the ephemeral job ring. Host batches append, rate
limits delivery and emits a final committed cursor. Selecting a different
item cancels the old load/stream. Window render includes a host-owned
rendered stamp. Slow readers resume from the last durable bookmark or get
an authorized reload response.

## 5. Sharing and aggregation

Members trust the Owner's installed plugins with data available to authorized
calls in that space. A warm worker can remember an Owner value and return
it to a Reader acting on their own ticket. Host-filled bindings protect
engine query results, and the installed-plugin trust rule still applies to
what a worker knew or previously wrote. Run all guest logic in the home
space charging its quota. Guest outputs and public links are host-rendered
and grant-checked, with free-form worker output excluded. A public
link serves a saved host view and never wakes.

Guest tickets cover the shared home item and only its host-recorded children
authorized by the grant. Propagate the same scope through every resolved
reference, child job, tool, room and media handle. View and Comment operate
under that scope. Explicit Prompt remains a product grant, off by default
with a warning and Owner approvals, and ships disabled until the entire
descendant/tool/machine path has proof. Cross-reference links are no grant.
“Continue in my space” copies the permitted item and attachments under the
receiving space's rights and quota. If the package is missing, offer
authorized install or refuse with a plain reason. Exclude credentials,
grants, approvals and run state.

“Shared with me”, Inbox, search and notifications are host features. Derive
their feeds from authorized item journals and job parks. Workers cannot
publish arbitrary feed summaries. Revalidate on delivery and remove
revoked entries and media from subsequent views.

## 6. Deliberate cuts

Private package database, description patches and base revisions, viewer
projections, worker-published feeds, room hook acknowledgments, standalone
inbound dispatch, raw TCP, private-network and open-internet proxy grants,
guest Prompt execution, snapshots, and gallery listing fields are outside
the first contract. Live room sync is an engine facility. Gallery listing
policy stays in the submissions repo. These cuts do not retire existing
features from the current API.

## 7. Author SDK and conformance

`defineApp`, `defineKind`, `defineOperation` and declarative package
metadata generate a checked manifest, method inventory and immutable
asset/digest index. Typed `ctx` methods derive from shared contract models.
A retry keeps the same request ID and typed target. Authors never pass
`space_id` to broker methods or see tickets, epochs, leases, seq, callback
IDs or renderer keys. Host-mediated client operations may still select an
authorized space. The SDK’s worker-v2 `host_call` remains private
to the fixture adapter. Space-channel `v:1` uses `call`. Generate each wire
contract and teach only the typed API in the public guide.

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
| Wire, quotas and control | Invalid UTF-8, duplicates, deep or oversized JSON, unknown fields, forged package/epoch, frame replay and wrong direction have no effect. Saturate one package and one space while another makes progress and cancel/control still arrives. |
| Independent permission matrix | Exercise Owner, Manager, Editor, Reader, foreign account, guest View/Comment and revoked ticket across methods and deliveries. Match expected capability/target outcomes from independent fixtures. A compromised supervisor reaches neither a second space nor machine control. |
| Commit and recovery | Race role/session/plugin revocation with effect commit and egress dispatch. Crash before and after save/park and reconnect with the same receipt. Race answer/deadline/cancel and assert one winning effect. Reject stale handles and changed agent-authored digest. |
| Scoped render and egress | Seed a warm worker with privileged data and assert the guest sees only host-bound fields. Check counts/windows, empty/load/error, selection coalescing and no wake on a cold read. Refuse disallowed URL/DNS/redirect without connection or foreign media. Verify audit redaction. |
| Lifecycle and migration | Coalesce wake, bound drain, replace after fault, verify clean boot and exhausted admission. Verify upgrade, remove, active job fences and real/fake parity. Test that parked jobs and signed wait signals survive restart. |

## 8. Build order and gates

| Step | Deliver and prove | Unblocks |
| --- | --- | --- |
| **0. Prove four paths** | Run the pre-freeze spikes below. | Transport and payload budgets. |
| **1. Freeze the smaller contract** | Generate Rust-backed schemas and TS SDK, align both renderer schemas, adapt existing broker/jobs on the container wall, and pass §7. Run the public-guide-only trial against both the fake broker and a real install. Freeze after Q1/Q2 and any applicable policy decisions. | Stable contract for app authors and client renderers. |
| **2. Integrate real placement and engines** | Wire host rendering and subscriptions, real space supervisor, commit fences, media/guest proofs, lifecycle and release-pair recovery. Use the measured wall results for later hosted snapshot enablement. | Preview with the real minimum Notes surface. |
| **3. Move Notes first** | Require parity for revision-safe save/history, large content, Trash/restore/rename/undo, collections, search, offline draft recovery, sharing and the ported block editor with Yjs rooms. Verify current app flows and rollback. Then move Threads. Mail waits for the later access contract. | Refactor apps without inventing app-specific broker paths. |

Step 0 uses real engines. Record tail latency, payload size, reads and flushes.

- **Carrier.** Two package processes on the enrolled link, with one stalled
  reader, a slow callback and reconnect. Prove control delivery,
  independent progress, revocation and same-receipt retry.
- **Cold page.** A 100,000-item kind with indexed windows and authorized
  count. Prove zero wakes, mid-page revocation and action receipt replay.
- **Large save.** Atomic 2 MiB save with asynchronous host journal consumers.
  Prove revocation ordering and one commit on crash/retry.
- **Transcript.** Batched durable append, slow reader and committed cursor
  reconnect. Prove mid-stream revocation and single submission on replay.

## Open questions for the Owner

- **Q1 Interactive deadline — OPEN.** Recommend a three-second execution budget after a distinct bounded wake allowance, subject to the carrier spike.
- **Q2 Ticket lifetime — OPEN.** Recommend at most 60 seconds or until cancel, terminal or deadline, with renewal only through live durable job admission.
- **Q3 Guest access to a future package store — OPEN.** Recommend granting none by default and revisiting only with an explicit item-bound profile.
- **Q4 Gallery age ratings and screenshots — OPEN.** Recommend submissions-repo policy with optional age rating and one representative screenshot for visual apps.
- **Q5 Private package database — OPEN for a later contract.** Keep the decided broker-only first release. Revisit only for a use case that existing engines cannot cover.
- **Q6 Destination approver — OPEN where the decisions overlap.** Preserve the earlier Manager-approved named envelope with personal hosts inside it and the later Owner approval for plugin-wide changes. Recommend confirming the split and who can widen an envelope.
- **Q7 Media default — OPEN (#80).** Recommend outbound disclosure at install and following #80’s recommended default on for a Manager-installed package once its destination policy is approved.
- **Q8 Plan mapping — OPEN pending measurement.** Recommend choosing plan ceilings from measured wake, memory and saturation rather than embedding tiers in the contract.

## Review responses

| Finding | Disposition |
| --- | --- |
| 1 | Accepted · §4 and schema define one host-filled binding in both formats, with indexed windows and viewer rights. |
| 2 | Accepted · §4 uses immutable template keys only and removes description patches and viewer projections. |
| 3 | Accepted · §5 states the same-space trust and enforces host-bound guest/public output. |
| 4 | Accepted · §4 and schema give both formats an Invoke action with ordinary admission and correlated results. |
| 5 | Accepted · §4 and schema give the five protected components configs/events and direct transcript streaming. |
| 6 | Accepted · §§1, 3 make rooms a kind attribute, fold collection filtering into generic queries and declare Turns. |
| 7 | Accepted · §3 specifies generic history, Trash, restore, rename/status/undo and collection mutations. |
| 8 | Accepted · §3 adds turn controls and host-mediated selection of the person's own eligible machine/login. |
| 9 | Accepted · §§1, 3 add runtime enroll/remove with creator and IANA time zone. |
| 10 | Accepted · Scope and §6 defer real Mail to a later contract and drop TCP. |
| 11 | Accepted · §1 and manifest add provider and command declarations and require a worker for broker executables. |
| 12 | Accepted · §3 serializes commit and dispatch with revocation, retaining already committed/sent effects. |
| 13 | Accepted · §2 bounds supervisor authority to one space, with negative conformance cases in §7. |
| 14 | Accepted · §5 attenuates descendants and keeps guest Prompt disabled until proven. |
| 15 | Accepted · §5 includes recorded children and install-or-refuse on copy. |
| 16 | Accepted · §3 specifies enrollment fences, sign-out and replay, with standalone dispatch cut. |
| 17 | Accepted · §3 binds handle redemption to invocation, actor, live grant and typed endpoint. |
| 18 | Accepted · §§1–2 require immutable digest approval, one-use restore epoch, reseeding and process restart when needed. |
| 19 | Accepted · §§2, 7 require pre-decode/launch/DNS/I/O charging and independent progress. |
| 20 | Open on approval/default · §3 adopts egress controls. Q6–Q7 retain the recorded choices for Owner resolution. |
| 21 | Accepted · §3 specifies revision-pinned ranges and atomic staged save within the encoded envelope. |
| 22 | Accepted · §§1, 3 and manifest restore the target field and fence it across durable transitions. |
| 23 | Accepted · §3 retains validated terminal park and the existing winning-signal transaction. |
| 24 | Accepted · §§2–3 specify out-of-wake execution, direction credits, reserved control, strict parse and reconnect. |
| 25 | Accepted · §4 specifies host paging and durable transcript bookmarks. §8 measures both. |
| 26 | Accepted · §8 makes the four spikes and their authority/replay proofs pre-freeze gates. |
| 27 | Accepted · §3 uses two authority counters and collapses ticket/invocation and request/receipt IDs. |
| 28 | Accepted · §3 specifies bounded read-audit batching and back pressure. |
| 29 | Accepted · §§7–8 make the typed SDK, fake, diagnostics and public author trial freeze gates. |
| 30 | Accepted · §7 distinguishes inner and outer framing and removes broker space selection from the author API. |
