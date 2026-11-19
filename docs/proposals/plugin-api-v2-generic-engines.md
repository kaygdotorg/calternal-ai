# Plugin API v2 generic engines

Proposal for sequential review. The host owns cold reads, security and
durability. Official and community packages compose the same engines.
The first release qualifies Notes and the orchestrator as ordinary packages.
Photos-scale engines are follow-up design. The changes require a numbered
revision of the sealed contract.
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
An unresolved intent blocks sheet-free archive. Direct Stop, pending-intent
cancellation and archive-triggered Stop require the current starter's
authority or live `PluginsManage`, alongside the relevant item right.
Confirmation pins the exact intent or attempt. Recheck both authorities
under the lifecycle fence. An attempt replacement requires a new
confirmation. Confirmed archive durably refuses the pinned pending intent,
including on replay after unarchive. Generic snooze and Undo are actor-keyed
and cannot change execution retry timers. [Current Stop check][stop]
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
effects and output egress. Revoke fences both immediately and stops renewal.

The machine supervisor contains every tool and harness process, including
descendants that start a new session. The placement runtime directly owns
the cgroup and gateway fences and one pair of attempt deadlines on its
local monotonic clock. It accepts host renewal only with a fresh pulse
from the current supervisor generation. An optional watchdog can report
faults. The runtime's clocks, fencing and teardown do not depend on it.

| Deadline | Runtime work |
| --- | --- |
| Freeze cutoff | Earlier of last supervisor heartbeat +2 s and last authenticated host renewal +5 s. Reserve the last 250 ms for gateway closure and cgroup freeze. Fence queued dispatch and egress and verify that the full subtree is frozen by the cutoff. |
| Verified teardown cutoff | Last authenticated renewal +10 min. Reserve the last 5 s to kill the subtree, close the gateway and verify both are gone. Authenticated exact-attempt Stop starts teardown immediately and moves the deadline to the earlier of this cutoff and Stop receipt +5 s. |

Runtime timers are armed while the attempt is admitted. Once fencing
starts, a late heartbeat or stale renewal cannot reopen the gate. Only the
runtime may thaw after safe provider continuation is verified and the
host issues a fresh one-use renewal for the exact attempt and current
runtime fence. Thaw is unavailable once teardown starts. Discard queued
requests that never dispatched. Reconcile uncertain dispatched effects.
Hold quota until verified teardown. If verification misses its deadline,
keep the placement fenced and quota charged. Direct and unproven
placements remain disabled. [Current freezer][podman] · [Direct code][direct]

A frozen attempt holds memory, capacity, workspace and locks. Keep the
reservations until verified death or continued ownership after thaw. Keep
live credential-bearing state outside snapshots. Verify that the provider
transport can continue the same request without replaying an uncertain
effect. A remote timeout with uncertain effects means interruption and
reconciliation before explicit Resume. Discard only under the exact
workspace lease. Retain explicitly preserved workspaces under quota.

Bind provider profiles to their authorized account or space and pinned
contribution. The supervisor-owned transport redeems an opaque handle
and mediates the approved destination. Adapter and tool identities get no
credentials, provider-home mount or readable credential channel. Machine-local
profiles stay with the trusted transport. Conformance tests containment and
protocol behavior, and cannot make a credential readable by untrusted code
safe. Disable any profile/harness pair that requires such access, including
existing adapters until their path is qualified. Enforce `network: "none"`
at both sandbox and gateway. A provider call needs a declared route.

Any package may qualify a digest for the same isolation/profile suite.
A labelled unqualified harness is eligible only on its owner's enrolled
machine with a personal profile and the same supervisor/credential boundary.
It receives no qualified host display or approval route. Other combinations
remain disabled. [Manual turns][turns] · [Provider protocol][provider] ·
[Claude Code][claude] · [Codex][codex]

`execution_input` binds bounded, receipt-checked steer input to the exact
live attempt. Under the tree reservation fence, `execution_spawn_child`
checks the exact current parent attempt, the requesting actor's live
spawn authority for that parent and the target kind and placement.
Atomically reserve process, memory and queued-work capacity across the
tree by receipt. Propose depth three and eight attempts including pending.
Grant the intersection of the request, the caller's live authority and
every ancestor's admitted item refs and effect ceilings. Recheck the live
caller and ancestor grants at broker effects and renewal. Fence the
subtree on ancestor revocation and release quota after verified cleanup.
The host commits the parent edge for any conversation kind under admission.

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

The adapter in the agent container emits a display envelope based on
`provider-events-v1`. Persist only inert assistant text and bounded tool
summaries and checked attachment and link refs eligible for the entire
conversation transcript audience. The host stamps the role, attribution
and record IDs. Render escaped text or eligible safe Markdown. No raw
HTML, description, action, trusted notice or renderer state comes from
the adapter. Provider question, approval, session, usage and done frames
are private protocol input. Mint lifecycle transitions and protected
controls only from supervisor evidence or checked broker/Inbox records.

Use one audience for all persisted transcript records. Exclude reasoning
or any other material that needs a narrower policy from the file and
public projection. Keep Inbox controls and private provider data outside
the transcript. Scrub before writing the canonical Markdown file
(interim, Q3). The content journal makes the committed file boundary,
receipt and consumed-input checkpoint recoverable. The machine ingest
ACK follows contiguous spool persistence. The distinct publication ACK
follows the durable file boundary and receipt. Keep the bounded
parser/scrubber carry with the consumed checkpoint before reclaiming any
prefix, including on cursor-only commits. Private frames, carry, native
sessions and checkpoints stay out of display, exports, search, shared logs
and ordinary backups. [Provider event schema][provider-events]

Choose record-level cursors so canonical Markdown stays plain and
old-readable. Use the stable UUID and order of its framed message records.
A cursor is scoped to the conversation and live viewer authority. Reopen
and refresh the cursor's boundary record on reconnect, including an active
record that may have grown, then continue in record order. The renderer
replaces the boundary row by UUID. In the same authorized reconnect
response, send a fresh copy of the bounded host-owned sign-in notice
prefix, even if the cursor is past it. The renderer replaces the prefix
as a unit. Use the existing 16-notice cap within the existing page byte
cap. Check the same transcript authority before including the prefix. [Current notice writer][notices]

A long record can supply a clipped preview and a checked revision-pinned
range ref for its own body. Coalesced token frame IDs and their boundaries
are transient. Page and search projections rebuild from file records.
Revise sealed §4.2 and the description contract’s transcript read route
and catalogue cursor, plus the §8 “Durable transcript” gate, to require
stable record IDs, order, cursors and exactly-once durable receipts across
replay. Token-event sequence reconstruction is outside this contract. Keep the latency and revoked-reconnect gates.
[Sealed plan][sealed]

An `execution` wait is bound to the owning job, package and attempt.
Park atomically against the observed durable transition cursor. In the
same transaction, deliver by receipt an already committed later
transition (including terminal state) or install the future wait.
Only bounded host-owned phase, checked request and terminal transitions
can signal it. Tokens and ordinary progress never wake the plugin VM.

Cold page, follow and resume recheck authority at egress and release
the DB reader before waiting. For the spike, cap frames at 64 KiB, the
unconsumed spool at 16 MiB or 4,096 records per attempt and 128 MiB per
space. Meter sustained input at 256 KiB/s and 64 frames/s per attempt,
1 MiB/s and 256 frames/s per space, and coalesce publication to at most
four FULL commits/s per attempt and eight per space. Reserve foreground
writer capacity. Ingest limits bound private spool traffic. Measure
durable transcript publication separately with ordinary assistant text
and a sustained canonical-byte rate. Apply credits and interrupt after 30 s of stalled drain
on a live link. Frozen attempts use their separate recovery deadline.
Persist the retirement deadline when the attempt terminates or its
recovery fence expires. Bar reuse then and start reclaim no later than
ten minutes afterward. Sweep expired spool before new admission on
restart, in steps of at most 4 MiB and 256 records. Finish expired cleanup
within 30 s of the deadline, or within 30 s of restart if the deadline
passed offline. Release quota
only after verified cleanup. Cleanup preserves committed transcript
records. Bound display retention by item policy. [Transcript spike][measurements]

### A.4 Protected requests

Use the host Inbox. A live eligible person resolves a checked request bound
to its actor, current attempt, target, expiry and verified action digest.
Recheck these at decision and when the tool gateway consumes the one-use
grant and dispatches the exact approved operation and canonical arguments
under the attempt fence. Persist the grant consumption before attempting
the external effect. If the dispatch outcome is unknown after a crash,
reconcile before any retry. A native reply or waking the job confers no
dispatch authority. A durable broker action can wake one parked job by
receipt. A native request is tied to the exact
live process and native request ID. Exit, replacement or revocation invalidates
it. Codex's current `can_approve: false` requests and Claude denial summaries
allow dismissal only. An actionable approval requires executable material the
host and gateway can verify and enforce. Keep native integrations that
cannot intercept the actual action dismissal-only. Guest Prompt stays
disabled pending the sealed proof.
[Provider protocol][provider] · [Sealed §5][sealed]

### A.5 Ownership and cutover

| Current paths | Transfer |
| --- | --- |
| Turns admission, launch, lease, control and progress | Host retains lifecycle and receipt fences, admission, stop, cleanup, paging, egress authority and durable machine/stream transport. Packages own prompt construction, adapter calls, phase labels and retry policy. |
| Turns phases, retry, tool steps, instructions and thread presentation | The orchestrator owns provider/model option policy, title and instruction assembly, tool-step presentation and automation inputs. The host retains the common conversation file format and its cold decoder under Q3. Use declared inputs and committed projections. |
| Notes tags, links and live saves | Notes declares extractors and owns link-rewrite policy. The host runs the generic extractors and keeps room concurrency, save and lifecycle/Undo commits and checked index publication. |
| Turns access, redact, verify and readiness | Host keeps principal and effect enforcement, placement verification and confidential custody. Adapters own provider-specific probes, login behavior and readiness interpretation. |
| `machines/execs.rs` shared adapter choice and thread sign-in notices | Replace hardcoded launch paths with digest-checked entry points and bounded notice labels from contributions. The host stamps the actual verified sign-in outcome and actor/time. Placement eligibility, private login custody and cold decoding require no package callback. |

First qualify both adapters and the independent runner. Integrate Notes.
For Threads, stop new old-engine admissions and wait for zero active
attempts. Preserve workspace custody, grants, pending intents and receipts.
Under the fenced writer, parse legacy `provider-session` comments and
metadata. Migrate into protected bindings keyed by account, provider,
conversation, machine and profile only when an authenticated historical
record ties that exact session to the profile. The profile currently
signed into the machine is insufficient. If historical proof is missing,
retire native Resume for that binding. Keep the thread readable and show
the authorized person who attempts Resume: “Previous session unavailable.
Start a new session.” A fresh execution needs current approval and checked context.
Remove legacy file/metadata copies in either case by an idempotent
migration. [Legacy session binding][bindings]
Sweep the file, metadata, history and journal for legacy private copies.
Keep public revision history available after sanitizing its private
fields. Quarantine unsanitized copies and pre-existing backups under
protected retention and expiry. Exclude them from ordinary backups,
exports and restore inputs. Block cutover until a new ordinary backup
can restore the complete public history without those private copies. Thread
message bytes, their stable IDs and public rendering must remain equal.
Raw whole-file hashes can change only for verified removal of private
records. Only proven protected bindings allow authorized Resume.

Keep the current length-framed Markdown message format: valid role,
UUID, heading, UTF-8 byte count and existing agent provenance fields.
Coalesce display deltas into the active record. Store supported tool
summaries as inert Markdown in that record. New private or control event
types never enter canonical files. Ship no new frame type in this
release. Rebuild with all projections deleted and diff every old/new
render, including text, order and checked attachments. Exercise post-cutover
writes and rollback against a release that understands this representation
and protected bindings. Verify it before switching writers, or abort the
cutover. No fallback restores a pre-switch file over new commits.
[Current record decoder][records] · [Legacy session encoding][sessions] · [Sealed §8][sealed]

### A.6 Conversation capability and sealed replacements

The orchestrator declares `threads` as its own package kind with
`conversation: true` (interim, Q6). Any kind may opt in. The capability
gets a durable transcript, the message corpus and an authority-checked
`active_execution` ref. Inbox decisions project `waiting_on` under
`viewer_self`. The item lifecycle supplies archived state and actor-scoped
snooze, plus generic parent edges with scoped child windows. These are
host engines any qualifying kind can use. Opt-in grants eligibility only.
Require package `reads`, the owner-declared corpus, exact field policy and
live viewer authority at transcript, search, quote, count and egress.
A title-only Bind ref cannot open a message corpus. Derive `waiting_on`
only from live Inbox requests addressed to and readable by this viewer.
Another actor's request cannot change the viewer's rows, count or version.

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

A single host-run extractor reads only manifest-declared fields or body
regions of the same committed item. Every output, including filtered
counts, is visible to at most the intersection of its inputs' audiences.
Use closed selectors for typed frontmatter keys, first content blocks and
inline Markdown tags. No regular expressions, callbacks or ref traversal.
Scan the checked body at most once, within the qualified 2 MiB input cap.
Propose at most eight selectors, three summary blocks excluding the title,
and 256 summary text bytes. Use empty declared values on cold create.
Room, device, agent and plugin saves publish these fields with the body
commit. No VM wake is needed.

Notes declares the shipped frontmatter and inline tag grammar, including
hierarchical parent expansion and exclusions for fenced/inline code, links,
addresses and escapes. Reuse the bounded existing scanner as a generic
Markdown selector. Notes owns tag rename and link-rewrite policy. It may
run bounded jobs using ordinary item writes. Require parity against the
shipped scanner before cutover. [Current tag grammar][tags]

Extend `RowTemplate` and the sealed §2.4 row rule with a bound portrait
image and second line (date and snippet for Notes). If there is no
authorized image, use the latest authorized summary for a miniature.
An old item without that projection or an empty note uses a blank
miniature. In revision 13.2 only permitted non-body display fields may
enter `MemberRowCacheRecord`. Body snippets and block summaries arrive
after fresh authorization. Restoring them offline is an owner option in
§C. Read and miss paths wake zero VMs. [Description contract][description] ·
[Revision 13.2 member cache][rev132]

### B.2 Derived preview custody

Serve a checked derived ref with no runtime wake. Keep pointers, indexes
and bounded hot previews in SSD appdata. The host-managed HDD media tier
holds bulk files when that feature qualifies. A one-disk installation can
use one filesystem. Canonical files keep their existing backup policy.
Orphan or evicted derived bytes are a miss. Revoke and purge fence delivery
before cleanup. Reuse the content outbox to coalesce derived work against
source revision when a later producer needs a post-save job.

For the Notes slot, cap encoded uploads at 512 KiB and decoded images at
256 × 384. Reserve space and check authority before ingest, with two
concurrent decodes, 64 pending grants and 8 MiB staging per space. Grant
and staging expiry is five minutes. Retire expired reservations and delete
their orphan staging within another 30 s, including a sweep before new
admission after restart. Recheck the upload session, expiry and item grant
before accepting bytes and before publishing the pointer. Make bytes
durable before pointer publication. A crash in between leaves the
previous authorized preview or fallback visible. Retire the durable
orphan and its reservation by the staging cleanup deadline. Eviction
withdraws the pointer before reclaiming bytes.

### B.3 Private scratch design

Owner-approved scratch remains private and disposable. Notes and the
orchestrator need no mount at first release. Enable the disk with its first
heavy consumer after qualification. Provision one size-capped appdata
volume per space and package, attached only to the heavy plugin VM. The
exclusive lease binds the package and producer generation, source scope
and attempt to a dedicated OS identity and mount namespace. Share no
handles or memory with interactive workers. Fence and kill the old worker
before detach or reuse, including recovery. Exclude the disk and worker
memory from VM snapshots and backup. Wipe it on uninstall. Guests and
item shares cannot access it or start scratch work indirectly. No person,
renderer, other package, agent VM or personal machine can query the disk.
Visible fields live on ordinary host-authorized items.

Rebuild from canonical files and items. Treat loss and exhaustion as a job
failure that leaves canonical writes available. Under the sealed interim
principal (Q2), a creator's lost authority stops that job. A Manager can
take over its schedule and start fresh work under their own rights. They
cannot take over its job or reuse a scratch generation out of scope.

### B.4 Photos-scale follow-up

Defer the new file upload and stream routes, bulk metadata and pointer
transactions, private embeddings and similarity status until Photos or
another real consumer. Deploy only the qualified Notes preview slot in
this release. The later design must preserve the following boundaries.

| Surface | Follow-up requirement |
| --- | --- |
| Originals and thumbnails | Resumable, quota-reserved HDD upload while asleep. Check session, live authority, offset and reserved length on every bounded chunk before storage. Exact offset, length and digest can replay, with a cap on chunk admissions and receipts and timed retirement of expired staging. Publish only durable bytes under an exact head and receipt. Qualify the device thumbnail producer in a fixture only until Q5 is approved. The operative photo slot permits job output only. |
| Broker reads and consent | Bounded whole-file streaming with a revision pin, credits and per-file authorization. Check the live file and creator fence on every delivery, including buffered bytes. Personal-machine assignments require explicit space and machine-owner consent bound to machine, owner, package, purpose and source scope. Recheck both consents at delivery and publication. Cut streams and queued work on withdrawal. Disclose that operators can retain previously delivered originals. |
| Labels and private derivatives | Default to source-local item fields and checked, source-content revisions that stay stable across output-only pointer patches. Private embeddings keyed by source-content revision and model digest can rebuild scratch after live item and producer rechecks. Erase artifacts and fence scratch reuse on source erasure or uninstall. A real batch transaction must commit expected heads, indexed fields, durable pointers, outbox and replay receipt. Admit bounded index work and backlog per space and coalesce only authorized changes. Seal only after a sustained offered-load test accounts for every eligible change and measures source-to-publication age, including deferred work. |
| Similarity | Background heavy-VM ML under the requester's authorized candidate set. Interim Q4 uses a bounded, requester-only status result (up to 50 refs, receipt retention). Store it separately from generic job results, checkpoint, progress and live stream. Use one egress projection for poll and replay, checking requester, expiry and current grants and revisions of all inputs to ranking, including outside the top 50. Invalidate on a relevant input change. No cross-space write and no change visible to foreign status viewers. |

Sandbox package-private artifacts and their model inputs. Pin model weights
by digest and qualify the heavy-VM CPU path. Declared multi-item outputs
need checked source refs and destination audiences contained in every input
audience. The installed-package trust rule still applies to intentional
copying. These requirements do not enable the deferred routes.

The locked perf-01 rusqlite probe completed 30,600 reads, with four streams
delivering 549–558 MB/s in its two corpora. That transfer fixture supports a
later contract test, independently of product authorization, revocation and
Photos processing. @thread:thr_tth3axchm4#msg=1857 · [Runtime measurement issue][bulk-read]

### B.5 Notes tag set write budget

Qualify `TextSet` with typed `contains` for Notes tags. Use normalized,
deduplicated elements and a scoped value/order index. Each tag count is
over distinct authorized rows. Qualify combined predicates separately.
Preserve the sealed 32-effective-scope cap and two keys per scope for
the existing scalar-only plan. In the numbered revision, add an explicit
total index budget for saves and scope mutations. Propose 64 tag elements
per field, 256 UTF-8 bytes per element, and 1,152 total old/new covering,
membership and count keys per foreground transaction across all items,
fields and scopes. Count hierarchical expansions and all affected keys.
Apply this to saves, sharing, membership and Undo, including scope-only
changes. Preflight the effective scopes and total work under the write
fence. Refuse an over-budget scope addition or ordinary mutation with
typed `index_work_limit` before any effect. A mandatory access revoke
always fences the grant first. If synchronous cleanup exceeds the bound,
refuse the affected cached projection until a bounded background pass
reconciles it. Revoked viewers get neither rows nor counts. An affected
still-authorized viewer gets either a fresh exact result or the sealed
loading state with neither rows nor count. Unaffected projections stay
available and correct.
Never truncate a valid legacy tag set.
Block Notes cutover if existing items exceed qualified limits or the
worst-case save misses 40 ms p95. If needed, bring the measured parity
tradeoff to the owner before changing the contract. [Sealed save gate][sealed]

## C. Owner questions and qualification

The defaults are interim. Q2, Q4 and Q5 are for the Photos-scale follow-up.
They do not block Notes and the orchestrator.

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
   index leaves, the creator-bound job stops. The follow-up needs a
   principal before it can define admission and retained artifacts.
   Creator rights limit exposure but interrupt work and can incur
   rebuild I/O. A Manager can take over the schedule and start a new job. Installation rights keep coverage and
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
   qualification: model both producers in a fixture with one slot and
   allow job output in product. The owner must approve the device
   producer before its path is enabled.
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

### C.1 First-release qualification

Use sealed §8's machines, repetitions and instrumentation. New quantities
are proposed limits. Record per-arm authorization reads, commits and actual
flushes. Start with an independent-runner paper exercise.

| Spike | Pass condition that can falsify the proposal |
| --- | --- |
| Generic runner, tool and lifecycle authority | On both shipped adapters and an independent one, drive actual Send, checked request, steer, child, Stop, archive, Undo and snooze through generic routes. Assert the eligible operation effects and read them back, including one tool dispatch for an exact approved digest in a broker-actionable arm. Exercise native dismissal-only integrations separately. A changed digest or consumed grant dispatches zero times. Race two child starts with one slot left and require exactly one admission. Revoke the ancestor while the admitted child acts. Cover unauthorized and replacement-attempt archive, a title-only transcript ref and another actor's Inbox changes. Assert zero unauthorized effects, bytes, count or version changes. |
| Sleeping-space Send and park | Verify zero VM residency before a prepared Send on the sealed link. Require one exchange and p95 ≤500 ms to painted Starting backed by a durable receipt that survives a lost reply and restart. Measure p95 ≤8 s to the real container's authenticated ready handshake for one wake and ≤15 s at 20 concurrent wakes. Require a first provider token and report its time separately. Inject terminal, approval and phase transitions on either side of park, replay each signal, and include a no-transition control. Verify exactly one resumed job per transition and none for the control, with no duplicate execution or plugin wakes for tokens. |
| Transport, supervisor and watchdog loss | Cut the real carrier with a queued provider request and detached tool process. Exercise 30 s and 5 min outages with surviving and timed-out provider transports. Timestamp heartbeat and renewal independently and stage both deadline orders. After demonstrated progress, verify by process and receiver-side probes that the runtime freezes and fences by the earlier cutoff, with the 250 ms fencing budget reserved. Kill and hang the supervisor. Separately kill and hang the watchdog immediately before each reserved work window and again 1 ms before each cutoff. Record runtime arm and completion timestamps. Require the same deadlines in those arms. Verify complete subtree and gateway teardown by the 10-minute cutoff or the earlier Stop cutoff, with quota held until verified cleanup and reusable afterward. Reject stale thaw. An intact attempt must resume on fresh proof with a safe transport. An uncertain effect requires reconciliation and explicit Resume. Require zero post-fence egress and sealed first-screen/control gates. |
| Stream custody and publication | Cut after spool persistence before ingest ACK, after file publication before its ACK and after that ACK before reclaim. Split an issued credential across frames and restart after the checkpoint. Compare exact committed records and receipts against the input oracle, with no credential or forged trusted control. Abandon at quota and cross expiry during restart. Verify cleanup and unchanged canonical records. For 120 s of **ingest** saturation offer twice the space byte/frame caps in 4 KiB frames across four attempts. After warm-up accept ≥90% of both ceilings, account for every offered frame and reconcile accepted work after drain. In a separate 120 s **publication** arm send real assistant text, rotate conversations at 8 MiB or earlier, and require ≥512 KiB/s of new durable canonical text in every 10 s window after warm-up. Read the canonical records independently and count only new correct bytes. Run the foreground load against the same writer throughout publication, with at least 100 samples per foreground operation across measured runs. Require p95 ≤40 ms save, ≤100 ms first cold window, ≤150 ms first transcript screen and ≤500 ms independent control, with bounded spool and commit work throughout. |
| File cutover, notice and rollback | At zero live attempts migrate both proven and unprovable bindings. Delete projections and rebuild. Verify exact historical message bodies, record IDs/order and render equality. The proven binding must Resume under its original profile. The unproven one shows the fresh-session notice and never attempts native Resume. Write new records and rebuild on the rollback target with projections removed. Disconnect after the attempted notice. Complete sign-in, then reconnect from the last record both with an unchanged tail and, separately, after it grows. The existing transcript reply must replace the attempted notice with exactly one completed notice in both arms and refresh the grown record in the second arm. Require no revoked rows, bookmarks or notices. Inspect canonical files and ordinary backups for private state and confirm exactly-once records and receipts after rollback. |
| Notes extractors, tags and scope changes | Compare room and ordinary-save outputs, ordered rows and exact scoped counts against the old scanner and a live-authority oracle. Exercise the peak input and total-key limits, an eligible scope add/Undo and an over-budget expansion with typed refusal and no effects. Revoke one grant across enough tagged items to prove synchronous work exceeds 1,152 keys. First prove all viewer windows and counts are ready. Pause cleanup after fencing and verify the over-budget work is deferred. Query the same windows and counts as revoked and still-authorized viewers during the pause. Require no revoked data and correct scoped results or the typed loading state for affected authorized projections. Unaffected viewers must still receive exact results. Resume cleanup and require fresh ready results within 5 s on the just-over-budget fixture, all matching the live-authority oracle. Require p95 ≤40 ms for saves and eligible scope adds/Undo, p95 ≤500 ms to revoke fencing, and zero wakes. Report committed-field and search publication lag separately. |
| Notes device previews | On a confirmed sleeping space require save intent to durable ACK p95 ≤40 ms, capture to local paint p95 ≤100 ms, and ACK to current paint on a second authorized viewer p95 ≤2 s. Use distinguishable revision images. Assert the second viewer sees the last authorized image marked stale during an intervening save, then the exact new capture and attribution. Without an authorized image, compare the painted miniature with the current summary. Race old-head and copied grants and cut before byte durability, after durable bytes but before pointer commit, and after pointer commit. At both pre-commit cuts, before byte durability and between byte durability and pointer commit, the previous authorized preview or fallback stays visible and no incomplete staged image is ever addressable. Verify the durable orphan is unaddressable and both it and its reservation are reclaimed by the staging cleanup deadline. Require the current image exactly once after committed replay. Both fresh and conditional revoked reads must return no image or `304`. Verify cleanup by the staging deadline and zero preview-driven wakes. |

### C.2 Deferred qualification

These gates apply only to the later consumer revision.

| Spike | Pass condition that can falsify the proposal |
| --- | --- |
| Scratch rebuild | Seed current, stale and revoked artifacts. Delete scratch during a real job and at rest. Require the same published fields and an independently checked rebuilt index. Count model calls and fail if a still-current artifact is recomputed or a stale/revoked artifact is read. Kill the worker with the disk attached, then require cleanup and exclusive reattach. Verify uninstall wipe and no scratch in ordinary backup or runtime snapshots. |
| Originals and thumbnails | Import 10,000 mixed-visibility 4 MiB originals over a provisioned 200 Mbit/s link in ≤35 min while driving sealed foreground traffic. Drive the same work across a lost chunk ACK and compare every final digest with its input. Require the first of the correct thumbnails within 15 s of the first eligible original commit on the heavy-job arm (5 s on the fixture-only device arm). Verify actual image-to-revision identity. Revoke upload and read grants mid-stream with bytes buffered. Assert zero writes and deliveries after the fence and bounded staging retirement. |
| Labels | Drive 300 eligible source changes/s for two minutes with contended writes. Credit each of the 36,000 inputs once, when its exact label or an oracle-correct label and pointer for a newer eligible revision of the same item are visible through the authorized item view. Unpublished coalescing earns no credit. Measure each input's age from its original source commit to that visibility, including superseded inputs. Compare label fields, pointers, windows and counts to the live-authority oracle. Require ≥300 visibility credits/s during steady load, p95 age ≤2 s and oldest eligible age ≤5 s. In a separate arm offer at least twice the configured admission rate and require the backpressure boundary to be reached. Verify bounded backlog, exact recovery through batch/journal/outbox cuts, full reconciliation after drain, and sealed foreground gates. |
| Similarity | Complete 1,000 authorized requests on warmed embeddings with p95 ≤5 s and compare ordered IDs and scores against an independent authorized oracle. Revoke a ranking input outside the returned window before status delivery and require invalidation. Poll by the requester as a positive control, then as another viewer, after expiry and through generic job status, checkpoint and stream. Require zero private result bytes on those paths and zero cross-space writes. |

### C.3 Numbered contract revision

Revise the manifest, description schema, catalogue, verifier and fixtures
together for §A.6, common execution/events, protected requests, declarative
extractors, generic cold create, Notes derived images and row fields, and
the tag set write budget. In the same revision apply §A.3’s record-level
transcript and single-audience contract. Carry forward default all-field search with an
optional field filter. Remove the planned first-release table store.
Qualify every new route against the sealed authority matrix. Reserve the
heavy-job design and Photos contracts for a separate revision.
[Sealed plan][sealed] · [Combined-search measurement][combined-search]

## Simplification opportunities

Reuse the Inbox, item lifecycle fence and current transcript framing.
Keep first-release image publication to the Notes slot and qualify
heavy-job volume and bulk paths with their first consumer.

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
| 11. Cold miniature and media | §§B.1, C.3 row schema and egress |
| 12. Scratch isolation | §§B.3, C.2 exclusive isolated leases |
| 13. Publication audiences | §§B.4, C source checks and owner question |
| 14. Batch feasibility and flood | §§B.4, C.2 deferred batch and offered-load caps |
| 15. Machine consent | §§B.4, C per-delivery and publication fences |
| 16. Bounds and qualification | §§B.2, C resource caps and observable gates |
| 17. Contract revision | §C.3 coordinated schema and conformance changes |

## Second review disposition

| Finding | Where addressed |
| --- | --- |
| 1. Restart window | §§A.2–A.3, C.1 freeze, fresh thaw and downtime arms |
| 2. Ordinary conversation kinds | §§A.6, C.3 generic capability and sealed-name replacements |
| 3. Canonical file | §§A.3, A.5, C (Q3) host file writer and projections |
| 4. Token path and wait | §§A.3, C.1 adapter output and execution wait |
| 5. Composer commands | §A.1 declared preflight parsing |
| 6. Agent discovery | §§A.2, C (Q1) labelled interim and parity cost |
| 7. Steer and child | §§A.2, A.6 exact parent and subset authority |
| 8. Harness qualification | §§A.2, C.1 shared suite and personal fallback |
| 9. Snapshot continuity | §§B.1, C.1 local paint, matching room state and stale image |
| 10. Declarative extraction | §§A.5, B.1 one cold-save facility |
| 11. Tags and labels | §§B.5, C.3 `TextSet`, `contains` and fan-out |
| 12. Original file I/O | §§B.4, C.2 deferred upload, stream and pointers |
| 13. Similarity custody | §§B.4, C (Q4) bounded status result |
| 14. Indexer authority and rebuild | §§B.3–B.4, C (Q2), C.2 creator authority and private artifacts |
| 15. Scratch placement | §§B.3, C.2 heavy VM only |
| 16. Spool caps | §A.3 unconsumed records |
| 17. Member row cache | §§B.1, C cache option and revision 13.2 restriction |
| 18. Reversible cutover | §§A.5, C.1 idle cutover and full transcript diff |
| 19. Spike pass conditions | §§C.1–C.2 timed Send, Notes and deferred Photos checks |

## Third review disposition

| Finding | Earlier partial findings covered | Where addressed |
| --- | --- | --- |
| 1. Full execution freeze | R1.02, R2.01, R2.19 | §§A.2, C.1 independent supervisor and real carrier cuts |
| 2. Frozen resources and transport | R1.02, R2.01 | §§A.2–A.3, C.1 held reservations and transport timeout |
| 3. Credentials, display and dispatch | R1.03, R1.04, R1.06, R2.08 | §§A.2–A.4 dedicated transport, allowlist and checked dispatch |
| 4. Restart-safe scrub and rate caps | R1.06 | §§A.3, C.1 durable carry, credit and load gate |
| 5. Lost execution wake | R2.04 | §§A.3, C.1 atomic cursor check and park race |
| 6. Child aggregate reservation | R2.07 | §§A.2, C.1 atomic tree budget and ancestor fences |
| 7. Conversation eligibility | R2.02 | §§A.6, C.1 package/corpus/field checks and Inbox isolation |
| 8. Archive-triggered Stop | R1.07 | §§A.1, C.1 exact-attempt checks and snooze isolation |
| 9. Legacy custody and rollback | R1.08, R2.03, R2.18 | §§A.3, A.5, C.1 protected bindings, old-readable frames and rebuild |
| 10. Extractor authority, tags and work | R1.09, R2.10, R2.11 | §§B.1, B.5, C.1 closed selectors, tag parity and total key budget |
| 11. Upload admission and cleanup | R2.12 | §§B.2, B.4, C.2 Notes staging bounds, deferred chunk and stream fences |
| 12. Private artifacts and job authority | R1.14, R2.14 | §§B.3–B.4, C (Q2) source-content identity and schedule-only takeover |
| 13. Requester status egress | R2.13 | §§B.4, C (Q4), C.2 deferred separate result and one projection |
| 14. Photo thumbnail producers | New | §§B.4, C (Q5) fixture-only device arm |
| 15. Offered-load qualification | R1.14, R1.16, R2.19 | §C.2 deferred offered-source rate, age and reconciliation |
| 16. Server provider policy | R1.09 | §A.5 checked entry-point and notice contributions |

## Fourth review disposition

| Finding | Earlier partial findings covered | Where addressed |
| --- | --- | --- |
| 1. Supervisor loss | R1.02, R2.01, R3.01 | §§A.2, C.1 independent watchdog, kill/hang and stale thaw |
| 2. Child admission | R2.07, R3.06 | §§A.2, A.6, C.1 live caller, exact parent and target under reservation |
| 3. Rebuildable transcript | R2.18, R3.09 | §§A.3, A.5, C.1, C.3 record cursors, one audience and sealed changes |
| 4. Legacy profile proof | R1.08, R2.03, R3.09 | §§A.5, C.1 historical binding proof or a fresh session |
| 5. Spool retirement | R1.06, R3.04 | §§A.3, C.1 durable expiry and bounded reclaim |
| 6. Scope-mutation key budget | R2.11, R3.10 | §§B.5, C.1 preflight, typed refusal and immediate revoke |
| 7. Reachable cuts and load scoring | R2.19 | §§A.3, C.1 distinct ACK boundaries, frame accounting and numeric gates |

## Fifth review disposition

| Finding | Earlier partial findings covered | Where addressed |
| --- | --- | --- |
| 1. Watchdog and deadline oracle | R1.02, R2.01, R3.01, R4.01 | §§A.2, C.1 separate clocks and watchdog exit/hang, teardown before quota release |
| 2. Sign-in notice on reconnect | R2.18, R3.09, R4.03 | §§A.3, A.5, C.1 bounded prefix in the existing response and disconnected completion |
| 3. Over-budget revoke | R2.11, R3.10, R4.06 | §§B.5, C.1 proven over-limit work, paused cleanup and scoped oracle |
| 4. Canonical publication load | R2.19, R4.07 | §§A.3, C.1 separate ingest/publication gates, rotating files and scored foreground load |

## Sixth review disposition

| Finding | Earlier partial findings covered | Where addressed |
| --- | --- | --- |
| 1. Runtime-owned deadlines | R1.02, R2.01, R3.01, R4.01, R5.01 | §§A.2, C.1 one pair of cutoffs, explicit fencing and teardown margins, late watchdog loss |
| 2. Durable preview orphan | New | §§B.2, C.1 cut between byte durability and pointer commit, visibility and reservation cleanup |
| 3. Label publication accounting | New | §C.2 per-input visibility credit and age from original source commit |

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
[stop]: ../../crates/server/src/threads/operations.rs
[podman]: ../../crates/machine/src/sandbox/podman.rs
[direct]: ../../crates/machine/src/sandbox/direct.rs
[records]: ../../crates/server/src/threads/records.rs
[sessions]: ../../crates/server/src/threads/document.rs
[tags]: ../../crates/server/src/notes/tags.rs
[bindings]: ../../crates/server/src/threads/provider_session.rs
[notices]: ../../crates/server/src/threads/provider_notice.rs
