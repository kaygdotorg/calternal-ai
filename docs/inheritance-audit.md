# Existing Calternal: inheritance audit

2026-11-16. Sources inspected read-only at original repository revision `ea57b84b9c9e40e8b67b7d1e1f1805915123b040` (`origin/dev` in `/home/kayg/Developer/calternal`). Paths below refer to that revision, not the sparse working checkout. This is a decision/document audit with targeted storage-code inspection, not an implementation certification or benchmark run. New Calternal-AI decisions override conflicting old decisions explicitly; the rest are inherited, not reopened by default.

## Storage: accepted design versus existing implementation

| Area | Inherit | Evidence / implementation limit |
| --- | --- | --- |
| Namespace and physical placement | One user-visible tree; SSD real files for format-plugin-claimed text, HDD real files for other content, separate SSD state. Tier trees mirror relative paths. | ADR 0062 tiering amendment. Existing `crates/calternal/src/storage.rs` implements earlier data/state roots; the final tiering amendment is explicitly docs-only. Adapt person ownership to spaces. |
| Materialization | Accepted FO1: acknowledged edits durable in state, files durably materialized within maximum 1,000 ms, including directory durability. | ADR 0062 FO1. Not a measured guarantee of current code. A state loss within the permitted lag can lose acknowledged unmaterialized edits. |
| Recovery | Validated file-only recovery includes protected identity, permissions, keys and unfinished-intent representations. Security/namespace changes reach those before acknowledgement. | ADR 0062. Current storage opening refuses missing auth state. Do not claim current databases can all simply be discarded. Preserve protected metadata and validate ownership before recovery. |
| Physical dedupe | Independent files/inodes, optional background reflink sharing, no observable cross-user duplicate signal or quota discount. | ADR 0062 supersedes ADR 0027 content-store/hardlink language. Unsupported filesystems keep full copies. |
| Attachments | Existing placement: images/video in `Photos/YYYY/YYYY-MM-DD`, other files in `Documents/YYYY/YYYY-MM-DD`; ordinary relative Markdown links; sniffed/limited staged uploads. | `crates/attachments/src/lib.rs`, `place.rs`; return contract is path/URL/hash, not generic stable attachment identity. Resumable uploads remain future work there. |
| IDs and links | Stable resource IDs, note frontmatter IDs, server-owned block identity lifecycle, rename/move aliases. | ADRs 0023/0056. Do not derive identity from title/path or assume attachment paths already provide this. |
| Undo and Trash | Retain created/displaced files with history and companions, including writes through existing open descriptors. | ADR 0073 specifies 30-day Trash. This is stronger than deleting and recreating equivalent bytes. |
| Sync | One protocol; format-aware conflicts preserve versions; outside edits are discovered. | ADRs 0044–0048. Old per-person logs/cursors become space-aware; cross-space references must not duplicate canonical editable resources. |
| Chats | Flat `AI/Cals/` and `AI/Chats/`; one Markdown file per chat; stable message blocks, fenced cards, folded steps, rebuildable message index; branch creates a file with `branched-from:`. | `ops/orchestrator/memory/calternal-decisions-ai.md`, Round 2 and Round M. Not a conversation-folder/per-message-file layout. |

### Storage work still required

- Specify canonical space roots, journal partitions and stable resource routes without changing the existing user-facing relative folder conventions. Preserve one canonical resource through cross-space sharing.
- Specify quota ownership for live files, attachments, Trash/history and ownership transfer. Do not charge a file again merely because it appears in another space.
- Add stable attachment identity and authorized relative-link/export resolution. Keep visible duplicate detection destination-scoped unless an explicit authorized reference/copy is requested.
- Reconcile the accepted hidden metadata layout with recovery records and transactional receipts; never treat edits to visible chat text as edits to authorization or external-action outcomes.
- Decide delivery timing for accepted tiering/FO1 requirements before claiming launch recovery guarantees. Reusing the old two-root binary is a staged limitation, not implementation of the full accepted design.
- Address the attachment duplicate-hint weakness: `place.rs` checks indexed hash and current file size, so a same-size external rewrite can leave a stale hint. Validate actual source/version before dedupe or reuse.

These are mostly contract/implementation tasks. A reduced launch recovery or tiering promise needs an explicit product decision; no such reduction is approved here. Add corruption, restart, external-writer, missing-mount and recovery tests to the validation plan, retaining the original performance gates.

## Deep links, accessibility and Apple clients

- **ADRs 0017/0056:** every view, item, block and UI state has an address; settings use stable path segments. Plain paths, stable plugin/item IDs, optional decorative titles, no title-based identity. Preserve block movement/retirement/alias rules. Adapt Home-wide block authority to spaces explicitly. URLs do not grant access.
- **ADR 0038:** keyboard and VoiceOver operation, names for icon buttons shared with tooltips, reduced motion and WCAG AA contrast across backdrops. ADR 0006 tests through accessible roles/names. These apply to plugin and generated UI too.
- **Native decisions in `calternal-decisions-voice.md` NA/N/C rounds, ADR 0072:** native editor with TextKit 2, UIKit iOS direction (performance-led, not dogmatic), SwiftUI watchOS, AppKit macOS baseline until Catalyst/AppKit comparison. No mobile UI imposed on Mac. Recorded minimum OS is 26.0 for iOS/watchOS/macOS; verify during release planning rather than invent a new minimum.
- Swift calls the Rust client via UniFFI; protocol rules remain in the core. Platform-owned networking effects must not turn into a second Swift sync implementation. File Provider on Mac/iOS, plain folders for Linux/CLI, small content synced fully, media on demand/pinned, multiple accounts, Keychain and OS protection are inherited requirements. `crates/client` is not implemented in the inspected revision.
- Voice recording works offline; originals are retained, cleaned transcript in Markdown, XMP descriptive metadata plus versioned transcript/timing sidecar and generated WebVTT. Distinguish these portable records from optional live-voice provider session internals.

## Orchestrator decisions to inherit, not grill again

From `calternal-decisions-ai.md` and the later Calternal-AI decisions:

- Branch from any message; switch Cal/model in the same conversation; identify the Cal/model per reply; selected-passage reply; visible edits; agent-visible timestamps and timezone context.
- Note block editor and attachment blocks reused in the composer; shared preview cards across chat/search/embeds; actual collaborative documents as artifacts. Same thread in the full AI app and optional pane; no nested AI pane inside AI itself.
- Friendly Cal configuration UI over Markdown and ready-made Cals. Existing later-engine commitment includes Hermes; validate its eventual adapter separately.
- Portable memory under `AI/Memory`, shared personal context and per-Cal notes; historical context searched on demand. Map the old personal memory model to space sharing and Honcho explicitly, preserving fresh-memory copies and previewed optional transfers. Do not assume Honcho internal state is reconstructible from text alone.
- Automations are editable notes with When/If/Do, any/all conditions, chat-based creation and manual controls, run details and repair UI. Failures now map to Activity. An old recorded run does not gain a timeless Undo button for external effects.
- Instant readiness remains a performance requirement. Old eager warming/preparation ideas must be validated against new isolation, credential and resource budgets, not blindly keep one process running per installed plugin or user.
- Selective transcript-sharing inclusion controls for headers, images, attachments and steps must operate through explicit scoped grants; filtered publication does not alter the authoritative private history.
- Dictate/Talk choice and retained voice transcript/audio; latest explicit controls supersede older conflicting auto-intent ideas.

New user decision on 2026-11-16: completed helper/subthreads automatically archive. Recommended implementation archives after durable result linkage, retains blocked/failed attention, and preserves discoverable parent-linked history. Archive is not deletion, workspace cleanup or cancellation. Ordinary user threads are not auto-archived by this rule.

## Plugin and subplugin mapping

The user explicitly requested later Notes, Tasks, Calendar, Mail, Files, Photos and Money app plugins with app-specific subplugins. Use one plugin system: an extension declares its parent contract/version and its own grants. UI grouping under a parent does not allow private table access or inherited unlimited permission. Routes and exported files remain usable when a parent or extension is disabled; unavailable rendering has a clear fallback. Register resource formats, operations, search, settings, import/export, background jobs and UI contributions through public contracts.

Developer tooling belongs in Browse → Dev, selected during onboarding and changeable later. Ordinary app extensions are grouped under their app, not all placed under Dev.

## App-by-app inherited contracts and extension points

Later recorded decisions take precedence over earlier ADR wording where explicitly amended. The following are specifications/decisions; this audit does not certify that the apps ship.

| App | Storage and behavior to retain | App-specific extension points | Primary sources |
| --- | --- | --- | --- |
| Notes | Markdown text is the collaborative document; rendered/source modes; no block-tree conversion on save. Stable IDs, relative links, unknown-field/untouched-byte preservation, outside-edit reconciliation and own-change undo. Later decisions require editable properties on notes with properties. | Editor contributions, property renderers, previews, actions, Composer phrases, format handlers and Collections views. | ADRs 0018–0025, 0068; Files decisions DB-G1–G3 and Nov 11 amendments. |
| Tasks | A checkbox is a task; untracked checkboxes remain Loose ends. Tracked task has an owning Markdown note, authoritative frontmatter and child links; status mirrors update atomically. When and Deadline differ. Explicit completion log; linked session hours do not complete tasks. | Grouping/list/board/matrix views, Composer/date grammar, reminder and interop providers, calendar sessions. | ADRs 0066/0068; Tasks decisions grouping model Oct 21; Calendar Nov 7. |
| Calendar | Time/all-day journal lines become events with one structured owner and stable occurrence identity. Calendar owns periodic notes and event notes. Private notes about another person's event remain private; person links do not send invites. Exact supplied URLs preserved. | Connected calendars/CalDAV, activity from every app, event describers, views/sets, recurrence/alarm adapters, Composer. | ADRs 0067/0068; Calendar decisions Nov 7. These ADRs are Proposed/specification, not implementation certification. |
| Mail | Provider-authoritative client, not mail hosting. Exact `.eml` mirror under `Mail/<account>/…`; derived thread/flag/category state. Gmail labels do not duplicate messages. JMAP or IMAP/SMTP, resumable backfill, offline drafts, explicit send queue, send-later and 10-second send Undo. Account removal keeps mail unless explicitly deleted. | Provider adapters, rendering/attachment processing, categorization, automation, scoped cal actions, Apple Mail proxy and cross-app conversions. | Mail decisions ML/MR/Composer/MA/MB rounds; ADR 0071; mail architecture plan. |
| Files | Real folder tree, not another silo. Stable move identity/new copy identity; ordinary relative links rewritten with moves. Shared Trash/history, per-owner accounting and permission-filtered queries. Keep Downloaded, resumable transfer, WebDAV and later native File Provider. | Previews, indexers, protocol adapters, actions, import/export and platform integrations. Core writes, identity and grants remain protected. | Files decisions FB/FD, Nov 5 final, FS1–FS6; ADRs 0027/0028 with later storage amendments. |
| Photos | Lossless originals; capture-local-day placement under `Photos/YYYY/YYYY-MM-DD`; marked external folders supported. Albums are Collections; favourite is independent of star rating. Strong evidence for Live Photo/RAW pairing; paired delete/restore; exact dedupe vs suggested near-duplicates. No editing in v1; optional transcoding. | Recognition models, OCR, semantic search, codecs, metadata, album/share views and Calendar activity. | Photos PH/PR/FP rounds, Nov 8–9; Files FS3. Nov 9 record explicitly says Photos crate not server-linked and perf run absent. |
| Money | `Money/<Budget>/Budget.md`, `Accounts/`, `Categories/`, `Months/YYYY-MM.md`; readable Markdown records with stable IDs, exact minor units and dated FX. Atomic record merges; malformed records surface Problems and withhold affected totals. Envelope budgeting and explicit card-payment semantics. | Importers, receipts, reports, views, Composer, automation and shareable cards. Exact arithmetic/merge/history contracts remain mandatory. | ADR 0057; Money decisions Oct 19 and Nov 4/7/9/10. Latest engine direction is server-only, with tested simple TS previews; no browser Wasm requirement. |

### Precedence fixes that the migration must not miss

1. Calendar's Nov 7 owner decision gives every app its own root folder, including `Tasks/` and `Calendar/`. This supersedes older `Notes/Tasks` and `Notes/Calendar` paths. Map the relative layout into each space rather than bringing back a second personal-space type.
2. Files FS1–FS6 and Nov 9 follow-ups place **app-authored metadata, sidecars, templates and settings in `.calternal/`, keyed by stable IDs**. User originals and imported XMP remain untouched; the UI combines imported and Calternal metadata. Export may place sidecars beside exported copies. This overrides older beside-original voice/photo sidecar placement, without discarding their formats or content. Protected foundation records still need their own access rules; a hidden folder alone is not a boundary.
3. ADR 0068 supersedes accumulated task/event proposals: logged session hours never auto-complete tasks. Its Proposed status means implementation needs proving, not that older contradictory semantics should be restored.
4. Later editable note properties supersede ADR 0025's hidden/read-only presentation. Photos favourites never become star-rating changes.
5. Files history policy: every change for 30 days, daily versions for six months, then monthly; Trash 30 days. Chat artifact retention and space quota accounting must map to this existing policy deliberately.
6. Typed plugin settings from ADR 0055 have stable IDs, scope and defaults; settings surfaces and palette entries derive from declarations. Subplugins use this same contract.

Source paths for recorded app decisions are `ops/orchestrator/memory/calternal-decisions-{tasks,calendar,mail,files,photos,money}.md`; native choices are in `calternal-decisions-voice.md`; orchestration is in `calternal-decisions-ai.md`. Inspect the pinned revision, not a stale working checkout.

## Accounting and portability resolved, 2026-11-16

The user approved charging canonical content once to its owning space, with the owner's plan funding it by default. Quota follows logical file size regardless of XFS reflink savings. Shared references do not duplicate usage; independent copies count separately. The user's description of existing reflink use establishes intent; the targeted code audit above still distinguishes implemented storage from later documented tiering/dedupe requirements.

Ordinary portable exports omit credentials and guide account reconnection, distinct from protected administrator disaster-recovery backups. History/Trash/runtime-storage accounting details still need a published breakdown, without silently reducing accepted retention.
