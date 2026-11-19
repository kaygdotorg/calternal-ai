# Plugin API v2 generic engines

Proposal for owner review, based on the decisions recorded on `dev` at
`74a62a6c`. The host owns cold reads, security boundaries and durable commits.
Packages own application behavior, including Notes previews and agent
orchestration. Use the same engine methods, approval paths and quota rules for
official and community packages. This proposal extends the next numbered
revision of the [sealed plan](../plugin-api-v2.md#engine-surface). Its existing
authority, activation, sharing and ticket rules apply. [Owner decisions](https://git.kayg.org/kayg/calternal-ai/src/commit/74a62a6c2011212f78e06b78cd7c25fbb02bd273/docs/open-questions.md)

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
| **Container execution** · Discover eligible placements, start from an approved image digest and entry point, get status, send bounded control, stop and reconcile. | Bind a lease to the actor or schedule creator, package generation, job attempt, space, admitted image, target and a host-checked subset of item, tool and network rights. The actor supplies authority. The sponsor supplies capacity. A person can choose their own eligible enrolled machine where existing policy permits. A package can request an approved target class. The host selects and checks the target, provider-home binding and resource limits. | Persist placement intent, IDs, last durable output position and stop/cleanup state. An enrolled machine can reattach only to its exact still-current execution. On expiry, replacement or uncertain execution, fence the old attempt and expose interruption for reconciliation. Restart alone launches no replacement. An active run has a lease independent of plugin-process residency. |
| **Durable streams** · Declare, append, page, follow and checkpoint a stable stream. | Tie a stream to a checked item or job and a registered bounded event schema. Only a current authorized producer can append. Page, resume and egress check live authority and the stream's display audience. Use a separate private machine-output channel for opaque frames. The plugin normalizes that output into transcript events. Logs, token material and provider session IDs stay off display routes. | Append the batch, producer sequence, receipt and consumed-input checkpoint together. ACK that checkpoint only after commit. Persist the scoped cursor, stable event IDs and retention boundary. Wake a bounded continuation on committed work if processing is needed. A stalled subscriber releases the DB read before waiting on transport and can reload from its committed cursor. |
| **Protected requests** · Create, inspect, answer, approve, deny and cancel through the existing Inbox model. | Only the current run may create a request for a checked action. Pin the actor, allowed decision maker, exact action/material digest, target and lifetime. A live eligible person resolves it through the protected surface. Consume the grant under the action fence. An answer may narrow or authorize one action within the allowed execution policy. | Persist requests, winning decisions and receipts. Resolution can wake a parked job exactly once. Cancel, expiry, revocation and stale run attempts cannot revive it. A second app can park for review of a document or a tool invocation through the same engine. |
| **Secrets and sign-in** · Use the existing opaque handles and typed credential calls. Bind machine-local provider profiles as approved lease inputs. | Resolve the handle against actor, package, purpose, approved destination and active attempt. A trusted broker redeems the credential and returns only the allowed projection. CLI sign-in stays private on its authorized machine. The agent's tools get neither a login mount nor the broker's secret channel. | Retain protected configuration and machine-local profiles under existing ownership and erasure rules. Revalidate and issue fresh handles on reattach or resume. An expired login exchange can be restarted by the person. |

The host owns the launch/stop receipt and sequence fences. Commit private
machine output before acknowledging the machine. A plugin may acknowledge
consumption only when the public append and input cursor commit together,
including a cursor-only checkpoint for input that produces no display event.
Reclaim the private spool through that checkpoint in bounded batches.
Apply backpressure at the spool and machine credit limits. On reattach,
resend the unacknowledged range. If the machine cannot prove continuity,
preserve the committed prefix and show the outcome as interrupted or unknown.
A provider call with an uncertain external effect requires reconciliation before
retry. A plugin can request a provider retry through a durable timer when the
prior effect is known safe.

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

## B. Derived outputs and bulk records

Declare each output name, applicable kind, schema or media type, producer
revision, execution class and quota in the package manifest. The host keys
a publication by space, package, item, source revision and output name.
Include producer version, settings revision and checked dependency
revisions when they affect the result. A derivative inherits all input
disclosure constraints. An output served under an item's own authority
may use only inputs every authorized viewer of that output may read.
Keep personal output in the person's space. Aggregates use §5's checked
audience semantics. Regenerability grants no access.

Any manifest can opt into cold creation from declared defaults. The host
publishes the empty item and its change record using the ordinary item
commit. The owning package consumes the committed change to do further
initialization. A blank note appears while the space sleeps.

| Operation | Commit or read boundary |
| --- | --- |
| **Enqueue** | A save writes its existing committed change/outbox. A durable subscriber consumes its cursor and coalesces by item and output, keeping the latest desired revision. Queue admission uses the live editor or the explicitly configured job creator's authority. Retry by receipt after a lost wake or restart. Save ACK waits for neither render nor queue delivery. Edits, purges and producer changes fence old generations without scanning their rows. |
| **Produce and publish** | The plugin reads checked source revisions through the broker. A Notes job runs the real editor headless to render a preview. The heavy plugin VM or an eligible enrolled machine handles bulk work. The host validates type, dimensions, byte and row bounds, reserves space before staging and verifies the digest. Compare source head, dependency revisions and run authority immediately before switching the visible pointer. Late completions are discarded. |
| **Read** | A row contains an opaque preview ref bound to output, revision and audience. The host resolves it with one indexed lookup and a bounded asset read under current item authority. Use the leading image slot. On missing, evicted or stale output, keep the row available with its ordinary icon and refresh only when the current asset is published. No read starts a job or a VM. |
| **Evict** | Account staged and live bytes, row/index growth, broker I/O, concurrency and execution against the admitting space and sponsor. Reserve before work, yield under contention and evict in bounded batches. A missing cache root or dropped generation reads as a miss. Purge and grant revocation deny access immediately, before reclamation. |

Keep the catalogue, generation pointers, bounded hot previews and query
indexes in the host's per-space SSD appdata. Put bulk artifacts and immutable
segments under a separate host-managed derived directory on the configured
HDD tier. These paths are outside the user-visible trees and invisible to
VMs. Write and verify the artifact before atomically publishing its index
pointer. A committed pointer to a missing file is a cache miss. Drop the
entire derived namespace on recovery if needed. Preserve original bytes,
revision history, approved manifests, durable job policy and authority in
their existing backups. An artifact that has become someone's only copy is
an ordinary item and must use the ordinary save path. [Storage ADR 0062 and tiering amendment](https://git.kayg.org/kayg/calternal/src/branch/dev/docs/adr/0062-user-files-on-hdd-app-data-on-ssd.md)

### Table store

Expose typed, brokered generation staging, bounded batch ingest, publish,
indexed lookup and keyset pages. A declared table has typed scalar columns
and bounded vector or blob values, a stable primary key, and a small fixed
set of declared indexes. A derived row pins its source item and revision.
Require an authorization-scoped plan before paging, counting or aggregating.
For recognition, the host can seek by scope and label then return authorized
photo refs. For embeddings, store bounded dimension/type vectors and qualify
any nearest-neighbor plan separately, including its disclosure and work.
A full scan belongs in a brokered heavy job. Reject unqualified interactive
plans.

Proposed first test bounds are 32 columns, four secondary indexes, 256 rows
and 1 MiB encoded per ingest call, 200 rows per read page and versioned
per-space byte/row quotas including indexes and retired generations. Publish
with a short pointer commit after validating the staged generation. Keep
the inputs pinned or invalidate the generation if one changes. Derived
tables are disposable. A future table holding primary records needs an
explicit durable class, a coordinated backup and a restore gate. Plugins
get neither SQL nor a database file. The host can enforce indexed-work
limits, current row authority, quota and recovery precisely because it owns
the schema, indexes and commit. This is the general table engine anticipated
by sealed §6.

## C. Owner decisions and qualification

1. **Should an agent reconnect by itself after a server restart when its
   original execution is still provably running?** A restart during a long
   edit can interrupt the person's work or silently duplicate it. Always
   requiring Resume is simpler and costs attention even when the original
   lease is healthy. Reattaching the same execution needs a durable cursor
   and proof of lease continuity. Recommend reattach only with exact proof.
   An expired or uncertain execution requires explicit Resume.
2. **Which editor features must a first-release note thumbnail render?**
   A note with callouts and an image exposes the difference. Rendering rich
   blocks and local images with the headless editor preserves the
   appearance and adds fetch, compute and publication time. A text-only
   editor render has smaller working sets and omits visual context.
   Recommend rich blocks and authorized local assets, with the normal
   icon while the preview is unavailable.
3. **May an enrolled personal machine index shared originals?** Indexing
   a shared photo library could take work off hosted VMs, but the machine
   operator may retain the bytes it receives. Using only the larger
   space VM is simpler and spends hosted capacity. Recommend the larger
   VM by default and an explicit space decision plus machine consent
   before sending shared originals to a personal machine.
4. **Does the first table engine need interactive similarity search?**
   Finding “photos like this” adds a vector query path, an index and
   audience-sensitive ranking tests. Storing vectors with indexed scalar
   queries gets recognition and batch embedding jobs working sooner, with
   similarity left to a background job. Recommend the narrower first
   contract and a separate qualification for interactive similarity.

Use sealed §8's machines, instrumentation and repetition rules. The new
latency targets below are proposed spike gates.

| Spike | Pass condition that can falsify the proposal |
| --- | --- |
| Agent recovery | In real agent containers, cut after stream commit before ACK, restart the server, stall a reader and revoke during reattach. An intact lease reattaches to the same process and ordered event IDs. An uncertain lease produces no replacement or duplicate effect. Zero revoked bytes or actions. Retain the sealed 150 ms warm first-screen and 500 ms independent-control p95 gates. |
| Agent isolation and parity | Run both shipped adapters and the independent runner with identical broker methods. Attempt foreign and revoked refs, path enumeration, credential exposure and an unauthorized placement. Zero successful attempts and no app-specific host dispatch. Park an approval, restart, then race decision against revoke. Exactly one eligible decision wins. |
| Sleeping-space thumbnail | Save a representative note through the host while the plugin sleeps. Inject duplicate notifications, a second edit during rendering and a worker restart. Require p95 ≤40 ms save ACK and ≤2 s from ACK to the authorized row painting its current preview on the idle small-VM arm. Show the ordinary icon while pending. Cold windows and asset misses wake zero VMs. No wrong-revision image is served. |
| 10,000-photo indexing | On a heavy plugin VM and an enrolled machine, index 10,000 mixed-visibility photos through bounded broker reads while driving taps and list requests. Keep sealed tap, cold-window and save gates. Record throughput, queue lag, active-job memory, I/O, bytes and row work. Claimed rows must reconcile exactly after interruption and reject unauthorized sources and unqualified interactive plans. |
| Derived eviction and restore | Delete the derived roots and cut at staging and pointer publication. Restore canonical content and rebuild from committed revisions. Original hashes and authority stay intact. Return zero orphan or stale refs, enforce byte/row caps including staging, and provide an authorized miss until the new generation publishes. |

## Simplification opportunities

Use one protected request store and one replayable job admission path.
Share the cursor and receipt engine between transcripts and other durable
streams. Use the same generation publication and expiry mechanism for
derived files and rows.
