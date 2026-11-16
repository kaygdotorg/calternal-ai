# calternal-ai: validation and native readiness

Date: 2026-11-16. Status: planned experiments, not benchmark results. No application implementation or benchmark runs are claimed. This refines the [proposal](architecture-proposal.md) and the host assessment's [V1–V6](server-host-assessment.md#7-small-discriminating-validation-plan).

## Order and exit criteria

| Stage | Experiment | Required evidence / exit |
| --- | --- | --- |
| 0 | Establish performance specification and original Calternal baseline | Pin source revisions, fixtures, designated hosts/devices, OS/browser/runtime versions, power/thermal conditions, actual refresh rate, noise bands and budgets before running. Reuse existing server/browser budgets; add worker and native budgets. |
| 1A — V1 | Rust host stub with isolated Node worker; Deno comparison | Unauthorized filesystem/network/credential access and cross-account/space state leaks fail closed in the actual hosted and self-hosted deployment shapes. Cold/warm/batched RPC, lifecycle, RSS, idle CPU and worker density meet predefined budgets. Choose pooling only after authority-switch leak tests pass. |
| 1B — V2 | Re-key existing Rust engine/store to spaces | Two accounts, two spaces and a shared canonical resource pass permission, sync and revocation tests. Compare 10k/100k-note server fixtures with the same-machine origin/dev baseline; record actual adaptation effort and regressions. Reconsider host choice if reuse fails its economic/performance case. |
| 1C — V5 | Swift → UniFFI → Rust and TypeScript browser contract slice | One note opens, edits offline, reconnects and resolves conflicts using real sync bytes. Shared operation/error/event fixtures produce equivalent specified outcomes. No lost edits, falsely confirmed save or account-local state leakage. Explicit cancellation and batched bridge costs pass. |
| 2 — V3/V4 | Integrate Notes, a dynamic plugin and real harness adapters | Preview/activate/revoke/upgrade/rollback without host restart. Host-brokered commits, history and permission rechecks hold through crash and cancellation. Codex/Claude edits survive interruption and reconcile against the base version. Credential protection is proven separately for each supported CLI/account mode before shared-account use. |
| 3 — V6 | Open the first replacement engine contract | A space replacement preserves the contract and cannot bypass commit checks; other spaces keep defaults. Unsupported client behavior degrades explicitly. This does not block native shell or initial Notes UI. |

Stages 1A–1C can run in parallel after stage 0 and minimal contract fixtures exist. Stage 2 uses their proven boundaries. Extend interactive load tests as each slice becomes executable, rather than claiming a host stub establishes whole-app responsiveness.

## Measurements

Record p50/p95/p99/max and raw traces for input-to-paint, missed frames, main-thread blocking, startup, palette/search, streaming, large Collections/Activity, memory/idle work and provisional-to-durable acknowledgement. Run the same interactions unloaded and under plugin, cal, indexing and other-space sync load. Use end-to-end trace IDs.

Target 120 Hz on capable displays (about 8.33 ms per frame); measure 60 Hz devices against their actual frame interval. A frame interval is not an input-latency budget. Set each metric's acceptance threshold and permitted noise separately before testing. Compare overlapping features against existing Calternal without regression outside the agreed noise bands. Include large documents, long conversations, attachment-heavy resources and high fan-out sharing. No machine allocation or numeric latency percentile is inferred from past infrastructure.

## Recovery and compatibility fault matrix

| Fault | Expected result |
| --- | --- |
| Duplicate admission with the same request ID | One admitted operation; stable result/status lookup |
| Crash before effect dispatch | Durable intent recovered according to the operation's replay class |
| External effect succeeds; crash before result is stored | Reconcile using provider status/idempotency support; otherwise outcome-unknown, never blind replay |
| Cancellation/revocation races with commit | Atomic host check determines order; stale invocation cannot commit later; prior external effects remain visible |
| Old worker returns after replacement/restart | Invocation generation rejects its commit; multi-host execution additionally needs lease fencing |
| Same work repeatedly crashes | Retry budget exhausted; inspectable blocked work, no hot restart loop |
| Pinned plugin missing, blocked or incompatible | Blocked status with diagnosis; authorized migration/resume/abandon; inspection does not execute work |
| Plugin upgrade migration fails | Preserve data and recover through the declared snapshot/compatibility strategy |
| Client/server or replacement-provider contract changes | Contract snapshot CI rejects undeclared breaking changes; negotiation disables incompatible features clearly |
| Offline permission changes or account switches | Retain pending local edits safely; deny unauthorized replay; isolate caches and sign-out cleanup |

Use controllable timers and explicit fault injection at these boundaries. Do not attempt deterministic replay of arbitrary CLI model execution or promise exactly-once external effects.

## When Apple work starts

Shell, navigation and shared-component prototypes can start immediately against generated types and fixtures. Functional Notes work should start with stage 1C, alongside the core; there is no need to wait for an abstract “complete core.”

The first functional screen requires versioned resource/operation/error/event contracts and capability negotiation; a working async UniFFI bridge; operation IDs with explicit cancellation; Swift-owned networking, Keychain and lifecycle; account/server-scoped local storage; and the real open/edit/offline/reconnect path. Cancelling a Swift wait is distinct from cancelling durable server work. Measure bridge copying/batching and first-frame/input/scroll costs on actual Apple hardware.

Full release still requires the supported-device matrix, local-state migrations, multi-process file coordination where extensions share state, compatibility commitments and distribution/license checks. A completed Rust library alone proves none of those.

## Reference lessons retained

- **bb-lite:** machine-readable public contracts and discovery, API snapshot/drift checks, explicit service replacement and hook composition. Scope hooks and define ordering, deadlines and errors; host authorization remains outside plugin control. Its trusted plugin model is not a multi-tenant sandbox.
- **pi-durable:** persist intent, own invocations explicitly, reject late commits, bound crash loops, inspect blocked work and classify replay safety. Its mechanisms do not establish distributed lease fencing or exactly-once effects for Calternal.
- Already covered: durable admission, explicit helper/background lifetimes, stable action IDs, version pins that do not freeze permissions, and client-independent execution.

Reference snapshots: [bb-lite plugin API](https://github.com/get-bb/bb-lite/blob/19e1804266d476a89def44d07361e2f992cec4aa/docs/plugin-api.md), [bb-lite architecture](https://github.com/get-bb/bb-lite/blob/19e1804266d476a89def44d07361e2f992cec4aa/docs/architecture.md), [pi-durable](https://github.com/earendil-works/pi-mono/tree/747c217b81980206063a89e46645b362de7ac8b1/packages/durable).

## V7: multiple machines (added after feature-gap review)

Before shipping remote execution, connect two execution daemons to one authority, including macOS and Linux. Test enrollment replay/expiry, wrong-server identity, cross-user/workspace denial, destination authorization, local permission ceilings, capability/version mismatch and credential non-transfer. Verify a thread selects the intended machine and survives client closure.

Inject network partitions, daemon/server restarts, duplicate dispatch, permission revocation while disconnected, lease expiry and late results after reassignment. Expect one accepted invocation generation, explicit uncertain external outcomes and no blind replay. Reconcile remote processes rather than assuming a central-server restart stopped them. Distinguish inability to cancel an offline process from confirmed cancellation.

Measure dispatch/acknowledgement latency, reconnect catch-up, artifact transfer, terminal/output backpressure, idle connections and UI responsiveness under high remote-output volume. Remote work must not sit on the typing/animation path. Test durable artifact availability after workspace retirement. Keep V7 separate from an optional future highly available multi-server control plane; multiple execution machines do not require multiple authoritative servers.

## Inheritance gates added after the existing-app audit

Before a plugin migration is accepted, map its source decisions using [inheritance-audit.md](inheritance-audit.md). Exercise stable deep links across rename/move, disabled/replaced plugins, authentication and revoked access; keyboard/VoiceOver names and focus; reduced motion and contrast; and real native editor/File Provider boundaries. Test helper completion/result persistence/auto-archive races separately from deletion and workspace teardown.

Storage proofs must cover the accepted tier layout and FO1 timing/recovery if claimed for the release: missing/swapped mounts, external open writers, restart during materialization, state loss before/after the one-second bound, protected recovery snapshots, restored/revoked grants, attachment identity and same-size external rewrites. Do not claim the existing two-root implementation proves later tiering specifications. Any staged omission must be explicit in release scope.

## Orchestration defaults: additional acceptance scenarios

Test parallel coding tasks in separate worktrees, explicitly shared/direct folders, PR artifact retention after helper archive and refusal of unauthorized publishing/merge. Exercise provider-limit waits across restart, changed reset times, unavailable reset information, concurrent manual resume and timer wakeup, revoked credentials, offline selected machine and Stop while waiting. Expect one admitted continuation, no relocation or ungranted fallback, and no replay of uncertain effects. Check attention notification deduplication/deep links and opt-in completion behavior independently of Activity state.

## Distribution and cleanup acceptance cases

Test GUI-bundled CLI installation, command-name/PATH conflicts, standalone/GUI updater coordination, short-code pairing expiry/replay/wrong-server checks, headless pairing, and safe idle daemon upgrades with rollback. Validate the actual macOS signing/distribution channel. Do not imply mobile Apple apps can install a system-wide CLI.

For automatic merged-work cleanup, test ordinary/squash/rebase merge proofs, dirty and untracked bytes, newer branch commits, another active thread/process/worktree, shared/stacked PR dependencies, protected branches, offline machines, artifact persistence failures, changed refs during deletion, crashes between cleanup steps and retry. Archive-only must never trigger deletion. No generic forced branch deletion based solely on a PR's merged badge.

Remote cleanup is accepted: test conditional deletion against the exact remote head, branch-protection denial, another actor pushing between checks, squash/rebase merge preservation, already-deleted refs and lost acknowledgements. Never fall back to unconditional remote deletion after a race.

## Managed provider authentication spike

Compare direct supported CLI authentication-file management with a CLIProxyAPI adapter/fork only where needed. Exercise real Codex/Claude account flows, login/refresh ownership, concurrent machines, rotation/reconnect, revoked or expired accounts, unpairing and secret exclusion from model tools/logs/support exports. Record support and terms per provider/account type; do not assume an arbitrary copied login file is portable or that a proxy preserves CLI harness behavior. Recommend the smallest implementation that passes; no proxy fork is authorized merely by listing this alternative. Confirm closed-unmerged PRs retain worktrees/branches independently of helper archival.

Browser/device takeover tests: human input renews idle time; no simultaneous cal input; countdown and Keep control; no resumption during composition, sensitive trusted interactions or after Stop/revocation; stale controller rejection; renewed observation before the cal resumes.

## Shared-memory and plugin update gates

Test personal/group separation, partial-resource shares, mixed-source derived memories, changed membership, source revocation, shared output to a wider audience, provider-session/cache contamination and absence of filtering in an adapter. No mixed-private inference followed by output-only filtering. Verify memory provenance/correction/removal and dependency invalidation across Honcho, local representations and cached summaries before claiming the boundary works.

Test automatic unmodified-plugin updates against unchanged grants, contract compatibility, migration failure and data-aware rollback; customized forks keep their changes and receive a merge preview instead of overwrite. Retaining a departed member's shared contributions never retains access to their private account/credentials.

Memory correction gates: forget a claim, rerun historical ingestion/reindex/provider restart and verify it is not automatically re-created; invalidate derived summaries and adapter caches. Source conversations remain intact. Test conflicting attributed assertions, explicit corrections and scoped suppression so one person cannot erase another person’s private memory.

Offline chat submission: test persistence across restart, edit/cancel before dispatch, duplicate/lost acknowledgements, multi-tab ownership, unavailable selected machine/provider, upload dependencies, sign-out/account switch and membership revocation. Deliberately sent messages dispatch once on reconnect; unsent drafts never auto-send. Shared-space cal memory tests include narrower per-cal grants and distinct personality/preferences.

Verify protected recovery remains reachable with broken shell/orchestrator plugins and does not permit privilege escalation. Installing competing handlers must not hijack deep links or rewrite resource ownership. Test onboarding-dependent per-request limits: enabled for non-developer mode, not enabled by default for developer mode, with deployment fairness/permission/provider ceilings always enforced.

Quota/export tests: shared references charge once; independent reflink copies charge logical bytes separately; ownership transfers validate destination allowance; physical cross-owner dedupe creates no quota signal. Portable exports omit secret material from config and provider state, enforce resource visibility and do not execute imported jobs/plugins or resurrect grants. Verify guided reconnection separately from protected full-backup restore.

GenUI/SDK first-slice acceptance: an independently installed Notes extension contributes a view/action through public extension points; browser and Swift render the same versioned view with accessible semantics, stable links, permission-checked actions and unknown-component behavior. Parent disable/re-enable, incompatible dependencies, streaming backpressure and client-version skew must behave explicitly. Use [early-use-plan.md](early-use-plan.md) for the concrete acceptance artifacts.

## Independent-review additions

Use the authoritative checkpoint matrix in [early-use-plan.md](early-use-plan.md). Add real resource-bound editor/IME/own-change undo fixtures, hostile GenUI size/binding/navigation and patch-gap cases, disposable preview-effect isolation, warmed-worker same-principal revocation and incompatible-codec offline replay. Credential protection gates each enabled private as well as shared provider mode. Capture baselines concurrently with contract scaffolding, but declare budgets before pass/fail judgments. Remote-machine use passes V7 regardless of limited-preview branding; a legacy-server bridge experiment does not establish final space-aware parity.
