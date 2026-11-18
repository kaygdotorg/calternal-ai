# Plugin-delivered interface

2026-11-18 · Proposal for review · [Lane #50](https://git.kayg.org/kayg/calternal-ai/issues/50)

Keep a protected renderer and mount points in the client. Package the visible
shell and orchestrator as ordinary plugins that use the same slots, tokens,
operations and confinement as community plugins. Use generated views by default, with sandboxed state and logic for rich interaction.
Add sealed components where they can stay within a bounded rectangle. This
fits the decision against page-wide JavaScript in the [plugin API](plugin-api.md)
and preserves the recovery surface described in the [architecture](architecture-proposal.md).
Terms follow the [glossary](glossary.md). The slot contract below is a prototype.

## What a plugin can contribute

| Host slot | Proposed contribution | Host responsibility |
| --- | --- | --- |
| Sidebar section | Generated view or sealed section | Own navigation, selection and the single sidebar |
| Card/window | Generated view or sealed component | Own window placement, chrome and stable address |
| Composer row | Generated view or sealed accessory | Own the editor, send and offline queue |
| Message decoration | Generated view | Virtualize with the transcript and keep one focus order |
| Thread edge | Generated view or sealed rail panel | Own edge placement and resize bounds |
| Settings section | Generated view or sealed section | Keep trusted security forms in the protected host |
| Palette commands | Generated action invoking a declared operation | Own search, shortcut routing and confirmation |
| Toasts | Generated view | Own timing, accessible announcements and queue limits |

A contribution has a plugin-local ID, slot and renderer declaration. An active
plugin contributes within its authorized space. The client selects where a
contribution is relevant using the current address and context. An address
identifies the item rather than its current renderer. Contributions never
authorize an operation. A personal shell resolves contributions separately for
each authorized space, with no shared worker across those spaces.
[Addresses](addresses.md) · [Orchestrator decisions](orchestrator-proposal.md#decided-in-the-grill-2026-11-17)

## Sealed components

Mount packaged assets in an isolated frame with an opaque origin, a restrictive
asset policy and no ambient app credentials, network access, popups, or top-level
navigation. Give each mount a dedicated message port. Bind it to the frame,
plugin, space and activation generation. The host parses bounded messages and
passes minimal context. It routes requested actions through normal dispatch and
rechecks authority. Cancel the port and discard stale messages on navigation,
disable or replacement. Asset serving, policy enforcement and resend/revocation
need isolation tests before enabling this renderer.

The host owns a versioned token snapshot including scheme, type scale, contrast,
reduced motion and material role. Bootstrap only mounted frames with that
snapshot and the permitted font assets. Share font downloads where browser
caching allows. Send updates only when the snapshot changes. The host composes
one adaptive backdrop for the enclosing surface. Frames receive transparent or
solid fills and foreground tokens, with no backdrop sampling or independent blur.
Mount frames on demand, cap visible mounts and dispose hidden mounts. Repeated
rows and scrolling transcripts use generated views. Measure mount and first
interaction, scrolling frame time, memory and font loading on native target
devices before assigning live mount budgets.

The host controls focus entry and restoration and supplies accessible frame
names. The frame handles local Tab order and reports boundary focus movement.
The host owns global shortcuts. Frames may request named, context-checked
actions and receive explicitly routed local shortcuts. The host owns drag
handles and resize gestures, coalesces size updates and clamps requested bounds.
Use typed drag payloads with host-side drop authorization. DOM inspection and
styling outside the frame, cross-frame drag previews, capture of app keystrokes
and a frame overlay across the host chrome are outside this mechanism.

## Stateful generated views

A third option is the recommended default for rich plugin interfaces. A
plugin runs ordinary TypeScript in an isolated worker. It produces a typed
tree of host components, updates that tree via bounded patches and receives
typed events. The same component IDs, property types, accessibility rules and
actions serve both a static GenUI program and a live tree. Keep the current
GenUI source format. Add an independently versioned patch and event protocol
on top of the same component catalog and normalized tree. Reuse the existing
validation where the representations meet.

Shopify introduced this pattern as remote-ui and now develops it as Remote
DOM. Its receiver can render native components and Shopify reports hosts in
JavaScript, Kotlin and Swift. Figma's sandbox is a precedent for separating
plugin logic and editor authority, with a separate frame for custom HTML.
[Shopify engineering](https://shopify.engineering/remote-rendering-ui-extensibility) · [Remote DOM](https://github.com/Shopify/remote-dom) · [Figma runtime](https://developers.figma.com/docs/plugins/how-plugins-run/)

On the web use a worker bootstrapped from a separately isolated environment
with a restrictive resource policy, without app storage, credentials or raw
network access. A same-origin worker by itself can fetch resources. Verify
the isolation and quota boundary before running a package. A server worker can
supply a live tree through its existing container and broker when the logic
needs server authority. That path adds transport latency. On Swift, an
isolated JavaScriptCore instance with no ambient bridges is a candidate for
local UI logic. Apply memory and execution limits and verify termination.
The runtime talks through the same frozen event and operation bridge on each
client. Every action is rebound to the authenticated scope and current
activation. An event ID names a declared callback, with no authority attached.

The shell becomes a layout tree assembled from host panes, windows,
sidebars, splitters, stacks, tabs, slots, command menus and navigation.
The orchestrator composes the same catalog with a virtual transcript, native
rich text and text input, attachments, chips, composer rows, thread rail,
progress/status and request forms. Selection, composition and IME, editing,
scrolling, focus traversal, gesture tracking and resizing stay in the host.
Emit bounded semantic events asynchronously. A plugin can update chips or
propose a new composer configuration without receiving each raw keystroke.
Host components apply tokens, fonts and adaptive material and provide
accessible semantics. The plugin still supplies labels and must pass checks
for its composition.

Use stable component IDs, batch mutations, and replace or invalidate the
tree when revisions diverge. The host keeps the virtual list and its scroll
position. The worker sends a bounded visible-range page and data deltas
rather than transferring a tree with 10,000 mounted rows. Coalesce streaming
text and deliver append deltas at a fixed rate. Prioritize input and layout
in the host loop. Mount a sealed frame for a custom drawing surface or an
HTML layout the catalog cannot express. Add broadly reusable primitives for
capabilities that need native gestures, platform editors or host-level
composition. Keep protected forms outside the tree.

## The shell decision

| Option | What the shell plugin can do | Cost |
| --- | --- | --- |
| Same trust tier with page JavaScript | Any approved plugin could run in the app page and replace live layout or intercept input | Sign-in, grants, secrets and recovery need a separate trusted origin or native surface. Every same-page plugin can still read or alter other app UI, counterfeit content and act with exposed page state. An approval and checksum identify the code a person agreed to run. They provide no runtime confinement. This would reopen the decision against page-wide JavaScript. |
| Static generated layout with a protected mount host | The visible shell is replaceable through declarative layout, theme and slots. A package can contribute at any supported slot subject to scope and grants. | The foundation still owns drawing boundaries, window and focus policy, and recovery. If “shell” means that privileged mount layer, that layer is host code. Custom global effects need a reviewed host primitive. |
| Stateful generated layout with a protected mount host | Any package can compose and update the host tree, run sandboxed layout logic and receive semantic events through the same interfaces. | Requires a shared catalog, bounded patch protocol and platform runtimes. The renderer retains control over the protected surfaces and fast interaction paths. |

Stateful generated views win for replaceable shell and orchestrator behavior.
Static GenUI remains the smallest authoring path. Sealed frames cover the
custom visual cases. All three use the same install process and renderer
permissions for official and community packages. Keep recovery, app-level
shortcuts and protected prompts in the foundation. Ask the owner to confirm where they want replacement to stop
before adding a shell-layout contract.

bb accepts full-trust server plugins and supports page content scripts as well
as named slots. Its Calternal UI plugin uses a content script for the frame and
registers an overlay, settings and sidebar navigation through the slot API.
That explains how it achieves global styling and where Calternal's sealed
model needs explicit host primitives. [bb Plugin Guide](https://getbb.app/plugin-guide)

VS Code separates normal contribution points from iframe-style webviews.
Webviews use message passing and get theme classes and tokens. The extension
host is a separate concern from the webview. [Contribution points](https://code.visualstudio.com/api/references/contribution-points) · [Webview API](https://code.visualstudio.com/api/extension-guides/webview)

Obsidian disables community plugin code in Restricted Mode and asks users to
trust the authors they enable. Its workspace API lets a plugin register a view
inside the app. [Plugin security](https://help.obsidian.md/plugin-security) · [Views](https://docs.obsidian.md/Plugins/User+interface/Views)

Figma runs plugin main code in a sandbox with editor APIs and uses an iframe
for browser UI. The two sides communicate by message passing. This supports the
separation between UI and document authority. [How Figma plugins run](https://developers.figma.com/docs/plugins/how-plugins-run/)

## Transport measurement

A headless Node worker prototype ran on `calternal-perf-02-backup01`
(x86_64, Node v22.23.3) under `/usr/local/bin/perf-lock.sh`. The timer
includes worker computation, structured clone and applying the update to
host state. The process and caches were warm. It excludes browser and Swift
rendering, sandbox bootstrap, GenUI validation, layout and frame time.

| Update | Samples | p50 | p95 |
| --- | ---: | ---: | ---: |
| Full 10,000-row data snapshot | 100 | 7.540 ms | 8.275 ms |
| 60 visible rows from a 10,000-row dataset | 1,000 | 0.079 ms | 0.118 ms |
| 256-character append to one streaming message | 1,000 | 0.034 ms | 0.070 ms |

The full snapshot nearly consumes one 120 Hz frame period at p95 before
rendering. Keep the virtual list in the host and transfer bounded pages or
deltas. This is an IPC comparison, not a UI performance gate.
[Transport benchmark](../scripts/bench-plugin-view-transport.mjs)

## Swift clients

Use the same contribution and operation identifiers, address and token
contract. Generated views render with platform controls after validation.
Provide a generated view for any sealed web contribution that must also work
on Swift clients. A separate native component protocol could add richer Apple
behavior later. It would require its own accessibility, versioning and safety
review. Web frames do not translate to AppKit or UIKit.

## Notes and Threads as ordinary plugins

The current broker has live-authorized single-item reads and writes. It
does not expose the Notes and Threads behavior currently wired into core
dispatch. Keep resource identity, journaling, receipts, recovery, membership,
search authorization and machine/provider enforcement in the host. Port from
the existing Calternal engines before exposing a new primitive.

| Missing public contract | Existing code to reuse | Estimated size |
| --- | --- | --- |
| Cursor-based kind listing, revision reads, lifecycle transitions and collection placement | Notes listing, history, Trash and file-store paths | 3–5 engineer-days including paging and isolation fixtures |
| Conditional patch and block-identity merge, revision conflicts and links/tags | Notes revision, merge and indexing paths | 4–7 days including recovery tests |
| Invalidation and incremental indexing, including search-provider registration | Space journal and Notes index | 2–4 days |
| Typed thread events, atomic append/checkpoint, paging and overflow attachments | Thread document, deltas, persistence and saved transcript | 4–7 days including crash/retry tests |
| Job admission, leases, typed provider output and durable user requests available to a plugin | Existing job/checkpoint code, turn admission, machine transports and requests | 5–9 days including revocation tests |
| Thin TypeScript operation and view port for both apps | SDK worker, existing manifest kinds and GenUI | 4–7 days after the contracts above |

These are implementation estimates for one engineer and exclude native UI work.
First expose versioned service calls over the existing engines, then move
product routing and composition to TypeScript. Direct userdata access would
duplicate the authorization and durability boundaries. Decide separately when
the public engine replacement contract should support alternative formats.

## Extension points in bb, bb-lite and pi-durable

The largest authoring gaps are **plugin-owned slots**, **live install and reload**,
**scoped reactive values**, **durable waits and continuations**, and **readiness,
logs and timing**. Together these would let a plugin compose with other plugins,
explain its own failure and resume unattended work. Prioritize the slot and
component contracts before adding individual React-style hooks. Prioritize
typed waits and run continuations before adding one-off scheduling or retry APIs.

The following inventory groups equivalent extension points across the sources.
A declaration without execution is marked as decided. Built includes the
prototype where explicitly indicated. Proposed additions require the owner's
decision. The [November 17 comparison](reviews/extensibility-research-2026-11-17.md)
contains the detailed constraints for the 24 previously adopted capabilities.

| Extension point in the reference systems | Calternal status | What an author can do there today |
| --- | --- | --- |
| bb-lite plugin-declared typed slots and single or multiple selection | Decided, unbuilt | Let another package add a new kind of entry without changing the foundation. This prototype has fixed slots. |
| bb and bb-lite local install, hot reload and development inspection | Partly built in this lane | Change worker code and receive an activation result without restarting the development server. |
| bb marketplace and bb-lite Git/local catalogs, dependency and engine ranges, upgrade rollback | Decided, unbuilt | Find packages by source, have compatibility checked and restore the last working activation. Local checksum review is built in this lane. |
| bb-lite fork existing server, app or runner entry and profiles | Decided, unbuilt | Fork one part of a default feature and keep updating the other parts. |
| bb status, logs, lifecycle timings, safe mode and bb-lite audit chain | Decided, unbuilt | See why a plugin needs setup, view handler latency and disable scoped plugins together. |
| bb-lite live data, changed notifications and audience-scoped events | Decided, unbuilt | Publish a typed value to one account and refresh its subscribers without polling. Job streams are built. |
| pi-durable typed signals and deadlines, replayable parked tools | Decided, unbuilt | Wait for a webhook or deadline with no live worker, then resume from a checkpoint. |
| pi-durable parked approval gates and argument-bound decisions | Decided, partially built | Park any declared tool on approval, then re-run its gates. Calternal's protected approvals currently cover the host turn path. |
| pi-durable `afterRun`, `afterFailedRun`, `onYield`, after-tools decisions; bb lifecycle retry | Decided, unbuilt | Schedule follow-up testing and withhold an answer until the child work completes. Calternal's core retries do not expose this as a plugin point. |
| bb workflow scripts and pi-durable Code Mode | Decided, unbuilt | Replay a deterministic tool pipeline without repeating successful calls. The reference Wasm runtime is outside the current platform rule. |
| bb lifecycle events for thread, turn, tools, environment, host and startup | Decided, unbuilt | Subscribe a plugin to changes and run scoped follow-up work without polling. |
| bb dispatch hook, bb-lite operation watch, rewrite and answer policies | Decided, unbuilt | Inspect or rewrite allowed operation inputs with an owner-declared ceiling and field redaction. Calternal accepts some metadata today. |
| bb schedules and cheap pre-checks, one-shot timers and long-lived background services | Decided, unbuilt | Trigger narrow condition checks and enqueue work only when it is needed. |
| bb and bb-lite app registration, home and replaceable navigation | Decided, unbuilt | Replace navigation, home panes or sidebar lists as a package. Stateful GenUI is the recommendation here. |
| bb sidebar footer, header, action menus, row glyphs, pin, drag/split and metadata | Decided, unbuilt | Put live usage, status and commands on individual rows and add split targets. |
| bb contextual fixed/dynamic panel tabs, file openers, Browser toolbar | Decided, unbuilt | Offer an editor panel or toolbar action relevant to the current tab and thread. |
| bb thread header, Info sections, thread context cards and stacked cards | Partly built as slot metadata | Register controls and context that update in place around an open thread. |
| bb and bb-lite message controls, directive renderers, file viewers, code fences and diffs | Decided, unbuilt | Take over one file, code block or message action with a specialized renderer. |
| bb-lite attributed timeline rows | Decided, unbuilt | Insert and update a plugin-owned progress row with one stable ID. |
| bb-lite composer contributions: menus, footer, send menu, banner, popup and text paint | Partly built as slot metadata | Add a conditional chip, send action, banner or syntax highlight around the editor. |
| bb mentions and composer command providers | Decided, unbuilt | Resolve a project document at send time and attach labelled context. |
| bb and bb-lite command shortcuts, palette entries, toasts and settings panels | Partly built as slot metadata | Bind a named shortcut and invoke it from the host command palette. |
| bb reusable app components and hooks for chat, source, diff, Info, model picker, icons, navigation and composer | Decided, unbuilt | Reuse production components with host focus, theme and state. Static GenUI has a smaller catalog. |
| bb browser extension state, query/mutation caching and tab-context hooks | Proposed | Bind a control to typed scoped host state and invalidate its query on change. |
| bb theme tokens, theme preview, code colors and fonts; bb-lite theme bundles and author reload | Decided, unbuilt as a plugin lifecycle | Test a complete theme, including native tokens and a code palette, before enabling it. |
| bb and bb-lite typed settings, pickers, scoped values and on-change handlers | Decided, unbuilt in full | Declare a validated per-person choice, project selector or multiline editor. |
| bb and bb-lite install/setup and teardown lifecycle | Decided, unbuilt for authored handlers | Seed package defaults during activation and dispose a watcher when disabled. |
| bb typed RPC and bb-lite operations, client/CLI/agent doors and breaking-change lock | Built for public operations | Expose one schema-checked operation. Typed cross-plugin RPC and consumer compatibility ranges remain unbuilt. |
| bb hand-written plugin CLI commands and bb-lite combined CLI routes | Proposed | Group several operations under a plugin-owned command with help and completion. Currently use `calternal invoke`. |
| bb tools, presentation and conditional offering; bb-lite agent doors; pi-durable caller restrictions, search and split result channels | Partly built | Offer a tailored tool to one agent and keep render-only output out of model context. Calternal already exposes authorized operations. |
| bb agent/provider environment entries; bb-lite runner parts; pi-durable per-conversation environment | Partly built | Supply a narrow executable or environment binding to one authorized machine-run command. Managed machine/provider invocation exists. |
| bb and bb-lite machine and environment providers, enrollment, checkpoint, suspend and reconcile | Partly built | Define a custom environment kind with its own provisioning, suspend and cleanup. Core machine enrollment exists. |
| bb and bb-lite agent provider adapters, capability/model catalog and native skills/commands | Built for the worker/provider protocol, partly unbuilt at discovery | Ship an agent backend, extend its native command and skill roots, and expose its configuration controls. |
| bb and bb-lite small completion, title fallback, usage providers and transcription | Decided, unbuilt | Register a provider for titles, quota feeds or voice without taking over the agent runtime. |
| bb skill layering and import, Honcho instruction contribution; bb-lite instruction sections; pi-durable wrapped system-prompt sections | Partly built | Add a selectable context source or skill with visible provenance. Space standing instructions and the guide asset exist. |
| bb and bb-lite trusted user forms and interactions, pi-durable ask-person example | Built for host forms, plugin entry unbuilt | Ask a typed question from an ordinary plugin and resume when the person answers. |
| bb operator-entered sandbox secrets and bb-lite runner credentials | Partly built | Bind an approved secret to a particular runner environment with no agent-visible value. Calternal has protected storage and brokered handles. |
| bb private stores and bb-lite KV; pi-durable document families, typed state and transactional entries | Partly built | Declare durable extension state and update it as part of a task transition. Calternal has brokered hidden kinds and revision writes. |
| bb-lite app and server contract calls, explicit caller chain | Decided, unbuilt in full | Compose operations across packages while preserving the acting caller and scope. |
| bb and bb-lite thread, project, terminal, file and Git SDK surfaces | Partly built as host operations | Build a plugin-powered tooling view over thread/workspace actions, Git status and a scoped terminal. The general worker broker is narrower. |
| bb and bb-lite OAuth integration, plugin routes and hooks; pi-durable signal ingress | Decided for brokered ingress, unbuilt | Receive an external system event with schema checks and wake only its owning job. |
| bb fixture APIs for app, host, providers and server; bb-lite contract harness; pi-durable scripted models | Partly built | Test UI, provider negotiation and scripted task recovery without the live app. This lane supplies a worker fixture. |
| pi-durable tool wrappers, nested calls, continuation, tool policy and abandonment | Decided, unbuilt as a public extension contract | Wrap a tool with approval and persist nested calls independently through retries. |
| pi-durable conversation configuration, named policies, context views, reset, handoff and compaction | Partly built as host/provider behavior | Install an extension that changes prompt assembly, compaction and handoff for one conversation. |
| pi-durable persistent child work, graph inspection, bounded retries and pinned registry reload | Partly built | Watch the complete child and task graph and keep its code pinned during live reload. Core jobs and durable checkpoints exist. |
| pi-durable conversation forks, transcript projections and watch subscriptions | Partly built | Define a fork or custom projection that updates as stored entries are added. Core thread feeds exist. |
| pi-durable usage, cost and live task telemetry | Proposed as scoped plugin data | Query accumulated model usage and live task state for an extension-owned diagnostic view. |
| bb full-trust server factories, host scripts and unrestricted process or network access | Excluded by worker isolation | Read ambient configuration, launch host programs, or fetch anywhere with server authority. |
| bb app content scripts, bb-lite same-page app parts and app-global overlays | Excluded by page isolation | Inject a stylesheet or change live DOM across other features and windows. |
| bb custom composer and bb-lite plug-in submit handler | Excluded for host-owned input and send | Read every keystroke and decide what a submit does. Chips and semantic requests fit stateful GenUI. |
| bb raw HTTP, WebSockets and broadcast realtime; bb-lite arbitrary server responses | Excluded on the app origin | Serve executable content beside protected sessions or broadcast tenant data to every client. Use scoped operations and subscriptions. |
| bb raw SQLite, bb-lite shared tables, pi-durable in-process storage replacement | Excluded by per-space access and protected recovery | Open another feature's tables or replace its durability substrate. Expose authorized service operations instead. |
| bb-lite pluggable authentication and authority | Excluded by protected identity | Replace the host authenticator. Approved sign-in integration belongs in protected host code. |
| pi-durable arbitrary in-process extensions and custom replay/storage adapters | Excluded for authored packages | Run beside the harness or replace how recovery commits. Versioned broker calls preserve the host's boundary. |

### Sources checked

- bb **0.49.0**, local `bb guide plugins` and authoring skill snapshot
  `478fd091c7850d45f0584dafd50035943b52e76f37d9cb6372bad547b6f1c0ae`.
  Checked the backend and frontend symbol indexes and registration,
  lifecycle, renderer, host, CLI, providers, RPC and slot references. Read
  Calternal UI and CLIProxy Usage app and server entries at local
  `bb-plugins` revision `0c485b1`, and the Honcho app, server and host
  entries at `145816f`. [Public Plugin Guide](https://getbb.app/plugin-guide)
- bb-lite public HEAD `19e1804266d476a89def44d07361e2f992cec4aa`.
  Checked the plugin guide and the server API and core contracts from the
  same commit. [Plugin API](https://github.com/get-bb/bb-lite/blob/19e1804266d476a89def44d07361e2f992cec4aa/docs/plugin-api.md) · [Server API source](https://github.com/get-bb/bb-lite/blob/19e1804266d476a89def44d07361e2f992cec4aa/packages/plugin/src/server-api.ts)
- pi-durable **1.5.2**, pi public HEAD
  `05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94`. Checked the current Durable
  README, package version and root export inventory against the earlier
  `747c217` reference and reviewed the earlier snapshot's example inventory.
  [Durable README](https://github.com/earendil-works/pi/blob/05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94/packages/durable/README.md) · [Public exports](https://github.com/earendil-works/pi/blob/05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94/packages/durable/src/index.ts) · [Examples](https://github.com/earendil-works/pi/tree/05fd3fbda0bbb9a1c7a76e4bf92ab58ed77c8d94/packages/durable/test/examples)
