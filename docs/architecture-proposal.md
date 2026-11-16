# calternal-ai: architecture proposal

Date: 2026-11-16

Status: consolidated design for review, revised after an architecture review on 2026-11-16 ([findings](architecture-review.md)). Product decisions below were accepted during the interview. Runtime choices and detailed contracts are recommendations to validate, not claims about existing implementation. Rules added by the review are marked proposed, and decisions it could not settle are listed in section 13.

## 1. Product

Calternal-ai is a full BB/T3-style AI orchestrator and a file-first personal productivity cloud. Coding orchestration, delegated work, machine/workspace management and review are first-class capabilities. Built-in apps are plugins whose actions are callable tools through the same public operation contracts used by people, CLI, MCP and API. The orchestrator itself is a plugin. Developer capabilities are discoverable under Browse → Dev and enabled through onboarding; this is a presentation choice, not a reduced orchestration model.

The initial agent providers are Codex and Claude Code, using their existing CLI harnesses. Calternal manages their lifecycle, context, permissions, delegation, and durable work. It does not implement another model reasoning loop. Additional harness providers can follow.

The product is AGPLv3, paid on calternal.cloud, and self-hostable. Web comes first. Native Apple clients follow using the same operation contracts and a shared native component vocabulary. Performance is the first concern and a design and release gate: interaction and animation target 120 frames per second on displays that support it, and everything is measured (section 11).

A shared client core is a day-one requirement, not a later extraction. Rust is a recommendation for that core; the host language and plugin-authoring language are separate decisions. The browser does not run that core in the initial scope: it stays TypeScript and is held to the same contracts and conformance fixtures (section 5). No Wasm runtime is part of the initial scope.

A thread is a full application operator. It can navigate the app, create and activate personal apps, configure permitted services, edit content and Collections, arrange the UI, and manage work through the same public operations as the user. Its authority comes from the acting person and explicit grants, never from being an AI or executing generated code. Section 6 defines how that authority is computed when more than one person, a shared cal or a space-installed plugin is involved.

## 2. The user model

- One concept: a **space**. There are no separate personal/group space types. A new space starts with its creator and is private.
- An account can belong to multiple spaces. A space is an ownership/access boundary, not a separate server instance.
- Invite people into a whole space to grant role-based access to its existing and future content. Share individual resources for narrower access.
- Resources can be shared from one space to people or other spaces. Sharing exposes the same live resource; copying and ownership transfer are separate operations.
- A space owns its contents and history. An owner must transfer their ownership role before leaving.
- Space managers control changes to shared apps. Members and their cals use apps within their permissions. Managers can delegate maintenance authority to a cal.
- Administrator status does not grant an ordinary content-browsing or impersonation feature. Administrator-assisted account recovery is an explicit trusted exception approved on 2026-11-16. Direct operator filesystem access and malicious deployment code are outside the agreed privacy threat model; this is not end-to-end encryption or cryptographic protection from the operator.
- Passkeys and OIDC provide authentication. Users can register multiple passkeys; any registered passkey can prove identity. There is no separate recovery-passkey category. OIDC authentication is supplied by its provider. Regaining access normally uses another registered passkey or a linked OIDC identity; someone who loses all of them contacts an administrator. Recovery restores the same account after identity verification, allowing new authenticators to be registered and compromised sessions revoked, rather than creating a new account or automatically transferring its spaces. The protected operation is audited; the operational verification and notification procedure remains to be specified. This supersedes the earlier no-admin-recovery proposal.
- Public signup on calternal.cloud; invite-only self-hosting by default, with configurable signup/admission.

## Inherited product contracts

Deep links, accessibility and native Apple behavior are required across every plugin, not optional plugin polish. The [inheritance audit](inheritance-audit.md) maps the existing decisions and distinguishes accepted requirements from implementation.

- Carry ADRs 0017/0056 forward: stable addresses for views, items, blocks and settings; stable plugin/resource identity rather than titles or machine paths. Renames, moves and plugin replacement preserve address resolution or explicit migration aliases. Opening a link checks current access; possessing its URL grants nothing. New space-aware routes must preserve old aliases. Agent UI operations, notifications, search and web/native navigation resolve the same target. Native link handling must resume navigation after authentication without leaking inaccessible metadata.
- Carry ADR 0038 forward: keyboard and VoiceOver support, shared accessible names/tooltips, reduced motion and WCAG AA contrast on every backdrop. Shared GenUI primitives and each plugin contribution must satisfy focus, accessible semantics and input requirements; arbitrary custom UI is not exempt.
- Retain native editors and platform controls. Existing Apple direction is UIKit/TextKit 2 for iOS, SwiftUI for watchOS, and AppKit as the macOS baseline pending the Catalyst/AppKit comparison. Swift plus UniFFI/Rust remains the architecture; a shared component vocabulary must not force mobile UI on Mac or replace native controls with web content. Existing platform minimums and native integration requirements are recorded in the inheritance audit for release validation.
- Notes, Tasks, Calendar, Mail, Files, Photos and Money arrive as app plugins, with app-specific extensions implemented as ordinary plugins declaring dependencies and versioned extension points. The parent app does not grant a subplugin ambient authority. Each extension declares its own capabilities, lifecycle, data ownership, discovery and routes. Disabling a parent makes dependent functionality unavailable with a clear reason and preserves data; code reuse is through public contracts, not private tables. Developer tools remain under Browse → Dev with onboarding-controlled enablement.

## 3. The navigation contract

Use the user's supplied layout sketch as the reference:

[Single-sidebar layout reference](assets/app-shell-reference.png)

There is exactly one left navigation sidebar. It contains:

1. App heading: a bounded horizontal app strip, with a tap on the heading opening an app list.
2. Persistent Activity near the top.
3. App-specific navigation, replaced when the selected app changes.
4. Footer controls.

The center is the app's main card, following the existing bb-like inner-card layout. An optional right AI chat pane is always closable. There is no additional global navigation rail and no second left sidebar.

Developer-oriented plugins appear under **Browse → Dev**. Onboarding asks whether to enable development tools; users can change this later. This groups coding workspaces, Git/PR review, terminals, developer previews and browser/device testing without adding another sidebar. Enabling the category configures discovery and activation preferences; it does not grant machine, credential or filesystem access. Ordinary features such as notifications, voice and secure sign-in remain outside Dev.

Conversations, Apps, and Cals are distinct destinations; they do not all need permanently expanded navigation beside every app. The default starting experience is a conversation with the user's chosen main cal.

Activity aggregates permitted work across all the user's spaces, labels its space, and supports filtering. Questions, approvals, failures, and results awaiting review get priority. Ongoing work has a quieter presentation; reviewing an item clears attention without deleting history.

Reuse the existing Calternal search palette. The baseline includes one search/action surface, contextual filters, Collection query editing, AI handoff, plugin search providers, and existing performance gates. The source distinguishes implemented behavior from broader mockup specifications.

## 4. Foundation versus plugins

The foundation enforces authority; each space chooses its product behavior. The proposed foundation is a small host/runtime. It understands contracts and enforcement mechanics, not Notes, calendars, cals, or the orchestrator's product behavior.

**Replaceability has a scope.** Infrastructure modules may be separately packaged and updated by deployment administration, but are not replaceable by tenant code. A space may replace its orchestrator, apps, and other permitted services without changing another space or asking a server administrator. The distinction is about authority, not whether a module is called a plugin.

### Foundation responsibilities — proposed

- **Contract registry and dispatcher:** versioned operation, event, resource, service, and UI-slot declarations; input/output validation; discovery; consistent errors.
- **Execution context:** authenticated principal, acting user or service identity, space, explicit grants, request/run identifiers, and attribution propagated through every call.
- **Enforcement:** mandatory checks at dispatch and storage/network boundaries; plugin code cannot replace the checks for its own calls. Replaceable policy services cannot turn a caller-supplied space ID into authority.
- **Plugin lifecycle:** package verification, loading, compatibility, dependency resolution, activation, disposal, update rollback, and scoped configuration.
- **Isolation and capabilities:** isolated execution for untrusted code and harnesses; brokered file/network/secret access; network egress denied unless a capability allows the destination; quotas and cancellation boundaries. Harness-native tools (shell, file, web fetch, harness-configured MCP servers) do not pass through operation dispatch, so the sandbox boundary, not the dispatcher, is what confines them.
- **Durability primitives:** atomic file operations, the change journal and sync protocol, version-history storage, durable admission, stable action IDs, checkpoints, and recovery coordination. Product-specific histories and workflows build on these; tenant code extends them with format and merge handlers but does not replace them.
- **Resource scheduling primitives:** bounded execution, backpressure, leases, and per-space/per-account resource accounting. Concrete automation and agent behavior remain plugins.
- **Transport and rendering hosts:** connection handling, typed operation transport, UI mounting, and declarative-view validation. Concrete API/MCP/CLI adapters and UI features use the contracts.

### Protected foundation: tenants cannot modify

- Session validation and identity verification, including the trusted passkey/OIDC authentication path.
- The account profile: authenticators, connected provider accounts, and account-local metadata such as personal layout, personal installation records and cross-space Activity review state. It belongs to the account, is not a space, cannot be shared or joined, and is exported with the account. Space membership never exposes it.
- Authoritative membership/grant records, ownership invariants, capability issuance/revocation, and cross-space sharing enforcement.
- Secret custody, authorized account bindings, provider credential isolation, and audit attribution.
- The execution sandbox, storage/network boundaries, deployment resource ceilings and accounting.
- The trusted login, permission-grant, account-attribution and recovery surfaces (another registered sign-in method or authorized administrator-assisted recovery), plus a way to disable a broken personal UI/plugin. These surfaces are not addressable by thread UI operations and are excluded from context capture: a thread can request a grant but cannot press the approval. Recovery is a protected administrative operation, not a tenant-overridable authentication bypass.
- The change journal, sync protocol and version-history storage. Released clients carry their own sync logic, and a space's history is its durability guarantee, so neither can depend on tenant code.

Users still manage their passkeys, connections, memberships and grants through authorized operations. They cannot redefine what a verified identity means, edit protected authority files directly, grant themselves capabilities in a manifest, suppress attribution, or lift deployment ceilings.

The user approved a minimal protected recovery screen that remains usable when the normal shell/orchestrator fails. It can disable a broken personal plugin, restore the last working layout and show diagnostics; space-level changes still require the appropriate role. It is part of the foundation and cannot be replaced by tenant code.

The normal app shell is customizable. Security-sensitive surfaces retain a trusted host boundary so a replacement theme or app cannot counterfeit a grant dialog or capture sign-in secrets.

### Space-owned behavior: administrator participation is not required

Space managers can install, fork, configure, replace, upgrade and disable their space's orchestrator, app backends, workflows, automations, search providers, memory adapters, file handlers, provider integrations, and UI contributions within existing capabilities. A generated app can include server-side behavior in the space's isolated execution environment.

Each person can independently customize their own layout and presentation. Shared defaults and shared app behavior require the space-management permission already agreed. Personal presentation preferences are not accidentally exposed as shared space content.

Resolve services against an installation scope. A space override may replace that space's orchestrator, with a documented deployment default as fallback. A personal UI preference may override a space default. A cal cannot register a global hook to intercept another tenant's operations. Running work pins its implementation/configuration version until a deliberate handoff or migration.

The pin covers code and configuration, never authority. Every operation re-checks current grants at dispatch, so revoking a grant, removing a member or disabling a plugin takes effect on work already running. A security block on a plugin version, or a resource-format migration the pinned version cannot read, also ends the pin: the work pauses at a safe point and is handed off or reported. Retention of superseded versions is bounded by the work still pinned to them.

There are three installation scopes, not two: deployment, space and personal. Surfaces that aggregate across spaces (Activity, the search palette, the shell, the conversation home) run at personal scope, using the deployment default or that person's own override. A space contributes to them only through contracts: its search provider returns results, its plugins emit Activity items. Code installed by one space never executes with reach into another space, including when a member of both invokes it. Personal-scope state lives in the protected account profile, not in a space: one user-visible space type does not require private account data to sit in something that can later be shared. A personal plugin's source is ordinary content in whichever space its author keeps it; the record that says 'this account runs it at personal scope' is in the account profile, so people who can read the source in a shared space do not thereby run it or see its owner's settings. That record pins the version its owner activated: if the source sits where others can edit it, their edits do not run in the owner's personal scope until the owner activates them.

Unmodified plugins receive compatible updates automatically at a safe activation boundary when existing grants suffice and migrations have a tested recovery path. New capabilities still require authorization; failed updates restore the last working version where data compatibility permits. Customized forks retain their changes: offer upstream improvements and let a cal prepare a merged preview before activation, never overwrite the fork automatically. Compatibility includes declared services, data and UI contracts, not just a version label.

Tenant-owned source and configuration remain portable files. Default plugins should support a 'make this my own' fork with an isolated preview and an explicit upgrade path. Forking product behavior must not require forking the host server.

A first-party app wholly compiled into the host cannot be forked by a space without a host release. Separate the compiled default engine (format, merge, indexing), behind a versioned service contract, from the forkable product layer (operation composition, views, workflows) in the tenant runtime. The contract also admits a space-owned engine implementation. All writes still pass through host authorization, validation, commit, journal and versioning. The user approved immediate product-layer customization and incremental engine contracts starting with Notes. Everything outside the protected foundation remains a plugin-API extension/replacement target; staging contracts is delivery order, not a permanently closed layer. This preserves bb-lite replaceability and pi-durable explicit, owned, recoverable work.

These are proposed implementation boundaries. Their API must be documented and tested before migration. The host must not silently accumulate feature logic.

### Multiple execution machines — added requirement, proposed contract

A space defines ownership and collaboration; a machine defines where work executes; a workspace identifies the files and tools available for a task on that machine. A phone/browser is a client, not automatically an execution machine. One central authority stores grants, canonical app content, conversations and durable work records. Users may connect multiple execution machines; this does not create a separate tenant or server instance for each machine. See the [BB/T3 feature-gap assessment](feature-gap-assessment.md).

Recommended first slice: connect an existing Mac or Linux machine through a background execution daemon, initially private to its owner. Enrollment requires proof of control of the machine and an authorized account; merely sharing a space must not enroll or expose a machine. The user can separately grant a space use of selected workspaces/capabilities. Personal enrollment within deployment policy needs no server-admin action; installing the daemon still needs permission on that machine. UI labels are “Computers” and “Working folder”; protocol terminology may remain machine/workspace.

The foundation owns machine identity, pairing/revocation, authenticated transport, scoped execution grants, invocation ownership/generations, quotas and audit. Machine providers, provisioning, workspace setup/teardown, routing policy, health/update UI, terminals, previews and Git integrations are plugins. A machine's locally enforced permission ceiling cannot be raised by a cal or ordinary server plugin. Effective execution authority additionally intersects the machine grant and local ceiling; host-native tools must be confined on that host.

Daemons connect outbound over authenticated encrypted transport; the exact direct/relay transport is a validation choice. A run binds a machine, workspace, provider/account reference and required capabilities. Advertised OS/tools/provider readiness are checked before dispatch. The user approved explicit routing or a saved computer default; moving a thread to another machine is a deliberate handoff with file/artifact transfer, not transparent process migration. Selecting a new machine never silently copies a credential or borrows a login from that machine's owner.

Machine operators can inspect data delivered to their machines. Shared-space authority alone does not authorize exporting data to an arbitrary participant's computer: dispatch requires permission to use that execution destination and send the specified data. Canonical Calternal edits still return through the protected commit route; external project files remain machine-owned working files until explicitly imported/published. Keep artifacts referenced by durable messages accessible after workspace cleanup through an authorized durable copy.

If a machine sleeps/disconnects, show waiting/interrupted/unknown status and preserve pending work. Never automatically duplicate an uncertain external action elsewhere. A disconnected daemon cannot immediately learn revocation: bounded execution leases, local expiry enforcement and current-authority checks on reconnect limit this gap. Stale invocations cannot commit; network partitions do not imply the old process stopped. Remote execution, reconnect, revocation and duplicate dispatch require an additional validation spike before this capability ships. “Server-side execution” means independent of the viewing client; a selected personal computer must remain available for its assigned work.

### Distribution — accepted roles and installation, implementation validation pending

Ship the central Linux server as a versioned OCI image, with rootless Podman as the documented self-hosting path. Include the Rust server, built web assets, first-party plugin bundles and pinned plugin runtime/bundler so ordinary startup does not depend on downloading its own runtime. Keep data in documented persistent volumes and supply backup, migration and health-check procedures. The image is packaging, not sufficient tenant isolation by itself.

Ship one native Rust executable, `calternal`, providing CLI commands (including the `cl` entry point) and background machine-service roles; the Linux server image uses its server role. Connected execution hosts initially support macOS and Linux. The machine role installs as a user-level launchd/systemd service. The macOS Apple GUI bundles and automatically installs the CLI, without silently enrolling the computer for execution. Mobile Apple apps do not expose a system shell installation; desktop CLI installation is the concrete platform target. Respect existing unrelated command names and installations; report installation/PATH failures clearly. Validate signed/notarized distribution and sandbox/App Store constraints for the selected desktop channel rather than promising an untested installation mechanism. It connects outbound to the central server using the same versioned protocol whether that server is containerized or runs directly. It discovers compatible existing harness tools and offers on-demand installation of official tooling into its own managed directory, without overwriting the user's installation. Verify downloaded artifacts using available publisher integrity/authenticity mechanisms, record exact versions and preserve recovery/update state. Install only runtimes needed by enabled capabilities; Xcode and other platform SDKs remain platform prerequisites. Do not describe the entire working product as dependency-free or a single executable.

The central server must not receive a general host container-engine socket. Any local execution broker accepts narrow authenticated operations and enforces authorization, isolation and resource ceilings independently; a remote daemon has no special trust merely because it shares the server's computer. Hosted tenant jobs require a proven hostile-workload sandbox; a user-owned Mac must not be advertised as an equivalent shared-tenant host without separate proof. Whether execution is isolated inside the server container or uses a separate supervised executor is decided by V1. Native machine enrollment is validated by V7. A Podman VM on a Mac is not the default route to host Xcode or Simulator access.

This split permits one-computer self-hosting and several execution computers without requiring a different protocol or a distributed authority database. A native all-in-one server installer may be added later, but maintaining it is not required for the initial supported deployment path. Machine enrollment does not require installing the Apple GUI; installing the GUI includes CLI installation but does not enroll a machine. Pairing uses a short-lived code confirmed in the signed-in browser, including for headless hosts; no SSH credentials are required. The machine authenticates the intended server, enrollment binds its own identity, and successful pairing establishes outbound encrypted transport. A self-hosted server must be reachable; pairing does not create reachability.

Machine-service updates are automatic when safe and idle: verify authenticity/compatibility, drain admissions around activation, keep a recoverable previous version and never interrupt active work silently. Coordinate GUI-bundled and standalone installations so competing updaters cannot overwrite a running version. Provider CLI updates remain separately managed. Server image updates are a separate deployment policy.

### First-party plugins

All first-party features use documented contracts available to compatible extensions. Installation scope and declared privileges differ; first-party branding is not an authorization bypass. The last column is a proposed classification that follows from the protected list above; it adds no restriction to the space-owned list.

| Plugin area | Owns | Who can replace it |
| --- | --- | --- |
| Trusted identity/access modules — deployment-controlled | Passkeys, OIDC, session verification, authoritative grants and sharing enforcement; not tenant-overridable | Deployment only |
| Spaces | Space lifecycle and membership UX through protected operations; ownership transfer and sharing invariants remain enforced by the host | Deployment only for the protected operations; space/personal for the UX |
| Files, synchronization, history | File/resource identities, synchronization, version/history semantics, conflict handling | Deployment only for journal, sync protocol and history storage; space for file handlers and merge handlers |
| Conversations | Durable visible threads, messages, participants, provider-session references, attachments | Deployment only for conversation identity and the durable record; space/personal for the experience |
| Orchestrator | Home/conversation experience, request lifecycle, coordination, delegation, contextual app interaction | Space |
| Cals | Saved assistant definitions, scoped abilities, memory bindings, sharing/copying/import/export UX | Space |
| Provider adapters | Codex, Claude Code, later other CLI harnesses; scoped native session controls and normalized events; credential custody remains protected | Space; credential custody stays deployment-controlled |
| Message board | Group conversation experience, presence, addressing the lead cal, collaborative interaction | Space |
| Activity | Cross-plugin attention inbox, running-work presentation, reminders and review state | Personal (aggregates across spaces); spaces contribute items |
| Automations | Scheduled and independently backgrounded work using the same durable execution services | Space; the durable execution service stays deployment-controlled |
| Search palette | Provider aggregation, search/action discovery, filters, contextual actions, AI handoff | Personal for the palette; space for search providers |
| Collections | Live queries, explicit membership/exclusions, named views, embeds, sharing | Space |
| App plugins | Notes first; then existing product areas such as Tasks, Calendar, Mail, Files, and others | Space: product layer, and engines through their contract (depth in section 13) |
| Shell and component library | Customizable app navigation, sidebar/footer slots, main-card layout, optional pane, supported GenUI components; trusted grant/login surfaces remain host-controlled | Personal/space for layout and contributions; deployment only for trusted surfaces and the component set a native client ships |
| Voice | Subscription-backed live voice, audio transport, interruption and delegation into ordinary conversations | Space |
| Memory integration | Honcho adapter and memory processing/retrieval | Space |
| Provider-account integrations | Optional CLIProxyAPI/account pooling and other account adapters | Space for adapters; deployment decides whether pooling is offered |
| Feedback | Captured current view, relevant diagnostic context, transcription, private report preview and sending | Deployment for the report destination; personal for capture |
| Machines and workspaces | Connect/manage computers, select working folders, route work, provision and clean up environments through protected execution contracts | Personal/space for providers and UX; identity, grants and local ceilings remain protected |
| Importers | bb and bb-lite conversations, attachments, projects, assistant setups; compatibility reporting and account reconnection | Space |
| Protocol adapters | API, MCP, CLI (`calternal`, alias `cl`), native-client integration, and other protocol adapters | Deployment only: they terminate authentication |

Competing compatible app plugins use explicit default handlers and an Open with choice. Installing a plugin does not silently take over existing resources or deep links. Handler selection changes presentation/operations within the format contract, not canonical resource identity, ownership or permissions; unsupported formats remain preserved.

Some plugins are required by the default product profile. Product services can have space-scoped replacements through versioned contracts. Only deployment administration replaces trusted infrastructure modules. Removing a required provider without a compatible replacement must produce a clear dependency failure. The plugin registry exposes both compatibility and allowed installation scope.

## 5. Rust and dynamic plugins — recommended approach

Decide three things separately: the shared client core, the server host, and plugin authoring. They do not need the same language.

| Area | Recommendation | Reason and limit |
| --- | --- | --- |
| Shared client core | Rust with platform adapters for native apps, CLI and MCP; shared contracts and conformance fixtures for every client including the browser | Swift, CLI and MCP link the same core. The browser client is TypeScript on the same contracts; no browser Wasm in the initial scope (see below) |
| Tenant plugins | TypeScript with a generated SDK and isolated worker runtime | Fast authoring, familiar tooling and bb-code adaptation; no Rust build required to add or edit a plugin |
| Server host | Rust is the current recommendation after the Opus assessment, subject to the first measured validation slices | Reuse suitable Calternal storage/sync/domain code; revisit if re-keying or isolation/IPC results invalidate the case |
| Web presentation | TypeScript and Svelte 5, reusing the existing web stack and component vocabulary | Browser DOM/editor and sync hot paths stay TypeScript, with shared protocol contracts and conformance fixtures |
| Apple presentation | Native UI, with the Rust core through UniFFI | Share behavior without replacing native controls with a web UI |

A TypeScript server could reduce early tooling friction. It still needs the same isolation, credential protection and durability machinery; worker threads or a JavaScript VM do not isolate malicious tenant code by themselves. Rust's resource control and existing code reuse are reasons to evaluate it, not proof that it will outperform the alternative in this product.

For dynamic personal extensions, prototype a server-side JavaScript/TypeScript worker runtime behind capability RPC and operating-system isolation. This aligns with AI-generated extensions and potential adaptation of existing bb plugin code. A JavaScript VM alone is not the security boundary.

Execution should be lazy and resource-bounded. Avoid an always-running process for every installed plugin or every user. Lazy start requires each plugin to declare what wakes it (operations, event subscriptions, schedules), and it has a cost on interactive paths: a tenant search provider started cold cannot be held to the palette's first-result budget (section 11), so built-in providers answer within the budget and tenant providers merge in under a deadline.

The strength of the operating-system boundary is unchosen and must be set against the hosted threat model: public signup plus AI-generated server-side code and shell-capable harnesses means mutually hostile tenants on one machine. The existing server ships as a rootless podman image (ADR 0009), which limits which nested sandbox mechanisms a self-hosted deployment can use. Hosted and self-hosted deployments may need different mechanisms behind one capability contract. Choose actual worker granularity through security and performance tests; do not co-locate mutually untrusted plugins merely to save memory.

Wasm is not part of the initial scope, as a plugin runtime or in the browser. This is a preference for experienced responsiveness and for one plugin runtime, not a measured finding about Wasm. Any later use needs its own evidence and justification.

Rust-written extensions are compiled when built; that does not inherently require rebuilding the server to install an independently hosted extension. Compiled-in Rust modules do require a host release. External workers can be updated separately through their protocol contract.

The web renderer uses shared components. AI-generated UI is a validated composition of those components with actions bound to registered operations. Native Apple renderers later implement the same view contract. Truly new native widgets may require an app release; existing supported components can be rearranged dynamically.

This is a recommended prototype target, not a measured performance result. Rust does not by itself guarantee responsiveness, and dynamic plugins are not allowed to bypass budgets.

### Plugin authoring experience — proposed

The intended loop is: create an app from a template, edit its Markdown/configuration/TypeScript and declarative views, preview in the user's space, then activate. The host handles TypeScript bundling; authors should not need a Rust toolchain, server rebuild/restart, marketplace review, or administrator installation for a personal plugin within granted capabilities.

Generate discovery, CLI/API bindings, documentation and validation from the same plugin contract. Show compilation/runtime errors against the plugin's source. A cal uses the same development/preview/activation operations as a person and can run the loop automatically in approve-for-me mode.

Activation checks API compatibility, requested capabilities and migration requirements. New capabilities need the appropriate grant; changing a manifest is only a request. It follows that a cal cannot approve a capability request for a plugin it wrote or for itself: approve-for-me covers actions inside authority already held, and a grant that widens authority (a new network destination, a new account binding, wider file scope) is given by a person who holds it. Retain a last-working version, version active runs, and keep content/history on uninstall unless the user explicitly erases data. Data migration rollback needs a real compatibility/snapshot strategy, not merely reverting JavaScript files.

### Day-one shared client core — proposed boundary

Create the shared client contract and the portable core alongside the first server/client contract. Do not build a web-specific sync client and promise to extract shared logic later.

"Shared" has two layers, and they should not be confused:

1. **One protocol definition for every client.** Versioned wire contracts, generated types for Rust, Swift and TypeScript, and one corpus of conformance fixtures (operation, sync, reconnect, conflict, error and cancellation scenarios) that every client implementation must pass in CI. This layer is required from day one whichever option below is chosen.
2. **One implementation.** A portable Rust core linked by the Apple apps (UniFFI), the CLI and the MCP server. This matches the existing accepted decision (ADR 0072), which did not include the browser.

The server is the only sync authority: it orders the journal, decides merges that are saved, and owns permissions, final writes, history and expensive jobs. A client implementation only tracks cursors, queues and retries its own edits, and applies what the server confirms. Two client implementations of one tested protocol are therefore not two sync authorities; two untested implementations would be, which is why layer 1 is not optional.

The Rust core owns portable document/resource models and compatible codecs, sync cursor/reconciliation state, conflict/retry decisions, domain commands/reducers, typed errors, and the fixture runner. It can reuse existing pure Rust crates after a portability audit. `crates/client` named by ADR 0072 does not exist on `origin/dev` yet, so this is new work, not extraction.

Platform adapters own effects: Apple networking, Keychain, file coordination/hydration and application lifecycle; for the CLI, plain files and a token file. The core emits effects and consumes results so protocol decisions are not duplicated per platform. Rendering, focus, animations and native controls remain platform-specific. This does not move agent execution or durable jobs onto clients.

#### The browser

The browser client is TypeScript: the editor, the sync client and every per-keystroke path. It uses the generated contract types and must pass the same conformance fixtures as the Rust core. No Wasm build of the core is in the initial scope. The cost is that client protocol logic exists twice, in Rust and in TypeScript, and parity is only as good as the fixture corpus; that corpus is therefore a release artifact with the same standing as the contracts, and a protocol change lands with its fixtures.

This is a responsiveness preference, not a measured comparison, and TypeScript is not assumed to be fast. The gates are end-to-end: main-thread time during typing while a sync is in flight, time per sync batch, and startup, on the oldest target phone and a desktop.

Browser constraints:

- Several tabs share one origin storage. Exactly one context (a SharedWorker or an elected leader tab) owns the cursor and outbox; otherwise two tabs race on the same queue.
- Live collaborative-editing state stays in the browser editor while a document is open (ADR 0043). The sync client carries its updates as opaque binary; it does not hold a second copy of the open document.
- Existing cached-note offline reopening/editing/reconnect and offline preferences are the reuse baseline, not a new subsystem to postpone. The browser already has IndexedDB Yjs persistence, close/hide recovery, service-worker shell caching, and conflict/reconnect logic. Broader plugin operations declare their offline capabilities; this is not a claim that every future app works offline.

#### Native bindings

Use UniFFI for Swift. Its limits shape the interface: streams are callback interfaces rather than a native stream type; byte buffers are copied across the boundary, so large content moves by file path or handle, not as arguments; and UniFFI does not support cancellation directly, so cancelling a Swift task stops neither the Rust work nor the server work by itself. Expose explicit operation IDs and cancel methods for anything that must stop.

On Apple platforms the core runs in more than one process (app, File Provider extension, share extension), and extensions have tight memory limits. Local state therefore needs cross-process locking and a core that can open with a small footprint. Local-state schema changes need forward migration across app updates.

Client and server versions skew in both directions: App Store releases lag the hosted server, and self-hosted servers lag the App Store client. The protocol needs capability negotiation at connect and a stated support window for older peers; a client facing a server it cannot speak to must fail with a clear message, not corrupt state.

#### Dynamic plugins and clients

Dynamic plugins publish runtime schemas and render through supported components. Installing a plugin must not require rebuilding the core or every native client. Typed SDK bindings can be generated for known contracts; unknown compatible plugin operations use the generic validated contract path.

Tenant plugins are TypeScript on the server, so their format and merge logic cannot run inside a native client. For a resource kind defined by a tenant plugin, a client treats the bytes as opaque, never rewrites what it cannot parse, and leaves merging to the server (the recommended answer in ADR 0048). Offline and optimistic behavior for such kinds is limited to whole-file edits. The same applies when a space replaces a default engine with its own implementation: native clients keep the compiled default for kinds it still matches and fall back to opaque handling otherwise.

Illustrative starting module boundaries:

- `contracts`: versioned wire definitions and conformance fixtures, with generated Rust, Swift and TypeScript bindings.
- `client-core`: portable Rust state machines and domain logic.
- `client-web`: the TypeScript browser sync client on the generated contracts.
- `client-native`: UniFFI bridge and native platform effects.
- `server-host`: identity/enforcement, lifecycle and durability primitives.
- `plugin-sdk`: TypeScript authoring APIs, development runner and schema tooling.

The first proof runs the same operation and sync/reconnect/conflict fixtures against the Rust core through a minimal Swift binding harness and against the TypeScript browser client, and records matching error, cancellation and conflict behavior. Measure the UniFFI bridge (call overhead, payload copying, batching of many small calls into few) and the browser gates above. This proves actual reuse without requiring a complete Apple UI in the first web milestone.

## 6. Plugin contract sketch — proposed

A plugin declares:

- Stable identity, version, compatible API versions, dependencies, execution/install scope (deployment, space or personal).
- Activation triggers: the operations, event subscriptions and schedules that start it, so nothing runs while idle.
- Private state: where plugin-owned records live inside its installation scope, so they are covered by quota, export, backup and **Erase Data too?**.
- Operations with typed input/output, documentation/examples, error types, read/write classification, permissions, retry/idempotency behavior, and cancellation support.
- Resource kinds with stable IDs, file formats, versioning, schemas and migration declarations. A file records its format version; a migration snapshots what it rewrites; a handler never rewrites bytes it cannot parse.
- Events with typed payloads, visibility rules, attribution, and replay semantics where applicable.
- UI contributions: app entry, sidebar items, views, settings, search providers, and allowed component compositions.
- Jobs: whether work belongs to a request or lives independently, resource needs, progress, and recovery policy.
- Requested capabilities: space-scoped files, specific network services, account bindings, and delegated external actions.
- Upgrade/disposal behavior and data-preserving uninstall behavior.

The registry supplies machine-readable discovery and human-readable documentation. Commit a machine-readable contract snapshot and gate breaking drift in CI, covering operations, events, services and UI contributions; check caller compatibility and replacement-provider compatibility separately. Version bumps and migration declarations accompany incompatible changes. The same operation appears through API, MCP, CLI and native adapters, with the same validation and permissions. Transports may differ; semantics do not.

Extension hooks declare observe, transform or provide semantics, scope, deterministic ordering, deadlines and failure policy. Each service slot has one selected implementation per scope; conflicting activation fails explicitly. Observer failures cannot change operation success, and transformations cannot widen authority or bypass host checks. Hooks must be registered extension points, not arbitrary interception of foundation behavior.

Apps never reach into another app's private tables or paths. They call operations and consume authorized events. The caller chain is recorded without permitting plugins to impersonate a more powerful actor.

### Effective authority — proposed rule

Every operation runs with the intersection of three things, checked by the host at dispatch against current records:

1. what the requesting person may do (or, for scheduled work, the person or role that set it up);
2. what the acting cal or thread has been granted;
3. what the executing plugin's installation scope and capabilities allow.

Consequences:

- A space-installed plugin invoked by a member acts only inside that space, even if the member has access elsewhere. Its author gains nothing from being invoked by a more powerful person.
- A shared cal directed by one participant cannot exceed that participant's authority. Standing authority delegated to a cal (maintenance, automations) is bound to the person who granted it and the context it was granted for: it applies to work that person or an equally authorized person set up, not to any request the cal later receives.
- Content is data, not instruction. Text a thread reads from a shared note, a web page, an email or a tool result carries no authority, and approve-for-me does not turn it into the acting person's request. Because injected instructions cannot be reliably detected, the controls are structural: default-deny egress, capability grants that only an authorized person can give, and attribution of every action to the run that took it. Approve-for-me as accepted is broad and covers sharing and inviting within the person's authority; requiring a person's confirmation for actions that disclose data outside a space would be a new product policy and is proposed, not adopted (section 13).
- A resource shared into another space keeps its owning space's plugin as the authority for writes and migrations of its kind. The recipient needs a compatible renderer and otherwise falls back to the plain file. Owning-space code handling a recipient's request acts only on the shared resource, with the recipient's grant.

Threads reach operations through the same adapters as other clients (MCP or the `calternal` CLI inside the harness sandbox) using a credential scoped to the run, short-lived and revoked when the run ends. The registry is expected to be large, so the harness gets discovery and search operations rather than every operation as a separate tool.

Personal plugins can change their user's UI and implement apps inside their authorized workspace. They cannot patch shared server software, configuration, or other users' spaces. Globally installed plugins do not automatically receive every space's content.

Provider plugins receive scoped account handles rather than ambient access to every credential. Where a CLI harness needs actual credential material, supply only its authorized account in an isolated harness environment. An adapter's ability to change its endpoint does not authorize sending an existing provider token to that endpoint.

Isolating the harness is not enough by itself. Codex and Claude Code can run model-directed shell commands, and their login material is held in the harness's environment. The threat to test, per CLI, account type and sandbox mode, is whether a command the model is persuaded to run can read that material; if it can, so in effect can anyone able to direct the cal. The requirement is that provider credentials are not readable by model-directed tools: for example, the tools run in a separate sandbox from the harness process, or an egress proxy outside the sandbox attaches the credential and the sandbox can reach only the provider endpoint. Whether each CLI supports such an arrangement with each account type is a validation (section 13).

Until a protected mechanism is validated for a provider and account type, that account cannot be bound to a shared cal or otherwise used on behalf of people other than its holder. A warning is not a substitute: this follows from least privilege and the accepted account-delegation rules (section 8), and is a gate on the integration, not a new choice. A person's own account used in their own threads exposes the credential to no one else, but the same test applies to it against injected content.

Harness session files (the CLI's own transcripts and resume state) contain conversation content. They are stored as space data under the same protection, export and erase rules, and kept durably if native session resume is expected after a restart.

The public contract covers app control as well as content. Navigation, selecting an app, opening a resource, arranging a pane, changing permitted settings, installing a personal plugin and managing work must be discoverable operations. Persisted changes use ordinary server actions/history. Ephemeral UI commands target an explicit client/view session; durable work continues if that client disconnects. In a group thread, a cal must not move every participant's screen because one person asked it to open a note.

The precise SDK, wire format, migrations, and service-replacement rules are engineering deliverables still to design and test.

## 7. File over app

Chat storage inherits the existing accepted AI design from 2026-11-02: flat `AI/Cals/` and `AI/Chats/` directories, one Markdown file per cal/chat, message blocks with stable IDs, fenced card blocks and folded steps blocks. A branch is a new chat file with `branched-from:` provenance. A server message index supports efficient access and is rebuildable; Markdown is authoritative content, not merely an export. The earlier proposal for a conversation folder containing separate message files was incorrect and is withdrawn. Carry these relative paths into the space-owned namespace without inventing another chat hierarchy. Attachment placement must follow existing file/attachment conventions; no new per-conversation attachment layout is approved here. Later Files decisions put app-authored metadata and sidecars in `.calternal/` keyed by stable IDs, while imported originals remain untouched. Later app-root decisions use `Tasks/` and `Calendar/`, superseding old Notes subfolders; see the inheritance audit for precedence. Original provider session state remains separate from the portable chat file.

On 2026-11-16 the user accepted: editing a sent message preserves its original in version history and never silently reruns actions; deleting a chat leaves independently created documents, tasks and apps intact; external Markdown edits import automatically as attributed revisions without triggering AI work. Externally edited text cannot forge approvals, actor identity or completed action receipts. Explicit retry/fork/send controls govern new execution. Branching never replays historical side effects.

Source: existing Calternal `origin/dev`, `ops/orchestrator/memory/calternal-decisions-ai.md`, Round 2 R2 and Round M storage decisions. These are recorded product decisions, not a claim that the chat backend has already implemented them.

Content remains portable files, Markdown-first wherever applicable. Collections, cal definitions, and portable app configuration should have readable file representations. Runtime records may need structured journal files; Markdown-first does not require encoding every machine event as a prose document.

Existing documents remain useful if a plugin is disabled or replaced. Removing an app preserves data/history by default, with an explicit **Erase Data too?** checkbox.

The user approved shared cals automatically building space-owned memory, separate from personal memory. Cals in the same space share authorized remembered knowledge while retaining separate personalities and working preferences. Shared knowledge never overrides source-level restrictions or a cal’s narrower grants. Cal memories have a portable, inspectable representation owned by the space, while Honcho supplies processing/retrieval through an adapter. The exact synchronization/export contract needs validation; do not assume all Honcho internal state is reproducible from a Markdown note.

Proposed enforcement: every memory has stable identity, provenance (source resource/message IDs and versions), attribution and effective visibility. Space ownership does not imply every space member may read every memory. Ingest only authorized source context, and keep personal memory out of shared-cal processing unless explicitly contributed. Derived memories cannot broaden their sources' visibility: mixed-source memories retain the combined restrictions or are separated into independently supported claims. Before retrieval/inference and before publishing a response, check current requester, cal and destination-audience authority. Do not query one unrestricted mixed-private memory pool and filter its answer afterwards. The Honcho adapter must prove isolation/filtering or use separate safe partitions; unsupported restricted sources stay out of shared processing. Provider sessions/caches/compaction summaries likewise cannot carry restricted material into a less-authorized conversation.

Revocation invalidates derived-memory retrieval and affected caches; recompute or withhold memories whose support is no longer authorized. Already disclosed information cannot be recalled. Provide human-readable “What this cal remembers” with source links and correction/removal controls under the same permissions; the user approved Forget preventing automatic relearning from the same historical sources. Maintain a protected suppression record keyed to the forgotten claim and its source lineage; invalidate dependent memory representations/caches and require equivalent suppression in the adapter. Do not erase the source conversation merely because its derived memory was forgotten. Suppression is not permission to hide matching source text from an explicitly requested document read; it prevents re-promoting it as remembered knowledge. Apply suppression within the authorized memory scope, not to another person’s private memory. An adapter that cannot enforce this cannot claim Forget is complete. The user also approved preserving attributed disagreement: conflicting statements are not silently collapsed to the latest one; ask for resolution when it matters, and retain provenance for explicit agreed corrections. Automatic learning is visible and reversible, using the inherited quiet Remembered/Undo pattern. A participant leaving revokes their future access but retains their prior shared contributions under the space retention rules, including legitimately supported shared memories; it does not transfer their private memories or personal provider credentials to the space.

Storage accounting follows logical ownership and file size, not physical allocation: charge canonical content once to its owning space, funded by the space owner’s plan by default. References shared into other spaces do not incur another content charge; independent copies are separate logical files and count separately even when XFS reflinks share physical extents. Physical dedupe must not leak another account’s matching content through quota discounts or visible behavior. Ownership transfers show and validate the destination quota impact before commit. Detailed accounting for retained history/Trash and plugin runtime storage still needs a published breakdown consistent with existing retention rules. At quota, pause storage growth while preserving pending work; allow reads, export and cleanup, never silently delete content or mark unacknowledged edits saved. Reserve bounded control/recovery capacity so the system can record Stop, revoke access and safely clean up even when the content allowance is exhausted; this is not an unrestricted content-growth grace allowance. Expose recoverable local pending edits when the server cannot accept them.

Ordinary portable exports include the authorized content, conversations, memories and portable plugin configuration, omit credentials, and guide account reconnection after import. Sanitize embedded secret fields rather than assuming configuration files are secret-free. Export authority does not include another member’s private data merely because the exporter owns a space. Import does not automatically activate arbitrary plugin code or restore old grants, machine enrollment, pending external actions or provider sessions as executable work. Administrative disaster-recovery backups are separate protected consistent snapshots, not ordinary shareable exports.

Indexes are rebuildable. Durable conversation/job/security state must not be mislabeled as disposable cache; its authoritative records and crash-consistency rules require an explicit storage design. Credentials use protected secret storage, not ordinary shareable content files.

File-over-app governs content. It does not require grants, sessions or job admission records to be loose files; they need transactional updates (a revocation and the check that honors it must not disagree) and a readable export. A deployment therefore has several stores: space files, the journal and history, authority records, secrets, and external memory state. Backup and restore must treat them as one set: restoring older authority records beside newer content would bring back revoked grants and shares.

Existing Calternal storage is one Home, one change journal and one search index per person (ADR 0044, ADR 0031), and shared files stay in the owner's Home. Ownership by spaces changes the storage root, the journal partition and the cursor: a person in several spaces follows several journals, each filtered by current access, and a revoked share must stop appearing at the next read rather than at the next reconnect. This is a redesign of the sync contract's partitioning, not a reuse of it (section 11).

Harness file tools work in isolated working areas. Changes to live content pass through Calternal's action/conflict/history path. This should feel like co-editing, with incremental visible results and attribution.

The working area is a checkout, which sets three rules. Placing a file in it is a read, so the run's authority is checked when files are materialized, not only when they are written back. Each file records the version it was copied from; write-back is an ordinary action against that base, merged by the rules for its kind (ADR 0045), never an overwrite. The working area is kept until every change in it is written back or explicitly discarded, so a crash or cancellation between the harness's edit and the write-back does not lose work.

Sharing a Collection includes current and future matching resources within the sharer's authority, with preview and disclosure. Query/membership changes affecting a share require appropriate management permission. Sharing must not amplify access to borrowed resources the sharer cannot onward-share.

## 8. Conversations, cals, and provider handoff

- Cals are optional. Ordinary conversations can use a provider/model directly.
- A cal adds saved personality, abilities, and memory. Internal files such as SOUL.md are implementation details, not customer-facing terminology.
- Cals belong to spaces. Shared cals use space-scoped memory and permissions. Copying starts fresh unless the user selects **Include memories** and reviews what will transfer.
- Each conversation has one lead cal at a time when a cal is present; it can delegate to helpers.
- Completed helper/subthreads automatically archive (user decision, 2026-11-16). Recommended lifecycle: archive only after terminal completion and durable result linkage to the parent; retain attention for failed/blocked work, and preserve history/artifacts under their own retention rules. Archive changes visibility, not grants or execution, and does not delete a workspace or cancel independent work. Reopening/follow-up can return the same helper to active state. Do not automatically archive ordinary user-started threads.
- Inherit branch-from-any-message, Cal/model changes within a chat, Cal/model attribution per reply, selected-passage replies, edited indicators and time-aware context. Forking never replays actions. The composer and attachments reuse note-editor components; artifacts are real collaborative documents. The optional AI pane and full conversation view share one thread, and the AI app does not nest a second AI pane inside itself.
- Group conversations are explicit shares. The lead cal responds when addressed or assigned work, rather than to every human message.
- Calternal owns conversation identity and history. Provider sessions are replaceable execution records beneath that identity.
- Selecting a new model/provider does not interrupt work. Changed options apply to the next submitted message.
- Deliberately sending a message offline creates an account/server/conversation-scoped durable pending submission that is visible, editable and cancellable. Reconnection automatically submits it with a stable request ID and the selected cal/model/computer settings captured for that message; reconcile acknowledgement loss before retrying. Re-check current grants and destination membership, preserve attachment dependencies, and surface unavailable selections instead of silently choosing replacements. Unsent drafts are distinct from submitted pending messages. Offline submission does not imply offline agent execution or universal offline plugin support.
- Ordinary messages steer by default. If a provider change is pending, the next message triggers a handoff: complete or safely interrupt current work at a supported boundary, end the old turn, then send the new provider the message and continuation context in the same visible thread.
- Same-provider model changes can use native session facilities when supported. Cross-provider changes need explicit history/context transfer; hidden model state is not transferable.
- Returning to an older provider session requires reconciling events it missed.
- A shared cal uses provider accounts explicitly authorized for its space, with visible allowance attribution. It never silently uses the last speaker's account. Billing/account identity is separate from permission to act on a resource.

### Accepted orchestration defaults — 2026-11-16

- **Managed provider authentication:** Calternal manages provider authentication rather than requiring manual per-computer file setup. The user prefers direct auth-file management if simpler; adapting/forking CLIProxyAPI is an alternative to evaluate, not an approved dependency or fork. A provider adapter declares supported login, storage, refresh, expiry and revocation behavior; credential custody and authorized account-to-machine/run binding remain protected foundation responsibilities. Validate each actual CLI/account flow before claiming portability. Account refresh must have explicit ownership so multiple machines cannot race token rotation or overwrite newer state. Transport/storage must protect secrets, logs/context/exports must exclude them, and revocation/unpairing must end future access. Reuse the existing shared-account credential-isolation gate; managing login files does not prove model-directed commands cannot read them.
- **Closed but unmerged work:** retain its task worktree and branch until explicitly discarded. PR closure alone is not merge/preservation proof and never triggers the merged-work cleanup policy.
- **Browser/device takeover:** the user clarified temporary yield, then automatic resume. Human takeover pauses cal input for that browser/device while unrelated work may continue. Renew the idle timer on human interaction; display a visible countdown before resuming and offer Keep control. Do not resume during active gestures/composition, a trusted credential/permission interaction or an explicit Stop. Re-check the control lease and current authority before resuming, and refresh the cal’s view of the changed page/device. The exact inactivity interval is a UX tuning choice; no duration is approved yet. Input ownership remains exclusive.

- **Coding isolation and review:** Git-backed coding tasks use isolated worktrees and branches with a PR workflow by default. Direct-folder execution remains an explicit choice; unsupported/non-Git folders require a clearly described alternative, never a falsely claimed worktree. Multiple helpers should not silently write the same checkout. PR creation, publishing and merging obey existing service grants; choosing this workflow is not blanket authority to merge or rewrite remote history. Completed helper archival does not remove worktrees, uncommitted work or linked review artifacts.
- **PR follow-through and cleanup:** the user approved monitoring PR checks until ready for review, addressing failures within the authorized task scope and surfacing blockers or review requests. This does not authorize automatic merge. Once a PR is merged and work is fully preserved, automatically remove its task-owned worktree and local task branch. Never delete default/protected branches, unrelated/user-owned checkouts, dirty or untracked work, branches with additional unmerged changes, or resources still used by another thread/process/worktree/review. Verify merge/preservation against exact revisions, including squash/rebase cases; simple Git ancestry is not sufficient for every merge method. Preserve durable results/artifacts first, use race-safe reference deletion and idempotent cleanup with retryable diagnostics. Unknown/offline state waits rather than assuming the worktree is unused. The user explicitly approved deleting both local and remote task branches. Remote cleanup uses the existing repository service grant, verifies the exact expected remote head immediately before conditional deletion, respects protection rules and preserves branches still used by other work. A changed head or uncertain merge/preservation proof blocks deletion and surfaces an actionable cleanup item; already-deleted branches are a successful idempotent outcome. Helper auto-archive remains independent of PR and filesystem cleanup.
- **Unattended work limits:** users who do not select developer during onboarding receive adjustable per-request work limits by default, with visible paused progress and Continue. Developer onboarding does not enable these additional per-request limits by default; they may be enabled later. Deployment resource ceilings, account/service grants, provider limits and tenant fairness remain mandatory in either mode. Track active work separately from allowance-reset waits. Limits and mode changes must be visible and cannot silently revive stopped work; numeric defaults are to be validated, not invented here.
- **Machine selection:** use the explicitly selected or saved computer. Unavailability waits visibly; no automatic relocation, credential transfer or destination change.
- **Provider allowance:** absent an explicitly configured authorized fallback, pause for the provider limit and automatically resume when it resets. Persist the waiting reason and next eligibility time so server restarts do not lose the wakeup. Use reported reset/retry information when available; unknown resets use bounded availability checks, not fabricated countdowns. Re-check current grants, account, machine availability, cancellation and invocation ownership before resuming; a stopped request never wakes itself again. A configured fallback stays within its account/service/spending grants and continues the same visible thread with attribution. Reconcile uncertain effects before retrying; a limit during a turn does not authorize replaying completed actions. The UI distinguishes waiting for allowance from waiting for a person.
- **External notifications:** default to attention items (questions, required approvals, failures), with opt-in top-level completion alerts. Ordinary helper completion does not notify separately. Notifications link to the exact authorized item, deduplicate delivery and honor user preferences; sensitive preview content is excluded by default.

## 9. Durable execution: lessons from pi-durable

Borrow the philosophy, retaining Codex/Claude Code harnesses:

1. Work outlives a process, tab, pane, or connection. Persist accepted intent, progress, pending work, and outcomes.
2. Distinguish received, admitted, delivered, running, waiting on a person, answered, cancelled, failed, and outcome-unknown states. Reconnecting clients reconstruct current state. Streamed model output is not journaled token by token: the stream is ephemeral, checkpoints and the final message are durable, and a reconnecting client resumes from the last checkpoint.
3. Request-owned helpers stop with their parent. Independently scheduled work has an explicit separate lifetime.
4. Use stable action/run IDs and reconcile outcomes before retrying. A crash after an external action but before recording its result is an uncertainty to resolve, not permission to repeat it blindly.
5. All agent, plugin and job execution runs on the server. Client rendering, microphone capture and user input do not own the lifecycle of agent jobs.
6. Restarting an execution daemon may end its harness processes. Restarting the central server need not kill remote harnesses, but reconnection must reconcile ownership before accepting results or launching replacement work. Each run records enough to be resumed, handed off or reported as interrupted afterwards; planned restarts drain or checkpoint first.

Host commits carry an invocation identity/generation; cancelled, finished or superseded invocations cannot commit ordinary late results. Persist cancellation intent and check it atomically with admission/commit. If multiple hosts can execute the same work, lease fencing is additionally required. Recovery has bounded restart budgets: unavailable code, incompatible versions, failed migrations and repeated crashes become inspectable blocked work in Activity, with authorized resume/migrate/abandon operations. Inspection must not start execution. Registered effects declare replay-safe, idempotent-with-key, or reconcile-before-retry behavior; persist intent before dispatch and retain outcome-unknown when reconciliation cannot establish the result. Exercise these races using the [validation plan](validation-plan.md).

Do not promise that arbitrary CLI internal execution can be replayed exactly, or that cancellation can undo sent email, payments, or publishing. All actions need attributable history; reversible actions offer undo, and external effects may require a compensating action.

Approve-for-me is the default. Personal plugin creation/activation is permitted within existing authority. External messaging, spending and publication use one-time per-cal/per-service grants. The user also approved this approach for external sharing, invitations and public links: acquire the relevant permission once, then do not prompt repeatedly within its scope. Changing that scope needs a new grant. Attribution and revocation remain available; revocation cannot recall received copies.

## 10. Context, feedback, voice

Each message includes appropriate context from the active app pane: resource identity/version, selection, visible controls and relevant view state. Screenshots can supplement structured context. User-facing descriptions such as 'the half-pie button' should resolve against that context.

Context attachment must honor conversation visibility. A private pane next to a shared conversation must not silently expose private data. The same holds across spaces: context from a pane showing one space is not attached to a conversation handled by another space's orchestrator without the person's explicit act. Passwords and secrets are excluded from routine context capture.

Feedback starts from the current view, prefills relevant context, offers voice transcription, and lets the user review the private report before sending. Sharing that report is an explicit act, not broad support access to the space.

Subscription-backed live voice is a first-class plugin target. The Codex app-server realtime path is a concrete implementation lead. A separate API-key route is optional, not a prerequisite. Voice delegates work to the ordinary cal/thread rather than owning a separate execution/history system.

## 11. Performance and reuse

Reuse existing Calternal domain code, file safety, identities, action/sync contracts, Notes, history, sharing and palette designs where they meet the new contracts. Reuse requires inspection and evidence, not blanket rewriting or blanket preservation.

This proposal departs from accepted Calternal decisions in these places. Each needs a superseding record before the affected code is reused:

| Existing decision | What it says | What changes here |
| --- | --- | --- |
| ADR 0009 | One Rust server | Host language is a candidate under evaluation |
| ADR 0010 | Plugins are areas compiled into one server | Tenant plugins load dynamically in an isolated runtime; compiled modules remain as default engines |
| ADR 0044, ADR 0031 | One journal and one search index per person | Spaces own content, so journals, cursors and indexes are partitioned by space |
| ADR 0072 | One Rust client for native apps, CLI and MCP; the web is not included | Unchanged; the browser stays TypeScript and is bound by shared contracts and fixtures (section 5) |
| ADR 0048 | Rust format plugins are shared by server and clients | Holds for native clients and default engines. The browser and tenant-defined kinds rely on server-side merge |
| Sharing decisions (#409) | A shared file stays in the owner's Home; named people on this server or a link | The owner is a space; sharing to a whole space is new |

Collections already cover live queries, hand-added items, exclusions, named views and live embeds. Their mockup design is a baseline; the full cross-app engine is not established as shipped.

Existing palette records specify first results under 50 ms, parser work at most 8 ms per keystroke on the oldest target phone, and backend p95 at most 10 ms for 10k notes / 30 ms for 100k notes. Preserve and revalidate those gates; these are requirements, not newly measured results. They were set for an in-process provider over one person's index; a palette that spans several spaces and calls isolated providers must be re-measured against them, not assumed to inherit them.

### Performance gate plan

The target is 120 Hz interaction and animation: a frame interval of about 8.33 ms on a 120 Hz display. On a 60 Hz display the limit is that display's own refresh interval. This is a gate on design choices and on release, not later polish. It is a target to be measured against, not a result: nothing here has been measured for the new architecture, and no architecture can promise that no frame is ever late. What it can do is make late frames rare, measured and attributable.

Design rules that follow from the target:

- Nothing expensive runs on a UI thread. Plugin code, model and harness work, indexing, sync reconciliation, parsing of large documents and history diffing run in workers or on the server. The UI thread applies results.
- UI and plugin operations are coarse. One user action causes a bounded number of cross-process or cross-thread calls; lists, search results, history and Collection updates arrive as batches sized to fit within a frame, not as one message per item.
- A declarative view from a plugin is validated and laid out off the UI thread where the platform allows, and is rendered by first-party components whose frame cost is known.
- Optimistic UI is explicitly provisional. A local edit shows at once and is marked pending until the server acknowledges durable storage; the UI never shows a write as confirmed before that acknowledgement (ADR 0047), and a rejected edit is visibly reverted or kept as a conflict, never silently dropped.
- Work is instrumented end to end with one trace identity from input event through client, transport, host, plugin worker and back to paint, so a late frame or slow action can be attributed to a stage.

What is measured, as distributions rather than averages:

| Area | Measurement |
| --- | --- |
| Input response | Input-to-next-paint for typing, tapping, selection and palette keystrokes |
| Animation and scrolling | Frames that miss the display's deadline, and longest run of consecutive misses, during sidebar transitions, pane open/close, app switching and scrolling long lists, conversations and history |
| UI thread | Count and length of tasks that exceed one frame interval; long tasks during streaming output |
| Startup | Cold and warm start to first usable view, and to first interactive conversation |
| Search | Palette first result and per-keystroke update, including across several spaces and with tenant providers present |
| Live views | Rendering and incremental update of version history, Collections and live embeds, and of a streaming conversation |
| Memory and idle | Client memory after startup and after long sessions; client and server cost of installed but idle plugins and of open but idle conversations |
| Under load | Every UI trace above repeated while plugins and cals run representative work for the same account and for other tenants on the same host |

How it is run:

- **Datasets.** Seeded, versioned fixtures at stated sizes, starting from the existing Calternal sizes (10k and 100k notes) and adding long conversations, deep version history, large Collections and several spaces per account. The representative plugin and cal load is itself a fixture.
- **Device matrix.** Real hardware with 120 Hz displays for the 120 Hz target (desktop and phone, each supported browser, and later the native Apple clients), plus the oldest supported 60 Hz devices measured against their own refresh limit. Each result records device, display refresh rate, operating system and browser version, power and thermal state, and build.
- **Regression baseline.** The existing Calternal budgets and perf harness (ADR 0005, `perf/budgets.json`) are the starting baseline: the same scenarios run against the new architecture, and a result worse than existing Calternal on a shared scenario is a regression to explain, not a new normal.
- **Merge gate.** Budgets are enforced per change on dedicated quiet machines, as ADR 0005 already requires; the 120 Hz device runs that cannot be automated per change run on a stated schedule and before each release.

To specify before testing begins, not to be filled in afterwards with whatever passes: the supported-device list, and the percentile acceptance budget for each measurement on each device class.

Add explicit tests/budgets for tenant isolation overhead, plugin startup/idle memory, sidebar transitions, streamed conversation rendering, history loading, provider handoff, and large multi-space Activity feeds. Installed but unused plugins should not impose continual execution cost.

Heavy cal/plugin work is queued under resource allowances. Keep ordinary app use responsive, expose progress, and allow stopping/reprioritizing work.

bb/bb-lite imports include conversations, attachments, projects and assistant setups with provenance. Reconnecting accounts is guided; imported history does not guarantee native provider-session resumability. Adapt useful bb plugins only when compatibility, licensing, isolation and performance are acceptable.

Licenses of the sources named in section 14, as published on 2026-11-16: bb-lite, pi-mono (pi-durable), T3 Code, hermes-agent and CLIProxyAPI are MIT; Codex is Apache-2.0; UniFFI is MPL-2.0; Honcho is AGPL-3.0. All can be combined with an AGPLv3 product with attribution kept. The local bb plugin checkouts carry no license file, so their terms must be confirmed before any code is adapted. Claude Code is distributed under Anthropic's own terms, not an open-source license; how it is installed in a deployment is a release gate (section 13).

## 12. Order of work

The [validation plan](validation-plan.md) defines the early parallel spikes, exit criteria and Apple development gates; proofs precede broad implementation. The [first usable build plan](early-use-plan.md) makes the plugin SDK, GenUI and shared-core contracts concrete and distinguishes a proposed personal-use checkpoint from the full release. The user approved the intermediate usable checkpoint and portable GenUI first on 2026-11-16; arbitrary web-only HTML/JavaScript surfaces are outside that early scope. Independent Fable 5.2 medium and GPT-6.1 Sol medium reviews are complete; their reconciled contract refinements and checkpoint matrix are in the first usable build plan. First-use deployment and first Apple target remain sequencing decisions; no storage/security guarantees have been waived.

1. Review the refined authority boundaries, shared-client design, and faithful single-sidebar orchestrator mockup. Specify the supported-device list and performance budgets, and stand up the measurement harness (section 11) so every later step is measured from its first slice.
2. Specify the operation/event/resource/service contracts and the conformance fixtures. Start the shared client core immediately and run one operation and one sync/reconnect/conflict scenario through a minimal Swift binding harness and the TypeScript browser client.
3. Prove a dynamically installed TypeScript personal plugin can be previewed, activated, revoked, upgraded and recovered without crossing a space boundary or rebuilding the host. Include a hostile plugin and a harness run that try to read another space, reach an unlisted network destination and read provider login material. Use this slice to validate the host/runtime recommendation ([server host assessment](server-host-assessment.md)). Then build the minimal orchestrator plugins with Codex and Claude Code, normal threads, cals, durable message admission and same-thread handoff.
4. Integrate Notes as the first real app: two isolated users, shared resources, human/cal co-editing, version history, Collections integration as supported by its engine.
5. Include the required first-experience surfaces and plugins: foundation-backed sign-in and space management, Activity, shared conversations, palette, personal memory, subscription voice, feedback, and a bb-import path. Each increment passes its applicable performance, isolation and recovery checks before the next expands the surface; the first plugin-under-load gate precedes this broader set.
6. Run the full performance gate under representative load, and validate crash recovery, permission revocation, plugin upgrade/uninstall, and native-renderer contract viability before migrating the remaining apps.

## 13. Remaining validations, release gates and open decisions

[The architecture review](architecture-review.md) records the reasoning behind the items in this section.

### Engineering validations

These need experiments or detailed contracts, not guessed answers:

- Host-to-dynamic-plugin RPC cost and batching, isolation mechanics, worker lifecycle and resource ceilings; the sandbox mechanism for hosted and for rootless self-hosted deployments.
- Rust-versus-TypeScript host implementation cost and measured behavior on the same plugin slice; shared-client language is a separate decision.
- Native UniFFI portability, explicit cancellation, payload copying and call batching, multi-process access to local state, and conformance of the Rust core and the TypeScript browser client against one fixture corpus.
- Journal, cursor and index partitioning by space; access-filtered sync across several spaces; revocation latency.
- Storage for authoritative runtime/security records (transactional, exportable), atomicity, journal compaction, index rebuild, and consistent backup/restore across all stores.
- Actual Codex/Claude steering, interruption, handoff and restart behavior, including partially completed external actions and resume after a host restart.
- Whether model-directed tools can read provider login material, per CLI, account type and sandbox mode, and a mechanism that prevents it (section 6). Shared-account use of a provider is gated on this passing.
- Working-area checkout, write-back and merge for harness edits, including crash between edit and write-back.
- Honcho memory export/synchronization and subscription-voice adapter integration.
- API versioning, client/server capability negotiation and support window, file-format migrations, compatibility declarations, plugin upgrade rollback, and the limits of version pinning.
- Engine service contracts: what a default engine exposes, and what an alternative implementation must satisfy.
- The performance gate plan in section 11: supported-device list and percentile budgets specified first, then measured, including under plugin and cal load and across tenants.
- Import coverage and license-compatible reuse of existing/plugin code.

### Release gates

These are checks against real terms and real behavior before the affected feature ships. None reopens a settled decision.

- **Provider credentials and distribution.** For each provider and account type offered (subscription login, API key, pooled accounts through an adapter): confirm against the provider's current published terms and the CLI's license that the intended use is permitted (server-side use on behalf of the account holder, use by a shared cal on behalf of a space, pooling), and how the CLI may be installed in a hosted or self-hosted deployment. Record the terms consulted and their date. No prohibition is assumed here; none has been established either.
- **Apple distribution of an AGPLv3 client.** Confirm that every part of the shipped Apple client can be distributed through the App Store under the terms its copyright holders have given, including any outside contributions and each linked dependency.
- **Plugin authors' rights.** State the license terms of the plugin SDK and generated bindings so that a tenant's ownership of plugin source they wrote is unambiguous.
- **bb plugin reuse.** Confirm license terms for any bb plugin code before adapting it; the local checkouts carry no license file.

### Product decisions resolved after review; remaining compatibility policy

The user resolved the first three choices on 2026-11-16. Source inspection resolved the mistaken assumption behind the fourth. The exact client/server support period remains unspecified; it does not block capability negotiation or current development.

1. **Fork depth — resolved.** Product behavior/UI/workflows are forkable immediately; compatible engine contracts open incrementally, beginning with Notes. All product behavior outside the foundation remains replaceable through the plugin API.
2. **Account recovery — resolved policy.** Contact an administrator after losing all passkeys/linked sign-ins. Specify and validate the protected identity-verification, authenticator-reset, notification and audit procedure; no ordinary admin content viewer is added.
3. **External sharing — resolved.** One-time scoped permission, with no repeated confirmation while it remains valid. Revocation ends future access but cannot recall copies already received.
4. **Offline browser reuse — verified baseline.** Reuse cached-note offline reopening/editing and reconnect/recovery, plus offline preferences. Adapt browser databases/channels/caches and sign-out wiping to account/server/canonical-resource identity and space grants. Cross-space references must retain one canonical editing identity. Tests were inspected, not rerun; first-ever offline startup, all uncached content and arbitrary offline plugin actions are not established capabilities.
5. **Version support window — duration unspecified.** This is the promise about which older app/newer server and newer app/older self-hosted server combinations continue to work. Negotiate capabilities now and explain incompatible features clearly. The user approved capability negotiation now and deferring the exact supported period/version range to native release planning on 2026-11-16; no duration is assumed approved.

A change to a settled product behavior should return to design review.

## 14. Evidence consulted

- Existing Calternal `origin/dev` (`ea57b84b9` when reviewed): `ARCHITECTURE.md`; `perf/budgets.json`; ADRs 0005, 0009, 0010, 0011, 0031, 0043, 0044, 0045, 0047, 0048, 0072; current product decision records, including the sharing decisions for #409.
- Existing AI mockup: `origin/review/calendar-2` at `f02b05d5e43a0644b04d7b4f053ad160574be965`, `apps/mockup/src/ai/`.
- Collections decision records and `origin/feat/collections-db-fixes` mockup; mockup storage is not evidence of production file persistence.
- bb-lite kernel/contracts: https://github.com/get-bb/bb-lite/tree/19e1804266d476a89def44d07361e2f992cec4aa
- pi-durable philosophy and limitations: https://github.com/earendil-works/pi-mono/tree/747c217b81980206063a89e46645b362de7ac8b1/packages/durable
- T3 Code attention inbox: https://github.com/pingdotgg/t3code/blob/629d5083b7b0278edf0a5f587f25b8d40c3fdfb2/docs/user/thread-sidebar.md
- Official Hermes cataloged subscription voice: https://github.com/NousResearch/hermes-agent/blob/0309b005235ba6f7c4d6f8518411aabd00dbf346/plugin-catalog/hermes-live-voice.yaml
- Native Hermes subscription implementation under review: https://github.com/NousResearch/hermes-agent/pull/136326
- UniFFI Wasm limitations (background to the no-Wasm initial scope): https://mozilla.github.io/uniffi-rs/latest/wasm/configuration.html
- UniFFI asynchronous APIs and cancellation: https://mozilla.github.io/uniffi-rs/latest/futures.html
- wasm-bindgen futures/Promises: https://wasm-bindgen.github.io/wasm-bindgen/reference/js-promises-and-rust-futures.html
- Rust browser-Wasm target constraints: https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html

- Offline browser evidence inspected on existing Calternal `origin/dev` at `ea57b84b9c9e40e8b67b7d1e1f1805915123b040`: `apps/web/sw/sw.js`, `apps/web/e2e/pwa.spec.ts`, `apps/web/e2e/lazy-offline.spec.ts`, `packages/editor/src/note.ts`, `packages/editor/src/offline/local-copy.ts`, `packages/editor/src/offline/replay.ts`, `packages/editor/test/local-copy.test.ts`, `packages/editor/test/sync.test.ts`, `packages/editor/test/recovery.test.ts`, `apps/web/src/lib/prefs-store.ts`, `apps/web/e2e/prefs-offline.spec.ts`, `apps/web/src/lib/wipe.ts`. The generic `action-outbox.ts` has no located production consumers; it is not evidence of general durable offline app actions.

## Storage inheritance follow-up, 2026-11-16

Existing ADR 0062 also specifies tiered user trees and protected recovery metadata, including Home-only recovery after materialization and a one-second acknowledged-edit materialization bound. The storage design must reconcile those accepted requirements with space ownership, durable job receipts and the multi-machine plan before claiming inheritance is complete. Do not assume every non-content record is disposable, or silently replace the existing recovery model with database-only authority. This requires a focused storage compatibility review; no new storage engine or physical layout is selected here.
