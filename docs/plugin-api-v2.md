# Plugin API v2

Specification for review, based on the owner's November 19 interface, runtime
and single-item sharing decisions in [Recently decided](open-questions.md#recently-decided-2026-11-17-now-in-the-decision-records).
Rules marked **Q** are proposed conformance targets awaiting approval. The
listed numbers are fixture settings where labelled, and confer no product
entitlement. Ship neither a new authority path nor an app migration on the
strength of a draft or an unmeasured isolation wall.

## Scope and supersession

| Earlier record | Exact provisions this specification replaces for v2 |
| --- | --- |
| [Plugin API](plugin-api.md) | **Decided (2026-11-17)** bullets on app order, the three regions and UI/HTML/CSS, manifest shape, plugin execution and sandbox placement, plugin storage, and network approval. **Provider and risk contract (step 1)** manifest-operation declaration and its activation contract only. **Authoring and interface prototype** worker activation and renderer/slot publication as a client contract. **Page a plugin’s own kinds** final paragraph on worker placement and cold calls. **Palette discovery** delivery of app contributions. **Sealed plugin content** declarations, approver and destination policy. **Durable job waits** and **Signed webhook delivery** invocation authority and wake rules only. **Open** cloud sandbox question. |
| [Interface proposal](plugin-interface-proposal.md) | **What a plugin can contribute**, including **App switcher entries**, **Sealed components**, **Stateful generated views**, **The shell decision**, **Plugin worker placement and network access**, including **Brokered network capabilities** and its subsections, for v2 approval and scheduling, **Swift clients**, **Notes and Threads as ordinary plugins** ordering and entry gates, **Can official plugins use the same contract?** placement, dispatch and renderer architecture. |
| [Plugin workers](plugin-workers.md) | Preamble through the paragraph on edited revision readiness, and all of **Placement and unresolved contracts**, as the target topology, worker transport, admission and activation rules. **Development and tests** remains the current container adapter procedure. |

Keep the earlier records as v1 references and evidence. In particular, item
revisions, receipts, merge semantics, room protocol, host-managed settings,
diagnostics, provider adapter and agent permission modes carry forward.
**Transport measurement** and **Warm browser-to-worker operation** remain
historical measurements. **Acceptance check for the author workflow** remains
a release gate. New app clients consume descriptions. Apple navigation, lists,
actions and catalogue renderers are native. Web uses the same descriptions in
Svelte. A content-only web surface remains the exception for the ported block
editor and sealed HTML. The shell, approval, secret and identity surfaces
remain host-owned.

### What exists today

[broker.rs](../crates/server/src/plugins/broker.rs) binds calls to an invocation
and checks live session, activation and capabilities. It handles item reads,
own-kind pages, writes, setting reads, job admission/checkpoints, secret handles
and media frame preparation. The old wire strikes unregistered callbacks and
returns a null failure payload. [Worker transport](../crates/server/src/plugins/worker/transport.rs)
starts leased per-package workers through machines. The current Podman profile
isolates the worker and strips sockets and host mounts. It is the starting
adapter for a no-KVM wall. Existing microVM code handles agent sandboxes, with
different placement and lifetime. There is no per-space snapshot supervisor or
space channel yet. [Jobs and waits](../crates/server/src/plugins/jobs/waits.rs)
already persist checkpoints and settle one winning signal in the jobs store.
[Media fetch](../crates/server/src/plugins/network/fetch.rs) is the implemented
anonymous image/font path. Other outbound profiles are proposed. The current
[sharing service](sharing.md) supports Notes and collections and public
view links. Thread Comment and Prompt below are target behavior.

## 1. Package and manifest

The host reads [Manifest v2](../contract/plugin-manifest-v2.schema.json) before
launching code. Validate JSON, referenced files and byte limits (256 KiB manifest and a 1 MiB worker in the first fixture profile).
Then check uniqueness, symbol references, operation namespaces, asset
digests, schema compilation and the consistency rules below. IDs in the manifest
are package-local identifiers unless qualified. Runtime addresses use the
installation's UUIDs and opaque revisions, independently of display names.

- An app declares its ID, label, icon and the chrome and content descriptions
  it publishes. The initial pair references bundled descriptions by ID so it
  can draw on a cold installation. A package may also ship zero apps and
  contribute an engine, command or extension. A kind registers its storage
  and metadata schema, search opt-in and optional host live editing.
- An operation declares input and output schemas, an existing capability,
  risk, effect flag, idempotency and optional item target. A read is
  `mutates: false` and `read_only`. A mutation uses a receipt-backed
  `request_id` or explicitly `never`. A non-idempotent external operation
  cannot be replayed automatically. Irreversible actions and widened access
  use the existing single-use approval path. Changing a target or effective
  capability through a hook requires a fresh admitted request.
- Jobs identify their run operations and allowed wait sources. Question and
  webhook declarations reference payload schemas. Schedules and inbound
  routes pair with declared operations. Settings declare personal or space
  scope, type and default. Secrets declare name, label, purpose, scope and
  optionality. The manifest grants a right to ask for a handle, subject to
  live approval. It contains no values.
- Install shows operations, capabilities, risks, broker methods, kinds,
  storage, background triggers, secret requests, outbound kinds and their
  destination envelopes. The host derives this view from the manifest. The
  gallery permission statement supplements it. The gallery carries author,
  source, license, summary, description, screenshots and age classification
  (**Q1**). Use the existing public submissions repository and review flow.
- The space may designate an owned database for each package (**Q2**).
  Proposed first adapter: a host-owned SQLite database in the space's private
  storage, accessed only by that package's brokered records API. The plugin
  names records, never a filesystem location or SQL. A limit includes live
  pages, WAL/sidecars and reserved transaction growth. Reject a full write
  without changing the old value. An expiry is optional within the approved
  retention ceiling. The database is backed up, migrated and erased with the
  space. Engines still own user-visible items. Preserve the old app's
  [storage order](https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0062-user-files-on-hdd-app-data-on-ssd.md).

## 2. The runtime belongs to the space

One isolated runtime and one active package process per enabled plugin in a
space. The process and its state persist between calls while awake. Each
interactive operation must answer within its deadline (3 seconds under
provisional Q3). Work that needs longer runs as a durable job and is outside
that interactive deadline. A plugin can operate on the whole space within its approved grants and
the acting person's rights. The same wall, authority checks and limits apply
to official and community packages. Each process has a separate OS identity,
read-only pinned code and independent resource accounting. The trusted
supervisor alone owns the channel. Processes cannot open another process's
files, pipes or identity. The host and engines live outside the wall.

| State | Host transition and visible result |
| --- | --- |
| Asleep | Serve items, lists, search and saved descriptions from engines or device cache. An admitted logic operation, runnable job, due schedule or verified webhook requests wake. Reject unauthorized, expired or duplicate work before a wake. A parked job is not runnable until its winning signal commits. A public link only reads the last saved view. |
| Waking | Reserve one runtime lease. Coalesce concurrent wakes. Verify wall, pinned bundle and snapshot identity. Queue bounded work outside the wall, with deadlines. Overflow returns `429`. Reauthorize before first dispatch. Late startup after cancellation cannot deliver old work. |
| Awake | Dispatch through the one channel. The supervisor starts package processes on demand. Count running and admitted work against the space and package before dispatch. A process crash fences that process's invocations immediately. Other packages keep their own channels to the supervisor. |
| Draining | Stop admissions to this generation and finish admitted work within a bounded deadline. Keep committed effects and checkpoints. On expiry cancel remaining invocations and fence callbacks. Newly admitted work goes through wake/admission after the transition. |
| Going to sleep | Verify quiescence and close I/O. Persist only the approved snapshot and seal its identity. Release capacity only on acknowledged stop or durable placement revocation. Snapshot failure falls back to clean stop. Transition to Asleep even if work arrives during snapshot publication, then wake once. |

State that must survive belongs in a host engine, durable job checkpoint,
published description or package database. Memory is opportunistic even
across successful snapshot restore. **Snapshot uniqueness:** bind an image
to this space, wall image, supervisor and package revisions. Issue a new
channel epoch and reseed randomness from fresh host entropy before resumed
code handles requests. Drop inherited descriptors, tickets, counters and
random buffers relied on for identifiers. Never clone live memory into another
space. A host or runtime crash invalidates the old epoch. Reconcile in-flight
effects by receipt, resume only replayable work and reopen cleanly if snapshot
verification fails. Return a safe retry/interruption result for other actions.

The Owner’s plan buys compute and apps and sets the space’s quotas. The exact
mapping from plans to ceilings is pending. **Q3** covers fixture clocks,
capacities and restart policy. Candidate fixtures use
64 queued and 32 active calls per space, 8 active calls and 16 queued per
plugin, 32 queued frames and 128 callbacks per invocation. Use fair admission.
CPU, RSS (including snapshots), PIDs, storage, traffic and waits need both
space and package limits. Inject small ceilings in quota tests. Count starts, exits awaiting proof and
upgrade candidates against reserved capacity. Exhaustion is visible and never
steals another space's allocation.

**Pending measurement — `thr_8t85rv4yx8`:** hosted space microVM wake latency
and idle/awake/snapshot memory. Rootless agent containers in the same space VM
with provider login isolation. Self-hosted KVM and no-KVM topology, performance
and boundary strength. The hosted choice is conditional on the first result.
Self-hosted operators choose the wall and see the weaker isolation clearly
labelled. Direct development execution supplies no isolation proof.

## 3. Broker and authority

### Wire proposal (Q4)

One authenticated, reliable bidirectional transport per awake space. Use the
enrolled machine link as carrier. A host handshake selects protocol major and
limits, binds space/placement and fences the previous epoch. Inside the wall a
trusted supervisor assigns package identity from the process endpoint and
stamps the outer frame. Example host-facing envelope:

```json
{"v":1,"epoch":"lease-generation","seq":12,"plugin_id":"calternal-notes",
 "invocation_id":"host-invocation","message":{"type":"call","call_id":1,
 "ticket":"host-issued-reference","method":"read_item","input":{"kind":"notes","item_id":"uuid"}}}
```

Use one UTF-8 JSON object followed by LF, 256 KiB per object excluding LF.
Reject duplicate keys, invalid UTF-8, nesting beyond 32 levels, unknown envelope
fields, invalid direction and gaps in the per-direction `seq` before effects.
A malformed process frame fences that process. Outer framing corruption fences
the channel. Message types are `invoke` (operation, validated input, ticket,
deadline), `call`, `reply` (call ID and result/error), `chunk` (job ID, ordinal,
data), `complete` (result/error), `cancel` and `ready`. IDs correlate within
the authenticated epoch. Each callback ID is unique inside its invocation.
Terminal completion is single-use. A transport replay is different from a
client retry with the same durable request ID. Bound decode, buffering and
back pressure before admitting another frame.

Every call and resulting disclosure enforces three checks:

1. **Space by channel.** The host uses the authenticated lease's installation,
   space and current epoch. A plugin's input never selects a space, root or
   machine. Resolve item ownership and target kind in the bound space.
2. **Plugin by capability.** Check supervisor-bound package identity, revision,
   activation generation and its currently approved host method and capability.
   Bind the method's engine capability to the parent operation and consent.
   A read-only operation may use only read methods. A callback cannot widen
   declared risk, target or consent.
3. **Acting person by ticket.** The host mints an opaque invocation reference
   after checking the real account/session or share. The host-side record names
   account, space, package/revision, operation, audience/target, effect and
   approval constraints, invocation, expiry and epoch. Effective authority
   intersects live account, item/share, package and operation permissions.
   Check live authority immediately before a durable effect and again before
   disclosure. A revision epoch may cache a decision only while unchanged.

The host clock controls expiry. A proposed ticket lasts until terminal/cancel, the operation deadline or
60 seconds, whichever comes first (**Q5**). A long job renews only through its
durable admission fence and the same current authority. Persist the creator
and grant references and operation scope for jobs/schedules, then mint a new
ticket at each run. Never persist a usable ticket. An authenticated inbound
route uses an explicit enrolled owner/creator delegation and schema, or a
previously admitted wait's creator, with live revalidation. Route proof alone
grants no person authority. Guest tickets name the guest account and one
`(home_space,kind,item_id)` plus the grant ID and allowed actions. Pointers to
other items never extend the scope. Public links don't get an execution ticket.

Members of a shared space trust its installed plugins with data that an
authorized person can access through the plugin. Members can see which plugins
are installed, and broker calls are logged. A long-lived plugin can remember
data from an earlier, more privileged call. By default, described lists and
items bind to host queries filled with the viewer’s rights, including while
the runtime sleeps. Showing free-form worker output to someone other than
the ticket’s acting person is an exception gated on proof that memory, durable
records, caches and outputs are isolated across authority scopes (Q13).

### Methods and bounded outcomes

Names and shapes below are the target wire surface (**Q4, Q6**). Optional
fields are marked `?`. `ref` means `{kind,item_id}` in the implicit space.
`receipt` means the normal host-issued request/effect key, scoped by actor and
space. The host validates both directions. Cost bounds exclude shared
authority checks. Reads perform zero durable flushes.
Methods marked “adapter” reuse the named engine or existing broker call.

| Engine / method | Input → success | Bound and specific refusal |
| --- | --- | --- |
| Items `read_item`, `write_item` (adapters) | `ref,revision?` → `id,revision,metadata,text`. `kind,item_id?,base_revision?,text?,metadata?,edit?` → `id,revision,receipt` | One route and one body read. One bounded write and normal journal/file durability. Keep exact replacement/append/patch rules and 256 edit spans. `404 hidden`. `409 stale_base` before a write. Route larger Notes payloads through the authorized engine and room paths. A sliced broker profile is optional. |
| Lists `list_items_v1`, `list_collection` | `kind,limit?,label?,cursor?` or `collection_id,limit?,cursor?` → `items:[{id,revision,updated_ms,title?}],next_cursor?` | One indexed page of at most 100 rows (collection engine may be lower), no bodies. Bind cursor to query/actor/space/package. Guests have no space lists. `400 invalid_cursor`, `404 hidden`. |
| Search `search_items` | `query,kinds?,limit?,cursor?` → `hits:[{ref,revision,title,snippet?}],next_cursor?` | One capped index window, one authorization pass, no unbounded scan. Cap query at 1 KiB and results at 50. `400 invalid_query`, `429 limit`. |
| Live rooms `room_head`, `room_hook` | `ref` → `revision,lineage`. `ref,base_revision,event_id,phase` → `acknowledged` | One authorized head lookup or one event receipt transaction. The host emits declared post-commit hooks. The worker may only acknowledge its invocation’s event. Engine owns Yjs and consistency. `404 hidden`, `409 stale_base`, `429 limit`. Client typing never waits for a hook. |
| Sharing `sharing_check` | `ref,action` → `allowed:true` | One item/grant check, no grants dump. `404 hidden` or `403 denied` for a visible item and refused action. Grants are written by the trusted sharing service. |
| Jobs `start_job`, `checkpoint_job`, `park_job`, `job_status`, `cancel_job` | `operation,input,parent_id?` → `job_id`. `job_id,checkpoint` → `checkpoint_seq`. `job_id,checkpoint,source,deadline_ms,reason` → `wait_id`. `job_id` → `job_id,state,wait?,result?` or `job_id,state,receipt` | One indexed read or fenced transaction per effect, payload limit below. `404 hidden`, `409 wrong_state`, `422 invalid_signal`, `429 limit`. Only the current run can park/checkpoint. Park commits and closes the invocation. Its terminal reply may only acknowledge that commit. Closed runs cannot emit chunks. Use existing person/Manager cancellation policy. |
| Settings `read_setting_v1`, `set_setting` | `key` → effective typed value. `key,base,value` → `effective,conflicted,receipt` | One scoped read or one checked storage transaction. Personal means the acting person. Shared edits need settings authority. `404 unknown_key`, `409 conflict`, `422 wrong_type`. |
| Secrets `request_secret_handle` (adapter) | `name` → `handle` or `request_id` when trusted entry is needed | One lookup and at most one durable Inbox request. Bound to purpose, package and authorized destination. Rotation invalidates use immediately. `404 unknown_secret`, `403 denied`, `409 needs_confirmation`. Redemption occurs in a trusted engine. |
| Network `content_prepare_v1`, `proxy_request` | `ref,revision,field` → `frame_id`. `profile,destination_id,address?,request,secret_handle?,receipt?` → `status,headers,body` or `job_id` | One bounded sealed parse for media. Proxy bounds below. Use purpose-built engines for persistent sessions. `403 egress_denied`, `404 hidden`, `422 invalid_destination`, `429 limit`. Allowlisted HTTP method and headers, with an inline body of at most 128 KiB in both directions. Named engines use their typed request profile. Upstream status is data. |
| Turns `start_turn`, `read_turn` | `ref,prompt,provider?,mode?,receipt` → `turn_id,job_id`. `turn_id,cursor?` → `status,events,next_cursor?` | Existing admission/approval rules. One durable admission or one capped 100-event page. `404 hidden`, `403 denied`, `429 capacity`. A provider login never becomes a plugin handle. |
| Descriptions and feed `publish_description`, `publish_feed` | `app_id,description_id,version,base_revision?,body_or_patch,ref?` → `revision`. `event_id,ref,kind,summary,revision` → `receipt` | One bounded schema validation and transaction, plus invalidation enqueue. `404 hidden`, `409 stale_base`, `422 invalid_description`, `429 limit`. Feed payloads are projections of a durable, authorized source. |
| Package DB `storage_get`, `storage_apply` | `key` → `value,revision,expires_ms?`. `changes:[{key,base_revision?,value?,expires_ms?}],receipt` → `revisions,receipt` | One keyed read or one bounded transaction. Omitted value deletes. Expiry rules and quota include write amplification. `404 missing`, `409 stale_base`, `429 storage_limit`. Guests cannot use this general package store. |

Refusals use one plain caller-safe `{status,code,message,retry_after_ms?}`
with 4xx-class status. Use `401` for expired tickets, opaque `404` for
foreign and absent targets, and `409 outcome_unknown` with reconciliation
instructions for an unconfirmed effect. Never report a refusal as 5xx, echo a
stack trace, or disclose hidden target or credential existence by status/body.
A machine failure is a host diagnostic and an interrupted invocation, with
no implicit effect retry. Audit records timestamp, opaque request/effect and
invocation IDs, space UUID, package/revision, actor and delegated grant type,
method, authorized target reference, risk, outcome, elapsed time, cost/bytes
and approved destination ID/host when applicable. Exclude tickets, secrets,
URL paths/queries, input bodies and foreign target identifiers. Bound audit
and diagnostic retention (**Q7**).

A worker cannot reach raw files, raw sockets, credentials, another process
or another space, mint authority, bypass receipts or high-risk approvals. Core auth, recovery, account
administration, machine execution and protected UI stay host-only. Engine code owns the actual reads/writes. Keep
transaction, queue and validation work space-local and bounded.

### Durable waits and egress

Reuse the existing job store, Inbox, webhook verification and write receipts.
Park with a typed question, approval, webhook correlation or deadline and a
durable checkpoint. Keep 64 KiB checkpoints, 16 KiB signals, seven-day
deadlines and caps of eight parked waits per package/space and 32 per space.
First valid signal wins against timeout/cancel in one transaction. A duplicate
gets its receipt. Late or invalid inputs cannot wake a second run. Resume after
crash with the same checkpoint, signal and effect IDs. Delivery can retry. Durable effects deduplicate within receipt retention. Sign webhook deliveries
using [the existing HMAC and replay rules](plugin-api.md#signed-webhook-delivery).
Foreground request receipts keep the existing seven-day default/operator
override. Bind a receipt to the request fingerprint and stable effect ordinal.
Reuse with different input is a conflict. Multi-engine mutations have no
implicit transaction. Unknown outcomes require reconciliation.

The Owner approves all named destination envelopes and re-approves a changed
envelope before use. Revocation takes effect immediately. The four permissions
are named host/port/protocol, anonymous stored-media fetch, open internet within
an explicit public envelope, and private hosts/ranges through an approved origin. Broader envelope semantics,
remote-media prompt defaults and private route eligibility are **Q8**. Preserve
current netguard protections: checked DNS answers, dial only a checked address,
recheck each redirect and current approved grant, enforce time/size/rate caps,
deny metadata and unapproved addresses. Strip ambient credentials and headers.
Only approved named engine calls may redeem a handle for that exact destination.
Show that a media URL can carry data even when the request is anonymous.
Current Class B media limits are the fixture baseline, including 10 MB,
three redirects, 10 seconds and 16 distinct hosts per space/package per minute.
Raw TCP access from a plugin process is outside the contract.

## 4. App descriptions

Use the [pinned chrome v1 draft](https://git.kayg.org/kayg/calternal-ai/src/commit/f32205db573716108093f98038f73d8bea3880f8/contract/chrome-v1.schema.json)
and its event definition for sidebar, header and actions. Content uses the
[GenUiView envelope](../contract/v0.schema.json#/definitions/GenUiView),
existing [catalogue fixtures](../contract/genui-fixtures/catalog.json),
[language fixtures](../contract/genui-fixtures/language.json) and
[resource budgets](../contract/genui-fixtures/budgets.json). These specify
renderer input and evaluation coverage. The catalogue fixtures are test
vectors, with the component vocabulary owned by the catalogue. No new
component primitives are specified here.

**Q9** defines the publication profile. Proposed: negotiate chrome and content
major versions and supported component features at connect. Serve the last
compatible revision from a host projection keyed by space, package revision,
app, description ID, target, viewer and authority revision. An audience-neutral baseline may use a separate key only if it contains no scoped data. Bundle a baseline pair for
cold install. Publish described data only from an authorized invocation. Keep the latest compatible head per key under package and space quotas. A
cached read serves with live authority and no runtime wake. The host fills
item and list bindings using the viewer’s rights on every read. Clients subscribe
to a host journal invalidation. Revocation, package removal and stale bookmarks
invalidate caches. A reconnect after a gap requests a fresh snapshot. An
unsupported optional feature uses an authored fallback, and a required
unsupported one yields a host compatibility message.

Local selection, disclosure, focus, drafts and pure bounded expressions run on
the device with catalogue inputs. They do not read files, fetch URLs, execute
plugin code or grant access. An action binds a declared operation to validated
inputs and the rendered revision. The host looks up that binding, checks
staleness and authorizes the action through ordinary admission. Chrome events
carry the draft revision. Event timestamps are telemetry, never authority.
Publication carries exactly one of `{body}` or `{patch}`. Only creation omits
the base revision. Proposed patches are at most 128 `set` or `remove`
entries with a schema-typed segment path and a value for `set`. Validate the
whole result and commit atomically. A missing base, gap or
unknown patch version reloads the whole saved description. Whole publication
is the first supported path. Web content exceptions are isolated to the content
card and receive only explicit, authorized action bindings.

## 5. Sharing and the host feed

The home space stays canonical. A signed-in guest needs no installed package.
The host renders the projected description and routes an authorized operation
to the home space using the guest's item ticket. View, Edit, Comment and
Prompt are separate grants. Thread Prompt is explicitly enabled with a warning
and is off by default. Thread approvals still go to the Owner, and guest
activity uses the Owner's quota. **Continue in my space** is an explicit,
authorized copy with a new ID, the recipient's authority, machine and hours
(**Q10** for the copy contents). Public links serve the last saved, link-safe
view. Cache misses degrade to a host fallback or opaque 404 without a wake.

The host aggregates Shared with me, Inbox, search and notifications across
authorized spaces using host-owned indexes and feeds, with per-item live
checks before delivery. Plugin feed events carry a stable ID, home item ref,
revision, event kind and bounded display summary. Host assigns audience,
sequence and notification eligibility. Plugins supply no routing identities or
push credentials. Preserve the existing rule that push shows a thread's title
and status, with message content withheld. Readers never wake contributing
runtimes to build these projections. Removal or revocation invalidates
outstanding read models and feed projections.

## 6. Install, update and removal

Use one trust path for official and community packages. Fetch gallery packages
from calternal.com/plugins and verify bytes against the approved checksum.
The gallery supports one-tap Owner approval with the permission summary visible.
Offers from git or a non-gallery address show source and a plain warning. Pin the fetched archive, including assets and manifest. An agent may write a
package into its space's checked directory through the host authoring tool. Validate schemas,
digest, metadata, fixtures and bounded readiness without feeding the candidate
space data. The Owner confirms before enable. All members can see the installed
package identity, state, permissions and destinations, without secret values,
personal settings or another space's diagnostics.

An awake update validates a pinned candidate with a data-free readiness probe
outside the active worker. Review widened permissions and changed destinations.
At handoff, fence old admissions, drain to deadline and stop the old worker
before publishing the new revision and approval generation. The space stays
awake. Narrowing or revocation fences affected effects immediately. A failed candidate leaves the last good
revision available. Parked jobs carry the old revision. Resume them only
under a reviewed retained compatible revision or stop with a visible reason.
Never switch code at a checkpoint silently. Version package data through the
shared migration runner. Run a reversible or restorable migration under the
space write fence before publication. Store package metadata outside auth.db.
See [the upgrade contract](upgrades.md) for restore and downgrade rules.

On disable/remove, invalidate tickets, suppress triggers, fence work, stop
the worker with teardown proof and revoke sealed/media delivery. Keep user
items and their authority under host lifecycle rules, with a host fallback for
existing shares. Recommend retaining private data for reviewed same-ID
reinstall/export and separate confirmed irreversible purge (**Q11**).
A removed package cannot reclaim orphaned data just by taking its display name.

## 7. Conformance and migration

| Test family | Required assertions |
| --- | --- |
| Manifest and approval | Reject unknown fields, unresolved references, duplicate IDs, invalid schemas/roles and unsafe paths. Review view exactly covers requested authority. An Owner action is needed before enable, widening or changed destination. Invalid candidate leaves the old revision available. |
| Space and tickets | Run two spaces with the same package and colliding local IDs. Forge space, process, epoch, ticket, callback ID and target. Replay frames after restart/restore. Assert no effects, opaque foreign responses and no out-of-scope data. Demote a member while warm and between effect and delivery. Prime an Owner-only value in memory and storage, then try to publish, return or send it as a Reader. |
| Guest and projections | Use one item ticket to try a second item, a list, settings, secret, share administration and out-of-scope turn. Test View/Comment/Prompt and Owner approval routing, quota attribution and authorized copy. Revoke a share with an open room and a cached description. A public miss or stale link never wakes. |
| Egress and secrets | Refuse undeclared destinations, disallowed DNS answer/redirect, expired approval, raw sockets and a rotated handle. Verify outbound anonymity and signed, authorization-bound media, including repeat refusal and foreign frame access. Exhaust queue/byte/host budgets. Verify audit redaction. |
| Lifecycle and durable work | Count wake signals for logic/job/schedule/verified webhook and zero for reads, cache misses and invalid requests. Race wakes, drain and cancel. Crash before/after commit, during wake and after checkpoint. Race answer/deadline/webhook/cancel. Find one winner and one receipt-backed effect. Restore repeatedly and compare epochs and random identifiers, and reject cross-space snapshots. |
| Descriptions | Verify native/web contract fixtures, unsupported versions/fallback, local state with zero calls, stale action rejection, atomic publication, cache invalidation and exact-base patch or reload. No renderer accepts a protected-surface declaration. |
| Capacity, removal and authoring | Test bounded admission at space and package limits, failed teardown reservations, update with parked jobs, removal with live shares and backup/restore of plugin records. Run the public-guide-only trial with the SDK, installed package, wait and anonymous media checks, including repeated verification. |

Preserve the existing broker engine implementations, typed validation, live
room/auth revision fences, receipts/journal, durable job and wait machinery,
installed diagnostics, package tooling and trial kit. Replace worker placement
and warm session ownership, the outer broker framing and invocation authority,
and v1 renderer publication with the versioned contracts above. Adapt the
container worker launcher for the no-KVM space wall. Keep the v1 wire fixtures
as regression evidence until a replacement test matrix covers their guarantees.
Pin and migrate approved packages through the reviewed catalog. No implicit
v1 manifest acceptance by a v2 parser (**Q12**). Keep the operation catalogue,
independent authorization matrix and conformance inventory aligned.

## 8. Shortest build order

| Step | Deliver and prove | Unblocks |
| --- | --- | --- |
| 1. Freeze a thin contract | Finalize manifest, wire and scope semantics. Adapt the existing broker and jobs onto a space-channel fixture using the container wall. Generate schemas and SDK types from shared models. Test malformed inputs, independent role/ticket matrix, revocation, scoped state/output, bounded calls, replay and waits. Start wall measurements in parallel. | Clients and SDK authors can code against one stable contract. |
| 2. Prove asleep reads and real placement | Publish and cache chrome/content, connect item/list/search/room/share engines and feeds, and prove protocol against an actual space supervisor. Test lifecycle/races, guest projections, destination and secret refusals, update/remove and upgrade recovery. Use whole publication first and gate hosted snapshot enable on measurements. Exercise the existing browser kit with only the public guide. | End-to-end preview with the minimum Notes surface and a validated runtime. |
| 3. Move Notes first | Require proven item/revision reads and durable writes, index/Trash/collections and sharing parity, the ported block editor plus Yjs room persistence and eviction, offline draft/recovery behavior, cached descriptions, approvals and rollback. Compare with the shipped Notes flows and run the real medium-model author trial. | Reuse the validated path for Threads, then Mail. Defer native editor work, optional patch optimizations and new outbound profiles until the contract and Notes parity gate pass. |

## Open questions

- **Q1 PROVISIONAL — Manifest and gallery** — Start with the draft schema, current PNG listing limits and a gallery-reviewed age band (all/13+/16+/18+ or unrated). Gate visibility policy separately.
- **Q2 PROVISIONAL — Private DB** — Start with a brokered record store, 16 MiB per plugin and 64 MiB per space including sidecars, 64 changes and 16 KiB per value. Leave automatic expiry opt-in, with a maximum 30-day TTL for expiring records.
- **Q3 PROVISIONAL — Runtime budgets** — Start fixture idle/drain/wake deadlines at 60/10/10 seconds with 3-second interactive calls. Use measured plan ceilings, bounded backoff and per-package fairness for production.
- **Q4 PROVISIONAL — Transport** — Use bounded NDJSON multiplexing on the enrolled link and the proposed message and error envelopes. Confirm strict version negotiation before SDK freeze.
- **Q5 PROVISIONAL — Delegation** — Use live 60-second renewable tickets bound to durable principal references. Reconcile the older jobs record that permits sign-out with current installation-session fencing, and require explicit enrollment delegation for standalone inbound routes.
- **Q6 PROVISIONAL — Host surface** — Freeze the listed adapters and proposed bounds as fixtures, prioritize Notes methods, and decide hook, settings-write, large-value and patch extensions behind explicit negotiation.
- **Q7 PROVISIONAL — Retention** — Start audit at 10,000 records per space or seven days, whichever comes first, and preserve bounded recent-error/stderr views under their existing authority.
- **Q8 PROVISIONAL — Egress** — Review exact-host policies and any intentionally broad public envelope separately, show outbound disclosure at install, and start media at Ask and private routes disabled until approved.
- **Q9 PROVISIONAL — Descriptions** — Adopt whole publication first, 256 KiB per description, a 16 MiB cache per package/space and typed patches later. Decide renderer support windows and offline eviction under revocation.
- **Q10 PROVISIONAL — Copies and notifications** — Copy the selected live item and permitted attachments, excluding credentials, grants, old approvals and private run state. Confirm which minimal feed summaries can notify.
- **Q11 PROVISIONAL — Removal** — Retain and charge private data for 30 days, support export/reinstall and offer an Owner-confirmed purge. Confirm parked-job downgrade and existing shares' fallback behavior.
- **Q12 PROVISIONAL — Rollout** — Run an explicit v1-to-v2 package conversion and release-pair gate before swapping an existing app. Block incompatible upgrades pending approval and recovery proof.
- **Q13 Confidentiality within a space** — Host-filtered bindings are the default. Showing free-form results to someone other than the ticket’s acting person is an exception gated on isolation proof.
