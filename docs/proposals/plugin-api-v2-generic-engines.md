# Plugin API v2 generic engines

Proposal for sequential review, based on the decisions recorded on `dev` at
`85add29a`. The host owns cold reads, security boundaries and durable commits.
Packages own application behavior, including Notes previews and agent
orchestration. Use the same engine methods, approval paths and quota rules for
official and community packages. This proposal extends the next numbered
revision of the [sealed plan](../plugin-api-v2.md#engine-surface). Its existing
authority, activation, sharing and ticket rules apply. [Owner decisions](https://git.kayg.org/kayg/calternal-ai/src/commit/85add29af840b225dd9a83cae2e5cd4ce3aecfc9/docs/open-questions.md)

## A. Execution without a server Turns engine

The composer stays a host component. It validates the active view, draft and
attachment refs and invokes the package's declared send action. The orchestrator
owns the `threads` kind, defaults and transition rules. It writes the message
and a replayable enqueue intent with the expected item head, then composes
the engines below. Cold transcript and status reads use committed host
projections. Generic stop takes effect while the plugin sleeps. Move the
Turns-specific routes and kind ownership in sealed §§3–4 to this model.

| Engine and proposed operations | Authority and effect | Restart and sleep |
| --- | --- | --- |
| **Admission and jobs** · Existing start, claim, checkpoint, park, status and cancel. Add a declared enqueue intent to an item commit and an optional exclusive job key. | A declared operation needs the live actor's rights, package approval and a resolved target. The host checks the expected live head when committing the intent. It reserves queue, memory, storage and execution budget before admitting the corresponding work. The exclusive key is scoped to space, package and resource. The orchestrator uses the thread as its key. | The host publishes the intent by receipt into one job and retires it after publication. Recover jobs, timers, reservations and park outcomes from durable records. Issue fresh run authority at claim and resume. A queued or parked job uses no worker. A due or winning signal can wake one bounded continuation. |
| **Container execution** · Discover eligible placements, start from an approved image digest and entry point, get status, send bounded control, stop and reconcile. | Bind a lease to the actor or schedule creator, package generation, job attempt, space, admitted image, target and a host-checked subset of item, tool and network rights. The actor supplies authority. The sponsor supplies capacity. A person can choose their own eligible enrolled machine where existing policy permits. A package can request an approved target class. The host selects and checks the target, provider-home binding and resource limits. | Persist placement intent, IDs, last durable output position and stop/cleanup state. An enrolled machine can reattach only to its exact still-current execution. On expiry, replacement or uncertain execution, fence the old attempt and expose interruption. Only an authorized person can resume it. An active run has a lease independent of plugin-process residency. |
| **Durable streams** · Declare, append, page, follow and checkpoint a stable stream. | Tie a stream to a checked item or job and a registered bounded event schema. Only a current authorized producer can append. Page, resume and egress check live authority and the stream's display audience. Use a separate private machine-output channel for opaque frames. The plugin normalizes that output into transcript events. Logs, token material and provider session IDs stay off display routes. | Append the batch, producer sequence, receipt and consumed-input checkpoint together. ACK that checkpoint only after commit. Persist the scoped cursor, stable event IDs and retention boundary. Wake a bounded continuation on committed work if processing is needed. A stalled subscriber releases the DB read before waiting on transport and can reload from its committed cursor. |
| **Protected requests** · Create, inspect, answer, approve, deny and cancel through the existing Inbox model. | Only the current run may create a request for a checked action. Pin the actor, allowed decision maker, exact action/material digest, target and lifetime. A live eligible person resolves it through the protected surface. Consume the grant under the action fence. An answer may narrow or authorize one action within the allowed execution policy. | Persist requests, winning decisions and receipts. Resolution can wake a parked job exactly once. Cancel, expiry, revocation and stale run attempts cannot revive it. A second app can park for review of a document or a tool invocation through the same engine. |
| **Secrets and sign-in** · Use the existing opaque handles and typed credential calls. Bind machine-local provider profiles as approved lease inputs. | Resolve the handle against actor, package, purpose, approved destination and active attempt. A trusted broker redeems the credential and returns only the allowed projection. CLI sign-in stays private on its authorized machine. The agent's tools get neither a login mount nor the broker's secret channel. | Retain protected configuration and machine-local profiles under existing ownership and erasure rules. Revalidate and issue fresh handles on reattach or resume. An expired login exchange can be restarted by the person. |

The host owns the launch/stop receipt and sequence fences. Commit private
machine output before acknowledging the machine. A plugin may acknowledge
consumption only when the public append and input cursor commit together,
including a cursor-only checkpoint for input that produces no display event.
Reclaim the private spool through that checkpoint in bounded batches.
Apply backpressure at the spool and machine credit limits. On reattach,
resend the unacknowledged range. After server restart, automatically
reattach only on authenticated proof that the original execution is still
running under the current lease. Otherwise preserve the committed prefix,
fence the old attempt and show Resume to an authorized person. Reconcile
uncertain external effects before retry. Automatic provider retry through a
durable timer remains available when the prior effect is known safe.

Each space has a small plugin VM for interactive and light work, an on-demand
larger plugin VM for heavy jobs, and an on-demand agent VM with rootless
containers. Plugins and agents run in separate VMs. Honor the decided weaker
self-hosted boundary when KVM is unavailable. No target mounts a userdata
tree. A brokered agent call reaches only checked item refs or an explicitly
delegated scope. An agent cannot enumerate the space's files. A run workspace
or private provider profile is distinct from userdata. A community package can
supply its own adapter, approved image, transcript schema and retry controller
through these same engines. It never needs a server registration for an
application-specific run type.

### Current code and migration

| Today | Engine seam and package ownership |
| --- | --- |
| Turns `admission`, `thread_admission`, `launch`, `lease` | Extract receipt-backed item-to-job admission, same-attempt placement and cleanup. Move prompt construction, provider command encoding, session selection and one-turn-per-thread policy into the orchestrator. Preserve the existing two-barrier send and archive ordering. |
| `run`, `progress`, `control` | Keep machine output sequencing, bounded transport, cancellation and fenced commits. Move parsing provider frames, assembling messages, choosing checkpoint boundaries and mapping steer/stop to adapter commands into the plugin. Expose checkpoints as generic durable data and render committed pages through the existing host component. |
| `access`, `redact`, `verify`, `readiness` | Keep capability and effect checks, secret-safe routing, private diagnostic delivery and verification of enrolled targets in the host. Remove bearer exposure to workloads where a brokered call suffices. Retain issued-credential filtering on legacy channels through cutover. The plugin owns the provider probe, login UX and readiness interpretation. |
| `phases`, `retry`, `tool_steps`, `instructions`, server thread presentation | Move phase labels, retry intervals and reset policy, provider/model selection, tool summaries, title generation and instruction assembly into the orchestrator and adapters. The host supplies timers, bounded progress and shared rendering primitives. |

Current Claude Code maps its CLI's print stream and resume into provider-v1
frames. Codex maps App Server stdio, saved native sessions, steer and interrupt.
Reuse those adapters behind the package boundary. The host can validate a
declared frame and pass bytes without understanding either provider. [Manual turns](../agent-turns.md) · [Provider protocol](../provider-protocol.md) · [Claude Code](../../plugins/calternal-claude-code/README.md) · [Codex](../../plugins/calternal-codex/README.md)

1. Qualify the stream, machine broker and protected wait using both adapters
   and a minimal independent runner. Reuse the existing auth, jobs, machine
   lease, journal and content engines.
2. Move the Notes rewrite onto generic items and derived output first. In
   parallel with that integration, prove the orchestrator against disposable
   data through the same declared interfaces.
3. For the Threads cutover, stop new old-engine admissions and drain or fence
   active runs. Convert and verify committed thread heads, stream positions
   and retained session bindings once. Reauthorize new runs and preserve
   provider-local profiles. Verify archive/stop, automations and receipts
   against the converted state before switching the single writer. Remove
   native Turns routing and provider-specific server policy in that cutover.
   Use a verified restore point for rollback.

## B. Derived files and private scratch

The host serves every list, count, search result and shared view from items
and their authorized projections while the space sleeps. Each package may
keep rebuildable private scratch in appdata. The first release drops the
host table store proposed in sealed §6.

Any manifest can opt into cold creation from declared defaults. The host
publishes the empty item and its change record using the ordinary item commit.
The package uses the committed change for any further initialization.

### Derived files and note previews

A package declares each derived output's applicable kind, name, media type,
producer and quota. The host keys a publication by space, package, item,
source revision and output name. Include checked dependency revisions and
producer version where relevant. An output inherits the authority of its
source and any additional inputs. Only inputs visible to the entire output
audience may contribute to a shared output. Personal outputs belong in the
person's space.

On save, the editing device takes a small snapshot of the note and uploads
it after the save ACK. The host issues an upload grant tied to that editor,
save receipt, exact note revision, output name and current edit authority.
Validate and re-encode a still image within declared host-capped type,
dimension and byte limits. Strip metadata and reserve staged bytes before
accepting the upload. Under the publication fence, recheck the grant,
current head and live authority, then publish the pointer with the
author and save revision. Expire unused grants and reclaim interrupted
staging. A member who can edit a shared note can submit misleading pixels
for that note and its revision. The image carries uploader attribution,
grants no action or access, and is superseded on the next save. The host
can verify image safety and provenance. The pixels are editor-supplied.
The next editor's valid upload supplies the replacement.

The leading image uses an opaque, audience-bound reference to the current
asset. Each request checks live item authority and version. When the image
is missing, stale or evicted, the renderer draws a simple miniature from a
bounded structured summary of the first blocks. The host supplies this
declared row field from the current revision using the same field permissions.
Keep the summary to simple block tags and short text with no external loads.
An agent or plugin edit can populate the summary without producing an image.
Device capture and image upload stay off the save path. The fallback and
asset reads wake zero VMs. Host or plugin rendering can qualify later.

Other derived files, such as photo thumbnails, come from brokered jobs.
A save records the existing change/outbox in its commit. The job subscriber
advances its durable cursor by receipt and coalesces by item and output to
the latest desired revision. Queue delivery and computation happen after
the save. Recheck the source and dependency revisions and the producer's
live rights before publishing. Keep the catalogue, pointers, indexes and
bounded hot previews in SSD appdata. Put bulk media in a host-managed
derived directory on the media tier. In a one-disk installation both roots
can share a filesystem. Commit the staged bytes before publishing their
pointer. Treat an absent file as a cache miss and evict in bounded batches.
Back up canonical files and items using their ordinary policy. Derived media
can be rebuilt or replaced by the fallback. [Storage ADR 0062 and tiering amendment](https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0062-user-files-on-hdd-app-data-on-ssd.md)

### Private scratch disk

Provision one size-capped virtual disk in appdata per installed package and
space. A declared background job in either the small or heavy plugin VM may
use it. The host attaches it to one VM at a time and mounts it only in
the leased job sandbox. The lease also serializes disk users within the
VM. The heavy VM is the default for indexing. Handoff requires quiescing
the old job, unmounting and detaching before the next attachment. After a crash, prove the old attachment
is fenced before reuse. The disk is outside VM snapshots and never attaches
to agent VMs or enrolled machines. Interactive calls and item-share or
guest activations receive no scratch handle. They continue while the heavy
VM holds the disk. The package may use SQLite or another local format.

**Rebuildable custody.** The host enforces capacity on the disk, including
auxiliary database files, and excludes it from backups. Uninstall fences
jobs and wipes the volume. The package declares how to reconstruct it
from the person's files and committed items, both accessed through the
broker. Originals, user edits and durable job checkpoints use their normal
host custody. Conformance deletes the volume mid-job and at rest and
requires recovery of the same visible state and deterministic fixture
index, with zero canonical loss. Disk exhaustion stalls or rebuilds the
scratch index while canonical writes continue.

**Host-only publication.** No disk access, reads or query endpoint is
issued to a renderer, guest, item-share, other package or agent. A package
publishes visible results by patching its declared indexed item fields
through the broker. Keep the scratch generation scoped to the declared
job's source authority. Reuse a generation only under the same checked
source scope. Fence it on grant or permission loss and rebuild before
reuse. The broker checks current write authority and source revisions on
publication. A photo's recognized labels become ordinary fields. Filtering
by label uses an ordinary Bind and item authority. An item share can expose only that
item's allowed projection and derived media. Guests and item shares
cannot enumerate the scratch index, other items or space-wide counts.

An enrolled personal machine may index originals from a shared space only
after an explicit space decision and the machine owner's agreement.
Check both when assigning or renewing work. On withdrawal, fence the
lease and stop further broker reads and publication. An enrolled machine
operator can retain bytes already delivered. Under the first contract,
“photos like this” runs in the on-demand heavy plugin VM as a background
ML job. A visible result uses ordinary item publication and checks each
target's read authority.

### Bulk item metadata writes

Use the existing brokered item commit and indexed field path. Propose at
most 128 item patches and 1 MiB encoded data per batch, with one outstanding
batch per job. Each batch carries a replay receipt and an expected source
revision per item. Before commit, check the producer generation, declared
fields, current actor/item rights and source revision. Merge the authorized
fields against the current head under the write fence. Report stale items separately for
recomputation. A metadata-only patch keeps its source input revision and
does not trigger its own producer.

Commit accepted patches, index/count changes, the receipt and a bounded
change record per batch together. Keep per-item revisions and authority
checks while avoiding an independent flush or notification per item.
Feed and search workers consume the committed range by cursor. Keep
the latest pending revision per item. Notify each affected authorized
Bind at most once per delivery interval, initially 100 ms for the spike.
Replace or invalidate the affected window. Preserve the immediate delivery
path for targeted revocations. Host admission meters bytes, item changes
and index work per space.
Yield between batches and return a retry delay when journal or projection
backlog reaches its budget. Reserve foreground capacity for taps and saves.
Validate the batch caps and delivery interval in the spike.

## C. Qualification

The owner resolved the execution, snapshot, machine consent, similarity
and scratch decisions. Remaining limits are qualification parameters.
Use sealed §8's machines, instrumentation and repetition rules. The new
latency targets below are proposed spike gates.

| Spike | Pass condition that can falsify the proposal |
| --- | --- |
| Agent recovery | In real agent containers, cut after stream commit before ACK, restart the server, stall a reader and revoke during reattach. Reattach only to a provably identical live execution. Ordered event and receipt IDs survive. An uncertain attempt stays fenced until explicit Resume, with zero duplicate effects or revoked bytes. Keep the sealed 150 ms warm first-screen and 500 ms independent-control p95 gates. |
| Agent isolation and parity | Use both shipped adapters and an independent runner. Assert zero foreign or revoked ref access, directory enumeration, credential exposure or unauthorized placement. Park an approval, restart and race decision against revoke. At most one eligible decision wins. |
| Device snapshot | Save and upload with the space asleep. Race a delayed upload against an agent edit. Inject foreign item and revision refs, a stolen or revoked upload grant, and oversized or invalid image bytes. Assert zero unauthorized or stale published bytes, correct attribution, and zero runtime wakes for save, upload or cold read. An authorized misleading image is confined to its note and revision. Its successor's row uses the miniature until a valid replacement arrives. Require p95 ≤40 ms save ACK and ≤2 s from ACK to current preview for a successful ordinary device upload. |
| Scratch deletion and fencing | Delete scratch at rest and during a job, fill the disk, and cut during small-to-heavy handoff. Recover from the same files and items, preserve canonical hashes and reconstruct the deterministic fixture index. Assert one live attachment at most, zero guest, share or cross-package reads, exclusion from backup and deletion on uninstall. |
| 10,000-photo label run | Index 10,000 mixed-visibility photos on the heavy VM and publish labels in bounded batches while driving taps, saves and reads. On a consented enrolled-machine arm, revoke either consent during the run and verify zero further broker reads or writes. Verify ordinary label Binds, exact index reconciliation after a cut, no unauthorized writes or changes to hidden readers' rows or counts. Keep the sealed tap, cold-window and 40 ms save p95 gates. Saturate the publication queues and record backpressure, flushes, feed deliveries, search lag and foreground p95. Assert bounded backlog and coalesced delivery. |
| Derived media eviction | Cut during staging and pointer publication and remove the derived root. Canonical hashes and authority survive. Assert bounded staging, no dangling asset delivery and an authorized miss until a valid current asset publishes. |

## Simplification opportunities

Use one protected request store and one replayable job admission path.
Share the cursor and receipt engine between transcripts and other durable
streams. Use the same item commit and change cursor for
bulk metadata and single edits.
