# Early-use plan: independent critique (Fable 5.2, medium)

2026-11-16. Reviews [early-use-plan.md](../early-use-plan.md) against the architecture proposal, validation plan, inheritance audit, feature-gap assessment and server-host assessment. Written before reading any other review. Nothing was implemented, run or measured. Inherited claims were spot-checked read-only against existing Calternal at `ea57b84b9c9e40e8b67b7d1e1f1805915123b040`.

Line references use `file:line`. "Plan" means `early-use-plan.md`.

## Verdict

The plan has the right shape: one end-to-end slice through SDK, GenUI and core instead of three frameworks, with gates that are not bypassed for speed. It is not yet executable as written, for three reasons.

1. **The checkpoint never says where it runs or who is trusted.** Without that, the hostile-tenant sandbox, managed provider login and server-side harness isolation all sit on the path to first use (findings 1 and 4).
2. **GenUI v0 lists properties but not the mechanism.** Who produces a view, where bindings are evaluated and how the Notes editor fits are undecided, and those choices determine both the Swift renderer and the 120 Hz risk (findings 2 and 3).
3. **The order hides two dependencies.** The contract toolchain comes before everything and is unnamed, and the core's "real sync bytes" proof needs a server that the plan schedules in parallel (finding 6).

All of it can be fixed in the plan with small edits. Three user decisions actually block progress; they are listed at the end.

## Findings

### 1. The checkpoint has no deployment or trust profile, so hosted-grade isolation is on the path to first use

- **Severity:** critical
- **Classification:** user decision (with a recommended default), then fix-in-plan
- **Evidence:**
  - Plan:34 defines the checkpoint by features only. Plan:36 says a credential or isolation failure "prevents the affected deployment or shared-account capability from shipping" but never names the deployment the checkpoint targets.
  - `validation-plan.md:10` (stage 1A) requires escape tests to pass "in the actual hosted and self-hosted deployment shapes".
  - `server-host-assessment.md:72` and `:202` record that nested namespaces, Landlock and seccomp inside rootless Podman are unverified and "may force a deployment change".
  - `architecture-proposal.md:192` ties sandbox strength to "public signup plus AI-generated server-side code".
- **Consequence:** read literally, the owner cannot use the build until a mutually-hostile-tenant sandbox is proven in the hosted shape. That is the largest unknown in the project, and it protects against tenants the first build does not have.
- **Recommended change:** add one paragraph to the plan declaring the checkpoint profile:
  - owner-operated, invite-only deployment
  - every account is someone the owner trusts
  - each person uses only their own provider accounts
  - no public signup, no shared-account cals
  
  Keep the structure that prevents a later rewrite: plugins out of process, capability RPC only, host-stamped caller and space, space-keyed storage, and the two-account isolation gate. Move the full hostile-workload escape matrix and worker-density budgets to a release gate for calternal.cloud public signup.
- **Acceptance proof:**
  - The plan names the profile and lists what it forbids.
  - The ported isolation gate (`crates/calternal/src/isolation.rs` at the pinned revision, already cited in `server-host-assessment.md:71`) passes for two accounts, two spaces and one shared resource, with worker-hosted actions included.
  - A worker has no ambient host path, network or credential, checked in the self-hosted shape only.
  - The deployment refuses public signup and shared-account binding while the hosted gate is open.

### 2. GenUI v0 has no producer or evaluation model, and the Notes editor contradicts "no privileged shortcuts"

- **Severity:** high
- **Classification:** fix-in-plan, plus one engineering experiment
- **Evidence:**
  - Plan:12 names tree, node IDs, tokens, bindings, form state and actions. Plan:19–24 add primitives and rules. None says who emits a tree, when, or where bindings are resolved.
  - Plan:11 requires Notes to use "the same public SDK as a generated plugin … without privileged shortcuts". Plan:19 and :23 put editors in "versioned capabilities" shipped in client releases.
  - `architecture-proposal.md:37` requires native editors (TextKit 2 on iOS), which no primitive catalog can express.
  - `architecture-proposal.md:190` already concedes that a cold tenant worker cannot meet an interactive budget.
  - No GenUI or declarative-view implementation exists in the pinned source. The closest precedent is ADR 0055 (settings surfaces derived from typed declarations).
- **Consequence:**
  - There are three different producers with different trust and latency: views shipped in a plugin bundle, trees returned by a worker per request, and views a model emits in a reply. The Swift renderer, offline behaviour and patch protocol differ for each, so native work started now would be built on a guess.
  - If rendering a view needs a worker round trip, app switching inherits worker cold start.
  - Without a stated rule, the Notes editor is either a privileged shortcut or impossible.
- **Recommended change:** add four sentences to the plan's GenUI section.
  1. **Producer for v0:** view definitions are static, versioned files in the plugin bundle, cached by clients per plugin version. Model-emitted reply views use the same schema and are validated by the host; they add no second format.
  2. **Bindings:** a binding names a registered query operation plus field paths. There is no expression language in v0. The renderer resolves bindings on the client through the ordinary operation path, with paging declared in the binding.
  3. **Client-shipped components:** the catalog has two tiers, primitives and capability components such as `notes.editor`. Any plugin may reference a capability component by ID and version, for a resource kind that component supports, with the plugin's own grants. That is what makes the Notes editor a public contract, not a shortcut.
  4. **Updates:** one keyed patch format for streaming and live data, coalesced at most once per frame by the renderer.
- **Acceptance proof:**
  - One schema file and one fixture corpus (valid tree, unknown optional node, unsupported required capability, each of loading, error, empty and pending) passes in the browser renderer and a Swift renderer.
  - A generated plugin that did not ship with the client embeds `notes.editor` for a note it is granted, and is refused for one it is not.
  - A plugin view opens with the plugin's worker stopped, from cache, and offline in the Swift harness.
  - Experiment: measure tree validation plus first layout of a representative view on the reference devices before the catalog grows beyond the first screens.

### 3. The plan does not say which Notes paths bypass the TypeScript product layer

- **Severity:** high
- **Classification:** fix-in-plan, engineering experiment (V1), release gate (regression rule)
- **Evidence:**
  - Plan:11 has Notes built on the public SDK. `server-host-assessment.md:29–31` splits a compiled Rust engine from a TypeScript product layer in a worker.
  - `server-host-assessment.md:129` says worker overhead "has not been measured" and "dominates user-visible latency for plugin-backed operations".
  - `architecture-proposal.md:453` makes any result worse than existing Calternal on a shared scenario a regression. The existing gates include `open.note` and `sync.save_ack` (`server-host-assessment.md:223`).
  - Plan:21 keeps typing and scrolling off server round trips but is silent on open, list, search and save acknowledgement.
- **Consequence:** if `notes.open` or the note list is a TypeScript operation composing engine calls, the first note opened after an idle period pays a worker cold start. The existing product has no such cost, so the checkpoint would regress an inherited gate on the first thing the user does.
- **Recommended change:** add a hot-path rule to the plan. Clients call engine operations directly for open, sync, save acknowledgement, list and palette search. The TypeScript product layer supplies view definitions, composed and occasional operations, and tool-call surfaces for threads. A plugin that puts a worker on an interactive path declares it and carries its own budget. Engine replacement (V6) stays out of the checkpoint, as `validation-plan.md:14` already allows.
- **Acceptance proof:**
  - Trace IDs for open note, typing with sync in flight, and save acknowledgement show no worker hop.
  - Those scenarios are compared with the `origin/dev` baseline on the same machine once stage 0 budgets exist. V1 reports cold-start distributions for the paths that do use a worker.

### 4. Execution v0 leaves the first computer ambiguous and omits the adapter that lets a thread call operations

- **Severity:** high
- **Classification:** fix-in-plan; one staged omission needs the user's acknowledgement (folded into decision A)
- **Evidence:**
  - Plan:34 requires "selected-computer execution" and "one real worktree/PR path". Plan:36 says "enrollment of a second machine follows the first end-to-end run" without saying what the first machine is.
  - Plan:15 lists "Codex/Claude adapters, machine/run/account binding" but not the MCP or `cl` adapter, the run-scoped credential, or operation discovery. `architecture-proposal.md:298` makes those the only way a thread reaches operations.
  - `architecture-proposal.md:367` accepts managed provider authentication and attaches a validation spike (`validation-plan.md:85`). `architecture-proposal.md:304–306` gates shared-account use on an unproven credential mechanism.
  - `server-host-assessment.md:119` quotes Anthropic's restriction on offering claude.ai login in third-party products.
  - `architecture-proposal.md:344` defines a working-area checkout and write-back path that the plan neither includes nor excludes.
- **Consequence:**
  - If the first computer is the server, harness sandboxing inside rootless Podman and managed login become prerequisites for the first coding task.
  - Without the MCP adapter in the artifact list, "a thread directs a Notes edit" has no stated mechanism.
- **Recommended change:**
  - The checkpoint runs harnesses only on the owner's enrolled Mac or Linux machine through the `calternal` machine role, private to its owner, using CLI logins already present there. `architecture-proposal.md:134` already has the daemon discover existing harness tools.
  - Server-side harness execution and managed login are named as staged omissions, not dropped commitments.
  - Add three artifacts to the Execution v0 row: the MCP adapter, the run-scoped short-lived credential, and discovery/search operations.
  - Notes edits from a thread go through operations in the checkpoint. File checkout and write-back for canonical content is deferred. The worktree/PR path covers machine-owned project files only.
- **Acceptance proof:**
  - One thread performs a Notes edit through the MCP adapter. The credential stops working when the run ends and after Stop.
  - The same thread completes a worktree task ending in a PR on the enrolled machine and survives closing the client.
  - Revoking or unpairing the machine blocks the next dispatch. A second account in the deployment cannot select that machine.
  - The model-directed-tool credential test (`architecture-proposal.md:304`) is run and recorded for the owner's own account, as that section requires even for single-holder use.

### 5. Persistence v0 does not state the storage guarantee the checkpoint gives, or what happens to data written into an early build

- **Severity:** high
- **Classification:** user decision, then fix-in-plan
- **Evidence:**
  - Plan:14 lists commit rules, chat format, artifact ownership, recovery and attachment identity as one artifact. Plan:38 mentions "disclosed storage/recovery limitations" without listing any.
  - `inheritance-audit.md:128` and `:131`: reusing the two-root binary is "a staged limitation", and "a reduced launch recovery or tiering promise needs an explicit product decision; no such reduction is approved here".
  - `architecture-proposal.md:539` asks for a focused storage compatibility review before inheritance is claimed.
  - `architecture-proposal.md:338` lists five stores that must be backed up as one set. The store for authority and run records is unchosen (`architecture-proposal.md:489`).
  - The chat backend is a recorded decision, not code: no `AI/Chats` reference exists under `crates`, `apps/web/src` or `packages` at the pinned revision.
- **Consequence:**
  - The owner will put real notes and conversations into a build whose layout and recovery behaviour are undeclared. A later change to space roots or journal partitioning then becomes a migration of live data with no stated promise.
  - Tiering and FO1 (ADR 0062) either block the checkpoint or are silently skipped.
- **Recommended change:**
  - Ask the user decision B below.
  - Whatever the answer, fix three things in the checkpoint: the space-rooted relative layout (`AI/Chats`, `AI/Cals`, app roots, `.calternal/`), a format version on every protected record, and a commitment to migrate checkpoint data forward.
  - Pick one transactional store for grants, admission and run records in the first slice. The existing per-Home SQLite journal with idempotency records committed in one transaction is the natural extension (`server-host-assessment.md:82`, `:123`).
  - Ship one backup command that snapshots all stores together before first use.
  - Stream to the Markdown chat file at checkpoints and at the final message only, as `architecture-proposal.md:383` already states.
- **Acceptance proof:**
  - Kill the server mid-edit and mid-turn: no acknowledged edit is lost within the declared guarantee, and no message shows as saved before its acknowledgement.
  - Restore from the combined backup and confirm that a share revoked before the backup stays revoked.
  - Upgrade a checkpoint data directory across one deliberate layout or format change with an automated migration test.
  - An external edit to a chat Markdown file imports as an attributed revision and cannot forge an approval or receipt.

### 6. The critical path hides two dependencies and puts the whole performance specification in front of everything

- **Severity:** medium
- **Classification:** fix-in-plan
- **Evidence:**
  - Plan:42 makes "capture performance baselines and define contracts/fixtures" a single first step. `validation-plan.md:9` requires hosts, devices, noise bands and budgets pinned "before running". `server-host-assessment.md:278` notes the benchmark host allocation is "still to be confirmed", and `:352` leaves the device list and budgets unspecified.
  - Plan:43 and `validation-plan.md:16` run 1A, 1B and 1C in parallel. 1C requires "real sync bytes" (`validation-plan.md:12`), which needs a server speaking the sync protocol.
  - Plan:28 and `validation-plan.md:43` start Apple work "against generated types and fixtures". No contract source of truth or generator is named for Swift. The only existing mechanism is Rust `schemars` → `contracts/actions.json` → generated TypeScript, with a drift test (`crates/calternal/src/plugins.rs:6`, `:114–123` at the pinned revision).
- **Consequence:** the first week stalls on a device list and a benchmark host, and the Swift bridge spike waits for space re-keying or silently substitutes a JSON demo, which plan:30 forbids.
- **Recommended change:**
  - Split step 1 into 1a and 1b. Step 1a is the contract toolchain and fixture revision 0, and it blocks everything. Step 1b is the performance specification and the `origin/dev` baseline; it runs in parallel and blocks only pass/fail judgements, while spikes record raw traces from the start.
  - Name the toolchain in 1a. Recommended: Rust types are the source of truth; JSON Schema and TypeScript are generated as today; Swift types for known contracts come through UniFFI; dynamic plugin operations and GenUI trees travel as schema-validated generic values.
  - Let 1C run its first bridge spike against the unmodified `origin/dev` server for the per-note sync protocol (y-protocols plus the 100/101 save check, `packages/editor/src/sync/protocol.ts:9–28`). Rebase it onto the space-keyed server when 1B lands. Journal and cursor fixtures wait for 1B.
- **Acceptance proof:**
  - One contract change regenerates Rust, TypeScript and Swift bindings, and the drift test fails when a snapshot is stale.
  - The Swift harness opens, edits offline and reconnects one real note against a real server before 1B completes, then passes the same fixtures against the space-keyed server.
  - The step 1b specification is committed before any result is labelled pass or fail.

### 7. Apple work is said to start "immediately" without a first platform, and the GenUI renderer multiplies across toolkits

- **Severity:** medium
- **Classification:** user decision, then fix-in-plan
- **Evidence:**
  - Plan:30 defers "UIKit/TextKit 2 iOS, the existing macOS toolkit validation and watchOS-specific UI" as "separate platform work", yet plan:12 requires a view to render in "native Swift".
  - `architecture-proposal.md:37` and `inheritance-audit.md:137` record three toolkits (UIKit, AppKit pending the Catalyst comparison, SwiftUI on watchOS). ADR 0072 states it "does not choose the macOS UI toolkit".
  - Neither document names the first platform. ADR 0072 also requires files, not a device database, to hold content on each device, which the Shared client core row (plan:13) does not mention.
- **Consequence:** a GenUI renderer has to be written per toolkit. Starting without a target either produces a throwaway SwiftUI demo or blocks on the Mac toolkit spike.
- **Recommended change:**
  - Ask decision C below.
  - Name the day-zero Apple work that needs no contracts: project and signing setup, the TextKit 2 Markdown editor prototype on a fixture note, native equivalents of the first primitives against design tokens, and a frame/hitch measurement harness on the reference device.
  - Functional Notes starts when finding 6's bridge spike passes. The GenUI renderer starts when finding 2's schema and fixtures exist.
  - State that the native Notes slice keeps content as files with a rebuildable index, per ADR 0072.
  - Keep App Store AGPL review, File Provider, multi-process state and the support window as release gates, as `validation-plan.md:47` already does.
- **Acceptance proof:**
  - The chosen platform's app, installed on the owner's device through development signing, opens and edits a real note through the core and renders one GenUI fixture view with VoiceOver names.
  - Hitch and input-to-paint traces are recorded on that device with its measured refresh rate.

### 8. SDK v0 and the checkpoint surface list are broader than the first proof needs, and vague where it matters

- **Severity:** medium
- **Classification:** fix-in-plan
- **Evidence:**
  - Plan:11 puts dependencies, extension-point IDs, lifecycle and "versioned migration contracts" in the minimum artifact, and "rollback" in the proof. `architecture-proposal.md:208` says migration rollback "needs a real compatibility/snapshot strategy", which does not exist.
  - Plan:34 says "recovery" without saying whether it means crash recovery or the protected recovery screen (`architecture-proposal.md:92`). It omits the palette, deep links and the right AI pane, all inherited contracts (`architecture-proposal.md:35`, `:53`, `:61`).
  - Plan:34 requires "two-account sharing" without saying whether sharing UX is in scope.
  - `validation-plan.md:103` already requires one independently installed Notes extension in the first slice.
- **Consequence:** the SDK row can absorb unbounded work (hooks, service replacement, a migration framework), while reviewers cannot tell whether the checkpoint is complete.
- **Recommended change:**
  - **SDK v0 cut:** manifest, API version, scope, capability requests, operations, resource kinds, events, activation triggers, view contributions, and exactly one extension point (a Notes action or side view). A v0 plugin may not declare a data migration; activation refuses one until the snapshot mechanism exists, and rollback means code and configuration only. Hooks, service-slot replacement and engine contracts are later additions to the same manifest version scheme.
  - **Checkpoint surface, one explicit table.**
    - In: one sidebar, conversation view and the optional pane on one thread, Notes, palette reusing the existing implementation, minimal Activity, Stop, stable deep links for notes and chats, the protected recovery screen in its minimal form, and crash recovery.
    - Proven at the gate, not as UX: two-account isolation and one shared resource.
    - Named as deferred: invite and sharing UI, cals, automations, notifications, Collections, group conversations.
- **Acceptance proof:**
  - A cal-generated plugin and the Notes product layer validate against the same manifest schema and contract snapshot.
  - A manifest that declares a migration is refused with a clear reason.
  - Disabling Notes makes its extension unavailable with a stated reason and leaves the files readable.
  - A broken personal plugin can be disabled from the recovery screen while the shell fails to load.

## Already covered: no new mechanism needed

These are adequately specified in the cross-checked documents. The plan should link to them, not restate them.

- Durable admission, invocation generations, late-commit rejection, outcome-unknown and crash-loop budgets: `architecture-proposal.md:378–391`, fault matrix at `validation-plan.md:26–37`.
- No false saved acknowledgement: `architecture-proposal.md:433`, with the existing 100/101 protocol at the pinned revision.
- UniFFI limits and explicit cancellation: `architecture-proposal.md:239`, `server-host-assessment.md:140–147`.
- Browser multi-tab outbox ownership and offline reuse baseline: `architecture-proposal.md:233–235`.
- 120 Hz as a measured target with distributions, device records and a regression rule: `architecture-proposal.md:426–456`, `server-host-assessment.md:217–270`.
- Effective-authority intersection and the cal-cannot-self-grant rule: `architecture-proposal.md:283–296`, `:208`.
- Multi-machine partition and lease behaviour: `architecture-proposal.md:128`, V7 at `validation-plan.md:57–63`. Only single-machine use is needed at the checkpoint.

## Smallest viable dependency order

1. **Decisions A, B and C** (below), plus the plan edits from findings 1, 2, 3, 4 and 8. These are documents only.
2. **Contract toolchain and fixture revision 0** (finding 6, step 1a): operation, error, event and resource envelopes, the GenUI schema, and capability negotiation. In parallel and not blocking: the performance specification and `origin/dev` baseline (step 1b).
3. **Three parallel tracks on that contract revision:**
   - Server: space re-keying of engine and store with the two-account gate (V2), then a worker process with capability RPC in the self-hosted shape (the reduced V1).
   - Core: Rust client core plus the Swift bridge spike, first against `origin/dev` sync, then the space-keyed server.
   - Web: shell, GenUI renderer and TypeScript client on the fixtures, reusing the existing editor and offline code.
4. **Integrate:** Notes on engine hot paths plus the product layer, conversations stored as Markdown, one generated plugin with one Notes extension, and the MCP adapter with a run-scoped credential.
5. **Execution:** machine role on the owner's computer, Codex and Claude adapters, one Notes edit and one worktree/PR task, with Stop, crash and restart reconciliation.
6. **Checkpoint gates:** isolation, recovery and backup, and responsiveness against the committed budgets. The owner starts using it here.
7. **After first use:** second machine (V7), managed provider login, server-side harness sandbox, the hosted hostile-tenant gate, and the remaining full-product integrations.

### When Apple can begin

- **Now, after decision C:** project setup, editor prototype, native primitives and the frame measurement harness. None depends on a contract.
- **After step 2:** the GenUI renderer against the schema and fixtures, and the Swift bridge spike.
- **After the bridge spike passes on real sync bytes:** functional native Notes and conversation screens. This does not wait for step 4 or for any plugin beyond the generic operation path.

## What can proceed without more grilling

- The contract toolchain choice, fixture corpus, GenUI schema details and catalog contents.
- Space re-keying, the transactional store for authority and run records, and the worker RPC shape.
- Sandbox mechanism selection for the self-hosted shape, and the hot-path rule.
- The MCP adapter and run-scoped credentials.
- The performance specification's metric list, fixtures and noise bands, and the `origin/dev` baseline capture.
- The SDK v0 cut and the checkpoint surface table. These are scoping edits inside decisions already made.

## User decisions that actually block progress

- **A. Checkpoint deployment and trust profile** (findings 1 and 4). Recommended: owner-operated, invite-only, trusted accounts, each person's own provider logins on their own enrolled computer. Hosted public signup, shared-account cals, managed provider login and server-side harness execution follow as gated increments. The alternative is first use on calternal.cloud, which puts the hostile-tenant sandbox and credential gates in front of first use.
- **B. Storage guarantee for the checkpoint** (finding 5). Either accept the existing two-root storage with a stated recovery limit, a combined backup and a forward-migration commitment, or require the ADR 0062 tiering and FO1 proofs before first use. The inheritance audit says this reduction needs the user's explicit approval. A related yes/no: must existing Calternal notes be imported before the checkpoint counts as usable?
- **C. First Apple platform and reference devices** (findings 6 and 7). iPhone with UIKit and TextKit 2, or Mac, which also needs the pending Catalyst-versus-AppKit comparison. The earlier records disagree (Mac and CLI first in September, iOS as the main place of use in October, per recalled notes and not verified in source), and the plan does not settle it. The same answer supplies the 120 Hz reference devices that the performance specification needs.

Not blocking: the version support window (already deferred), numeric budgets (team-owned, set before judging results), engine fork depth (resolved), and the takeover idle interval (tuning).
