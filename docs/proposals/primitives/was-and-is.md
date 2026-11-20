# Was and is: the host before and after the primitives-only design

Status: integrator's working file for #107 (2026-11-21). Line counts are
from `origin/dev` at the time of writing, counted with `git grep -c ''`
over `.rs`, `.ts` and `.svelte` source including tests. They are rough
and meant for scale, not accounting. "Revision 14" means the contract
frozen at 14.45 on `lane/v2-host-base` (`1cff1c0f`).

The design side ("is") follows the coordinator's thesis
(`docs/proposals/primitives-only-thesis-2026-11-21.md` on `dev`) and the
section files under `docs/proposals/primitives/`. Where a section changes
it, the integrated design wins over this file.

## 1. What the server was

### 1.1 Code on `dev` (Rust, about 160,000 lines)

| Area | Where | Lines | What it does today |
| --- | --- | ---: | --- |
| Accounts, sessions, spaces, sharing | `crates/auth`; `server/src/account_*`, `api/sharing*`, `api/people.rs`, `authz` | 12,800 + 4,900 | Sign-in (passkeys, OIDC), sessions, spaces by UUID, roles, groups, invitations, per-item sharing, guest links |
| File storage and durability | `crates/store`, `crates/fsio`, `crates/roots`, `crates/recovery` | 5,900 + 4,500 + 1,100 + 3,200 | Space roots, safe paths, atomic swap, usage quotas, tiering, backups and restore |
| Content engine | `crates/content` | 11,600 | Per-space `content.db`, file index, search (2,200), rename job and link rewriting, rooms, settings, Notes-only trash and linked-write branches, the snooze trigger |
| Native chat ("turns", "threads"), automations, live rooms | `server/src/{turns,threads,automations,live_rooms}` | 3,700 + 3,200 + 2,000 + 2,100 | The old app's chat engine, provider turns, scheduled automations, presence rooms |
| Native Notes | `server/src/notes`, `core/note_operations.rs`, `bundled_notes` | 900 + 500 + … | Notes operations, tags, links, pages, retention |
| Providers and agents | `server/src/provider_sign_in`, `machine/sandbox/{shared_broker,provider_home}.rs` | 900 + 1,000 | Claude and Codex sign-in, shared provider broker, provider homes |
| Machines and sandboxes | `crates/machine`, `server/src/machines*` | 12,300 + 7,900 | Enrolled machines, link and connection, Podman and microVM sandboxes (microVM 4,300), runner |
| Plugins (v1 runtime plus v2 pieces) | `server/src/plugins`, `crates/plugin-host`, `crates/genui` | 17,700 + 5,200 + 3,300 | Package store, workers, broker, jobs, live catalog, network broker (2,900), space broker, registry v2, GenUI |
| HTTP API and appearance | `server/src/api` | 8,500 | Routes; appearance and wallpapers alone 3,000 |
| Upgrades and migrations | `server/src/upgrades` | 2,100 | Migration chains and the running-release upgrade check |
| Push, network guard, secrets | `server/src/{web_push,netguard}`, `crates/secrets` | 900 + 700 + 2,100 | Web push for chat events, outbound address guard, secret store |
| Models, CLI, client, bindings | `crates/{core,cli,client,types,bindings}` | 7,900 + 5,700 + 2,300 + 1,500 | v0 models and validators, `calternal` CLI, Rust client, UniFFI |
| Performance harness | `crates/perf` | 4,000 | Timing suites |

The read-only audit on #78 counted about 17,900 lines as app-specific
(native chat engine, Notes engine, provider routes), plus about 3,400 lines
of appearance and wallpapers.

### 1.2 TypeScript on `dev` (about 24,000 lines)

| Package | Lines | Role |
| --- | ---: | --- |
| `packages/editor` | 5,300 | Note editor (real web code) |
| `packages/ui` | 5,100 | Shared web UI |
| `packages/contracts` | 4,800 | Generated contract types and the schema-to-types generator |
| `packages/description` | 4,400 | Web renderer of the description contract |
| `packages/genui` | 2,000 | v0 generated UI |
| `packages/plugin-sdk` | 1,300 | SDK (on `dev`; the authoring lane adds 5,800) |
| `plugins/*` | 1,800 | Claude Code, Codex, Reading List, guides |

### 1.3 The revision 14 contract (about 17,200 lines)

| Document | Lines | Contents |
| --- | ---: | --- |
| `docs/plugin-api-v2.md` | 1,845 | Host specification, cutover gates |
| `docs/description-contract.md` | 842 | Interface description |
| `contract/host-catalogue-v2.json` | 9,906 | 24 host operations, 42 component routes, 47 host methods, about 40 rule blocks |
| `contract/description-v2.schema.json` | 3,636 | 87 shape definitions |
| `contract/plugin-manifest-v2.schema.json` | 949 | 18 top-level fields, 20 kind fields, 13 operation fields |
| Verifier and fixtures | about 5,000 Python plus 450 fixtures | Pinned conformance |

The audit on #78 counted about 60 concepts for a Chat-class plugin and 312
numeric limits.

## 2. What the server is

The same areas, in the primitives-only design. The primitive names follow
the thesis; the integrated design may merge or rename them.

| Area | Becomes | Primitive |
| --- | --- | --- |
| Accounts, sessions, spaces, sharing | Kept as is: people, groups, spaces, roles, invitations, per-file grants | People and permissions |
| File storage and durability | Kept: userdata files with revisions, safe paths, quotas, backups | Files |
| Content engine | Rebuilt as the index over files, frontmatter, text, links and origin stamps; collections are files | Questions |
| Native chat, automations, live rooms | Deleted from the host. Chat, automations and presence become plugins. | — |
| Native Notes | Deleted from the host. Notes is a plugin. | — |
| Providers and agents | Deleted from the host. A harness is a plugin that runs a program. Logins are folders the person picks. | Running code |
| Machines and sandboxes | Kept as the place code runs: sandbox, deadline, Stop, mounted folders. The relay protocol goes. | Running code |
| Plugins | One package store, one validator, one call table, one work table | Calls, Running code |
| HTTP API | One call route plus file sync and the change feed. Appearance becomes a shell plugin or client setting. | Calls, Drawing |
| Upgrades and migrations | One initial schema per database while installs are fresh | — |
| Push, network guard, secrets | Kept, behind calls: outbound requests to declared hosts, secrets usable but unreadable, notifications | The outside |
| Models, CLI, client | v0 models deleted; the CLI calls the same call table | Calls |
| Interface | Closed component catalogue, sealed frame, shell parts filled by descriptions; collections supply every list | Drawing |

## 3. Fate table: every revision 14 concept

**K** kept, **R** replaced (by what), **D** deleted.

### 3.1 Host operations (catalogue `operations`)

| Operation | Fate |
| --- | --- |
| `host.item.create` | R: file create of a kind (declarative create from the kind's defaults) |
| `host.item.rename`, `host.collection.move` | R: file move (stable file ID; links resolve at read) |
| `host.item.trash`, `host.item.restore`, `host.trash.purge` | K as file operations |
| `host.undo` | R: write the previous revision back, grouped by one change ID |
| `host.approval.resolve` | R: the host challenge (Drawing) resolving a pending call |
| `host.execution.stop` | R: Stop on a work unit |
| `host.route.set`, `host.session.end` | K as shell calls |
| `host.item.share` | K as grant change (with the host challenge for widening) |
| `host.item.patch-metadata` | R: frontmatter write |
| `host.action.status` | R: work unit state |
| `host.item.archive`, `host.item.unarchive` | R: plain `archived` field; no execution idle predicate |
| `host.layout.*` (6), `host.layout.unplace`, `host.viewer.reorder` | R: collections (hand-added items in order) |

### 3.2 Host methods (manifest `hostMethod`, 47)

| Methods | Fate |
| --- | --- |
| `item_create`, `item_save`, `item_save_begin/chunk/seal`, `item_head`, `item_range`, `item_history`, `item_trash`, `item_restore`, `item_purge`, `empty_trash`, `item_rename`, `item_rename_status`, `item_rename_undo` | R: file read, write (with expected revision), append record, move, trash, history; uploads keep one staged form |
| `item_append` | R: append record |
| `item_spans` | R: records with origin stamps |
| `viewer_state_read`, `viewer_state_write` | R: the person's own small file |
| `items_list`, `items_search` | R: query |
| `collection_create`, `collection_membership` | R: collection file writes |
| `sharing_check` | R: grant check (internal to every call) |
| `start_job`, `checkpoint_job`, `job_status`, `cancel_job`, `alarm_set`, `alarm_cancel` | R: work units (state column, due time, Stop) |
| `execution_start`, `execution_status`, `execution_stop`, `execution_input`, `execution_spawn_child`, `execution_wait`, `execution_session_bind` | R: work unit running a program on a machine; input and Stop through its outbox; child units |
| `contribution_call`, `contribution_read` | R: calls to the part's operations; output is a file |
| `read_setting`, `set_setting` | R: settings are files |
| `request_secret_handle`, `proxy_request`, `credential_call` | R: the outside (secret use, outbound request) |
| `item_view_publish`, `item_view_context` | D: no retained item views; drawing reads files and queries |
| `extension_choices` | R: package slots with pick scope |

### 3.3 Component routes (42)

| Routes | Fate |
| --- | --- |
| `view.open`, `view.renew`, `app.open`, `selection.*`, `search.settled`, `status`, `undo` | R: one call route plus the change feed |
| `activation.resolve`, `activation.confirm`, `activation.dispatch`, `activation.receipt` | R: one call with a request ID; the host challenge for risky effects |
| `layout.arrange` | R: the sidebar is a collection file |
| `item_view.open/read/follow/asset/selection`, `entry_composer.*`, `request-card.read`, `request_resolve` | D: Chat draws records with list and composer components |
| `row_images.prepare` | R: file upload with an image kind |
| `sealed_frame.*` | K |
| `share_sheet`, `trash_sheet`, `choice.options`, `extension.choices` | K as shell sheets and choices |
| `editor.open` | K: the catalogue editor bound to a file |
| `attachment.*` | R: file upload |
| `offline.download`, `offline.range`, `device.reconcile` | R: file sync |
| `search.plugin` | D: one index |
| `execution.control` | R: work unit outbox |
| `item.locate` | R: stable file ID |

### 3.4 Catalogue rule blocks

| Block | Fate |
| --- | --- |
| `audiences`, `window_field_delivery` | R: one rule: a query runs as the asker |
| `kinds` (`collections`, `requests`, `layout_sections`) | R: collections are files; requests become pending calls; layout sections deleted |
| `query` | K, smaller: filter, sort, window, count, search; extractors become kind field declarations |
| `generic_item_fields` | K: id, title, updated, trashed, archived; `provider_icon` D |
| `confirmation` | R: one host challenge shape |
| `cache_policy`, `view_result_retention`, `subscription_catch_up`, `device_reconciliation`, `offline_download` | R: file sync and the change feed |
| `execution_attempt_authority`, `execution_placement`, `execution_intents`, `execution_recovery`, `attempt_authority_end`, `activation_capture` | R: one work table, sandbox rules, grants |
| `carrier` | R: one channel, one message format |
| `extension_points` (`harnesses`, `calls`, `harness_abi`, `choices`, `identity`) | R: package slots with pick scope; a harness is a plugin that runs a program |
| `alarms` | R: work unit with a due time; no grace or missed bit |
| `file_append` | R: append record |
| `view_snapshots`, `item_views_entries` | D |
| `viewer_item_state` | R: the person's own file |
| `sidebar_arrangement` | R: sidebar file (a collection of collections) |
| `trash` | K, retention as an operator default |
| `item_engine` (`relocate`, `quotes`, `title_binding`, `format_open`, `attested_write`, spans, saves, placement, locators) | R: file write, move, append; quotes become references; framing D |
| `group_people_scope` | K: people visible only through a shared group; loss-of-group policy moves to plugins |
| `attempt_disclosure` | R: per-author grant to a run |
| `provider_marks` | R: publisher-verified package icon |
| `open_cutover_requirements`, `application_cutover_requirements` | D: no cutover from revision 14 |

### 3.5 Manifest fields

| Field | Fate |
| --- | --- |
| `api_version`, `id`, `name`, `version`, `icon`, `worker` | K |
| `apps` (`description`, `extension_points`) | R: an app is kinds, operations and a default arrangement of collections; slots are declared with pick scope |
| `kinds.schema`, `title_field`, `storage`, `indexed_fields`, `searchable` | K, merged: a kind is a schema over frontmatter plus row, card and cell |
| `kinds.viewer_fields`, `view_snapshots`, `live_room`, `attempt_discovery`, `execution_target`, `device_row_images`, `search_profiles`, `claimed_text`, `host_create`, `extension`, `extractors` | D or folded into the kind schema |
| `kinds.file_format` | R: `{id, version, default_app}` only |
| `kinds.append_file` | R: "this kind is a record file" |
| `operations` (`input_schema`, `output_schema`, `risk`, `idempotency`, `target`, `creates`) | K, declared once |
| `operations` (`required_capability`, `methods`, `write_origins`, `material_inputs`, `credential_routes`) | R: the plugin's grant (kinds and operations it may use) |
| `reads` | R: the plugin's grant |
| `jobs`, `wait_webhooks` | R: triggers (a call, a time, a change) |
| `settings`, `secrets`, `credential_routes`, `network`, `content_fetch` | R: the outside |
| `contributions` | R: parts are plugins that fill a named slot; dependencies declared |

### 3.6 Limit groups (312 numbers)

| Group | Fate |
| --- | --- |
| Layout (marks, swipe, sections, rows, menu items, header actions) | D: one description size budget |
| Query (predicates, sort fields, window size, search terms) | K as resource limits, operator defaults |
| Item engine (relocate, quotes, framing, staging) | D or K as resource limits (upload size, staging) |
| Execution (steers, frames, spool, rates, deadlines) | K as resource limits: run deadline, output rate, quota; spool D |
| Activation (contexts, interaction bytes) | K: request size (security) |
| Retention (trash, receipts, Undo) | K as operator defaults |
| Security (request size, grant expiry maximum, run deadline maximum) | K fixed |

## 4. The code: what survives

Rough line counts. "Rewrite" means the idea and tests survive, the code is
reshaped.

| Crate or branch | Survives | Rewritten | Deleted |
| --- | --- | --- | --- |
| `auth` (12,800) | Nearly all | Grant checks move to one place inside the call table | — |
| `store`, `fsio`, `roots`, `recovery` (14,700) | All | — | — |
| `content` (11,600) | File index, search engine, writer | Index over frontmatter, records and stamps (about 4,000) | Notes-only trash and linked-write branches, snooze trigger, rename-job link rewriting, rooms (about 2,500) |
| `server/src/{turns,threads,automations,live_rooms,notes,provider_sign_in}` (12,800) | — | — | All, after the plugins replace them |
| `server/src/plugins` (17,700) | Package store, worker launch, network broker | One call table, one work table (about 5,000 new) | v1 broker paths, GenUI glue, live catalog duplicates (about 8,000) |
| `plugin-host` (5,200) | Runtime, space channel | Registry to the one validator | Registry v1 |
| `machine` (12,300) | Sandboxes (Podman, microVM), enrollment, link | Runner to "run a program, sync its output file" | Shared provider broker, provider homes (1,000) |
| `server/src/machines*` (7,900) | Enrollment, connection | Space channel to the one message format | — |
| `server/src/api` (8,500) | Sharing, spaces, people | One call route | Appearance and wallpapers (3,000) as host code |
| `server/src/upgrades` (2,100) | — | — | Migration chains |
| `core` (7,900), `genui` (3,300), `client` (2,300) | Validation helpers | — | v0 models, note operations, GenUI |
| `lane/v2-host-registry` (+2,600) | Request-ID deduplication, registry interfaces | — | Contribution spool and window |
| `lane/v2-host-items` (+5,200) | Live enable and disable with the disable fence, the writer path, upload staging and reclaim | Spans to records with stamps | Attested framing, viewer record |
| `lane/v2-host-execution` (+4,100) | Exact Stop, request IDs before acknowledgement, launch binding | Five tables to one work table | Update gate in memory |
| `lane/v2-host-harness` (+3,900) | Podman entry, login folders, egress allow-list, host-set deadline, paused-clock tests | Entry channel to one stream | Relay window, `EntryAttached`, ACK rules |
| `lane/v2-host-integration` (+11,400) | The real-installer end-to-end test harness | Against the new calls | Merges of the above |
| `lane/sdk-v2-authoring` (+5,800) | Fake host, `defineOperation`, typed calls, build command | Against the new calls | Catalogue-specific helpers |
| `packages/description` (4,400) | Renderer core, lists, windows | Component catalogue | Entry views |
| Revision 14 contract (17,200) | Ideas and tests for authority | — | The documents themselves, after the new contract is written |

## 5. Build plan

Each step ends with a test on a running server. The first step already
does one real thing end to end: the integration lane showed that four
libraries were still far from one running Send.

| Step | Goal | Test that proves it | Estimate |
| --- | --- | --- | --- |
| 1 | **One plugin does one real thing.** Install Reading List through the one validator; a person calls `reading.add` over the one call route; the file appears in userdata; a query returns it. | An end-to-end test starts the real server binary, installs the package, calls the route, reads the file from disk and gets the row back from the query. | 2–3 days |
| 2 | Questions and collections: index rebuilt from files, collections as files, live windows from the change feed | Edit a file by hand on disk; the query and an open window update. Delete the index; it rebuilds to the same answers. | 3–4 days |
| 3 | People and permissions: grants per file and folder, the plugin's grant at install, one check per call | A second person and a guest see only what they are granted, in queries, counts and the change feed. A plugin outside its grant is refused. | 3 days |
| 4 | Running code: a worker in a sandbox started by a call, a time and a change; one work table | Notes snooze: a due work unit clears a field after a restart in between. Stop ends a running worker. | 3–4 days |
| 5 | Records and origin stamps | A person and a plugin append to a record file; the stamps are the host's; a forged author is refused; the file stays hand-editable. | 2–3 days |
| 6 | Agents on a machine: a run writes its output file, which syncs | Kill the server while a harness writes; restart; the output arrives once, in order; Stop reaches the exact run. | 4–5 days |
| 7 | Agents reading other people's writing | A run sees only records whose authors granted it; revoking a grant mid-run stops further reads. | 2 days |
| 8 | Calls between plugins | The composer plugin calls `chat.send`; it cannot exceed the intersection of rights. | 2 days |
| 9 | Drawing: catalogue, shell parts, collections as sidebar sections | Notes, Reading List and Chat draw on web and native from descriptions only. | 1–2 weeks (interface lanes) |
| 10 | Packages: dependencies, slots with pick scope, fallback, the disable warning | Uninstall a picked composer; the host falls back with a notice; disabling the official Chat warns. | 2–3 days |
| 11 | Delete the native engines and v0 models | Workspace builds without them; the three apps still pass their end-to-end tests. | 2 days |

Steps 2–8 can overlap after step 1 lands, two or three lanes at a time,
because each adds one primitive behind the same call route.
