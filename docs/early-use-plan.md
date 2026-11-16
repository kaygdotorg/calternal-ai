# First usable build and Apple parallel start

2026-11-16. Intermediate-use delivery scope and portable-GenUI-first policy approved by the user on 2026-11-16; implementation plan under review, not implemented software or a reduced full-product commitment. The user wants to use Calternal-AI soon while Apple development starts, with extensible plugins, GenUI and shared client core present from the beginning.

## What must be concrete before broad implementation

A plugin promise is not an SDK; a shared-component promise is not a GenUI protocol; a list of Rust modules is not a usable client core. Specify and execute one end-to-end contract slice for all three, rather than completing independent frameworks first.

| Contract | Minimum reviewable artifact | End-to-end proof |
| --- | --- | --- |
| Plugin SDK v0 | Manifest, API versions, scope/grants, dependencies and extension-point IDs, resource/operation/event schemas, lifecycle/disposal and versioned migration contracts; generated authoring template | Notes and orchestrator use the same public SDK as a generated plugin; install/preview/activate/revoke/rollback without privileged shortcuts or a host rebuild |
| GenUI v0 | Versioned declarative view tree with stable node IDs, layout tokens, accessibility semantics, bounded query/data bindings, local form state and registered-operation actions; versioned capability negotiation | One plugin view renders in browser and native Swift, including loading/error/empty/pending states, streaming updates, deep links and keyboard/VoiceOver semantics |
| Shared client core v0 | Rust async resource/actions/events interfaces, cursor/outbox/retry state, explicit cancellation, permissions/errors, platform effect boundaries and UniFFI bindings | Browser TS and Swift/Rust pass the same real Notes open/edit/offline/reconnect fixtures; a generic plugin operation also works without a plugin-specific core rebuild |
| Persistence v0 | Space-owned canonical resources, protected state/journal commit rules, inherited Markdown chat format, artifact ownership, recovery and attachment identity | Two accounts and a shared resource, crash/retry/revocation, recoverable conflicts and no false saved acknowledgements |
| Execution v0 | Durable message admission, Codex/Claude adapters, machine/run/account binding, cancellation and permission checks | One thread can direct a Notes edit and a coding task through the public action contract, with history, review and retained results |

### GenUI details that cannot be left implicit

- Initial built-in primitives should cover stack/grid, text/Markdown, buttons, forms, lists/tables, resource cards and status/progress. The resource-bound Notes editor is required in v0; additional specialized charts/media/editors are added through versioned capabilities. Exact catalog follows the first real screens, not a speculative exhaustive toolkit.
- A GenUI tree contains data and operation references, not executable JavaScript. Actions still pass through current server authorization; rendering does not confer permission.
- Keep rapidly changing local input/focus/animation state in the renderer. Do not make typing or scrolling depend on a server/plugin round trip. Subscriptions and updates need bounded paging, backpressure and keyed incremental patches, not repeated whole-transcript rerenders.
- Persist saved generated views as portable resource definitions with stable identity and deep links. Define ephemeral reply rendering separately from saved views. Renderers must survive unknown optional nodes and refuse unsupported required functionality explicitly.
- Adding a new arbitrary native control cannot happen merely by installing server-side TypeScript. Extend the built-in versioned component catalog in client releases; the user selected portable GenUI first, so arbitrary web-only HTML/JavaScript surfaces are outside the early scope.
- Accessibility, reduced motion, theme tokens and responsive layout are part of the schema/render contract, not metadata added after the visual design.

### Client core boundaries

Begin Rust core and a Swift harness alongside the first web contracts. Include generic operation discovery/invocation and GenUI schema decoding so the native client is not hardcoded to the first apps. Swift owns native rendering and platform effects, while the core owns protocol decisions. TS browser logic shares contracts/fixtures; no Wasm is introduced.

Native shell/component prototypes can start immediately. Functional native Notes and conversation work begins with the tested core slice; it does not wait for every plugin or a full core implementation. UIKit/TextKit 2 iOS, the existing macOS toolkit validation and watchOS-specific UI remain separate platform work. Do not count a JSON demo as proof of sync, credentials or cancellation.

## Approved first personal-use checkpoint

A working one-sidebar orchestrator with Codex and Claude Code, persistent Markdown conversations, selected-computer execution, a real Notes plugin, a small GenUI catalog and private-plugin authoring loop. Include sign-in, space/permission isolation, minimal Activity, Stop, recovery, history and honest pending/save states. Prove two-account sharing even if the first everyday user is the owner. Exercise one real worktree/PR path rather than postponing all orchestration until productivity apps exist.

The common machine protocol must exist immediately; enrollment of a second machine follows the first end-to-end run. Credential/isolation failure prevents the affected deployment or shared-account capability from shipping. Do not bypass those gates to meet the early-use goal.

This checkpoint is an intermediate usable build, not the full first public release. The full agreed product still includes Honcho, subscription voice, feedback, BB import, broader orchestration and later app migrations. The user approved using this intermediate build before the full integration set is complete. Full product commitments remain; implementation acceptance and disclosed storage/recovery limitations still require evidence.

## Critical path and parallel work

1. Capture performance baselines and define contracts/fixtures, including SDK and GenUI. Build only the host/client scaffolding needed for the first proof.
2. In parallel, validate sandbox/credentials, space storage adaptation, and Swift/TS client conformance using the same contract revision.
3. Integrate Notes + conversations + one generated plugin and a harness edit; run recovery, isolation and responsiveness gates as the slice becomes executable.
4. Expand remote-machine/worktree/PR integration and native screens against those proven boundaries; add full-product integrations incrementally without a universal framework rewrite.

The detailed experiments remain in [validation-plan.md](validation-plan.md). No duration estimate is justified until the reuse/sandbox/native bridge spikes return evidence. Neither the whole app nor its performance has been implemented or measured here.

## Independent review reconciliation, 2026-11-16

Reviewed independently by Fable 5.2 medium and GPT-6.1 Sol medium; see [Fable](reviews/early-use-fable.md) and [Sol](reviews/early-use-sol.md). Both produced eight findings. Their severity labels are reviewer judgments, not measured failures. This section refines the earlier outline where the outline is less precise.

### Concrete contract refinements

- **GenUI production and state:** versioned declarative view definitions ship in plugin bundles and are cacheable by immutable bundle/version. Model-produced reply views use the same host-validated schema. Bindings reference registered queries and typed field paths; v0 has no arbitrary expression execution. Local selection/composition/form state stays in the renderer. Subscription IDs and ordered revisions govern patches; stale patches are rejected, a gap requests a bounded snapshot, and delivery is coalesced for paint without dropping underlying durable events.
- **Native Notes is in v0:** include a resource-bound document-editor capability alongside primitives. Any authorized plugin can reference it; it is not a Notes-only private API. The component mounts the web editor or native TextKit editor over canonical resource/session identity and compatible edit/update bytes. Selection, IME and own-change undo are local/native; pending/conflict/read-only states are explicit. Do not serialize whole documents into view trees or replace collaborative text on every render.
- **Renderer limits:** inert validated Markdown, authorized resource/media handles, explicit external navigation, bounded tree/depth/text/page/subscription/update/layout values, and no arbitrary fetches through styles/bindings. Permission loss detaches subscriptions and clears protected view state. Host and clients enforce negotiated limits; unrenderable views fail visibly. The hostile corpus covers web and Swift.
- **Notes hot path:** open/sync/save acknowledgement and indexed list/search go through public host-brokered engine contracts without an obligatory TypeScript worker hop. This still checks permissions and uses the normal operation contract. TS plugins supply views and composed operations; custom engines remain replacement targets and must meet their declared budgets. Trace measurements verify the actual path rather than assuming it is fast.
- **Preview effects:** a preview has a distinct installation identity and disposable data overlay/fixtures. It gets no production mutation, migration, schedule or external-effect/account handle by default. Deliberate live testing is separately authorized and attributable. Activation selects the verified immutable code/config/schema revision, not whatever changed after preview.
- **Authority invalidation:** bind workers/subscriptions/results to an authority generation. On relevant revocation, conservatively retire affected workers, subscriptions and temporary state; reject stale deliveries/commits. Checking the same user's authority only at a new read is insufficient if a warm worker retains old bytes.
- **Codec compatibility:** advertise resource-format/codec identity and supported edit encodings separately from GenUI/API/plugin versions. Incompatible native clients preserve opaque bytes or use explicit read-only handling. Queued edits retain their original codec identity and become recoverable conflicts if the server cannot safely reconcile them after engine replacement.
- **SDK v0 bounds:** implement manifest/API/scope/grants, typed operations/resources/events, activation/disposal, public view contributions, one Notes extension point and simple declared dependency validation. Defer general-purpose service replacement/hook breadth and data migrations. Unsupported migration declarations fail before activation. Until recoverable migrations exist, rollback applies only to compatible code/config versions and never claims to undo arbitrary data changes. This stages features without permanently closing the extension API.
- **Thread-to-app adapter:** Execution v0 explicitly includes `cl`/MCP discovery and operation invocation with short-lived run-scoped authority. Test Notes changes through operations first; the broader canonical-file checkout/write-back mechanism is a later increment, still required before offering that editing mode.

### Authoritative checkpoint matrix

| Behavior | Required evidence before enabling it | Breadth that follows |
| --- | --- | --- |
| One sidebar, full conversation + optional same-thread pane, Notes, minimal Activity/Stop, palette, stable links, protected recovery | One user completes a note-edit request and reviews its history; broken shell/plugin recovery works; keyboard/VoiceOver and applicable responsiveness gates pass | Full settings/polish and later app migrations |
| Codex and Claude adapters, persisted Markdown chat, basic cal definitions and ordinary threads | Real-account safety/terms checks per enabled mode, durable admission, Stop, crash/retry and one public-operation Notes edit | Rich cal editor, full memory and live voice |
| Dynamic plugin plus Notes extension | Same public schema for first-party/generated code; isolated preview, activation, disable/re-enable, incompatible version refusal and compatible rollback | General hooks/service/engine replacements and data migrations |
| Selected-computer coding, worktree/PR review | Explicit machine/run/account binding, artifact preservation, check monitoring and safe merged local/remote cleanup | General fleet provisioning, terminal/browser/device feature breadth |
| Multiple accounts/spaces and shared resource | Two-account isolation, grants/revocation, shared editing and permission-filtered delivery; minimal access controls for any enabled collaboration | Full invite/group-conversation UX and public signup |
| Native/core slice | File-backed native content with rebuildable index, real collaborative edit/offline/reconnect fixtures, generic operations, explicit cancellation and one GenUI/editor fixture | Full Apple product, File Provider/extensions and App Store release |
| Storage/recovery | Written profile mapped to inherited guarantees, fault tests and coordinated backup/restore; forward migration of checkpoint data | Additional storage optimization only where not required by the accepted profile |
| Remote execution | V7 before enabling a remote machine, even for an owner-only deployment; exact account/isolation mode recorded | More machines/provisioners/platform combinations |

Honcho/full memory, live voice, richer feedback UI and BB importer do not block this intermediate exit; their full-product commitments remain. Preserve diagnostics needed for safety/debugging in the checkpoint. Basic cals are not silently removed from scope.

### Dependency correction and decision boundary

Contract revision zero is the shared prerequisite: Rust transport types generate JSON Schema and TypeScript; UniFFI provides Swift core bindings; dynamic operations and GenUI use validated generic values where static types cannot name tenant-defined contracts. Commit schema snapshots/fixtures and test drift. Plugin authors declare extensions through the public schema; they do not edit server Rust to register a plugin.

Baseline capture and the performance specification run alongside contract/native scaffolding. Budgets precede pass/fail judgments, not every line of code. A Swift bridge experiment may use existing Notes sync via a temporary adapter before space re-keying finishes; that is protocol research, not evidence of final space-aware parity. Final acceptance uses the new space-aware server and common fixtures.

The first Apple platform remains a user sequencing choice. Platform-neutral core work, fixtures and design tokens do not wait for it. No AppKit/Catalyst comparison blocks all Apple work. Native content stays in files, not solely a device database.

Owner-operated limited preview versus publicly hosted first use remains a deployment choice. Choosing a limited preview changes exposure and rollout breadth; it does not waive generated-plugin confinement, current permissions, provider-secret isolation or V7 for remote execution. Existing local CLI sign-in may reduce login UI work but is not proof of credential protection. Managed authentication remains the accepted product direction.

Storage defaults to the inherited guarantees. No temporary two-root/backup-only recovery deviation has been approved, and this reconciliation does not introduce one. Before real-data use, publish exact acknowledgement, materialization, state-loss, backup and missing-mount behavior. If experiments justify a faster limited profile, present the exact deviation and migration path for a separate decision. No general storage rewrite is assumed necessary.
