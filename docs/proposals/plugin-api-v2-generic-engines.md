# Plugin API v2 generic engines

Proposal for sequential review. The host owns cold reads, security and
durability. Official and community packages compose the same engines.
The changes below require a numbered revision of the sealed contract.
[Owner decisions][owners] · [Sealed plan][sealed] · [Step 0 measurements][measurements]

## A. Execution without a server Turns engine

### A.1 Admission, Send and lifecycle

The host keeps the sealed composer preflight, including attachments, quotes,
destination and execution material. The final-text receipt covers the exact
text and prepared inputs. Text parsed as a change to recipients, routing or
permissions needs a fresh preview. The package owns the send policy and
declares a bounded job intent. The host commits the message and intent with
the expected live head, reserves quota, and admits by receipt under the same
lifecycle fence. A scoped exclusive key lets the orchestrator serialize runs
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
operations and bounded effects. Even Full Access cannot list, search, count,
enumerate or bulk-read this space. Agent principals cannot enqueue or delegate
work that obtains discovery rights. Only independently admitted plugin jobs
can acquire indexer capabilities. A scratch disk or provider profile is never
a route to userdata. [Current Turns access][access]

Execution leases are separate from sealed 60-second invocation tickets.
Renew only against the live session or scheduled principal, grants, package
generation, eligible placement and exact attempt. Recheck authority at broker
effects and output egress. Revocation fences both immediately and stops
renewal. Use a machine-local monotonic deadline, provisionally at most 5 s
after the last authenticated renewal, that kills the container and its tools
even when control is stalled or the link partitions. Discard only under the
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
An unqualified profile, harness or isolation combination remains disabled.
[Manual turns][turns] · [Provider protocol][provider] ·
[Claude Code][claude] · [Codex][codex]

### A.3 Durable streams and same-execution reattach

Persist machine, boot, execution and attempt identifiers and the fence before
dispatch. Replace the in-memory ACK and unknown-exec adoption in
`machines/sessions.rs::output`. ACK only a contiguous private spool position
after durable commit. Accept restart reattachment only with fresh authenticated
proof of the same machine boot, running container, execution, attempt and
current fence, plus continuity from the committed position. Otherwise fence
the old attempt, preserve the committed transcript, and require explicit
Resume by a currently authorized person. A saved provider session is useful
for Resume and provides no live-execution proof. Reconcile an uncertain
external effect before retry. The package can use durable timers to retry
a provider failure whose effect is known safe. [Machine output path][output]

The package normalizes private input into a common bounded display envelope.
Commit public events, stable sequence IDs, the receipt and the consumed-input
cursor together. Cursor-only commits cover input with no display event.
Reclaim private spool pages through that cursor. Bind opaque frames, native
session IDs and checkpoints to the private attempt. Exclude raw data from
display routes, shared logs, search, exports and ordinary backups. The
qualified display path validates quote and attachment refs, accepts only
designated display fields and scrubs issued credentials across chunks before
publication. A schema cannot prove arbitrary text safe. Keep adapters without
qualified display separation private.

Cold page, follow and resume check the current audience at egress and release
the reader before waiting on transport. Measure first with a 16 MiB or 4,096
record private cap per attempt and 128 MiB per space. Apply credit backpressure and stop as interrupted after 30 s of stalled input or drain. Retire
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
| Turns phases, retry, tool steps, instructions and thread presentation | The orchestrator owns provider/model option policy, title and instruction assembly, message/Markdown decoding, tool-step presentation and thread automation inputs. Use declared inputs and committed projections. |
| Notes tags, links and live saves | Notes owns frontmatter tag extraction and link-rewrite policy. The host keeps room concurrency, save and lifecycle/Undo commits and checked index publication. Cold views use committed projections. |
| Turns access, redact, verify and readiness | Host keeps principal and effect enforcement, placement verification and confidential custody. Adapters own provider-specific probes, login behavior and readiness interpretation. |

First qualify the shared engines with both shipped adapters and a minimal
independent runner. Integrate Notes on the generic item and derived-file
surfaces first. Then stop old Turns admissions, drain or fence active
attempts, and convert thread heads, stream positions, pending intents and
receipts once. Preserve native session bindings by account, provider, thread,
machine and eligible profile, plus preserved workspaces and their quotas.
Verify converted heads, grants, archive/Undo, exact-lease discard and recovery
against the original data before switching the single writer. Remove old
Turns routes in that cutover. Exercise the release pair with an attempted
downgrade after new commits. Refuse downgrade unless the old release reads
those commits exactly once. A restore point can help before cutover.
[Sealed §8][sealed]

## B. Derived files and private scratch

The host serves lists, counts, search and sharing from authorized items while
the space sleeps. A manifest may opt into cold empty-item creation from
declared defaults. Further initialization consumes its committed change.
Private per-package scratch replaces the proposed first-release host table
store.

### B.1 Device snapshots and cold previews

Declare the output name, kind, producer, media type and quota. Bind the
device upload grant to the authenticated save session, editor, save receipt,
exact committed revision and output slot. Capture asynchronously from the
editor state corresponding to the saved revision. If the local state has
advanced or a merge yields a different revision, skip the upload and use
the fallback. After save ACK, stage bounded still-image bytes, validate
type and dimensions, re-encode, strip metadata and make the file durable.
Under the publication fence, recheck the upload grant, live edit authority
and head. Commit the pointer and attribution with a one-use receipt. The
same digest can replay that receipt under current authority. Different bytes
or a different session are refused, even for another editor.

An editor can upload a misleading picture of a note they may edit. The
host verifies attribution and revision binding. The editor supplies the
pixels. Confine the image to that note and revision, attribute it to the
uploader, and supersede it on the next save. The next editor's upload
replaces it. Host or plugin rendering can qualify later.

Extend `RowTemplate` in the numbered revision with an authorized
revision-bound leading image and a bound second line. Notes uses a portrait
preview and a second line with date and snippet. The manifest declares a
first-block projection over the committed structured content, initially
three typed blocks and 256 bytes of text total. The generic save/index
path computes it in the same revision for room, device, agent and plugin
saves. Cold create supplies
the declared empty summary. The renderer uses the compact block summary for
a miniature if the image is missing, stale, evicted or pending. An empty
note or an older item without that projection gets a blank miniature.
No client or worker reconstructs this by fetching an entire note at row read.

Resolve an opaque image ref against current audience, head and all dependency
grants and versions before sending bytes or returning a cache revalidation.
A stale or revoked ref cannot yield a `304`. Use session-only guest/public
media, with no persistent cache. Member caching follows sealed §4.2 and
must drop revoked media at reconnect. Read and miss paths wake zero VMs.
[Description contract][description] · [Sealed §§4.2 and 5][sealed]

### B.2 Other derived files and storage bounds

Brokered jobs can produce files such as photo thumbnails. The existing
change outbox drives a durable subscriber by receipt. Coalesce work to the
latest desired revision and recheck dependencies and producer rights at
publication. Save ACK waits for neither job delivery nor asset work.
Keep pointers, indexes and bounded hot previews in SSD appdata, with bulk
media under the host-managed HDD tier. Support the decided one-disk
installation. Back up canonical files and items under their ordinary policy.
Treat missing staged or evicted derivatives as misses and reclaim in bounded
batches. Purge and revoke deny delivery before cleanup.

For qualification, cap a device upload at 512 KiB encoded and 256 × 384
decoded pixels. Allow at most two active decodes, 64 pending grants and 8 MiB
aggregate staging per space. Reserve before ingest, expire grants and
abandoned staging within five minutes, and refuse new staging at the cap.
Other asset types also require finite declared and host-capped resource
limits. Cross a crash cut only after durable bytes and a fenced pointer
commit. Eviction switches off the pointer before reclaiming bytes.

### B.3 Private scratch isolation and custody

Provision one size-capped appdata disk per package and space. Declared
background jobs in either plugin VM can use it. Bind the exclusive mount
lease to space UUID, installed package and producer generation, source scope
and exact job attempt. Run each lease in a dedicated OS identity and mount
namespace with no shared memory or handles with interactive workers. On
handoff or revoke, fence the lease, terminate the old process and detach
before reuse. After a crash or restore, prove the old attachment fenced.
Scratch and scratch-job memory stay outside both backup and VM snapshots.
Agent VMs and personally enrolled machines have no scratch mount. Guests or
item-share activations cannot invoke or indirectly enqueue scratch work.

Enforce the size quota at the volume, including auxiliary files. Fence jobs
and wipe the disk on uninstall. Disk exhaustion stops scratch work without
blocking canonical writes. Rebuild only from brokered files and committed
items. Originals, user edits and durable job checkpoints stay in ordinary
host custody. A package must recover when conformance deletes its disk.
Reuse a generation only under the same live source scope and rebuild after
a grant or package permission shrinks. Renderers, item shares, guests,
other packages and agents have no disk, row or query access. They receive
only ordinary authorized item fields and derived files.

### B.4 Publication audiences, machine consent and similarity

Publish source-local fields by default. A photo's recognized labels go onto
that photo and are filtered by ordinary Binds. Multi-item outputs require
host-checked source refs, expected revisions and a destination audience
contained in every input audience. Apply that check to identifiers, scores,
order, counts and even the existence of a result. Dependency change or
revocation fences stale joins, indexes, versions and counts before delivery.
For first release, publish viewer-specific similarity results only in the
person's space with dependency-checked refs. Execute “photos like this” as
a background ML job in the heavy plugin VM. Scope candidate matching to the
viewer's authorized set before ranking. Bound the stored result refs and
recheck their revisions and grants at delivery. Refuse the stale result
as a whole if an input fence has changed. Sealed installed-package trust still
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
bounded outbox range and receipt. The receipt fingerprints normalized
input and records per-item committed, stale or refused outcomes. An exact
replay rechecks delivery authority and returns those outcomes without writes.
Recovery idempotently publishes the committed range, retires the outbox and
reclaims unreferenced staging. An output-only patch never requeues its
producer for the same source revision.

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

## C. Owner question and qualification

**Who should see a photo similarity result?** A result can tell someone
which other photos exist. For example, a member looking for photos of a
trip might find a picture that other members cannot open. A personal result
lets that member revisit their matches and costs a private result item per
run. A shared result would
let several members curate matches from the same collection, and costs
dependency-indexed invalidation and audience checks for every source and
reader. This determines which publication surface the first contract
needs. Recommend personal results for the first release. The proposed
spike uses that scope.

Use sealed §8's machines, repetition and instrumentation. Numeric additions
below and in §§A–B are proposed spike limits.

| Spike | Pass condition that can falsify the proposal |
| --- | --- |
| Agent recovery and live revoke | Cut before private spool commit, after commit before machine ACK, and after ACK before public append. Restart, stall the reader and partition the machine with control blocked. An exact still-running attempt resumes its committed sequence once. Missing proof or range requires explicit Resume. Zero broker effects or output deliveries after fencing, and the process and tools stop within 5 s of the last renewal in every run. Verify the spool bounds and terminal outcome. Keep the sealed 150 ms first-screen and 500 ms independent-control p95 bounds. |
| Agent isolation, approvals and cutover | Use both shipped adapters with personal and shared profiles, plus an independent runner. Forge refs, delegate discovery, attempt credential access and network egress, and replay native approval across attempts. Zero unauthorized effects or bytes. Race decision against revoke and exit, with at most one eligible decision. Exercise Send/Stop/Archive/replay and downgrade after new writes. Preserve exact receipts, workspaces and committed state or refuse downgrade. |
| Device snapshot and derived media | On a sleeping space, measure save intent to durable ACK p95 ≤40 ms and ACK to current painted preview p95 ≤2 s. Race unsynced local edits, room and agent changes, revoke and a copied grant in another editor's session. Try a changed-digest replay and invalid/oversized images. Cut before file durability, before/after pointer commit and during eviction. Enforce staging and cleanup caps, assert zero wakes on the preview path, zero stale/unauthorized bytes including through cache revalidation, correct attribution and clean authorized fallback. |
| Scratch deletion and isolation | Delete scratch at rest and mid-job, fill it, and cut small-to-heavy handoff. Restore from a VM snapshot and try a guest activation after a warm privileged job. Assert one mount, no stale process data or scratch work from the guest, no cross-scope access, no scratch in backup/snapshot, deletion on uninstall, and exact reconstruction without canonical loss. |
| 10,000-photo labels and similarity | Index mixed-visibility originals while concurrent writers drive taps, saves and reads. Compare labels and personal similarity IDs, order and counts to an independent authorized oracle. Changes to hidden sources cannot affect those results. Withdraw either consent mid-delivery on the personal-machine arm and verify zero further bytes or commits. Sustain the write limit and cut through batch commit, journal and outbox retirement. Require exact recovery, the sealed foreground gates, bounded staging and p95 publication lag ≤2 s under steady load with oldest unpublished age ≤5 s. Record reads, FULL commits, actual flushes, feed deliveries, search lag and per-arm queue depth. |

### C.1 Numbered contract revision

Revise the manifest, description schema, catalogue, verifier and fixtures
together for three runtime roles, shared execution/stream engines, protected
composer dispatch, generic cold create, scratch, derived images and row
fields, and bulk item publication. Replace Turns-specific dispatch and the
first-release table store. Carry forward all-field search by default with
an optional field filter. Qualify every new route against the sealed
authority matrix before enabling it. Qualify execution, approvals and
streams with both adapters and the independent runner.
[Sealed plan][sealed] · [Combined-search measurement][combined-search]

## Simplification opportunities

Reuse the protected Inbox, item lifecycle fence and change cursor. Keep
ordinary labels source-local and keep similarity in personal space.

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
| 11. Cold miniature and media | §§B.1, C.1 row schema and egress |
| 12. Scratch isolation | §§B.3, C exclusive, isolated leases |
| 13. Publication audiences | §§B.4, C source checks and owner question |
| 14. Batch feasibility and flood | §§B.5, C transaction, caps and cuts |
| 15. Machine consent | §§B.4, C per-delivery and publication fences |
| 16. Bounds and qualification | §§B.2, C resource caps and observable gates |
| 17. Contract revision | §C.1 coordinated schema and conformance changes |

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
