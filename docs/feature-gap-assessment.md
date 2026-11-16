# BB / T3 Code feature-gap assessment

2026-11-16. Documentation comparison, not an exhaustive implementation audit. Multiple machines were requested by the user; the design below is proposed. Other additions are recommendations unless subsequently accepted below; this is not a launch-completeness claim.

## Multiple machines is an architectural omission

The previous proposal covered isolated server workers, not enrolling multiple user-owned execution hosts. Add machines independently of spaces and client devices: one central authority, multiple authenticated execution daemons, explicitly authorized workspaces and destinations. The [proposal](architecture-proposal.md#multiple-execution-machines--added-requirement-proposed-contract) defines the boundary; [V7](validation-plan.md#v7-multiple-machines-added-after-feature-gap-review) validates it.

Example: one cal can work in a Linux folder and delegate an Apple build to an authorized Mac workspace, with both results recorded in the same thread. This requires explicit file/artifact transfer and account selection; a path on Linux is not a path on the Mac. Installing a native client does not silently enroll its computer for execution.

Use a central authoritative server first. Server federation, active-active storage and transparent live process migration are separate features and are not implied by connecting machines. A remote machine can remain running while the central server restarts; ownership reconciliation and bounded leases matter more than assuming every subprocess died.

## Accepted discovery decision

The user specified Browse → Dev for developer-oriented plugins, with onboarding deciding whether to enable them. This is a discovery/activation preference, not a permission grant. Users can change it later. Ordinary notifications, voice and secure credential entry are not developer-only features.

## Gaps and proposed placement

| Capability | Existing plan | Recommendation / placement |
| --- | --- | --- |
| Multiple computers and machine readiness | Missing enrollment, routing and disconnect semantics | Add protected identity/leases/authorization and plugin-owned computer UX, provisioning and routing. Private by default; explicitly grant selected workspaces/capabilities. |
| Workspaces and reproducible setup | Isolated harness editing copies covered; external project environments missing | Workspace provider plugins for existing folders, fresh checkouts and default isolated Git worktrees with PR workflows for coding; setup/teardown, ownership, retention and failed-cleanup recovery. Do not make Git mandatory for Notes or ordinary users. |
| Persistent terminals and service previews | Missing explicit contract | Optional terminal/preview plugins with reconnect, bounded output, permissions and port access. Closing a view must not imply killing a task; explicit Stop controls. |
| Review what a cal changed | History/undo covered; coherent per-run review missing | Human-readable operation/change summaries with attribution, before/after and conflict-aware undo. Coding plugin adds diffs, commits and PR integrations; preserve durable reports when working folders are cleaned up. |
| Notifications and actionable attention | Activity covered, delivery channels unspecified | Notification plugins for web/native push, deep links, per-user preferences and deduplication. Do not put private contents on lock screens by default. Actions always re-check current authority. |
| Guided connection and secure credential entry | Credential custody covered; user-facing setup/request flow missing | Trusted secret-entry surface, scoped handles and optional password-manager plugins. Explain missing CLI/sign-in/tool readiness and how to fix it; never ask users to paste credentials into conversation history. |
| Usage, remaining allowance and resource pressure | Account attribution and quotas covered; user visibility underspecified | Usage providers expose reported quota/reset/usage with timestamps and unknown states, plus per-cal/machine resource limits. Do not invent token counts or subscription dollar charges. |
| Conversation controls | Steer, Stop, handoff and cal copying covered | Inherit already-approved fork-from-message, selected-passage replies and Cal/model switching. Specify draft preservation, queue inspection/edit/removal, retry and explicit context selection/compaction. Forking a conversation must never replay past side effects automatically. These are conversation-plugin features. |
| Browser and device verification | Current-app context/feedback covered; external verification missing | Optional browser automation and Apple Simulator/device plugins with visible control ownership, permissioned sessions and captured results. Useful to build native apps; not a prerequisite for ordinary Notes users. |
| Outside-app capture and native handoff | App-pane context covered | Later native plugins can add OS share-sheet input, selected-text/window capture and accessibility context with explicit OS consent. Preserve the private-report preview and cross-space disclosure rules. |
| Updates and diagnostics across machines | Version negotiation/recovery covered; fleet UX missing | Show daemon/harness versions, availability and actionable failures. Safe drain/update/reconnect behavior, redacted support bundles and retained last-working versions. No uncontrolled update or credential replication. |

Already covered: plugin orchestration; multiple harnesses and same-thread handoff; cals and helpers; durable jobs/schedules; shared conversations; Activity; GenUI; feedback/voice; memory; imports; history; palette; browser offline Notes reuse; API/MCP/CLI parity.

Recommended priority: design machine/workspace identity, destination grants, artifact ownership and disconnect behavior now because they change the foundation. Define notification, secure input and change-review contracts before broader UX work. Keep Git, terminals, browser/device control and provisioning as installable specialist plugins. Avoid turning Calternal's default nontechnical experience into an IDE merely to match a feature checklist.

## Evidence and limits

BB installed documentation read on 2026-11-16: `bb guide`, `bb guide machines`, `bb guide environments`. These describe one server with connected execution machines, provider-owned machine lifecycle, permission ceilings, environment/worktree setup and retirement, persistent terminals, and Git/PR plugins. Documentation claims were not exercised against connected hosts in this review.

T3 Code documentation inspected at revision `629d5083b7b0278edf0a5f587f25b8d40c3fdfb2`:

- [Feature overview](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/README.md)
- [Documentation inventory](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/README.md)
- [Remote access](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/remote-access.md)
- [Source control](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/source-control.md)
- [Messages/context](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/composer.md)
- [Secure input](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/secrets.md)
- [Usage](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/usage.md)
- [Device control](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/devices.md)
- [External window capture](https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/snap-shot.md)

The table separates our recommendations from those sources; not every proposed feature is claimed to be implemented in both products. No exhaustive feature-parity or performance claim is made.

Completed helper threads auto-archive per the user’s 2026-11-16 decision; preserve parent-linked results and history. See the inheritance audit before treating old AI features as new product choices.

## Orchestrator defaults accepted 2026-11-16

The user explicitly affirmed a full BB/T3-style orchestrator with built-in productivity apps available as tool calls. Coding uses isolated Git worktrees and PRs; execution uses the selected/saved computer; allowance exhaustion waits unless a fallback is configured and automatically resumes at reset; external notifications default to attention items. These supersede the corresponding open recommendation entries above. The proposal records permission, cancellation and recovery implications.

## Installation and completion decisions, 2026-11-16

Accepted: one native executable with separate roles; macOS GUI bundles/auto-installs CLI; browser-confirmed pairing without SSH credentials; Mac/Linux execution first; safe idle machine-service updates; monitor PR checks until ready for review; automatically clean merged/preserved task worktrees and task branches. The proposal distinguishes desktop CLI installation from mobile Apple apps, archival from filesystem cleanup, and local and remote task-branch cleanup from protected or still-needed branches. These policies are not implemented yet.

## Remaining grilling frontier

Machine/provider credential placement, closed-but-unmerged task cleanup and human takeover of browser/device control still need explicit product defaults. Unsent draft behavior should inherit existing cross-device draft expectations and protect private drafts in group conversations; do not re-ask basic draft persistence without a conflicting requirement. Plugin update/fork UX, shared-space memory boundaries and first-run/error-state mockups need focused review against existing decisions. Storage space partitioning, stable attachment IDs, recovery proofs and sandbox mechanisms are engineering work, not questions for the user to solve. Any proposed reduction of accepted storage guarantees or initial release scope must be brought back explicitly. Exact native compatibility support duration is intentionally deferred.

Follow-up resolved: Calternal manages provider authentication, with direct auth-file handling preferred if simplest and CLIProxyAPI adaptation/fork evaluated only if useful; closed-unmerged work remains until explicit discard; browser/device takeover temporarily yields and automatically resumes after inactivity with visible control state. Detailed credential mechanisms require validation.
