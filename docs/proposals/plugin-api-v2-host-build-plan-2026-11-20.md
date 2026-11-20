# Plugin API v2 host build plan

Scope is an agent-only Threads app using its own item files and a second installed
package for the harness. Use the pinned API v2 draft on `lane/v2-host-base` and the [Threads
spike](https://git.kayg.org/kayg/calternal-ai/src/commit/23471bfa/docs/reviews/threads-plugin-spike-2026-11-20.md).
The code map is against dev `d3d37a4c`. Estimates are engineering days per lane,
including focused qualification and excluding review queue time. People-thread consent,
custody and delivery have their own open gates.

Start registry, content, execution and machine work together once they agree on the
pinned package, item, job, attempt and receipt types. Keep the browser or native renderer
on the fixture until the host route shapes are usable. The slices use generic host routes.

## Slices

### 1. Installed contributions and the broker handoff · 3–5 days

Implement `extension_points.resolve` and `extension_points.calls`. Index the
package-qualified app, point, exact version and contribution in the installed catalogue.
Verify both declared interfaces and executable asset pins on install. Expose only checked
choices. At admission check the encoded input bound and pin the exact contribution input
before a prompt can commit. Persist ordered output and its terminal under the attempt
fence. Replay a call or frame receipt without relaunching the provider.

Reuse `crates/plugin-host/src/registry_v2.rs` and `claims.rs`, and
`crates/server/src/plugins/{registry_v2.rs,package_store/v2.rs,space_broker.rs}`. Use the
existing job receipt namespace in `api/plugins/receipts.rs` and the durable job and bounded progress
boundaries in `plugins/{jobs,job_streams.rs}`. The current job progress stream is
ephemeral. Put committed contribution output in a bounded durable spool.

Qualify two installed packages, same local app IDs with different owners, wrong-version
and denied interfaces, and uninstall while a call is queued. Try escape-heavy input with
zero writes or launch, out-of-order frames, lost output and append ACKs, and a terminal
after revoke. The registry can land first. Live dispatch needs slices 3 and 4.

### 2. Item writer and readable placement · 4–6 days

Implement `item_engine`, `file_append` and `created_outcome`. Cover typed create, save,
head, revision range and append. Commit metadata title, body and leaf rename together.
Use one writer transaction for the checked revision, publication intent, receipt and the
opaque-write discovery fence. Add bounded begin, chunk and seal for staged saves.

Reuse `crates/content/src/{mutations.rs,files.rs,item_head.rs,revision_slice.rs,
linked_write.rs,journal.rs,recovery.rs}`, `crates/fsio/src/{beneath.rs,swap.rs}`,
`crates/store/src/{writer.rs,migrations.rs}` and
`crates/server/src/{store/plugin_items.rs,plugins/broker.rs}`. The existing writer outbox
and revision pin handle crash recovery. Use the existing rename engine for link rewriting
when the app asks for it.

Qualify checked path collisions, stale head, exact replay, rename during output, power
cut around publish, retained range after head advance, and opaque import fencing across
list, search, direct read and derived output. Measure the 2 MiB staged-save gate on the
locked perf VM. Inline write, head, pinned range and append can land first. Start
independently of slice 1 and use slice 8's journal projection for list visibility.

### 3. Execution admission and attempt fences · 5–8 days

Implement `execution_intents`, `execution_engines`, the protected Stop context and
`attempt_authority_end`. Use durable admission with one head and one successor, live
authority checks at promotion and claim, and idempotent Stop. Reuse the Inbox to bind
approval to the actual effect. Lift the generic attempt fence, exact runtime proof and
quota accounting from native Turns into the execution engine. Keep application phase and
title logic in Threads.

Reuse `crates/server/src/plugins/jobs/{claim.rs,complete.rs,waits.rs}`,
`plugins/space_ticket.rs`, `turns/{admission.rs,lease.rs,control.rs,
space_execution.rs}`, `api/request_action.rs` and `crates/core/src/requests.rs`. Keep
SQLite work per scope using rusqlite and the existing writer. Slice 2 owns the per-space content writer transaction
and its migration sequence. Slice 3 owns the execution lifecycle rows and
supplies the archive fences and cleanup in that same FULL transaction. Dispatch
remote cleanup after the fence commits. Hold quota until verified teardown.

Qualify lost receipt replay, two actors and a queued successor, commit before Stop or
archive reply followed by crash, device and grant revoke, and server restart with live or
failed exact reattachment. Race approval with tool dispatch. Start the state machine and
crash tests in parallel with 1 and 2. Use 4 for real proof, fencing and cleanup. For the
first Send gate keep child, Resume and tool paths disabled until qualified.

### 4. Enrolled entry ABI and images · 4–7 days

Implement `extension_points.harness_abi`, the live `carrier` shapes and the placement
rules in `execution_attempt_authority`. Verify source and digest, mount the separately
pinned entry and selected login scope, and run with the declared execution network and
exact attempt ticket. Wire the bounded, ordered entry stream and terminal to slice 1.
Runtime deadlines and teardown must work independently of supervisor and server liveness.

Reuse `crates/machine/src/runner/{lifecycle.rs,space_channel.rs}`,
`sandbox/{podman.rs,provider_home.rs,worker.rs}` and the supported `sandbox/microvm`
placement. Reuse `crates/plugin-host/src/space_channel/{wire.rs,flow.rs}`,
`crates/server/src/machines/{sessions.rs,execs.rs,space_channel.rs, state/placement.rs}`
and `plugins/network/admission.rs`. The enrolled carrier already exists. Adapt the
checked worker dispatch and entry messages rather than adding another transport.

Qualify source or digest changes, an unapproved placement, egress denial, profile
separation, retained output and backpressure, and kill and hang arms for supervisor and
watchdog. Require no stale thaw or egress and verified cleanup before quota release.
Start image verification and entry tests now. Integrate authority with 3 and dispatch
with 1. Use a scripted digest-pinned entry in the real sandbox first. Then qualify one
supported provider image, login and network scope on a personal machine.

### 5. Item views and scoped subscription delivery · 3–5 days

Implement `view_snapshots`, `item_view.open`, `item_view.follow`, `item_view.asset`,
`item.locate`, `subscription_catch_up` and unchanged renewal. Draw the retained latest
page cold, then follow committed appends. Keep one scope for the route operands and apply
invalidation only to that scope. Bind each page, anchor and asset to current item
authority.

Reuse `crates/server/src/api/plugin_slots/live.rs` for bounded subscriptions and
authority watch, `crates/content/src/publication.rs` for committed changes and the window
lane's `api/window_views.rs` for lease and view state. Reuse the strict description
validation and the content revision pin from 2. Register the watch before reading
catch-up. Test cold open without a wake, commits on both sides of registration, older
pages, exact anchors, body or asset revoke and unchanged renewal. Start after the row and
item wire agrees with 2 and 8. A text-only view and head follow are enough for the first
Send gate. Keep unavailable leaf and asset shapes disabled through their qualification.

### 6. Placement and extension choice routes · 2–4 days

Implement `app.open`, `extension.choices`, `choice.options`, activation
resolve/confirm/dispatch and `execution.control` for the Threads surface. Bind the chosen
live harness and placement to the exact activation and receipt, and show only
host-checked confirmation material.

Reuse `crates/server/src/api/{dispatch.rs,plugin_slots.rs,authorization.rs}`,
`plugins/{harness_admin.rs,live_catalog.rs}` and
`machines/{space_execution.rs,readiness.rs}`. Exercise cold open through Send and Stop, a
forged choice, changed grant between confirm and dispatch, and an exact retry. Start
route and context tests in parallel with 1 and 5. Needs 1's registry, 3's execution
admission and 4's checked placements. First demo may offer one actual eligible machine
and one approved harness. Keep the real confirmation and exact Stop fences.

### 7. One-shot alarms · 2–3 days

Implement `alarms`, `AlarmSet` and `AlarmDelivery`. Pin the job, item, harness and
preflighted input. Consume an alarm and admit its job once under the durability boundary.
Keep scheduled and actual times and the grace-based missed bit on replay. Reuse the
deadline selection loop in `crates/server/src/automations/scheduling.rs` and its worker,
and claim and recovery from `plugins/jobs`. Keep recurrence in the app.

Test restart before due and after commit before reply, duplicate claim, disable and
revoked placement, and grace with late admission. Needs 3; may follow the first
interactive Send gate.

### 8. Window engine · already in gate; allow 1–2 days for adoption

Use `query.window`, `query.list_publication`, archive filtering and the
scope/invalidation parts of `cache_policy`. The [window
lane](https://git.kayg.org/kayg/calternal-ai/src/commit/2afa2a19/docs/reviews/window-engine-2026-11-19.md)
already implements journal replay and covering reads in
`crates/content/src/window/{replay.rs,read.rs,runtime.rs}` and the host surface in
`crates/server/src/api/window_views.rs`. Take its backend gate first. Qualify the Threads
kind against the real registry, publication watermark and live route scope. Cover
archive/Undo, save-to-ready recovery, revoke mid-window and the worst supported scoped
list plans. Add later draft wire features at that same route boundary.

### 9. Device reconciliation and offline downloads · 3–5 days, later

Implement `device_reconciliation`, `offline_download` and signed row erasure rules. Use
`crates/server/src/api/{authorization.rs,sharing.rs}`, the existing device/session and
grant checks in `crates/auth`, plus `crates/content/src/revision_slice.rs` for pinned
ranges. Sign checked selection grants using the host proof boundary. Qualify paged
reconnect across spaces, an old reply after retry, omitted scoped rows and a revoked
grant while no view is open. Keep cold first paint independent of reconciliation. Needs 2
and 5/8 for source and cache identities. Leave protected persistent rows and offline
download disabled in the early live app.

## Integration and deletion

First run a new space with the gated window lane, inline content methods, one real
enrolled carrier, one registered harness and a text item view. Drive create, Send, first
output, replay and Stop against the actual server and writer. Follow with the supported
provider and test login. Keep alarm, staged edits, richer view assets, offline, children,
Resume and people paths disabled until their gates pass. Rollout waits for the fresh
install requirements in §8 and the measured 500 ms p95 product wake gates. Use
additive migrations and the running-release upgrade check for changes merged before
cutover.

The owner permits a fresh production redeploy at the Threads cutover. Qualify freshly
created files and jobs under §8. Slice 3 removes the native Turns job interpreter, admission,
phases, progress, control, instruction and retry paths after the generic fences take
over. Slice 4 retires native provider-specific launch and raw output adapters after the
contribution placements qualify. Keep enrollment, runtime isolation, request approvals
and generic credential mounting. Slice 7 removes the Threads-specific scheduling dispatch
when alarms replace it.

In the final cutover change owned by 6, remove native Threads and Notes dispatch and
authority aliases in `api/dispatch.rs` once each app passes its own gate. With 1, remove
the transitional `threads_owner()` lookup and its entry in
`crates/server/plugin-namespace-owners.json`, the bundled-ID exceptions and native
owner routing in startup, jobs and turn admission. Retire server-owned thread presentation,
titles and provider-session wrappers with 3 after the fresh-install gate passes. Keep generic registry collision and trust checks, the content engine, and shared
search and worker plumbing. Remove the catalogue's per-app cutover tables after cutover.
