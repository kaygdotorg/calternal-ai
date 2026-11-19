# A smaller Calternal server

Source inventory at `85add29af840b225dd9a83cae2e5cd4ce3aecfc9` on `lane/server-inventory`. The decision record calls the seal revision 12.5, while the checked-in specification heading reads 12.3. References here point to the checked-in text and catalogue. The later owner decisions govern the proposed revision. Interactive work has a three-second plugin budget after a separate wake allowance. Longer work becomes a job. In particular, Turns, host tables, one-mode search, a worker-backed New Note, and the row examples require changes. The generic-engines proposal is read at `origin/lane/api-v2-step0` `3421680277b77c998cc42b063136ce2109bd45d0`. Its first review was against `5c4ac691`; the later proposal has a disposition at line 321. This report labels proposals and gaps separately from implemented behavior. `docs/open-questions.md:44–59`, `docs/plugin-api-v2.md:1,329,1101`, `docs/description-contract.md:405–425`, `contract/host-catalogue-v2.json:1201`, `docs/proposals/plugin-api-v2-generic-engines.md:304,321` (proposal branch).

## Where to draw the boundary

Keep the host as the place that can answer **what exists, who can see or change it, and what survived a crash** without starting a guest. Let packages decide what their files mean, which work should happen, and how it appears. Keep common interaction components on clients, drawing package descriptions and trusted host actions. A useful test is to turn the space off and uninstall the default package. A current member must still be able to reach their files, export them, manage sharing, inspect pending work and stop it. A guest must see only their live grant or the authorized saved public view. Waking a package can reinterpret a file or act on it.

Migrate Notes first. Qualify the generic item and room paths with a second independent package over the same format before removing the native route. Then migrate Threads with both provider adapters and an independent runner using identical primitives. Disable unqualified guest prompting, profile combinations, and cross-audience indexing. Keep the three roles distinct: light plugin VM, on-demand heavy plugin VM, and on-demand agent VM holding rootless containers. No userdata tree is mounted into any guest. Readers, agents and indexers use purpose-scoped broker calls. Independent agent execution placement and leases belong below policy; the runner is replaceable. `docs/open-questions.md:45–50,57–58`, `docs/plugin-api-v2.md:187,940,1101`, engines proposal §§ A.2, B.3–B.4.

## What else can move

Each row describes a proposed ownership boundary. An open engine or catalogue facility needs a numbered revision and qualification before the replacement is enabled.

| Candidate | Package ownership, host facility, and what becomes replaceable | Security, convenience, and performance |
| --- | --- | --- |
| Automations and schedule policy | `calternal-orchestrator` owns the automation Markdown, recurrence editor, permission defaults, late-run behavior and turn inputs. Any package can write an automation over its own actions. Host owns an indexed due time, enrollment, stable occurrence identity, cancel/takeover, claim/outbox and current-creator checks. Use sealed Schedules and Jobs; open: safe next-due publication and bounded catch-up without a daily wake. Prefer a small checked recurrence form for first release or prepublished deadlines with a durable renewal alarm. | Revoked author stops, Managers see why and can take over. Host rejects duplicate claims even across restart or clock rollback. Preserve once on return within the same local day, otherwise skip with a reason, and default to Approve for me. A due job can pay wake cost, but showing upcoming rows and changing authority stays cold. One indexed next-due scan per due batch replaces per-tick enumeration of spaces or Markdown. `crates/server/src/automations/scheduling.rs:1,592`, `crates/server/src/automations/scheduling/worker.rs:334,361`, `docs/open-questions.md:55,65–68`, `docs/plugin-api-v2.md:329–352`. |
| Skills and instructions | Move reserved skill-folder reading, skill grammar, instruction inclusion and ordering to an orchestrator/skills package using typed item and range access. Another package can offer a team playbook or a different prompt assembler. Keep bounded slot lists and attachment/mention resolution; introduce a typed contribution declaration if sharing skills between packages. | The host gates each item and checked attachment. Treat instructions as data and preserve the host/agent permission hierarchy. A saved skill list remains searchable asleep; assembling it may wake at Send. Page and range in batches, pin exact revisions, and avoid one broker exchange per line. `crates/server/src/api/search_skills.rs:1`, `crates/content/src/instructions.rs:1`, `crates/server/src/turns/instructions.rs:1`. |
| Tags, backlinks and rename | Notes owns Markdown, heading and tag grammars, relative-link rules, backlink presentation and rewrite policy. A community wiki can use its own parser or tags with a shared indexed scalar/set and reference projection. Keep rename, revision fencing, inverse receipts and grants in host. Sealed typed metadata is a starting point; open: trusted revision-bound relation publication, bounded multi-item rewrite and room-origin extraction. | Hidden incoming refs must never leak through a count or snippet. Deliver cold authorized projections and show indexing lag. Keep edited source and generated fields revision-bound. Rename the target first only if linked changes and conflict behavior remain explicit, with a resumable plan and checked Undo. No scan of every file and no VM callback under a storage lock. `crates/server/src/notes.rs:9,75,101`, `crates/content/src/rename_job.rs:15,315,417`, `docs/plugin-api-v2.md:329,753`. |
| Retention and Trash | Notes owns Trash views and copy, with the accepted 30-day default expressed as host lifecycle policy. A records package could request a supported policy where operator limits and the owner allow it. Host pins candidates and performs restore/purge, clock checks and retries, including while no package exists. Open: manifest policy representation, bounds and precedence if configurability is allowed. | A package must never cut another editor’s restore window or bypass Manager/Owner purge. Users can restore a trashed file without waking. Expiry work is bounded by due index and batch byte budgets. `crates/server/src/notes/retention.rs:1`, `crates/content/src/trash_job.rs:1`, `docs/plugin-api-v2.md:341,759`, `docs/open-questions.md:73`. |
| Agent-provider sign-in and readiness | Adapter packages own provider/model choices, safe display labels, login recipe and interpretation of checks. The host and qualified machine harness own account/space binding, pinned versions, protected prompt transport, routing, expiry, credential isolation, and factual machine/capacity/readiness receipts. Sealed secrets, settings and credential calls cover pieces, engines § A.2 adds execution, and a typed provider-profile and private setup boundary is open. A community runner can offer a different provider without a hardcoded server adapter. | A provider login is separate from Calternal identity. One member must not receive another’s sign-in challenge or credentials. Render last known readiness and its age asleep; an explicit check or login wakes the worker and can start a machine. One bounded value per profile and no machine polling on every row or keystroke. `crates/server/src/provider_sign_in/mod.rs:293`, `crates/server/src/provider_sign_in/flow.rs:43`, `crates/server/src/provider_sign_in/inbox.rs:1`, `crates/server/src/machines/readiness.rs:1`, `docs/agent-turns.md:222,287`. |
| Appearance and themes | Move palettes, presets, theme rules and settings descriptions to a per-person appearance package or client contribution. Keep native material and accessibility rendering in clients and a bounded personal preference store on the host. A community collection can offer palettes while the client enforces the accepted readability floor. The sealed contract has typed settings and app chrome; open: a restricted initial-shell appearance contribution. | A space package must not control global sign-in chrome or another person's UI. Cache the last applied personal selection with a safe baseline before a VM can wake. No palette computation, request per role, or remote fetch on shell paint. `crates/server/src/api/appearance/prefs.rs:1`, `crates/server/src/api/appearance/db.rs:1`, `docs/open-questions.md:60,78`, `docs/description-contract.md:247`. |
| Fonts and wallpapers | Appearance packs own remote source adapters, catalogues, attribution, presets and crop choices. Host owns authorized asset custody, bounds, safe delivery, cache partitioning and the proxy. Fonts need a checked download/upload path and platform-level validation; wallpapers can use typed asset refs, opaque published variants and background jobs. A person can choose a local wallpaper set or a different licensed font catalogue. | Validate and cap decoded formats, deliver fonts and images without sending viewer credentials to catalogues, keep old cached assets and system fallbacks at startup. Browsing fresh catalogues may wake; applying a saved ref stays cold. Cache by digest and rendition, queue fetch/decode separately from saves. `crates/server/src/api/appearance/http.rs:1`, `crates/server/src/api/appearance/face.rs:1`, `crates/server/src/api/appearance/remote.rs:1`, `crates/server/src/api/wallpapers/catalog.rs:1`, `crates/server/src/api/wallpapers/upstream.rs:1`, `crates/server/src/api/wallpapers/cache.rs:1`, engines § B.2. |
| Onboarding | An onboarding package owns tours, suggested starter packs and progress through optional steps. Keep first owner, invitation admission, sign-in, recovery codes and verification routes trusted. Use sealed settings, jobs and description, with a small open personal-shell entry point. An operator could supply a welcome checklist for a team. | A tour never collects sign-in or recovery secrets. Account creation and recovery remain available with zero spaces awake. Only personalized tips or sample creation may wake. Read one small completion marker during first-run, and defer suggestions and starter content. `crates/server/src/account_http/onboarding.rs:1`, `crates/server/src/onboarding.rs:1`, `crates/server/src/owner_setup.rs:1`. |
| Palette and settings pages | Packages own indexed commands, keywords, shortcut suggestions and settings sections. The renderer owns inputs and navigation; host merges indexed contributions against authority and owns signed-in account/security routes. Package settings already have a surface; cross-app appearance/palette placement and collision handling are open. A package could publish a specialized command provider or settings pane. | Passive discovery cannot dispatch an action. Reserve protected action IDs and distinguish publishers. Keep opening the palette and filtering at key-repeat speed cold; use activation for expensive suggestions with cancellation when selection changes. Bound candidates and issue one result page per query, never wake all installed packages. `crates/server/src/api/search.rs:1`, `crates/core/src/palette.rs:1`, `docs/plugin-api-v2.md:398,662`. |
| Notifications and wording | Packages own event intent, subscriptions to package events and selectable wording within a bounded template. Host owns account/installation subscriptions, authorization at delivery, secrets, quotas and dispatch. Use durable jobs/events and add an open notification intent engine. A mail or task plugin can ask for the same delivery facility and the person can choose a digest. | For Threads, host supplies the authorized title and allowed needs-you/finished state, with no message text. A plugin-supplied arbitrary title/body would bypass that restriction. Queue after a committed event, coalesce by message identity, and drop revoked envelopes. Alerts still reach the person when the source space has slept. `crates/server/src/web_push/mod.rs:1`, `crates/server/src/web_push/db.rs:1`, `crates/server/src/web_push/transport.rs:1`, `docs/open-questions.md:65`. |
| People and invitations UI | A package may own member views, onboarding guidance, team directory organization and entry points. Use scoped account/space metadata and description actions; cross-plugin settings links and a safe directory Bind are open. Core owns role tables, invitation issuance, accept/revoke, the protected audience sheet and `Shared with me`. A team package can group profiles or explain onboarding in its own terms. | Keep invitation tokens, hidden groups and access-changing controls behind the trusted route. Access administration stays cold, including removal of the package's installer. Cache bounded authorized profile labels and invalidate on directory version; no full directory load for mentions. `crates/server/src/api/people.rs:1`, `crates/server/src/api/sharing.rs:1`, `crates/auth/src/invites.rs:1`, `crates/auth/src/spaces/membership.rs:1`, `docs/plugin-api-v2.md:753,811`. |
| Search ranking and indexing | Packages choose declared source fields, produce bounded text/segments/features and can propose ranking presets or background analysis. Host owns authorization-first corpora, ranked page execution, stable cursors and invalidation. Sealed Queries supply bounded shapes; open: combined title/body/message ranking and qualified ranking profiles. A space can pick e.g. recency bias, tags first or optional semantic related results. | Changing a hidden document must not change visible rank or count. Host scorers only consume scoped inputs; experimental plugin search is an explicit job, not a replacement for cold ordinary search. Keep search publication off save ACK. At 32 scopes, adding FTS to the save changed p95 from 30.685 to 66.775 ms. `crates/content/src/search.rs:1,53,367`, `docs/measurements/plugin-api-v2-step0.md:348–368,416–473`, `docs/open-questions.md:50`. |
| Import and export | Importers own format mapping, duplicate policy, edit previews and diagnostic reports. Host grants bounded ingress, stable IDs, range reads, checked commits and a consistent per-space export with readable manifests. Current Notes port is file movement by owner decision. An independent importer could add an external format without the server learning it. Whole-space checkpoint/export protocol and large generic import batches are open. | Export only currently authorized items and authorized versions of their attachments. Use checked names and paths, bound expansion and stage before publish. Plain export can finish with packages asleep; interpretation and large imports are explicit jobs. Batch staging avoids wake and commit per record where possible. `crates/fsio/src/beneath.rs:1`, `crates/fsio/src/bulk.rs:1`, `crates/content/src/publication.rs:1`, `docs/open-questions.md:69`, engines § B.5. |
| Live rooms policy | Notes owns block editor behavior, selection affordances, room opt-in and the Markdown representation. Keep the sealed generic room protocol, membership, revision guards, checkpoint, backpressure and presence attribution. A third-party Markdown editor can adopt the declared format and Yjs profile. An alternative room protocol needs a versioned host qualification, and revision-bound metadata for the existing Yjs save path is open. | Guests cannot forge cursors or keep receiving data after revocation. A saved room must open and sync without a package wake. A background parser may lag; it must never sit inside the room ACK path. Preserve compaction and external-editor conflict handling. `crates/server/src/live_rooms/access.rs:73`, `crates/server/src/live_rooms/delivery.rs:1`, `crates/server/src/live_rooms/update.rs:100`, `crates/content/src/rooms/external.rs:1`, `crates/content/src/rooms/wire.rs:1`, `docs/plugin-api-v2.md:344,541`. |
| Requests and approvals | Packages own typed questions, explanation and next-step workflow. The renderer can show contextual package details beside the protected host request card. Host owns the person-only decision, verified action, live proof, attempt binding, receipts and wait winner. Existing sealed approval and Jobs cover most; engines § A.4 adds native attempt custody. An independent runner can provide a better escalation flow without controlling Approve. | A package text label cannot change the operation that is approved. Pending decisions and safe dismissals remain cold; only the winning continuation may wake. One winning transaction, bounded progress, no waiting worker. `crates/server/src/requests.rs:1`, `crates/server/src/api/request_action.rs:1`, `crates/server/src/plugins/jobs/waits.rs:1`, `docs/plugin-api-v2.md:374–396,753`. |
| Agent memory, summaries and snooze | Memory providers, title suggestion and retry strategy belong to packages. Threads owns what counts as a finished conversation or when to suggest a title. Persist declared status and per-person snooze via projections and a cold expiry rule. A wiki memory package or a different naming policy can replace the built-in behavior. Generic viewer projection and cold deadline semantics for registered fields need qualification. | Scope private provider state to its account and exact attempt. Never bring memory into the core or make private titles visible to a guest. Lists and expiry stay cold; compute title or summary as a bounded job. `crates/server/src/threads/title.rs:1`, `crates/server/src/threads/snooze.rs:1`, `crates/server/src/threads/turn_events.rs:51`, `crates/content/src/db.rs:62`, `docs/open-questions.md:65`. |

A package can implement expensive indexing, similarity and media transforms with disposable scratch attached to an isolated plugin job. Use source-local outputs first. When an output depends on several items, its audience must fit within every input audience and a change or revoke must retire the result. Scratch is per package and space, rebuilt from files, excluded from backups and snapshots, and retired on uninstall. People read through indexed item fields and supported assets. A heavy job on a personal machine needs both the space's and machine owner's consent, with revocation checked at broker delivery and publication. Start with the hosted heavy role and asynchronous similarity results. `docs/open-questions.md:45–49`, engines §§ B.2–B.5.

### Measured costs that constrain the split

- A 256 MiB Firecracker fixture before explicit reseed measured 269.12 ms warm and 488.74 ms cold settings p95, with a cold maximum of 862.76 ms. Twenty simultaneous settings wakes raised p95 to 922 ms cached and 2,519 ms evicted. The independent kernel plus Node reseed experiment measured 219/316/341 ms p50/p95/max for its broker path and covered one Node process. Treat these as separate experiments. Awake simple broker calls were 1.859 ms p95 on the 256 MiB arm, measured one invocation at a time. Avoid a chain of calls for every row. Heavy VM and shared-template/XFS bulk paths have no established budget in these measurements. `docs/measurements/space-runtime-walls.md:20–56,99–123,303–320`.
- The scoped dynamic collection fixture visited 201 rows per 200-row page at sampled offsets with covered indexes. The full-text Notes scope uses one snapshot per page with an extra lookup per delivered row and zero commits/flushes. A shared-corpus BM25 candidate leaked a private edit through a score change. Already-qualified Notes and Threads fields fit the depth-200 probe, and thread search catches up asynchronously. These ready windows measure separate modes. `docs/measurements/plugin-api-v2-step0.md:28–39,294–305,348–368,424–473`.
- The proposal branch measured combined search. At depth 200 the slowest measured Notes window was 5.958 ms p95 and the slowest ready Threads window was 1.192 ms p95. The title-first, deduplicated thread merge uses one scoped read-only snapshot and zero commits or flushes. It matches AND terms within one title or one message. Busy Threads visibility lag reached 562.630 ms p95. Weighted and semantic ranking remain unmeasured. `docs/measurements/plugin-api-v2-step0.md:660–727` (proposal branch).
- The staged 2 MiB save probe measured 168.451 ms p95 and identical reconstructed bytes. A normal acknowledged update includes one item header and placement-guard read plus receipt/sequence reads, a FULL content commit, FULL journal publication and FULL outbox retirement. Range gets are indexed BLOB reads with no flush, staged chunks have no synchronous body flush, and file/directory placement has its own barriers. The product small-save gate is 40 ms p95, with 70.92 ms seen on the documented warm Notes arm. Measure user-save intent to durable ACK with thumbnail capture off that path. The proposed bulk batch and execution append have no verified read or flush count in this lane. `docs/measurements/plugin-api-v2-step0.md:185–227`, `docs/performance.md:7–42`.

## What must stay in the host

| Boundary | What breaks if it follows the application into the guest |
| --- | --- |
| Identity and sign-in | First owner, invitations to the service, passkeys, recovery, OIDC account links, installations, cookies and rate limits exist before there is a trusted space. A guest choosing identity or linking by email could grant itself another person's account. Account security mail and escrow remain host transport. `crates/server/src/account_http.rs:1`, `crates/server/src/account_http/oidc.rs:1`, `crates/auth/src/oidc.rs:1`, `docs/open-questions.md:62–63`. |
| Membership and sharing | The host is the source of membership, roles, groups, collection/item grants, recipient discovery, revocation versions and share links. A package filtering its own results could leak membership or continue serving after revoke. A public link resolves against a saved view under live host grant, and a guest is routed to the item's home space with one-item rights. Keep the audience chooser, personal separation, and all protected access administration usable asleep. `crates/auth/src/sharing.rs:1`, `crates/server/src/api/sharing/guest.rs:1`, `crates/server/src/api/sharing/eviction.rs:1`, `docs/plugin-api-v2.md:811`. |
| Item store and durable commit | The authoritative writer stages bytes, checks grant and expected head under a shared fence, records effect and receipt, and publishes a recoverable journal and file placement. If a plugin can ACK its own write or own the only database, a killed guest or uninstall can lose the file or duplicate the effect. An uploaded Markdown file still passes the same checks. Keep conflict, room and filesystem reconciliation below parser policy. `crates/content/src/mutations.rs:1`, `crates/content/src/publication.rs:1`, `crates/content/src/files.rs:1`, `crates/content/src/recovery.rs:1`, `crates/store/src/highwater.rs:1`, `docs/plugin-api-v2.md:335–370`. |
| Lists and search | Do the authority filter before candidates, counts, joins and ranking. Retain indexed scope bounds, watermarks, result cache fencing and stable cursors. A package query callback would wake on browsing and could leak hidden input through rank and counts even if the output rows are filtered. Packages publish source metadata or bounded derived features. `crates/content/src/discovery.rs:1`, `crates/content/src/search/worker.rs:1`, `docs/measurements/plugin-api-v2-step0.md:33–39,363`. |
| Broker and tickets | Match the enrolled channel, package revision, generation, subject, space, purpose, target and expiry before each host effect and delivery. A plugin issuing its own ticket can confuse package/agent roles or impersonate a Manager. Invocations get 60 seconds per run with fresh authority on renewal. Machine attempts separately require leases, local termination deadlines and exact proof for reattach. `crates/server/src/plugins/broker.rs:450`, `crates/server/src/plugins/worker/session.rs:1`, `docs/plugin-api-v2.md:187,378`, engines § A.2. |
| VM lifecycle and machine execution | Host owns the package image stamp, startup, resource caps, sleep snapshots, process reaping, scratch isolation and machine fencing. A guest cannot prove its own wall, reseed itself reliably on behalf of all workers, or fairly admit competing tenants. VM snapshots are disposable and never custody for committed user files. `crates/server/src/machines/admission.rs:1`, `crates/server/src/machines/state.rs:1`, `crates/machine/src/sandbox/microvm/vmm.rs:1`, `crates/machine/src/sandbox/microvm/thread.rs:1`, `docs/open-questions.md:47`. |
| Egress proxy | Keep user-approved install destinations, target checks at resolve/dial/redirect, caps, and anonymous media delivery outside guest code. Otherwise a worker’s HTTP wrapper can ignore the grant or route through Calternal credentials. A No network package gets no route, proxy included. Plugin-owned remote adapters take bounded results from the approved path. `crates/server/src/plugins/network/mod.rs:1`, `crates/server/src/plugins/network/admission.rs:1`, `crates/server/src/netguard/fetch.rs:1`, `docs/open-questions.md:55`, `docs/plugin-api-v2.md:309`. |
| Jobs, schedules, stops and wakes | Keep durable enrollments, claims, pending actions, wait winners, revocation, status, cancel and resource limits. Without a host scheduler a sleeping plugin cannot notice a deadline, and a crashed worker can start a duplicate effect. The host wakes from a committed due occurrence and reauthorizes; a native adapter never unlocks its own stop fence. `crates/server/src/plugins/jobs/claim.rs:8`, `crates/server/src/plugins/jobs/complete.rs:1`, `crates/server/src/plugins/jobs/waits.rs:1`, `docs/plugin-api-v2.md:378–396`. |
| Secrets | Host binds ciphertext, key lifecycle, rotation, audited purpose and narrow credential-use handlers. Plugging a private DB into a VM for credentials would put them in the wrong custody and risk access through cold snapshots. Provider login UI and tool capability can vary without lending provider homes, tokens or arbitrary outbound calls to plugins. `crates/secrets/src/local.rs:1`, `crates/secrets/src/lifecycle.rs:1`, `crates/server/src/secrets.rs:1`, engines §§ A.2–A.3, B.3. |
| Erasure and recovery | First revoke authority, then drain jobs/rooms/machines and retry deletion of protected keys, bytes, references, snapshots and outstanding effects. Keep independent high-water and recovery evidence. An absent package cannot be the only way to delete its data or reestablish a consistent directory after restore. Export likewise cannot stand in for protected recovery. `crates/server/src/erasure.rs:1`, `crates/server/src/machines/erasure_queue.rs:1`, `crates/recovery/src/sink.rs:1`, `crates/store/src/restore.rs:1`. |
| Upgrades | Host checks guarded roots, inventories authoritative formats, pins package generations, and makes cutover and rollback decisions. Otherwise a package can change the only writer or roll back over acknowledged data. Allow a package to rebuild its projections or adapt its format with checked writes. Reject a downgrade before new-format history can be dropped. `crates/server/src/upgrades/intent.rs:1`, `crates/server/src/upgrades/inventory.rs:1`, `crates/server/src/upgrades/full.rs:1`, `docs/plugin-api-v2.md:940`, engines § A.5. |

## More control for people and spaces

**Two Notes implementations over one Markdown format.** Separate immutable format identity from package and UI identity. The short kind can remain `notes`, with a registry-bound versioned format identity known to the host. Specify UUIDs, filename/path semantics, UTF-8 and BOM handling, Markdown/frontmatter and unknown-field round-trip, stable revision and attachment references, link targets, the Yjs interoperability profile and indexed minimum fields. ACLs, Trash and history remain host metadata across the switch. A new package declares read, edit, and optional projection compatibility separately. Let both read with authorization, and let the space choose which edits or produces projections. Switch an editor only after checking it can round-trip and share the same room schema, or close/fence the old rooms and perform an explicit conversion with conflict checks. A plugin must not claim another package's kind by spelling it the same. Default to independent kinds for incompatible formats. The sealed package-owned-kind rule needs a numbered exception in the form of explicitly registered compatible formats. `docs/plugin-api-v2.md:23,339–356`, `docs/description-contract.md:413–425`, `crates/content/src/files.rs:119`, `crates/content/src/rooms/wire.rs:1`.

**Space choice of search, thumbnails and runners.** Bind the selected ranking profile to a query/result generation, retaining the qualified cold search route and a fallback if optional features expire. Let a space choose an authorized producer for each derived image slot, with exact revision, producing package or save session, bounded dimensions and audience checks. Keep the accepted editor snapshot and miniature path first. An independent runner gets the same typed job, container, private spool, prepared Send, request and stop rights as the orchestrator, after installation and selection. Changing runners affects new attempts. A running attempt keeps its verified profile until it ends or is stopped. Saved sessions can resume only via a compatible adapter/device with the same authority. These three choices replace policy while retaining the host's read, authorization and cancellation behavior. `docs/open-questions.md:45–49`, engines §§ A.1–A.5, B.1–B.4.

**Extend what another plugin shows and records.** Let a package declare named versioned slots in its description, with output component and context types, and let the space opt into contributions from named installed packages. Compose validated descriptions into a cached published view. Derived actions retain the contributor's identity and get freshly checked context. A slot cannot extend the actor's rights or replace a protected card. For data, let the kind registration explicitly permit typed extension fields. Namespace writes by package and apply expected-head checks, field ownership, audience and quota at host commit. Put portable authored fields on the item or a readable sidecar in the export. Tie derived fields to source revisions and clear/rebuild on incompatible upgrade. Reserve core fields and require explicit bounded index opt-in for extension fields. On uninstall preserve ordinary data for round-trip or explicit user removal; retire routes, derived output and scratch. An independent backlink panel can use these mechanisms without granting independent access to hidden source notes. The v1 slot modules demonstrate composition at registered targets. Arbitrary cross-package slots and field extension writes are open under the sealed contract. `crates/server/src/plugins/slots.rs:1`, `crates/plugin-host/src/slots.rs:1`, `docs/description-contract.md:247,268`, `docs/plugin-api-v2.md:23,329`.

**A portable space and a local space.** Export from a pinned authorized snapshot into normal files and a human-readable manifest of kind/format versions, stable IDs, relative paths, attachment checksums and package identities/checksums. Include portable settings and optional eligible history only by explicit scope. Preserve unknown package fields and open raw files even if a package is unavailable. Validate checksums, sizes, symlinks, path collisions, format compatibility and membership mapping on import. Package pins are recommendations until the destination operator approves installation and network permissions. Create a new space UUID, map item references if IDs conflict, and recreate invitations and account/profile bindings through host controls. Omit secrets, scratch, bearer links, in-flight jobs, provider-native state and caches from portable exports. A personal backup package may request the same export under a bounded job, while installation recovery remains separate. Locally the person runs that same trusted core, item store, broker and package catalogue on their own machine with optional one-disk roots. KVM uses the hosted wall; the accepted non-KVM option uses sandboxed plugin processes and rootless Podman, plainly labelled as sharing the kernel. A laptop that is off misses due work until it returns unless the person explicitly delegates execution. Offer no automatic cloud copy and require a separate opt-in for remote connectivity and machine custody. `docs/open-questions.md:45–50,69`, `crates/roots/src/guard.rs:1`, `crates/fsio/src/beneath.rs:1`.

## Notes and orchestrator migration setbacks

**SEALED** names the current contract surface. **ENGINES** means the proposal describes the replacement and is still to be qualified. **OPEN** means a new decision, contract or recovery proof is needed.

| Current in-server dependency | Replacement and status |
| --- | --- |
| Notes dispatch has hardcoded create/get/update/rename and special item-state checks, with native Notes types and catalogue declarations. `crates/server/src/api/dispatch.rs:155,458,613`, `crates/server/src/api/notes.rs:1`, `crates/core/src/note_operations.rs:1`. | **SEALED** typed operation, receipt, reads and mutation through item engines. **ENGINES B.1** generic cold create. **OPEN** declared defaults for filename/body/fields, plus package removal without breaking raw item access. |
| Native `Store` reaches per-space databases, creates under `Notes/`, extracts heading and tags before a write and calls Rust link retargeting at guarded commit or job step. `crates/server/src/store.rs:304,370,419,512,524`, `crates/server/src/notes.rs:9,101`. The v1 broker only exposes its checked subset. `crates/server/src/plugins/broker.rs:268,295,330`. | **SEALED** generic immutable revisions, saves and relay. **ENGINES B.5** revision-bound bulk metadata. **OPEN** compatibility format, foreground vs asynchronous projection rules for every writer, and expected-head rewrite plans applied without reentering package code inside the lock. |
| Native note pages build title links, label counts, Trash results and same-space collection membership in host. `crates/server/src/api/note_pages.rs:1,65,81`, `crates/server/src/api/sharing/collections.rs:1`, `crates/content/src/discovery.rs:242`. | **SEALED** typed cold fields, windows, collection and lifecycle operations. **OPEN** typed set/edge projection and audience-safe counts for backlinks and mixed scope. |
| The generic storage crate embeds a native note marker, linked-write kind check, Notes rename/Undo and Trash machinery, and thread snooze triggers. `crates/content/src/files.rs:119`, `crates/content/src/linked_write.rs:138,151`, `crates/content/src/rename_job.rs:15,179,315`, `crates/content/src/trash_job.rs:1`, `crates/content/src/db.rs:62`. | **SEALED** generic rename/status/Undo, Trash, collection paths and room-checked metadata. **OPEN** eliminate hardcoded kind/operation triggers, preserve prior durable jobs and payloads through cutover, resolve generic multi-item inversion and lifecycle deadlines. |
| Native authorization, CLI, resolver and public sharing routes assume Notes or Threads. Public collections compose Notes HTML and use a Notes-specific position. The upgrade inventory names the automation scheduler DB. `crates/server/src/authz/mod.rs:120,226,393`, `crates/server/src/cli.rs:37,49`, `crates/server/src/resolve.rs:123`, `crates/server/src/api/sharing/guest.rs:25,152,253`, `crates/server/src/upgrades/inventory.rs:150`. | **SEALED** host-authorized operation lookup, opaque address checks and saved guest views. **OPEN** generic saved collection routes, package command registration and verified writer inventory and cutover for due schedules. |
| The room update holds host authority and shared room/content fences, and the room hub can reconcile external writes. Operator paths and guest rescue bypass the package for Markdown persistence. `crates/server/src/live_rooms/update.rs:100,216`, `crates/server/src/live_rooms/access.rs:133`, `crates/server/src/store.rs:282,419`, `crates/content/src/rooms/external.rs:1`, `crates/server/src/api/sharing/rescue.rs:1`. | **SEALED** direct cold editor/room, checkpoint admission and metadata under same revision fence. **OPEN** keep client/external/offline/agent commits coherent without package-dependent ACK; authorize rescue to a compatible target in personal space. |
| Bundled Notes had a host package-store patch to opt into room editing; startup launches Notes retention; the journal hardcodes Notes entry kinds. `crates/server/src/plugins/package_store/bundled_notes.rs:1`, `crates/server/src/main.rs:585,597,602`, `crates/server/src/live.rs:240`. | **SEALED** declared room participation and host-authorized live delivery. **OPEN** generic retention trigger, journal referent and cutover of the bundle patch once replacement manifests are validated. No runtime needed at startup to replay lifecycle work. |
| Threads routes, parser, rendering headings, status and model selection call Rust directly; cold list reads join per-space jobs SQL, and search consumes thread-record byte ranges and a Rust decoder registered at startup. `crates/server/src/api/mod.rs:112,429,460`, `crates/server/src/threads/operations.rs:9`, `crates/server/src/threads/index.rs:72`, `crates/server/src/threads/document.rs:1`, `crates/server/src/threads/headings.rs:1`, `crates/server/src/threads/records.rs:1`, `crates/server/src/threads/search.rs:1`, `crates/content/src/search/messages/source.rs:1`. | **SEALED** cold Bind and transcript surfaces currently assume Turns. **ENGINES A.3/A.5** generic committed event projection. **OPEN** portable authored text framing, indexed viewer status, preprocessed revision-bound message segments, combined search and old transcript equivalence. |
| Trusted composer Send writes thread intent then admits an exact job; archive and Stop run shared lifecycle fences, release pending work and update the content head. Native resume/retry uses saved provider binding and avoids an extra worker dispatch. `crates/server/src/threads/mod.rs:50,127`, `crates/server/src/threads/operations.rs:479`, `crates/server/src/turns/thread_admission.rs:5`, `crates/server/src/turns/lease.rs:124`, `crates/server/src/turns/retry.rs:1`, `crates/server/src/api/mod.rs:460`. | **SEALED** checked composer, protected actions and archive rules, with current Turns IDs. **ENGINES A.1/A.5** generic intent, cancel, same-head refusal and custody of saved session metadata. **OPEN** qualify the replacement's same/fewer hot-path exchanges and crash interleavings; host-resolved draft must be cold even when the package interprets it after Send. |
| Native turns get a separate jobs interpreter, a shared machine and job lock/fence, direct output readers, in-memory run controls, private session material and per-thread checkpoints. Jobs DB schemas/triggers emit specialized turn status, and startup separately recovers turns and plugin jobs. `crates/server/src/plugins/jobs/claim.rs:16`, `crates/server/src/plugins/jobs.rs:252`, `crates/server/src/plugins/jobs.rs:585`, `crates/server/src/plugins/live.rs:43`, `crates/server/src/plugins/runner.rs:69`, `crates/server/src/turns/run.rs:1`, `crates/server/src/turns/progress.rs:34`, `crates/server/src/turns/control.rs:1`, `crates/server/src/turns/redact.rs:1`, `crates/server/src/threads/persistence.rs:52`, `crates/server/src/threads/turn_events.rs:51`, `crates/server/src/main.rs:585,597`. | **ENGINES A.1–A.5** uniform admission, exact attempt fencing, durable private spool, approved outward events, restart and same-execution proof. **OPEN** qualify machine ACK durability before discarding bytes (`crates/server/src/machines/sessions.rs:430` today), pin first/last sequence and live transcript recovery, and migrate existing jobs/retained session custody. |
| Agent/API dispatch and approvals use native Turns authority, request helpers, readiness, thread-specific follow-up and native action correlation. `crates/server/src/api/guard.rs:182`, `crates/server/src/api/request_action.rs:4`, `crates/server/src/api/dispatch.rs:69`, `crates/server/src/plugins/agent_access.rs:216`, `crates/server/src/plugins/requests.rs:313`, `crates/server/src/plugins/requests.rs:455`, `crates/server/src/requests.rs:46,257,352`. | **SEALED** one protected request model. **ENGINES A.2/A.4** distinct attempt principals and current verified action, dismissal-only for a native request that cannot be approved. **OPEN** prove revocation/cancel/park races, guest separation and scope on independent runner; keep package explanations outside approval payload. |
| Start prereads provider pins, constructs native sandbox and checks machine-local provider storage. A sign-in flow writes transcript notices directly, then restarts the thread; readiness looks up native thread context. `crates/server/src/turns/run.rs:239,269`, `crates/server/src/provider_sign_in/mod.rs:293`, `crates/server/src/provider_sign_in/mod.rs:333`, `crates/server/src/provider_sign_in/flow.rs:43`, `crates/server/src/provider_sign_in/flow.rs:145`, `crates/machine/src/sandbox/provider_home.rs:1`, `crates/machine/src/sandbox/shared_broker.rs:1`. | **ENGINES A.2/A.3** qualified profile, secret custody and attempt state. **OPEN** machine-harness service for protected setup and generic readiness assertions with sanitized notice publication. |
| Scheduler calls the native thread admission and release functions and has direct scheduling SQL. Title generation additionally starts on committed completion; Web Push and the live feed inspect committed thread-specific event shapes. `crates/server/src/automations/scheduling.rs:592,641`, `crates/server/src/automations/scheduling/worker.rs:334`, `crates/server/src/turns/run.rs:118`, `crates/server/src/web_push/mod.rs:1`, `crates/server/src/live.rs:256`. | **SEALED** enrollment, typed operations and authorized event delivery. **OPEN** recurrence admission/catch-up, safe notification envelope and format-agnostic committed event feed. Keep built-ins functional through cutover or explicitly hold them disabled pending proof. |

For Notes qualify cold open, generic cold create, block-editor saves and room convergence, competing revisions, link rewrite/Undo, Trash expiry and current-audience assets across package removal and restart. For Threads qualify prepare/Send, archive/Stop while cold and between handoffs, same-execution versus explicit Resume, machine spool ACK/replay, approved and dismissal-only requests, and mixed combined search across differently authorized callers. Require replay from committed files/outbox at each cut and compare visible results across the old and new writers. Use the sealed independent-author exercise before removing native paths. `docs/plugin-api-v2.md:940,1101`, engines proposal § C.

## Morning decisions, ranked

1. **What compatibility promise should the Notes files carry?** Two packages could edit the same Markdown and collaborate, but the initial format and room contract must be picked before replacing the native writer. For example, a wiki editor should preserve block-editor frontmatter it does not understand. A portable versioned format with explicit editor capability limits overwrite risk, makes switching easy, and costs validation and fixture work. Isolated package kinds simplify grants and indexing, but switching means a copy/conversion job and possible feature loss. Recommend a portable Markdown profile with one selected writer and shared room qualification.
2. **Who computes the visible fields on edits made without a plugin?** Room edits, offline recovery and generic New use cold writers from the first migration. For example, a pasted heading and two tags need a coherent row and an eventual searchable projection. Publishing a small checked basic row during save with package fields arriving asynchronously preserves save latency, at the cost of brief label or preview lag and a strict revision fence. A synchronous package parser gives immediate fields, but adds a wake and puts availability and save timing under package control. Recommend a guaranteed cold basic row, separate revision-bound package fields and visible updating state.
3. **How powerful should the first replacement agent runner be?** The execution and credential boundary must be fixed before Threads migrates. For example, a community runner can start an alternative adapter without seeing the shared provider login. A qualified profile with pinned adapter and separate attempt identity keeps containment and cold Stop, but limits providers and adds a lease/spool cost. Arbitrary images with the same secret and network rights ease installation, but weaken credential custody and widen failure modes during revocation and replay. Recommend qualified profiles with exact attempt fencing and an independent runner trial.
4. **How should a sleeping space keep its calendar?** Automations are part of the first-invite bar. For example, Monday at 09:00 must run at most once when its machine returns before midnight, even after DST or a restart. A small trusted recurrence vocabulary keeps schedules visible cold and requires bounded host calculation, but limits unusual calendars. Prepublished finite deadlines give a plugin more freedom with more renewal wakeups and gaps if its package is disabled. Recommend a small checked recurrence vocabulary, with richer rules able to prepublish bounded dates later.
5. **How much search policy can a space choose at first?** Combined title, body and message search changes the sealed query contract before the Notes and Threads ports. For example, a space might boost recent titles without allowing hidden thread contents to alter rank. A qualified default with bounded selectable weights protects corpus isolation and cold latency, at the cost of a limited palette. Arbitrary scorer packages ease niche discovery but need a wake, stronger validation, and bounded work across every candidate. Recommend the authorized host-ranked default, then qualified presets and explicitly requested background alternatives.
6. **Who may add fields and interface contributions to another app?** A wiki plugin could add a backlinks panel to Notes and maintain its own fields without acquiring Notes authority. Explicit kind opt-in, slots and contributor namespaces protect approvals and keep render cost bounded, but require compatibility declarations. Automatic contributions reduce setup and increase ambiguity about authorship, field collisions and query/index cost. Recommend explicit named slots and namespaced fields, with ordinary authorship and read rights unchanged.
7. **When should the first portable space export ship?** A person should be able to move a space before relying on permanent hosted storage. For example, a local import should recover Markdown and attachments even when a package is unavailable. A pinned files-and-manifest archive is easy to inspect and can stream while asleep, but needs checksum, quota and audience checks and preserves less history. Including history and settings from day one is convenient for fidelity but adds IO, validation and accidental-disclosure risk. Recommend basic portable files, package pins and authored metadata first, with an explicit later history option.
8. **Can a package choose shorter or longer Trash retention?** A records plugin and Notes may want different lifetimes, but the storage scheduler must behave after uninstall. For example, replacing Notes must preserve the existing 30-day restore window on its files. A host-fixed default keeps the deadline predictable and needs one small due index. Manifest-selectable windows improve fit, but invite a plugin to shorten another person's restore time or retain excessive bytes. Recommend the accepted 30 days for this release with explicit storage limits and trusted per-kind controls only if demand warrants them.
9. **How much can a package say in a push notification?** The first-invite workflows need cold delivery, and notification policy must be set before the orchestrator becomes the producer. For example, an off-device alert can show a thread's authorized title and “needs you.” A host-supplied title plus enumerated event avoids transcript disclosure and minimizes payload, at the cost of less custom copy. Arbitrary package text is flexible but could expose messages outside the current audience and cost more per dispatch. Recommend checked templates and delivery-time authorization.

## Inventory by source file

The 11 crates total 109,440 physical lines. Counts cover tracked Rust files under each crate's `src/`, including comments, blank lines and inline tests. Test modules are listed with the responsibility they cover. The reference in each row is relative to the enclosing crate's `src/`. **BECOMES GENERIC** retains the trusted half and uses the named engine below. **STAYS** includes existing generic plumbing even where its v2 implementation must change.

| Key | Host reason and concrete failure, or destination and facility |
| --- | --- |
| A | Security. Caller-controlled identity, membership, directory or grants would expose protected accounts or items. |
| B | Security and durability. A package could approve its own effects or replay stale approval. Protected requests and exact attempt grants. |
| C | Sleep, security and durability. Rooms would wake on open or lose checkpoint and revoke fencing. Sealed Rooms. |
| D | Durability. Interrupted startup, admission, draining or asset writes could publish incomplete state. Asset delivery also needs protected refs, quotas and validation. Engines B.2 plus personal asset custody. |
| F | Security and sleep. Preferences, palette and slots could mix identities or wake every provider. Sealed Settings/Bind plus open personal shell and contribution catalogue. |
| H | Sleep and security. Sleeping producers could miss alerts, and arbitrary alert text can leak protected data. Open checked notification intent over host transport. |
| I | Sleep and security. Awake-only reads delay browsing, and wrong scope leaks rows, counts or ranks. Host queries, live delivery and cursors. |
| J | Sleep, durability and security. Missed wakes, duplicate claims and obsolete authority otherwise survive restart. Sealed Jobs/Schedules plus engines A.1. |
| K | Security. Untrusted callers must not pick authority, executable payloads, origins or bindings. Trusted broker, egress, host validation and local author tooling. |
| L | Host lifecycle engine. Sealed Restore/Purge/Empty Trash plus generic deadline and cancellation fencing that survives removal or restart. |
| Q | Host projection engine. Sealed typed indexed fields, Bind and queries plus open relations, extraction and combined ranking. |
| R | Host revision engine. Sealed Rename, status and protected Undo plus an open expected-head multi-item plan. |
| T | Fenced execution, durable streams, and private/public event custody. Engines A.1–A.5. |
| V | Qualified execution and credential profiles. Engines A.2–A.3 plus open generic protected setup and readiness. |
| W | Durability and security. Interrupted saves and replay can lose bytes, double-apply effects or cross space boundaries. Host filesystem, commit and journal. |
| X | Security and durability. An obsolete attempt could execute or reattach, or exceed placement rights. Host machine/VM control and leases. |
| Y | Security and durability. Guest access could extract key material or lose custody after restart or erasure. Host secret store and broker. |
| Z | Security and durability. A removed package could retain bytes, or a stale restore could authorize against regressed state. Host erasure, recovery and upgrade. |
| N | `calternal-notes` via sealed Items/Rooms/Queries plus open format, projection and rewrites. |
| P | `calternal-orchestrator` or a provider adapter via engines A.1–A.5, typed profiles and declared projections. |
| U | Automation or Skills package, initially `calternal-orchestrator`, via sealed Items/Jobs/Schedules plus open scheduling and skill contribution rules. |
| E | Personal appearance package via sealed Settings/Proxy and open personal assets and shell contributions. |
| O | Optional onboarding package via sealed Settings/Jobs and an open personal-shell entry point. |

### `crates/server/src`

235 files, 55,612 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `account_http.rs:1` | 599 | Trusted account HTTP endpoints ported from the original auth transport. **STAYS (A)** |
| `account_http/erasure.rs:1` | 102 | Person-account erasure via the trusted origin and installation checks. **STAYS (Z)** |
| `account_http/mail.rs:1` | 111 | Bounded private SMTP queue. Mail and SMTP credentials never enter diagnostics. **STAYS (A)** |
| `account_http/oidc.rs:1` | 452 | Protected OIDC transport. The host owns the browser ceremony and identity proof. **STAYS (A)** |
| `account_http/oidc/exchange.rs:1` | 286 | Discovery, outbound boundary checks and ID-token verification. **STAYS (A)** |
| `account_http/onboarding.rs:1` | 236 | Private account onboarding and recovery requests. Never enter receipt replay. **STAYS (A)** |
| `account_input/db.rs:1` | 389 | Owner-local SQL and bounded invalidation history. Events carry no content. **STAYS (A)** |
| `account_input/dispatch.rs:1` | 397 | Account-derived operation admission and pending submission state machine. **STAYS (A)** |
| `account_input/http.rs:1` | 230 | One private account SSE route, account inferred exclusively from authentication. **STAYS (A)** |
| `account_input/mod.rs:1` | 205 | Private account input storage, following the secrets per-owner database pattern. **STAYS (A)** |
| `account_input/plugin_settings.rs:1` | 223 | Owner-local plugin overrides, durable receipts and private invalidation. **STAYS (A)** |
| `account_input/reader.rs:1` | 78 | A live account feed retains its existing SQLite connection and admission lease. **STAYS (A)** |
| `account_input/wake.rs:1` | 63 | Account-scoped wake hints using the journal feed's subscribe-before-replay pattern. **STAYS (A)** |
| `admin_cli.rs:1` | 482 | Operator commands for recovery, machine administration and guarded setup. **STAYS (Z)** |
| `api/appearance/catalogue.rs:1` | 139 | Per-provider refreshes and deterministic, cached merged catalogue snapshots. **MOVES (E)** |
| `api/appearance/db.rs:1` | 582 | Account preferences and shared font manifests → generic private preferences and asset custody. **BECOMES GENERIC (F)** |
| `api/appearance/face.rs:1` | 166 | Validated face descriptors and original-format byte identification. **BECOMES GENERIC (D)** |
| `api/appearance/http.rs:1` | 169 | Multipart upload and authenticated same-origin face delivery. **BECOMES GENERIC (D)** |
| `api/appearance/mod.rs:1` | 392 | Signed-in per-account appearance and installation-owned font downloads. **BECOMES GENERIC (F)** |
| `api/appearance/prefs.rs:1` | 191 | Browser-compatible defaults, bounded patches and role traversal. **MOVES (E)** |
| `api/appearance/remote.rs:1` | 559 | Guarded adapters for Google Fonts, Fontsource and Fontshare. **MOVES (E)** |
| `api/appearance/remote/css.rs:1` | 90 | Parse the restricted font-face subset of the providers' CSS. **MOVES (E)** |
| `api/apple_app_site.rs:1` | 75 | Public site association for passkeys in an iOS web view. **STAYS (A)** |
| `api/assets.rs:1` | 87 | HTTP responses and rolling admission budgets for same-origin binary assets. **STAYS (D)** |
| `api/authorization.rs:1` | 131 | Request policy admission, separate from storage-handler scheduling. **STAYS (A)** |
| `api/cursors.rs:1` | 166 | Stateless wire cursors over ordered keys selected by the existing handlers. **STAYS (I)** |
| `api/dispatch.rs:1` | 694 | Hardcoded native operation dispatch → manifest dispatch and trusted receipts. **BECOMES GENERIC (K)** |
| `api/guard.rs:1` | 293 | Host allowlist, Origin and session checks and security headers for every response. **STAYS (K)** |
| `api/mod.rs:1` | 526 | Native and generic HTTP surface → cold checked engine dispatch and common engine registration. **BECOMES GENERIC (K)** |
| `api/note_pages.rs:1` | 124 | Title links, tag counts and Trash rows → declared fields and authorized windows. **BECOMES GENERIC (Q)** |
| `api/notes.rs:1` | 147 | Native Notes mutations called after the operation gate. **MOVES (N)** |
| `api/people.rs:1` | 242 | People operation dispatch, with scoped receipts and sanitized authority errors. **STAYS (A)** |
| `api/plugin_slots.rs:1` | 52 | Resolve package activation and indexed slot declarations in the selected space. **BECOMES GENERIC (F)** |
| `api/plugin_slots/live.rs:1` | 227 | One bounded authenticated stream of replacement projections for a package mount. **BECOMES GENERIC (F)** |
| `api/plugins.rs:1` | 265 | Manifest-declared plugin operations on their (plugin, space) worker, and previews. **STAYS (K)** |
| `api/plugins/receipts.rs:1` | 29 | Recovery of a plugin operation from its committed host effect. **STAYS (W)** |
| `api/request.rs:1` | 20 | Request identity hashing uses the existing blocking pool, never the executor. **STAYS (W)** |
| `api/request_action.rs:1` | 105 | Consume once authority before dispatch and replay completed responses. **STAYS (B)** |
| `api/search.rs:1` | 386 | Palette and mention pickers → declared scoped content and command sources. **BECOMES GENERIC (Q)** |
| `api/search/shared.rs:1` | 87 | Recipient search reuses owner projections with an explicit allowed ID set. **STAYS (I)** |
| `api/search/tests.rs:1` | 43 | Fixture. Regression coverage for search. **BECOMES GENERIC (Q)** |
| `api/search_skills.rs:1` | 328 | Bounded reads and guarded writes of the reserved per-space skill directory. **MOVES (U)** |
| `api/sharing.rs:1` | 429 | Host-authorized sharing operations for items in the content catalogue. **STAYS (A)** |
| `api/sharing/collections.rs:1` | 219 | Notes-specific saved collections → generic same-space collection membership. **BECOMES GENERIC (Q)** |
| `api/sharing/eviction.rs:1` | 63 | Recheck the surviving authority before evicting any active rooms. **STAYS (A)** |
| `api/sharing/guest.rs:1` | 266 | Scoped bearer transport with native Notes rendering → keep bearer checks and serve generic saved views. **BECOMES GENERIC (F)** |
| `api/sharing/rescue.rs:1` | 66 | Offline edits rescued to personal Notes → checked compatible personal target. **BECOMES GENERIC (W)** |
| `api/spaces.rs:1` | 82 | Stored space-directory operations and their shared content request receipts. **STAYS (A)** |
| `api/wallpapers/cache.rs:1` | 91 | Wallpaper catalogue slot files → bounded generic asset cache. **BECOMES GENERIC (D)** |
| `api/wallpapers/catalog.rs:1` | 98 | Fixed catalog and rendition table shared by HTTP admission and the cache. **MOVES (E)** |
| `api/wallpapers/mod.rs:1` | 174 | Same-origin wallpapers → scoped rendition delivery with checked refs. **BECOMES GENERIC (D)** |
| `api/wallpapers/tests.rs:1` | 107 | Fixture. Regression coverage for wallpapers. **BECOMES GENERIC (D)** |
| `api/wallpapers/upstream.rs:1` | 233 | Unsplash catalogue and fetch adapter. Keep credential custody in the host proxy. **MOVES (E)** |
| `api/web.rs:1` | 136 | Static serving with a fallback limited to Calternal application addresses. **STAYS (K)** |
| `asset_cache.rs:1` | 59 | Bounded binary I/O for trusted private asset caches. **STAYS (D)** |
| `auth.rs:1` | 343 | Account directory, optional development credentials, and stored space grants. **STAYS (A)** |
| `authz/mod.rs:1` | 429 | Host operation checks with named Notes and turn policies → manifest-derived checks bounded by host authority. **BECOMES GENERIC (K)** |
| `automations/definitions.rs:1` | 442 | Validation and rendering for the versioned automation Markdown document. **MOVES (U)** |
| `automations/mod.rs:1` | 104 | Authored automation definitions, using the normal guarded content writer. **MOVES (U)** |
| `automations/scheduling.rs:1` | 732 | Schedule cursor and native thread handoffs → generic due occurrences and receipt-backed claim. **BECOMES GENERIC (J)** |
| `automations/scheduling/contention.rs:1` | 181 | Concurrent scheduler writers sharing a file or using one file per space. **BECOMES GENERIC (J)** |
| `automations/scheduling/tests.rs:1` | 112 | Fixture. Regression coverage for scheduling. **BECOMES GENERIC (J)** |
| `automations/scheduling/worker.rs:1` | 384 | Background discovery and native handoff reconciliation → bounded generic due work and outcome replay. **BECOMES GENERIC (J)** |
| `blocking.rs:1` | 177 | Foreground blocking work, classified by the existing operation declarations. **STAYS (D)** |
| `cli.rs:1` | 308 | Command definitions shared by the parser and startup dispatch. **BECOMES GENERIC (F)** |
| `cli_output_tests.rs:1` | 207 | Fixture. CLI presentation regression fixtures. **STAYS (F)** |
| `erasure.rs:1` | 181 | Shared storage cleanup after protected owner authority has been revoked. **STAYS (Z)** |
| `fault.rs:1` | 193 | Crash-point fault injection for durability tests. Compiled into debug. **STAYS (W)** |
| `lib.rs:1` | 47 | Calternal-AI localhost development-preview host. **STAYS (K)** |
| `live.rs:1` | 328 | Multiplexed journal feed with Notes/Turns special cases → declared referents and visible fields. **BECOMES GENERIC (I)** |
| `live_rooms.rs:1` | 510 | Versioned WebSocket transport for opted-in host content rooms. **STAYS (C)** |
| `live_rooms/access.rs:1` | 271 | Check viewer and editor authority on room join, update and delivery. **STAYS (C)** |
| `live_rooms/delivery.rs:1` | 104 | Check authority after socket backpressure clears, before handing off data. **STAYS (C)** |
| `live_rooms/hub.rs:1` | 595 | Per-item room admission, subscriber lifetimes and journal fan-out. **STAYS (C)** |
| `live_rooms/hub/cache.rs:1` | 91 | Room cache refresh, idle retirement and participant shutdown. **STAYS (C)** |
| `live_rooms/hub/presence.rs:1` | 74 | Ownership and host attribution for ephemeral cursor state. **STAYS (C)** |
| `live_rooms/hub/tests.rs:1` | 52 | Fixture. Regression coverage for hub. **STAYS (C)** |
| `live_rooms/update.rs:1` | 229 | Binary sync admission, durable check ordering, and protocol refusals. **STAYS (C)** |
| `live_rooms/wire.rs:1` | 205 | The reference lib0 room envelope and numbered fragmentation. **STAYS (C)** |
| `machines.rs:1` | 651 | Trusted machine enrollment, sessions, placement and fenced execution. **STAYS (X)** |
| `machines/admin.rs:1` | 476 | Private operator socket for machine administration and first-owner setup. **STAYS (X)** |
| `machines/admission.rs:1` | 308 | Shared fenced lease admission for agents and plugin workers. **STAYS (X)** |
| `machines/capacity.rs:1` | 116 | Machine and account capacity accounting under the existing registry lock. **STAYS (X)** |
| `machines/colocated.rs:1` | 161 | Resolve a rootless deployment's private, local machine handoff. **STAYS (X)** |
| `machines/db.rs:1` | 627 | `system/machines.db`: machine placement, reservations and erasure delivery. **STAYS (X)** |
| `machines/discards.rs:1` | 222 | Durable explicit workspace deletion. Pending requests survive server restart. **STAYS (X)** |
| `machines/erasure.rs:1` | 135 | Remove account or space placement after its owner-local steps have drained. **STAYS (Z)** |
| `machines/erasure_queue.rs:1` | 224 | Durable delivery of erased owners' machine-role storage instructions. **STAYS (X)** |
| `machines/execs.rs:1` | 161 | Machine command admission with named provider profile → generic pinned execution. **BECOMES GENERIC (X)** |
| `machines/http.rs:1` | 436 | Authenticated machine HTTP endpoints. **STAYS (X)** |
| `machines/install.rs:1` | 52 | A personal, single-use installer rendered from the standalone machine script. **STAYS (X)** |
| `machines/microvm.rs:1` | 65 | Account VM admission → per-space roles. **BECOMES GENERIC (X)** |
| `machines/owners.rs:1` | 76 | Owner and operator machine administration through guarded authority. **STAYS (X)** |
| `machines/provider.rs:1` | 135 | Explicit provider lease resumption → exact attempt identity and proof. **BECOMES GENERIC (X)** |
| `machines/readiness.rs:1` | 583 | Placement and provider facts → checked profile observations with age. **BECOMES GENERIC (V)** |
| `machines/sessions.rs:1` | 546 | Enroll machines, authenticate signed sessions and accept fenced output. **STAYS (X)** |
| `machines/space_execution.rs:1` | 147 | Space-owned placement on the current owner's enrolled sandbox machine. **STAYS (X)** |
| `machines/state.rs:1` | 712 | Registry of machines, reservations and live executions under a shared lock. **STAYS (X)** |
| `machines/workers.rs:1` | 579 | Worker admission and ordered stdin on the existing fenced machine exec channel. **STAYS (X)** |
| `machines/workers/tests/erasure.rs:1` | 174 | Fixture. Regression coverage for erasure. **STAYS (X)** |
| `main.rs:1` | 741 | Starts roots, recovery, host services and current native maintenance → register shared recovery and due work. **BECOMES GENERIC (D)** |
| `netguard/deny.rs:1` | 85 | Reserved and non-public address ranges ported from the existing app. **STAYS (K)** |
| `netguard/fetch.rs:1` | 202 | HTTPS URL admission, explicit redirect handling, and limited response reads. **STAYS (K)** |
| `netguard/local.rs:1` | 142 | Extra local ranges for anonymous Class B fetches. **STAYS (K)** |
| `netguard/mod.rs:1` | 230 | Fixed-destination HTTPS with checked addresses for trusted host integrations. **STAYS (K)** |
| `notes.rs:1` | 140 | Native Markdown projection for the official Notes kind. **MOVES (N)** |
| `notes/links.rs:1` | 281 | Relative Markdown note links, adapted from ADR 0024 and the reference engine. **MOVES (N)** |
| `notes/retention.rs:1` | 169 | Notes Trash sweep → generic indexed lifecycle deadline. **BECOMES GENERIC (L)** |
| `notes/tags.rs:1` | 55 | Tag grammar ported from the reference app's notes engine. **MOVES (N)** |
| `notes/tags/front.rs:1` | 83 | Tags in the properties: `tags: a, b`, `tags: [a, "b"]`, or a `tags:`. **MOVES (N)** |
| `notes/tags/scan.rs:1` | 329 | Finding `#tags` in the text of a note. **MOVES (N)** |
| `onboarding.rs:1` | 70 | Private account completion marker, independent of protected sign-in authority. **MOVES (O)** |
| `owner_setup.rs:1` | 137 | Operator-only first-owner codes issued by the server that holds the root locks. **STAYS (A)** |
| `people_maintenance.rs:1` | 67 | Server-lifetime maintenance for bounded authority rows. **STAYS (A)** |
| `plugin_cli.rs:1` | 530 | Local TypeScript author commands. Author scripts run as the invoking user. **STAYS (K)** |
| `plugin_cli/dev.rs:1` | 280 | Local development watch loop. The server validates and activates each build. **STAYS (K)** |
| `plugin_cli/question.rs:1` | 66 | Authoring entry points for the shared Inbox form validator. **STAYS (K)** |
| `plugin_cli/scaffold.rs:1` | 125 | Reliable local starter packages and their README files. **STAYS (K)** |
| `plugin_cli/submission.rs:1` | 322 | Offline gallery envelopes that retain the existing installable package format. **STAYS (K)** |
| `plugins/agent_access.rs:1` | 446 | Provider/app action grants → exact attempt and action grants. **BECOMES GENERIC (B)** |
| `plugins/agent_settings.rs:1` | 111 | Native recovery defaults → typed scoped settings. **BECOMES GENERIC (F)** |
| `plugins/broker.rs:1` | 608 | Trusted host-call brokers: the only way a worker reaches host state. **STAYS (K)** |
| `plugins/broker/item_pages.rs:1` | 89 | Indexed kind pages scoped to the invocation's package and authority. **STAYS (K)** |
| `plugins/draft_diagnostics.rs:1` | 174 | Bounded local draft telemetry. Capture only codes, safe schema pointers and timings. **STAYS (K)** |
| `plugins/harness_admin.rs:1` | 294 | Installation administrator operations for sandboxed harness versions. **BECOMES GENERIC (V)** |
| `plugins/installed_diagnostics.rs:1` | 179 | Bounded manager diagnostics on each immutable package revision. **STAYS (K)** |
| `plugins/job_api.rs:1` | 383 | Generic job status and live stream with turn-specific redaction → declared safe events. **BECOMES GENERIC (J)** |
| `plugins/job_claim_tests.rs:1` | 122 | Fixture. A plugin result cannot select the native agent interpreter for an ordinary job. **STAYS (J)** |
| `plugins/job_sql.rs:1` | 287 | Per-space job SQL, row mapping and durable state names. **STAYS (J)** |
| `plugins/job_streams.rs:1` | 104 | Bounded stream ownership and subscriber admission, separate from durable jobs. **STAYS (J)** |
| `plugins/jobs.rs:1` | 588 | Durable jobs and thread status triggers → remove built-in thread schema and interpreter. **BECOMES GENERIC (J)** |
| `plugins/jobs/checkpoint.rs:1` | 97 | Checkpoint publication fenced against cancellation before external admission. **STAYS (J)** |
| `plugins/jobs/claim.rs:1` | 89 | Native interpreter and claim SQL → typed execution admission. **BECOMES GENERIC (J)** |
| `plugins/jobs/complete.rs:1` | 284 | Durable terminal job transitions and cancellation of pending requests. **STAYS (J)** |
| `plugins/jobs/database.rs:1` | 141 | One space's jobs connection and its journal publication scheduling state. **STAYS (J)** |
| `plugins/jobs/waits.rs:1` | 492 | Signal waits share the owning job's transaction and run fencing. **STAYS (J)** |
| `plugins/jobs/waits/tests.rs:1` | 129 | Fixture. Migration and replay guarantees through the durable job store. **STAYS (J)** |
| `plugins/live.rs:1` | 179 | In-memory job output with provider control slot → bounded attempt-scoped progress. **BECOMES GENERIC (T)** |
| `plugins/live_catalog.rs:1` | 370 | Private operator transport for publishing validated package revisions. **STAYS (K)** |
| `plugins/live_catalog/drafts.rs:1` | 236 | Single-space development packages. Never compiled into release images. **STAYS (K)** |
| `plugins/machine_worker.rs:1` | 135 | Server adapter for the machine role's sandbox and exec interfaces. **STAYS (K)** |
| `plugins/mod.rs:1` | 669 | Trusted package registry, manifest-derived policy and enabled generations. **STAYS (K)** |
| `plugins/network/admission.rs:1` | 205 | Shared per-space ceilings for brokered outbound activity. **STAYS (K)** |
| `plugins/network/audit.rs:1` | 209 | Bounded network activity records. Only normalized hostnames and port. **STAYS (K)** |
| `plugins/network/cache.rs:1` | 164 | Viewer-scoped, bounded memory cache for fetched media and pending results. **STAYS (K)** |
| `plugins/network/db.rs:1` | 244 | Durable per-space network preferences. The shared runner owns all DDL. **STAYS (K)** |
| `plugins/network/eligibility.rs:1` | 58 | Remote content fetch timing is chosen by the host, before queue admission. **STAYS (K)** |
| `plugins/network/fetch.rs:1` | 467 | Admission and delivery for anonymous outbound image and font requests. **STAYS (K)** |
| `plugins/network/frames.rs:1` | 97 | Short-lived viewer-scoped references to sanitized content. **STAYS (K)** |
| `plugins/network/http.rs:1` | 230 | Trusted same-origin sealed documents. The general API guard handles identity. **STAYS (K)** |
| `plugins/network/media.rs:1` | 64 | Remote media allow-list. Verify the declared type against the binary prefix. **STAYS (K)** |
| `plugins/network/mod.rs:1` | 277 | Trusted network policy and content handling for plugin invocations. **STAYS (K)** |
| `plugins/network/quota.rs:1` | 226 | Admission counters shared by the bounded Class B queue and checked IP dials. **STAYS (K)** |
| `plugins/network/sanitize.rs:1` | 331 | Parse untrusted HTML and emit only host-controlled markup and references. **STAYS (K)** |
| `plugins/network/sanitize/css.rs:1` | 119 | Parse the supported CSS subset and serialize new declarations from safe tokens. **STAYS (K)** |
| `plugins/network/tests.rs:1` | 196 | Fixture. Regression coverage for network. **STAYS (K)** |
| `plugins/package_store.rs:1` | 675 | Installation-local package catalog and checksum-backed worker materialization. **STAYS (K)** |
| `plugins/package_store/bundled_notes.rs:1` | 15 | Notes-only room enablement patch → ordinary Notes manifest and cutover. **MOVES (N)** |
| `plugins/package_store/provider_runtime.rs:1` | 262 | Trusted provider login policy and version pins → qualified execution profiles. **BECOMES GENERIC (V)** |
| `plugins/package_store/tests.rs:1` | 243 | Fixture. Regression coverage for package_store. **STAYS (K)** |
| `plugins/package_store/worker_cache.rs:1` | 53 | Reuse of disposable worker files under the locked, private installation root. **STAYS (K)** |
| `plugins/requests.rs:1` | 503 | Protected request writes with native phase/action coupling → typed protected intent. **BECOMES GENERIC (B)** |
| `plugins/runner.rs:1` | 256 | Runs admitted plugin jobs on their (plugin, space) worker. **STAYS (J)** |
| `plugins/runner/waits.rs:1` | 158 | Validate parked worker output and restore deadline tasks without worker permits. **STAYS (J)** |
| `plugins/session.rs:1` | 145 | The caller's sign-in, carried into plugin work so it can be rechecked. **STAYS (K)** |
| `plugins/settings.rs:1` | 197 | Manager edits and package-scoped read access to shared settings. **STAYS (F)** |
| `plugins/signals.rs:1` | 127 | Inbox and transport admission for the durable job signal transaction. **STAYS (J)** |
| `plugins/signals/webhook.rs:1` | 166 | Signed, bounded transport for one waiting job. The worker never listens. **STAYS (J)** |
| `plugins/slots.rs:1` | 157 | Compile package-owned and fixed targets once with the immutable catalog. **BECOMES GENERIC (F)** |
| `plugins/worker.rs:1` | 372 | Long-lived worker processes speaking protocol v2, one per (plugin, space). **STAYS (K)** |
| `plugins/worker/admission.rs:1` | 351 | Revision-scoped lookup, cold start, capacity arbitration and dispatch. **STAYS (K)** |
| `plugins/worker/session.rs:1` | 353 | Per-space worker session, authorization and deadline enforcement. **STAYS (K)** |
| `plugins/worker/transport.rs:1` | 281 | Machine sandbox startup and bounded protocol frame transport. **STAYS (K)** |
| `provider_sign_in/flow.rs:1` | 278 | Private challenge transport and native transcript notices → generic challenge and sanitized event. **BECOMES GENERIC (V)** |
| `provider_sign_in/inbox.rs:1` | 152 | Volatile private challenge state → generic account/space setup challenge. **BECOMES GENERIC (V)** |
| `provider_sign_in/mod.rs:1` | 468 | Private provider setup → checked recipe and binding. **BECOMES GENERIC (V)** |
| `question_form.rs:1` | 82 | Shared validation for Inbox questions, parked jobs and author tooling. **STAYS (B)** |
| `rebuild_probe.rs:1` | 19 | Disposable benchmark adapter for Notes tags and thread records → generic projection publication. **BECOMES GENERIC (Q)** |
| `requests.rs:1` | 355 | Protected Inbox and native restart/stop callbacks → host decision and generic continuation. **BECOMES GENERIC (B)** |
| `resolve.rs:1` | 180 | One existence-hiding address resolver over identifier-only content routing. **STAYS (A)** |
| `secrets.rs:1` | 433 | Secret operations, the trusted provide endpoint and the plugin-host adapter. **STAYS (Y)** |
| `shutdown.rs:1` | 140 | Stop admitting HTTP work and close idle streams while admitted requests drain. **STAYS (D)** |
| `startup_io.rs:1` | 139 | Durable startup files use blocking workers rather than the async root task. **STAYS (W)** |
| `store.rs:1` | 647 | Native Notes heading, tag and link callbacks over shared storage → keep host reads, receipts and fenced writes. **BECOMES GENERIC (W)** |
| `store/plugin_items.rs:1` | 122 | Journal receipts and reads for durable plugin content effects. **STAYS (W)** |
| `store_live.rs:1` | 16 | Startup-only content handle extraction for request-independent live streams. **STAYS (I)** |
| `threads/automation.rs:1` | 113 | Start an automation turn through the same content receipts and job admission as a send. **MOVES (P)** |
| `threads/delta.rs:1` | 63 | Exact checkpoint deltas, addressed by adjacent revisions and UTF-8 bytes. **MOVES (P)** |
| `threads/document.rs:1` | 213 | Small Markdown envelope. Message content is never parsed for authority. **MOVES (P)** |
| `threads/headings.rs:1` | 132 | Presentation-only labels and offset-aware timestamps. **MOVES (P)** |
| `threads/index.rs:1` | 270 | Saved thread and jobs SQL join → cold declared item and execution projections. **BECOMES GENERIC (Q)** |
| `threads/mod.rs:1` | 141 | Threads product engine: manifest policy, content files and the existing turn engine. **MOVES (P)** |
| `threads/models.rs:1` | 58 | Cached provider declarations and the space's current activation policy. **MOVES (P)** |
| `threads/operations.rs:1` | 527 | Thread edits and archive controls → generic cold lifecycle with package format policy. **BECOMES GENERIC (L)** |
| `threads/persistence.rs:1` | 355 | Thread/provider checkpoint → durable event and append protocol. **BECOMES GENERIC (T)** |
| `threads/provider_notice.rs:1` | 168 | Bounded sign-in notices in the transcript prefix, separate from authored records. **MOVES (P)** |
| `threads/provider_session.rs:1` | 98 | Native provider session ownership for saved conversations. **BECOMES GENERIC (V)** |
| `threads/records.rs:1` | 214 | Self-delimiting transcript records. Authored Markdown is treated as bytes. **MOVES (P)** |
| `threads/search.rs:1` | 85 | Adapter between the current thread frame and the generic content index. **MOVES (P)** |
| `threads/snooze.rs:1` | 74 | Thread Inbox expiration → indexed viewer-specific deadline. **BECOMES GENERIC (Q)** |
| `threads/title.rs:1` | 345 | Opportunistic titles on the account's own machine after a saved turn ends. **MOVES (P)** |
| `threads/turn_events.rs:1` | 307 | Turn-specific status triggers → generic visible event projection. **BECOMES GENERIC (T)** |
| `threads/turn_status.rs:1` | 14 | Sidebar labels for the existing durable job lifecycle. **MOVES (P)** |
| `transport.rs:1` | 17 | Accepted HTTP sockets send small live-feed frames without Nagle batching. **STAYS (D)** |
| `turns/access.rs:1` | 288 | App-prefix turn authority → exact attempt and action grants. **BECOMES GENERIC (B)** |
| `turns/admission.rs:1` | 52 | Temporary manual-turn port. Thread storage will implement this admission seam. **BECOMES GENERIC (J)** |
| `turns/control.rs:1` | 110 | Bounded, run-scoped interactive provider input, fenced by the admitted exec. **BECOMES GENERIC (T)** |
| `turns/input.rs:1` | 83 | Bounds and canonical identifiers at manual turn admission. **MOVES (P)** |
| `turns/instructions.rs:1` | 67 | Condensed trusted instruction layers and the bounded space Markdown reader. **MOVES (P)** |
| `turns/launch.rs:1` | 69 | Provider command preparation and startup preflight for a fenced job. **MOVES (P)** |
| `turns/lease.rs:1` | 137 | Native lease admission and checkpoint share the per-space cancellation fence. **BECOMES GENERIC (X)** |
| `turns/mod.rs:1` | 662 | Native turn orchestration over durable jobs and machine streams. Preserve generic admission and Stop. **MOVES (P)** |
| `turns/phases.rs:1` | 135 | Observable turn phases and compact per-attempt timing on the existing checkpoint. **MOVES (P)** |
| `turns/progress.rs:1` | 186 | Provider output checkpoint → fenced durable event and private spool. **BECOMES GENERIC (T)** |
| `turns/readiness.rs:1` | 74 | Authorized operation for machine and provider readiness. **MOVES (P)** |
| `turns/reason.rs:1` | 108 | The compact checkpoint projection shared by the thread index, detail and journal. **MOVES (P)** |
| `turns/redact.rs:1` | 219 | Cross-chunk credential scrubbing → qualified host/harness boundary. **BECOMES GENERIC (V)** |
| `turns/retry.rs:1` | 275 | Provider-specific backoff and retry. Keep timed claims in Jobs. **MOVES (P)** |
| `turns/run.rs:1` | 800 | Provider execution and parsing → host machine framing and spool, package adapter policy. **BECOMES GENERIC (T)** |
| `turns/space_execution.rs:1` | 104 | Resolve space placement without lending the Owner's API permissions. **BECOMES GENERIC (X)** |
| `turns/thread_admission.rs:1` | 133 | Thread-file admission reuses manual turn validation and durable jobs. **BECOMES GENERIC (J)** |
| `turns/tool_steps.rs:1` | 100 | Fold scrubbed tool state into the existing Markdown agent message. **MOVES (P)** |
| `turns/verify.rs:1` | 82 | Outcome-only diagnostic over the existing durable Check job and readiness cache. **MOVES (P)** |
| `upgrade_cli.rs:1` | 129 | Local deployment commands over the guarded server installation. **STAYS (A)** |
| `upgrades/full.rs:1` | 533 | Inventory and replay a complete restore point without resolving workspace links. **STAYS (A)** |
| `upgrades/intent.rs:1` | 205 | Durable local state around the point where a release can admit writes. **STAYS (A)** |
| `upgrades/intent_tests.rs:1` | 257 | Fixture. Fault boundaries of the local deployment intent. **STAYS (A)** |
| `upgrades/inventory.rs:1` | 299 | Resolve the deployment footprint through each writer's frozen schema descriptor. **STAYS (A)** |
| `upgrades/mod.rs:1` | 442 | Local upgrade snapshot and activation state, held under the installation locks. **STAYS (A)** |
| `upgrades/proof.rs:1` | 53 | Evidence supplied by the local operator's trusted release gate. **STAYS (A)** |
| `upgrades/snapshot.rs:1` | 150 | Copies and validates the regular files named by an additive manifest. **STAYS (A)** |
| `upgrades/tree.rs:1` | 174 | Descriptor-relative filesystem operations for full restore points. **STAYS (A)** |
| `web_push/db.rs:1` | 307 | Private subscription storage owned by an account and keyed by installation. **STAYS (H)** |
| `web_push/mod.rs:1` | 399 | Opt-in push on committed thread events → checked generic notification intents. **BECOMES GENERIC (H)** |
| `web_push/transport.rs:1` | 218 | VAPID signing, Web Push encryption, and guarded HTTPS delivery. **STAYS (H)** |

### `crates/auth/src`

40 files, 9,839 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `account.rs:1` | 360 | Library entry points for protected account authentication. **STAYS (A)** |
| `account_details.rs:1` | 177 | Private account settings and one-time backup credentials. **STAYS (A)** |
| `account_details_tests.rs:1` | 154 | Fixture. Regression coverage for account details. **STAYS (A)** |
| `account_email.rs:1` | 325 | Account email verification. SMTP transport belongs to the host. **STAYS (A)** |
| `account_recovery.rs:1` | 452 | Proof and grant state for replacing lost passkeys. **STAYS (A)** |
| `agents.rs:1` | 87 | Protected agent authority on the existing auth handle and recovery barrier. **STAYS (A)** |
| `ceremony.rs:1` | 237 | Open passkey prompts. **STAYS (A)** |
| `config.rs:1` | 223 | Sign-in settings. **STAYS (A)** |
| `db.rs:1` | 480 | `auth.db`: users, passkeys, setup tokens and Installations. **STAYS (A)** |
| `db/role_batch.rs:1` | 68 | Group commits for role writes on the auth worker. **STAYS (A)** |
| `db/role_batch_tests.rs:1` | 289 | Fixture. Regression coverage for role batch. **STAYS (A)** |
| `db/sharing_schema_tests.rs:1` | 86 | Fixture. The dormant grant schema upgrades the accounts release and follows erasure. **STAYS (A)** |
| `erasure.rs:1` | 189 | Protected authority cutover and a replayable list of owner trees to erase. **STAYS (A)** |
| `http_facing.rs:1` | 107 | Missing host adapters for anonymous discoverable sign-in and private profile reads. **STAYS (A)** |
| `invite_key.rs:1` | 103 | The keyed lookup and recoverable ciphertext from the source invite key. **STAYS (A)** |
| `invites.rs:1` | 287 | Single-use space invitations, adapted from the source invite actions and setup. **STAYS (A)** |
| `lib.rs:1` | 144 | Protected identity storage primitives ported from Calternal's auth crate. **STAYS (A)** |
| `limit.rs:1` | 348 | Bounded, independent rate-limit buckets. Forwarded headers never identify clients. **STAYS (A)** |
| `machines.rs:1` | 294 | Machine authority: which machines may run code for whom. **STAYS (A)** |
| `machines/colocated.rs:1` | 70 | Protected authority commit for a deployment's one-time local handoff. **STAYS (A)** |
| `oidc.rs:1` | 624 | Host-only provider configuration and verified-identity binding. **STAYS (A)** |
| `passkey.rs:1` | 377 | Passkeys, through WebAuthn. **STAYS (A)** |
| `passkeys.rs:1` | 279 | Managing your own passkeys, once you are signed in. **STAYS (A)** |
| `people.rs:1` | 482 | Protected server people and local groups, ported from ADRs 0058 and 0060. **STAYS (A)** |
| `secret_keys.rs:1` | 74 | Protected key fingerprints and irreversible erasure markers, never key bytes. **STAYS (A)** |
| `session.rs:1` | 536 | Installations: one per signed-in client. **STAYS (A)** |
| `setup.rs:1` | 529 | Issue and consume first-owner setup tokens and recovery credentials. **STAYS (A)** |
| `setup/owner_code_tests.rs:1` | 42 | Fixture. Expiry checks for operator-issued first-owner codes. **STAYS (A)** |
| `setup/validation_tests.rs:1` | 147 | Fixture. Regression coverage for validation. **STAYS (A)** |
| `sharing.rs:1` | 320 | Item grants share the directory's protected transactions and publication barrier. **STAYS (A)** |
| `sharing/links.rs:1` | 220 | Recoverable item credentials and lookup by a keyed digest. **STAYS (A)** |
| `sharing/pages.rs:1` | 318 | Bounded management and discovery projections of current sharing authority. **STAYS (A)** |
| `spaces.rs:1` | 612 | Stored space authority, adapted from the source auth roles and people actions. **STAYS (A)** |
| `spaces/development.rs:1` | 91 | Explicit preview fixture provisioning. **STAYS (A)** |
| `spaces/erasure.rs:1` | 38 | Protected space revocation before the existing owner lifecycle drains. **STAYS (A)** |
| `spaces/membership.rs:1` | 257 | Direct grants and group grants, checked against the same role catalog. **STAYS (A)** |
| `spaces/receipt.rs:1` | 81 | Atomic directory request receipts shared by single and batched writes. **STAYS (A)** |
| `spaces/recovery.rs:1` | 69 | Protected commit and recovery publication for the authority worker. **STAYS (A)** |
| `spaces/search.rs:1` | 90 | Current search authority and the space's mentionable direct grants. **STAYS (A)** |
| `token.rs:1` | 173 | Secrets: setup tokens and Installation tokens. **STAYS (A)** |

### `crates/content/src`

40 files, 10,277 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `collections.rs:1` | 391 | Collection records and Notes defaults → generic records and manifest defaults. **BECOMES GENERIC (Q)** |
| `db.rs:1` | 504 | Per-space database with a hardcoded thread snooze trigger and Notes storage names → schema declarations and shared fences. **BECOMES GENERIC (W)** |
| `discovery.rs:1` | 350 | Per-space discovery over item heads. Projections share the content transaction. **BECOMES GENERIC (Q)** |
| `files.rs:1` | 676 | Durable placement with a native Notes format marker → generic format registration. **BECOMES GENERIC (W)** |
| `instructions.rs:1` | 158 | Reserved instruction file placement and replay → brokered typed item and revision placement. **BECOMES GENERIC (W)** |
| `journal.rs:1` | 238 | Append, look up and replay in each space's journal. **STAYS (W)** |
| `lib.rs:1` | 630 | Generic per-space item storage shared by Notes and other plugins. **STAYS (W)** |
| `lifecycle.rs:1` | 156 | Bounded lazy space lifecycle shared by admission and idle retirement. **STAYS (W)** |
| `lifecycle/tests.rs:1` | 100 | Fixture. Regression coverage for lifecycle. **STAYS (W)** |
| `linked_write.rs:1` | 218 | Revision staging for same-kind link changes with literal Notes check → checked declared rewrite plan. **BECOMES GENERIC (R)** |
| `mutations.rs:1` | 737 | Content/control mutation admission, shared receipts and guarded Trash placement. **STAYS (W)** |
| `pool.rs:1` | 57 | A few reusable read connections to one SQLite file, so point reads never queue. **STAYS (W)** |
| `publication.rs:1` | 133 | Identifier routing and idempotent outbox publication after a content commit. **STAYS (W)** |
| `recovery.rs:1` | 164 | Per-space recovery performed under the existing cache slot lock before admission. **STAYS (W)** |
| `recovery/tests.rs:1` | 242 | Fixture. Regression coverage for recovery. **STAYS (W)** |
| `rename_job.rs:1` | 508 | Durable Notes rewrite and inverse batches → generic checked multi-item apply and Undo. **BECOMES GENERIC (R)** |
| `revision_slice.rs:1` | 62 | Indexed reads of byte ranges in a live item's immutable versions. **STAYS (W)** |
| `rooms.rs:1` | 381 | Durable state for the host live-editing service. A checkpoint is committed. **STAYS (C)** |
| `rooms/diff.rs:1` | 265 | Turning "old text, new text" into small edits. **STAYS (C)** |
| `rooms/external.rs:1` | 189 | Outside file edits fork the checkpoint for the last file we can prove. **STAYS (C)** |
| `rooms/style.rs:1` | 58 | Preserve each file's BOM and dominant line ending while sharing LF text. **STAYS (C)** |
| `rooms/wire.rs:1` | 76 | The content boundary accepts only one plain Markdown text and closed causal state. **STAYS (C)** |
| `search.rs:1` | 423 | Markdown indexing with hardcoded thread routing and ordering → declared index sources and sort modes. **BECOMES GENERIC (Q)** |
| `search/fuzzy.rs:1` | 90 | Bounded typo verification after the title bigram index narrows candidates. **STAYS (I)** |
| `search/messages.rs:1` | 63 | Thread record postings → generic revision-bound segment indexing. **BECOMES GENERIC (Q)** |
| `search/messages/codec.rs:1` | 95 | Elias–Fano encoding of increasing offsets in a 16,384-position block. **STAYS (I)** |
| `search/messages/query.rs:1` | 315 | Newest-first postings and exact revision reads → authorized declared segment search. **BECOMES GENERIC (Q)** |
| `search/messages/source.rs:1` | 118 | Read only committed heads and exact revision byte ranges. **STAYS (I)** |
| `search/messages/writer.rs:1` | 458 | Build and replay message postings → bounded generic revision-bound segment publication. **BECOMES GENERIC (Q)** |
| `search/messages/writer/tests.rs:1` | 71 | Fixture. Regression coverage for writer. **BECOMES GENERIC (Q)** |
| `search/readers.rs:1` | 128 | Reuse per-space read-only handles. Each probe checks the current watermark. **STAYS (I)** |
| `search/source.rs:1` | 152 | Read the committed content snapshot without borrowing its writer. **STAYS (I)** |
| `search/wal.rs:1` | 156 | A byte ceiling at the SQLite WAL write boundary for the derived search writer. **STAYS (I)** |
| `search/worker.rs:1` | 461 | One bounded maintenance lane, independent of request read/write admission. **STAYS (I)** |
| `search/worker/tests.rs:1` | 74 | Fixture. Regression coverage for worker. **STAYS (I)** |
| `settings/document.rs:1` | 203 | Versioned settings document and per-setting conflict handling. **STAYS (W)** |
| `settings/mod.rs:1` | 301 | Recoverable space settings. A blocked settings intent is local to settings. **STAYS (W)** |
| `trash.rs:1` | 383 | Trash queries and recoverable removal → generic placement and purge by kind. **BECOMES GENERIC (L)** |
| `trash_job.rs:1` | 318 | Pinned Notes Trash selection and progress → generic lifecycle job. **STAYS (W)** |
| `types.rs:1` | 175 | Generic item contracts, separate from plugin-specific document wire formats. **STAYS (W)** |

### `crates/core/src`

33 files, 7,753 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `account_input.rs:1` | 270 | Private account input contracts. account identity always comes from the session. **STAYS (A)** |
| `agent.rs:1` | 279 | Manual provider turns, before the Threads file-storage step. **MOVES (P)** |
| `appearance.rs:1` | 365 | Account appearance and local font library wire models. **MOVES (E)** |
| `automation_operations.rs:1` | 358 | Space operations for authored skills and scheduled provider turns. **MOVES (U)** |
| `automation_schedule.rs:1` | 232 | Deterministic, account-zone calendar schedules for space automations. **BECOMES GENERIC (J)** |
| `automation_schedule/tests.rs:1` | 67 | Fixture. Regression coverage for automation_schedule. **BECOMES GENERIC (J)** |
| `bin/export-contract.rs:1` | 379 | Generate schemas and TypeScript declarations from the pure Rust contract. `--check` fails on drift. **STAYS (K)** |
| `harness.rs:1` | 85 | Administrator-visible harness release pins and smoke-test inputs. **BECOMES GENERIC (X)** |
| `jobs.rs:1` | 95 | Wire contracts for plugin jobs: durable host records of long-running plugin work. **STAYS (J)** |
| `lib.rs:1` | 79 | Pure v1 preview contract. No networking, async runtime or platform effects. **STAYS (K)** |
| `live.rs:1` | 41 | One server connection follows journals. clients never reconcile file replicas. **STAYS (I)** |
| `models.rs:1` | 427 | Shared operation and wire models → derive package operations from registered manifests. **BECOMES GENERIC (Q)** |
| `note_operations.rs:1` | 500 | Native Notes operation declarations and bounded picker payloads. **MOVES (N)** |
| `notes.rs:1` | 220 | Native Notes inputs/results and a generic resolver → package Notes types and retain host address resolution. **MOVES (N)** |
| `onboarding.rs:1` | 23 | Wire models for the signed-in person's first-run completion state. **MOVES (O)** |
| `palette.rs:1` | 70 | Data declarations consumed by the palette without running plugin code. **BECOMES GENERIC (F)** |
| `people.rs:1` | 470 | Wire contracts and policy metadata for people, group and invitation operations. **STAYS (A)** |
| `plugin_diagnostics.rs:1` | 53 | Space-scoped diagnostics for installed packages. **STAYS (K)** |
| `plugin_manifest.rs:1` | 700 | Data-only manifest v1. Declarations request authority. they never issue grants. **STAYS (K)** |
| `plugin_settings.rs:1` | 99 | Addressed operations for shared and personal plugin settings. **STAYS (K)** |
| `plugin_wire.rs:1` | 137 | Plugin listing/enablement results (with the v1 manifest) and generated-UI views. **STAYS (K)** |
| `provider.rs:1` | 310 | Provider adapter protocol v1: bounded JSON lines over stdin/stdout. **MOVES (P)** |
| `provider_sign_in.rs:1` | 63 | Account-only provider sign-in commands and their private outcome. **BECOMES GENERIC (V)** |
| `readiness.rs:1` | 200 | Wire types for a person's machine and provider readiness. **BECOMES GENERIC (V)** |
| `requests.rs:1` | 471 | Account request and standing instruction contracts. **BECOMES GENERIC (B)** |
| `search.rs:1` | 282 | Wire shapes and admission metadata for the palette and pickers. **BECOMES GENERIC (Q)** |
| `secrets.rs:1` | 130 | Wire contracts for secret operations. No type here carries a secret value. **STAYS (Y)** |
| `sharing.rs:1` | 445 | Sharing operation inputs and discovery metadata. **STAYS (A)** |
| `spaces.rs:1` | 91 | Wire contracts for stored space directory operations. **STAYS (A)** |
| `time_zone.rs:1` | 59 | Validate and interpret time-zone inputs. **STAYS (J)** |
| `validation.rs:1` | 627 | Validate bounded shared inputs and identifiers. **STAYS (K)** |
| `wallpapers.rs:1` | 37 | Wire models for the built-in wallpaper catalog. **MOVES (E)** |
| `web_push.rs:1` | 89 | Account-scoped browser push operations and the encrypted payload shape. **BECOMES GENERIC (H)** |

### `crates/fsio/src`

13 files, 4,533 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `beneath.rs:1` | 541 | Files inside one folder, and never outside it. **STAYS (K)** |
| `bulk.rs:1` | 356 | Stage and sync batches of new files for import or bulk writes. **STAYS (W)** |
| `directory.rs:1` | 237 | A disposable image of a new folder. Its files can be written and synced. **STAYS (W)** |
| `format.rs:1` | 42 | Format tags shared by Calternal-owned file envelopes. **STAYS (W)** |
| `lib.rs:1` | 467 | Filesystem placement, staging and sync APIs for crash-safe writes. **STAYS (W)** |
| `names.rs:1` | 71 | File names people can trust. **STAYS (K)** |
| `names/directory.rs:1` | 349 | A current view of directory names without rescanning a large folder on. **STAYS (K)** |
| `paced.rs:1` | 98 | Writing big files without making note saves wait (#298). **STAYS (W)** |
| `private.rs:1` | 136 | Owner-only (0700 directory, 0600 file) preparation for protected state:. **STAYS (A)** |
| `swap.rs:1` | 691 | Writing a file without losing what another app wrote a moment before. **STAYS (W)** |
| `tests.rs:1` | 782 | Fixture. File publication and rename guarantees on the host filesystem, including case aliases. **STAYS (W)** |
| `usage.rs:1` | 575 | Incremental logical disk usage. File mutations refresh only their changed paths;. **STAYS (W)** |
| `usage/files.rs:1` | 188 | Exact file sizes, grouped by parent directory to share path prefixes. **STAYS (W)** |

### `crates/machine/src`

34 files, 8,348 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `bin/calternal-microvm-agent.rs:1` | 35 | Guest image entry point. The host machine role does not invoke it directly. **STAYS (X)** |
| `bootstrap.rs:1` | 104 | Resume the co-located deployment handoff without sending a network code. **STAYS (X)** |
| `cli.rs:1` | 764 | `calternal machine …`: the machine role's commands. **STAYS (X)** |
| `connection.rs:1` | 322 | The machine's outbound connection: open a session, then long-poll `sync`. **STAYS (X)** |
| `enroll.rs:1` | 79 | Enrollment: exchange a one-time code for a machine identity. **STAYS (X)** |
| `identity.rs:1` | 189 | Store the enrolled machine's local Ed25519 key and identity. **STAYS (X)** |
| `lib.rs:1` | 68 | The machine role of the `calternal` binary. **STAYS (X)** |
| `link.rs:1` | 342 | HTTP plumbing to the server and the current session token. **STAYS (X)** |
| `memory.rs:1` | 52 | Machine-local memory telemetry for placement heartbeats. **STAYS (X)** |
| `ownership.rs:1` | 53 | Serialize the daemon with a machine owner's explicit local discard. **STAYS (X)** |
| `protocol.rs:1` | 607 | Wire protocol between the server role and the machine role, version 4. **STAYS (X)** |
| `ready.rs:1` | 56 | Notify the co-located systemd unit once the machine has reached the server. **STAYS (X)** |
| `runner.rs:1` | 658 | Applies server messages to sandboxes and streams exec output back. **STAYS (X)** |
| `runner/erasure.rs:1` | 47 | Apply an acknowledged server erasure under the machine role's sandbox root. **STAYS (Z)** |
| `sandbox/configured.rs:1` | 122 | Select the machine owner's server-stored execution setting at each new session. **STAYS (X)** |
| `sandbox/direct.rs:1` | 455 | Direct workspace processes on a machine its account owner marked direct. **STAYS (X)** |
| `sandbox/microvm/disk.rs:1` | 50 | Format marker for per-account microVM data disks. **STAYS (X)** |
| `sandbox/microvm/guest.rs:1` | 179 | Rootless guest agent. The guest init binds vsock to this private Unix listener. **STAYS (X)** |
| `sandbox/microvm/image.rs:1` | 53 | Verify immutable operator-installed guest image files once per role startup. **STAYS (X)** |
| `sandbox/microvm/kvm.rs:1` | 42 | Probe the KVM device from the same user that will start Firecracker. **STAYS (X)** |
| `sandbox/microvm/measure.rs:1` | 273 | Perf-only measurements for the shipped backend and the Podman control. **STAYS (X)** |
| `sandbox/microvm/mod.rs:1` | 615 | Current Firecracker account disk layout → distinct per-space runtime roles. **BECOMES GENERIC (X)** |
| `sandbox/microvm/network.rs:1` | 165 | Host-installed TAP policy and bounded attachment leases. **STAYS (X)** |
| `sandbox/microvm/tests.rs:1` | 297 | Fixture. Host-only framing regression and optional KVM substrate coverage. **STAYS (X)** |
| `sandbox/microvm/thread.rs:1` | 188 | Thread handles tied to guest generation → exact attempt handles. **BECOMES GENERIC (X)** |
| `sandbox/microvm/vmm.rs:1` | 245 | Firecracker process, persistent block device and optional TAP device. **STAYS (X)** |
| `sandbox/microvm/wire.rs:1` | 79 | One request per vsock connection. Length prefixes bound allocations before parsing. **STAYS (X)** |
| `sandbox/mod.rs:1` | 380 | Per-thread sandbox interface → purpose-scoped runtime and attempt resources. **BECOMES GENERIC (X)** |
| `sandbox/podman.rs:1` | 640 | Rootless thread containers → generic sandboxed attempts. **BECOMES GENERIC (X)** |
| `sandbox/podman/ownership.rs:1` | 107 | Podman artifact ownership is the canonical machine sandbox root. **STAYS (X)** |
| `sandbox/provider_home.rs:1` | 154 | Per-space and Claude-specific homes → qualified private profile custody. **BECOMES GENERIC (V)** |
| `sandbox/shared_broker.rs:1` | 535 | Machine credential gateway for leased harnesses → typed private profiles. **BECOMES GENERIC (V)** |
| `sandbox/shared_broker_tests.rs:1` | 328 | Fixture. The Claude and Codex fixtures share the machine-local broker and Podman boundary. **STAYS (X)** |
| `sandbox/worker.rs:1` | 65 | Worker-only sandbox profile shared by the direct and Podman backends. **STAYS (X)** |

### `crates/plugin-host/src`

6 files, 1,382 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `bounded.rs:1` | 56 | Bounded JSON serialization and allocation-free size checks for in-memory values. **STAYS (K)** |
| `input_diagnostics.rs:1` | 106 | Convert schema failures into diagnostics that never echo submitted values. **STAYS (K)** |
| `lib.rs:1` | 38 | Data-only plugin registry and bounded worker-v2 session engine. **STAYS (K)** |
| `registry.rs:1` | 502 | Manifest parsing, semantic validation and atomic registration. **STAYS (K)** |
| `runtime.rs:1` | 583 | Transport-independent, single-worker session state machine and newline JSON frames. **STAYS (K)** |
| `slots.rs:1` | 97 | Validate contribution references and the renderer allowed at each mount point. **STAYS (K)** |

### `crates/recovery/src`

9 files, 3,223 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `account_tests.rs:1` | 103 | Fixture. Regression coverage for account. **STAYS (Z)** |
| `lib.rs:1` | 568 | Protected recovery coordinator and restore proofs. **STAYS (Z)** |
| `model.rs:1` | 123 | Protected inventory, receipt and witness data model. **STAYS (Z)** |
| `people_tests.rs:1` | 188 | Fixture. Regression coverage for people. **STAYS (Z)** |
| `sink.rs:1` | 105 | Authority-barrier attachment and publication using the existing coordinator cache. **STAYS (Z)** |
| `startup_tests.rs:1` | 23 | Fixture. Startup cache reuse must retain replacement detection before authority admission. **STAYS (Z)** |
| `state.rs:1` | 406 | Durable recovery state and publication. **STAYS (Z)** |
| `storage.rs:1` | 164 | Protected backing files and recording for recovery. **STAYS (Z)** |
| `tests.rs:1` | 1543 | Fixture. Regression coverage for recovery. **STAYS (Z)** |

### `crates/roots/src`

2 files, 578 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `guard.rs:1` | 536 | Descriptor-pinned startup pairing, validation, setup and lock lifetime. **STAYS (Z)** |
| `lib.rs:1` | 42 | Three-role storage startup guard, ported from Calternal's storage startup. **STAYS (Z)** |

### `crates/secrets/src`

8 files, 2,139 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `deployment.rs:1` | 55 | AEAD for host-only deployment credentials embedded in protected authority. **STAYS (Y)** |
| `key.rs:1` | 177 | Per-owner keys, ported from the existing app's invite key. **STAYS (Y)** |
| `lib.rs:1` | 204 | Secret storage: values encrypted at rest, released only through audited use. **STAYS (Y)** |
| `lifecycle.rs:1` | 78 | Host recovery attachment and irreversible owner erasure. **STAYS (Y)** |
| `local.rs:1` | 555 | On-disk backend: one SQLite database and private key per account or space. **STAYS (Y)** |
| `tests.rs:1` | 575 | Fixture. Regression coverage for secrets. **STAYS (Y)** |
| `tests/keys.rs:1` | 207 | Fixture. Check protected key custody, rotation and erasure. **STAYS (Y)** |
| `types.rs:1` | 288 | Value types shared by every backend. None of them can carry or print a value. **STAYS (Y)** |

### `crates/store/src`

19 files, 5,756 lines.

| Source | Lines | Responsibility and verdict |
| --- | ---: | --- |
| `contracts.rs:1` | 152 | Journal values and token parser ported independently of the old engine. **STAYS (W)** |
| `highwater.rs:1` | 596 | The high-water witness (#918), stored outside backups. **STAYS (W)** |
| `lib.rs:1` | 842 | Open per-space journal, witness and signed cursor services. **STAYS (W)** |
| `migrations.rs:1` | 179 | Forward-only SQLite schema changes shared by the durable stores. **STAYS (W)** |
| `projection.rs:1` | 95 | Read-only journal access for rebuildable per-space projections and diagnostics. **STAYS (W)** |
| `proof.rs:1` | 298 | Small, bounded caches for signed sync tokens and verified cursor proofs. **STAYS (W)** |
| `read.rs:1` | 125 | Reads. Each runs on its own read-only connection, so reads never wait. **STAYS (W)** |
| `restore.rs:1` | 283 | Finding out the journal went back in time, and what to do about it (#918). **STAYS (W)** |
| `schema.rs:1` | 277 | The versioned `index.db` journal schema and its opening rules. **STAYS (W)** |
| `shared.rs:1` | 550 | State the writer, the readers and the timer all look at. **STAYS (W)** |
| `test_support.rs:1` | 17 | Fixture. Disposable fixtures for the inherited store tests. **STAYS (W)** |
| `tests.rs:1` | 356 | Fixture. Regression coverage for store. **STAYS (W)** |
| `tests/restore.rs:1` | 900 | Fixture. Exercise independent high-water witness detection of journal rollback. **STAYS (W)** |
| `tests/restore_failures.rs:1` | 361 | Failure branches in the restore witness: missing entries, changed entries. **STAYS (W)** |
| `timings.rs:1` | 52 | Stage timings for disposable storage benchmarks. **STAYS (W)** |
| `timings/note_save.rs:1` | 120 | Per-save timing collected only on the foreground blocking callback. **STAYS (W)** |
| `token.rs:1` | 268 | Signed sync tokens (#918): `epoch-seq-c-m`. **STAYS (W)** |
| `types.rs:1` | 39 | Store errors: a stable code plus a private diagnostic for trusted logs. **STAYS (W)** |
| `writer.rs:1` | 246 | The single writer for one space's `index.db`. **STAYS (W)** |

## Simplification opportunities

- Register format, field, event and operation policy once. Generate catalogue and query registrations from those declarations and remove repeated Notes and Threads switches from dispatch, search, journal and jobs.
- Keep one protected decision, receipt and attempt model across jobs, execution and notifications. Reuse the committed content outbox for authorized projections and assets. Preserve a small cold item writer and one authorized, versioned read path.
- Use source-local projections and shared plain files first. Require an independent runner to prove the host profile is generic before removing the old runner.
