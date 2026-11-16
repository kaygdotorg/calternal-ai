# Independent early-use plan review — Sol

2026-11-16. Reviewer assignment: GPT-6.1 Sol medium. Reviewed the six requested documents independently, without opening another review. This is a pre-implementation document/source review: no application, sandbox, benchmark or device behavior was exercised. Only this report and its JSON companion were written.

The direction is viable: one Notes/conversation/plugin/core slice is a better dependency boundary than building complete frameworks. The approval for an intermediate usable build and portable GenUI is clear. No arbitrary HTML/JavaScript surfaces, browser Wasm, substitute harness or reduced orchestrator is needed. The fixes below make the slice implementable and its release claims reviewable; they do not reopen the accepted architecture.

Already covered and not findings: durable admission and replay classes, protected commit and late-result fencing, current-grant checks, two-account sharing proofs, offline browser reuse and multi-tab ownership, explicit cancellation, native effects, capability negotiation, attachment identity work, unknown-node behavior, accessibility, recovery UI and measured performance gates. Their implementation is still required. Exact SDK schemas being unwritten in a planning repository is not itself a failure; the findings identify specific contract decisions that the listed artifacts could otherwise omit.

Severity: **high** threatens integrity, credential/privacy boundaries or usability of the first real app; **medium** risks avoidable delay or compatibility failures. Classification gives the primary disposition, not permission to bypass applicable release gates.

## Findings

### 1. The checkpoint lacks one authoritative scope and dependency table

**Medium · fix-in-plan**

**Evidence:** early-use-plan.md, “Approved first personal-use checkpoint” and “Critical path,” lines 34–45; validation-plan.md, stages, lines 9–16 and V7 lines 59–63; architecture-proposal.md §12, lines 470–475.

**Consequence:** The checkpoint requires selected-computer execution and a real worktree/PR path, but line 36 places second-machine enrollment after the first run and line 45 defers expansion. Validation V7 requires Mac and Linux daemons before remote execution ships (validation-plan.md:59-63). Architecture §12 still lists the full integration set without an explicit intermediate exit (architecture-proposal.md:470-475). It is unclear which machine, PR lifecycle and integration proofs gate personal use. Stage 0 also risks becoming a complete future-device/performance program before any contract work (validation-plan.md:9,16).

**Change:** Add a checkpoint matrix naming required behavior, proof and deferred breadth; distinguish first executable run from first usable release. Keep the common machine/run protocol now, integrate one selected-machine worktree/PR path, and pass V7 before enabling remote execution; do not require a fleet UI. Explicitly put Honcho/voice/feedback/import after this checkpoint. Capture baseline and predeclare budgets for the first slice while contracts/native scaffolding start; expand device/load coverage with implemented surfaces, preserving applicable inherited gates.

**Acceptance proof:** A reviewer can trace each checkpoint item to a named acceptance scenario and identify exactly where V7 and PR follow-through/cleanup apply. No deferred integration is accidentally required to reach the intermediate exit; no accepted machine safety gate disappears.

### 2. Intermediate-use approval does not resolve inherited storage guarantees

**High · user decision**

**Evidence:** early-use-plan.md:38; inheritance-audit.md:9–11,25–28; validation-plan.md:69; architecture-proposal.md:537–539. Pinned source ADR 0062:10–15,84–163,255–289,359–367 (see source pin below).

**Consequence:** The plan says storage/recovery limitations require evidence but never states what the usable build promises. Inheritance-audit.md:9-11,25-28 retains tiering, FO1 materialization and protected file-only recovery, explicitly saying no reduction is approved. Validation-plan.md:69 makes these proofs conditional on claims. Pinned ADR 0062 distinguishes the old two-root implementation from the accepted amended design. Approval to defer integrations is not approval to omit storage semantics.

**Change:** Before enabling real-data personal use, publish an exact storage/recovery profile mapped to ADR 0062 and the space adaptation. Default to inherited guarantees. If the team proposes an earlier build with a coordinated-backup-only recovery profile or without tiering/FO1, present that concrete temporary limitation for user approval; disclosure alone is insufficient. Continue namespace, journal and client work meanwhile. Do not build a replacement storage engine solely to settle this scope question.

**Acceptance proof:** A written profile lists acknowledgement, materialization, state-loss recovery, backup and missing-mount behavior; it either passes inherited fault cases or cites explicit approval of the exact temporary deviation before real-data use.

### 3. Portable GenUI omits the editor contract needed by its first app

**High · fix-in-plan**

**Evidence:** early-use-plan.md:13,19,28–34; architecture-proposal.md:234–239; inheritance-audit.md:65. Pinned ADR 0018:7–15 makes Markdown text the shared document and identifies prior converter corruption.

**Consequence:** The initial catalog places editors among capabilities to add later, yet the checkpoint promises real Notes and browser/Swift open-edit-offline parity (lines 13,28-34). Architecture-proposal.md:234 retains the browser live Yjs document; inheritance-audit.md:65 retains collaborative Markdown text. A generic form value or Markdown display is not that editor. Native full-buffer replacement and server-round-trip input would undermine co-editing, undo and responsiveness.

**Change:** Specify one built-in document-editor capability for v0: canonical resource/session identity, collaborative text/update format, local composition/selection ownership, origin-aware undo, pending/conflict states and read-only fallback. Reuse the web editor; define the narrow TextKit 2-to-core edit/update interface concurrently. GenUI mounts the trusted native/web editor by resource reference, without embedding executable UI or serializing the document into the view tree. Keep richer editor customization incremental.

**Acceptance proof:** Browser and Swift edit the same note concurrently and offline, including IME input, remote changes and own-change undo, then reconnect with identical text, preserved unknown content and honest durable-save state. The editor is mounted through the public capability with keyboard/VoiceOver semantics.

### 4. Declarative views need enforceable client safety limits

**High · fix-in-plan**

**Evidence:** early-use-plan.md:12,20–24; architecture-proposal.md:78,87,94,198,310,431–432. These already require validation, trusted surfaces and coarse bounded updates; the missing item is the executable rejection/effect/revision policy.

**Consequence:** The plan already calls for bounded bindings, backpressure and host-authorized actions (lines 12,20-24). It does not specify what the renderer rejects or how client-originated network and navigation effects are authorized. A data-only tree can still exhaust layout/memory, issue excessive queries or disclose context through remote Markdown images/links; removing JavaScript does not constrain those effects. Keyed patches also lack revision/gap/reset rules.

**Change:** Add a small renderer profile: validated inert Markdown, resource-handle media, explicit external-link behavior, no arbitrary URL fetches from bindings/styles, tree/depth/text/page/subscription/update quotas, and bounded layout values. Use subscription identity plus ordered view revisions; reject stale patches and request a bounded fresh snapshot after a gap. On permission loss detach bindings and clear protected data. Limits are host/client-enforced and negotiated; allow unsupported views to fail visibly rather than freeze.

**Acceptance proof:** The same hostile view corpus in browser and Swift rejects excessive trees/bindings and unsafe content/URLs; reordered, duplicated and missing patches recover without stale state or unbounded buffering. Typing/scrolling remains within predeclared budgets under sustained updates.

### 5. Preview isolation is promised without a side-effect boundary

**High · fix-in-plan**

**Evidence:** early-use-plan.md:11; architecture-proposal.md:110,204–208; server-host-assessment.md:318–322. Isolation is an intention, not yet a preview storage/effect mechanism.

**Consequence:** The authoring loop previews inside the user space; architecture-proposal.md:110 promises an isolated preview and :208 grants migrations/recovery rules, while early-use-plan.md:11 tests preview and activation. Neither defines whether preview can mutate canonical Notes, run jobs, use live accounts or migrate production data. Existing grants alone would let an automatically generated preview change real content before activation.

**Change:** Make preview a separate installation identity/version with a disposable data overlay or fixture resources, no production migrations, schedules or external-effect/account handles by default. Existing grants bound maximum authority but do not silently bind preview to live effects. A deliberately authorized live test uses an explicit operation/context with attributable outcomes. Activation selects a verified immutable bundle/schema/config revision and applies its declared migration once; it does not promote arbitrary preview effects.

**Acceptance proof:** A preview that attempts a canonical write, migration, scheduled task or external request cannot affect production. Discard removes preview state; activation uses the tested revision. The generated plugin and Notes extension use this same path.

### 6. Retained worker data needs authority-change invalidation

**High · engineering experiment**

**Evidence:** server-host-assessment.md:61–70,287–291; architecture-proposal.md:104,285–296; validation-plan.md:31,37. Current-authority checks exist; retained-state invalidation is the additional case.

**Consequence:** Isolation domains include effective authority and each RPC checks current grants; both are good existing controls. V1 tests different callers/contexts (lines 287-291), but does not explicitly cover a caller losing a narrow resource share while the same worker retains its bytes. After revocation a read check can deny new access while an existing cache, subscription or in-flight result still discloses old content through an otherwise allowed operation. Static domain keys do not establish invalidation semantics.

**Change:** Include an authority/visibility generation in worker and subscription lifetime. For v0, conservatively kill/recreate affected domains, cancel subscriptions and reject stale result generations when relevant grants change; do not require per-object cache taint tracking. Also recheck delivery/commit authority. Extend V1 with same-principal share revocation, membership loss and account-unbinding during cached and in-flight calls; bound cleanup and prevent reuse of old temporary state.

**Acceptance proof:** After the protected revocation boundary, a warmed worker cannot return or stream revoked bytes to the caller; late results are rejected and old processes/temp state cannot re-enter service. Test the same user before and after revocation, not only two initially different users.

### 7. Make the personal-use credential gate explicit and move its proof forward

**High · release gate**

**Evidence:** early-use-plan.md:36,43; architecture-proposal.md:302–306,367,491,503; validation-plan.md:83–85; server-host-assessment.md:119. The underlying requirement is already present; its checkpoint applicability and timing need alignment.

**Consequence:** Credential/isolation failure is phrased as blocking the affected deployment or shared-account capability, which could be read as allowing unprotected private-account use. Architecture-proposal.md:304-306 explicitly requires that model-directed tools cannot read credentials and says the injection test also applies to the holder's own threads. Direct login-file management is accepted (:367), but does not itself isolate those files. The managed-auth spike is listed separately (validation-plan.md:83-85), so failure could arrive after building both integrations.

**Change:** Name each offered provider/account/deployment mode in an early gate table. Run real login/refresh plus malicious shell/file-read/egress tests alongside V1, before substantial provider UX; record current distribution/use terms for that exact route. Private use is not an exemption from injection protection. Select the smallest supported mechanism that passes; neither a mandatory proxy fork nor substituting another harness is authorized by this plan. A failed mode stays disabled and is reported as a checkpoint blocker if required.

**Acceptance proof:** For every enabled Codex/Claude account mode, retained evidence shows model-directed tools cannot recover login/refresh secrets via files, environment, process interfaces, logs or allowed outbound requests. Rotation/restart/unpairing and applicable terms are checked. No result is assumed here.

### 8. Engine replacement needs a codec identity the native core can actually compare

**Medium · fix-in-plan**

**Evidence:** architecture-proposal.md:243–249; early-use-plan.md:13,28; validation-plan.md:14,36. Pinned ADR 0048:7,19–23 supports safe opaque fallback; no new engine framework is requested.

**Consequence:** The fallback for tenant-defined kinds is already specified: preserve opaque bytes and let the server merge. But the phrase default kinds it still matches has no concrete engine/codec compatibility signal. Operation and GenUI API negotiation alone cannot tell an older native client whether its compiled Notes codec and offline edits remain safe after a space changes its engine. Queued edits can span that change.

**Change:** Add a resource-format/codec compatibility identifier and revision to resource/session negotiation, separate from plugin version and view capabilities. The server declares supported edit encodings; the core selects its compiled codec only on an explicit compatible match. Otherwise preserve opaque bytes or use read-only/plain-file fallback as supported. Revalidate queued edits after engine change and surface a conflict/rebase requirement instead of decoding or overwriting optimistically. Add one replacement-between-offline-edit-and-reconnect fixture; full replacement-engine implementation can remain V6.

**Acceptance proof:** An old Swift/core client opening an incompatible engine preserves bytes and clearly disables unsupported edits; compatible codecs keep working. Pending edits created under the old codec survive as recoverable work and cannot silently overwrite the replacement format.

## Smallest viable dependency order and Apple start

1. **Freeze one narrow contract revision and release profile.** Define resource/account/space IDs, the Notes edit/session format, operation context and error/cancel semantics, journal/outbox acknowledgements, generic operation discovery and the small GenUI/editor profile. Include a generated plugin manifest and one Notes extension point. Write the corresponding fixtures and baseline/budget specification. This is the start of executable contracts, not a complete SDK catalog or complete cross-app platform.
2. **Run the discriminating work concurrently.** Adapt one real Notes store to spaces with two-account sharing; prove one bounded sandboxed TS worker and each required harness credential mode; implement the Rust state/effect core plus minimal async UniFFI harness and adapted TS browser sync. Use the same revision and real collaborative bytes. Node/Rust are the working recommendations; Deno or a host rewrite is conditional on evidence, not another full implementation to finish first. Existing-code baselines can be captured alongside this work before comparisons; new-slice budgets must be declared before its measurements.
3. **Integrate the vertical slice.** Foundation-backed sign-in and a protected recovery entry; one sidebar; Notes plus durable Markdown conversations; both required CLI adapters through the public operation route; one generated plugin and one Notes subplugin; minimal Activity/Stop/history; offline pending states; preview/activation/revocation/recovery. Run applicable integrity, authorization and responsiveness tests as each path exists.
4. **Complete the checkpoint's selected-computer coding path.** Use the common machine protocol and one isolated worktree/PR workflow with retained artifacts and clear review state. Prove the Mac/Linux two-daemon V7 cases before exposing remote execution, even if only one computer is used day to day. Include applicable accepted check-monitoring and safe cleanup behavior; do not confuse a one-off PR creation demo with the whole accepted workflow. No general fleet/provisioning UI, persistent terminal product or browser/device automation platform is needed for this proof.
5. **Enable personal use only after its applicable gates pass.** Resolve the storage profile, real-account credential gates, recovery/isolation and measured first-slice performance. Incrementally add the full-product integrations and replacement-engine breadth afterwards. This changes delivery order, not the accepted end product.

**Apple can begin at step 1.** Native shell/navigation/component prototypes and generated-type integration can start immediately. Functional TextKit Notes work starts concurrently with the first real Rust core/UniFFI slice in step 2, with its text-edit interface settled as part of that slice. A nonfunctional JSON renderer is not a sync proof. Neither all plugins, V6, Honcho, voice, BB import nor a finished web product is a prerequisite. App Store support-window/distribution commitments and full extension/File Provider coverage gate the relevant native release, rather than the first local harness. Keep the inherited UIKit/TextKit 2 direction and AppKit baseline; do not make a macOS toolkit comparison a prerequisite to all Apple work.

## Source pin and review limits

Inherited source claims above were checked using read-only `git show` in `/home/kayg/Developer/calternal` at **`ea57b84b9c9e40e8b67b7d1e1f1805915123b040`**:

- `docs/adr/0018-the-markdown-text-is-the-document.md`, lines 7–15: one collaborative Markdown text, no converter save path.
- `docs/adr/0048-format-plugins-run-on-server-and-clients.md`, lines 7,19–23: shared format implementation, preserve unknown bytes and defer unsupported merges.
- `docs/adr/0062-user-files-on-hdd-app-data-on-ssd.md`, lines 10–15,84–163,255–289,359–367: old implementation versus amended tiering/recovery obligations, explicit migration and materialization/recovery proofs. These are specifications, not claims that this revision implements them. Historical machine names in that ADR are not assumed to identify currently available test hosts.

The other inherited behaviors are cross-references to the requested inheritance audit, not independently certified implementation claims. Recent explicit user decisions take precedence. No other review was read; no deadlines or benchmark results are inferred.

## Verdict and unresolved user decisions

**Proceed with narrow contracts, discriminating experiments and concurrent Apple work; revise the checkpoint gates before broad implementation or real-data use.** The plan has the right architecture but leaves enough ambiguity to build a convincing demo with unsafe preview/state behavior or no real portable Notes editor.

No further grilling blocks contracts, source adaptation, GenUI/editor work, sandbox/auth experiments or the native harness. Scope rows, renderer limits, codec negotiation, worker invalidation, preview overlays, test-device selection and metric budgets are engineering responsibilities. Provider compliance/isolation is an evidence gate, not a preference question.

The actual potentially blocking user choice is **whether to accept a specifically described temporary storage/recovery deviation if the team proposes one for early personal use**. Without that proposal, inherit the existing guarantees; intermediate-integration approval does not waive them. The exact native client/server support duration is already explicitly deferred to native release planning. The takeover inactivity interval is later UX tuning. Neither blocks the first slice. No other unresolved user choice was established by this review.
