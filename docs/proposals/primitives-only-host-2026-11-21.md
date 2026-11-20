# A primitives-only host: first draft for grilling (2026-11-21)

Status: **draft for the owner to grill.** Nothing here is decided unless it
says "owner decided". It replaces nothing yet. Revision 14.45 on
`lane/v2-host-base` is the frozen baseline it is compared against.

Owner decisions this draft is bound by (2026-11-20, recorded in
`docs/morning-2026-11-20.md` on `dev`):

- The server is primitives only; app logic is plugin logic. Disabling an
  official plugin that provides important functionality warns the person.
- One plugin may call another plugin's operations, with both the plugin's
  and the person's rights.
- Userdata is strictly files. Appdata is rebuildable and uses whatever is
  fastest. Anything that cannot be rebuilt, including small per-person
  state, is a file. Indexes and caches are appdata.
- Drawing is a closed component catalogue plus a sealed frame.
- The host enforces agents reading other people's writing. Authorization
  and security stay in the host.
- A sidebar section is a collection shown small. A collection is a saved
  query over items of any kind plus hand-added items, with big and small
  views. Each kind declares how it looks as a row, a card and a cell. An
  app is kinds, operations and a default arrangement of collections. A
  sidebar is a collection of collections, stored as a visible file. The
  middle is a canvas of windows.
- Chat is made of several plugins with declared dependencies. Who picks a
  part is split by effect. A missing part falls back to another one, or
  shows an error that suggests the official part.

## 0. Compared with the coordinator's thesis

The thesis (`docs/proposals/primitives-only-thesis-2026-11-21.md` on `dev`)
has seven primitives; this draft has twelve. They differ in grouping more
than in substance.

| This draft | Thesis | Which is better |
| --- | --- | --- |
| Files (P2) and Records (P3) | One primitive, Files | The thesis: append is one of two ways to write a file, not a second idea to learn. |
| People and spaces (P1) and Grants (P4) | One primitive, People and permissions | The thesis, but grants must stay a clearly named part, since agent reads depend on them. |
| Index (P5) and Collections (P6) | One primitive, Questions | The thesis: a collection is a saved question stored as a file. |
| Work (P7) and Sandboxes (P8) | One primitive, Running code (started by a call, a time or a change) | The thesis. Its three triggers are clearer than my "work unit". |
| Sync and changes (P9) | Not a primitive; implied by "a change" and "sync resumes" | **This draft**: devices, machines and live windows all need the same change feed, so name it once. |
| Calls (P10), with network and secrets as calls | Calls, plus a separate "The outside" | The thesis: the outside has its own threat model (exfiltration). |
| Drawing (P11) | Drawing, with replaceable shell parts | The thesis: shell parts are needed for the owner's sidebar card. |
| Packages (P12) | Not a primitive | **This draft**: dependencies, slots, pick scope and fallback are owner decisions that need a home. |
| Plugin-to-plugin rights: callee runs with its own rights, limited by the caller's and the person's | Callee runs with the intersection of caller, callee and person | Open: see question 6. The thesis is safer; mine lets a part ask the owner's plugin to write on its behalf. |

The integrated design (after the section lanes) starts from the thesis's
seven, adds sync and packages only if the sections confirm the need, and
keeps the rights question for the owner.

## 1. The primitives

There are twelve. Each has three lines: **does** (what it does),
**guarantees** (the security property) and **replaces** (what it replaces
in revision 14).

| # | Primitive | Does / guarantees / replaces |
| --- | --- | --- |
| P1 | **People and spaces** | **Does:** accounts, spaces, groups, roles as capability sets (ADR 0058, 0059, 0060), invitations, sign-in. **Guarantees:** every call has one authenticated person, and you see people only through a shared group. **Replaces:** the six audiences, `group_people_scope`, `personal_layout`, `viewer_self`. |
| P2 | **Files** | **Does:** a space is a tree of files (Markdown first). Read, write with an expected revision, move, trash, restore and history. A move keeps a stable file ID. **Guarantees:** no write without a write grant (P4); every change is a revision with its author; paths stay inside the space. **Replaces:** `item_engine` save, staged save, relocate, title binding, trash, `file_append`, `host.item.*`. |
| P3 | **Records** | **Does:** a file may be an append-only list of records: `{id, author, at, bytes}`. Edit and delete append a new record that points at the old one. Readers see the latest state. **Guarantees:** the host stamps `author` from the caller (a person, or an attempt acting for one); a plugin cannot forge it. **Replaces:** spans, attested prefix and suffix framing, entry anchors, `item_spans`, authorship triggers, quotes copied as bytes. |
| P4 | **Grants** | **Does:** who may read or write a file or folder: roles, groups, named people, and running work (P7) for a stated scope. Records can also be granted per author ("records by Lee may go to this run"). **Guarantees:** one check, inside the commit, for every read and write, including agents reading other people's writing. **Replaces:** activation contexts as authority, `attempt_disclosure`, consent modes, `attempt_authority_end`, discovery rows, five write-authority checks. |
| P5 | **Index and query** | **Does:** appdata. Kinds declare fields read from frontmatter or records. A query filters, sorts, windows, counts and searches, always as the asking person. Rebuilt from files. **Guarantees:** a result, count or rank never includes what the asker cannot read. **Replaces:** Bind sources, `indexed_fields`, extractors, search profiles, `window_field_delivery`, plan gates, `viewer_item_state` projections. |
| P6 | **Collections** | **Does:** a collection is a file: a saved query (P5) plus hand-added file IDs in a stated order, with big views (table, board, gallery, calendar, list) and small views (compact list, strip). A sidebar is a collection of collections. **Guarantees:** a collection grants nothing; each member is still checked against the reader's grants. **Replaces:** `layout_sections`, `host.layout.*`, `sidebar_arrangement`, `Section.repeat`, pinned strips, `host.viewer.reorder`, `host.collection.move`. |
| P7 | **Work** | **Does:** one table of work units with a state column (`waiting`, `due`, `running`, `held`, `ended`). Each unit has a request ID, an input, a kind-declared concurrency key, an optional due time, and an outbox of deliveries (input, Stop). **Guarantees:** each request runs at most once; Stop reaches the exact unit; nothing starts while admission is closed. **Replaces:** jobs, waits, alarms and grace, `execution_intents`, successor, controls ledger, update fence, receipts tables. |
| P8 | **Sandboxes** | **Does:** runs a program (a plugin worker or an agent harness) on the host or on an enrolled machine, with only the folders it was granted mounted, a deadline, and quota. **Guarantees:** no network except through P10; no file outside its mounts; it stops at its deadline or Stop, even with the server down. **Replaces:** `harness_abi`, `execution_attempt_authority`, `execution_placement`, provider login custody special cases. |
| P9 | **Sync and changes** | **Does:** files and records sync between host, machines and devices by revision. Agent output is an ordinary file that the run appends to and that syncs. Any authorized subscriber gets a change feed for a folder or a query. **Guarantees:** a change is delivered only to readers who may read it now; a synced append is applied at most once. **Replaces:** spool, relay, credit, `EntryAck`, the machine log protocol, `subscription_catch_up`, `device_reconciliation`, `offline_download`, live windows. |
| P10 | **Calls** | **Does:** one table from call name to handler. The interface, a plugin, a job or another plugin calls an operation by name over one channel and one message format. **Guarantees:** one authority check per call: caller plugin's declared rights ∩ the person's rights ∩ the target's grants. Network, secrets and provider APIs are calls too. **Replaces:** carrier variants, broker methods, `space_broker`, `space_ticket`, the egress proxy socket, the activation resolve/confirm/dispatch split for ordinary calls. |
| P11 | **Drawing** | **Does:** a closed component catalogue (lists, cards, cells, forms, editor, transcript, canvas windows, sheets) plus a sealed frame for anything else. Kinds declare row, card and cell. The host draws the one challenge for risky effects. **Guarantees:** a plugin cannot draw outside its frame, cannot hide the challenge, and sees no data it could not read. **Replaces:** the description contract's chrome, marks, swipe caps, request cards, entry views, `confirmation.material_by_kind`. |
| P12 | **Packages** | **Does:** install, enable and disable plugins with declared dependencies; extension points with a pick scope (per person or per space), a recommended plugin, and fallback. One validator used by the installer and by the authoring tool. **Guarantees:** a plugin gets only the rights it declared and the person approved; disabling an important official plugin warns first. **Replaces:** manifest v2 sprawl, `extension_points`, provider marks, bundled-ID exceptions, the separate conformance checker. |

**What is cut:** entry views and reading state, the viewer-private record
shape, quotes as copied bytes, attested framing, layout sections and the
sidebar arrangement, one attempt and one successor per item, one call per
attempt, the spool and relay protocol, consent modes, alarm grace, request
cards, the confirmation material table, fixed layout caps and fixed
retention numbers. Section 3 says where each one goes.

**Undo** is not a primitive. It is "write the previous revision back" (P2)
or "append a record that reverts" (P3).

## 2. Sufficiency: three apps built from the primitives

### 2.1 Reading List (the tiny app)

| Part | How |
| --- | --- |
| Kind | `reading`: a Markdown file with frontmatter `url`, `title`, `read: false`. Row: title and site; card: title, site and image; cell: title. |
| Operations | `reading.add` is declarative ("create a file of this kind from these fields"), so it needs no worker. `reading.mark-read` sets one field. |
| Collections | "Unread" (query `read = false`, newest first) and "All". The app's sidebar is these two, shown small. |
| Code | None. The manifest and two collection files are the whole app. |

### 2.2 Notes

| Part | How |
| --- | --- |
| Kind | `notes`: Markdown files; fields from frontmatter (`tags`, `snooze_until`). Row: title and excerpt; card: preview. |
| Editing | The catalogue's editor component bound to the file (P2 write with an expected revision). |
| Links | Links carry the stable file ID; a move needs no rewrite. Readable link text may be refreshed by a Notes job from the change feed (P9). |
| Snooze | A work unit due at `snooze_until` (P7) that clears the field. |
| Sidebar | Collections "All notes", "Tags" (grouped query) and "Snoozed". |
| Sharing | Grants on the file or folder (P4). |

### 2.3 Chat (several plugins)

| Part | Plugin | How |
| --- | --- | --- |
| Chat file | `calternal-chat` | Kind `chats`: a record file (P3) in `Chats/<name, date>/`. Each message is a record; the host stamps the author. Edit and delete for everyone are records that replace or tombstone; readers never see the old bytes. |
| Unread, read position, draft, mute, pin order | `calternal-chat` | The person's own private file, for example `People/<me>/chat-state.md`, granted to that person only (P4). The index (P5) counts records after the read position, for the asking person. |
| Pins | `calternal-chat` | A personal collection "Pinned" (P6) with hand-added chats in the person's order, shown as a strip. |
| Sidebar | `calternal-chat` | Default arrangement: Pinned (strip), Groups, People, Cals, Harnesses, Archived; each one a collection. The person edits the sidebar file. |
| Composer | `calternal-chat-composer` (depends on `calternal-chat`) | Fills the composer extension point, picked per person. It calls `chat.send`. |
| Agents | `calternal-chat-ai` and `calternal-chat-<harness>` | `chat.call` creates a work unit (P7) that runs the harness in a sandbox (P8). The run gets a read grant on the selected records whose authors consented (P4); the host refuses the rest. The run appends to its own output file, which syncs (P9). The plugin appends the reply as a record authored by the run. |
| Quotes | `calternal-chat` | A quote is a reference `(file, record)` drawn with the reader's grants. A deleted message shows nothing wherever it is quoted. |
| Notifications | `calternal-chat-notify` | Subscribes to the change feed and calls the push call (P10); picked per space. |

### 2.4 What a plain app author learns

About fifteen concepts: **space, person, file, record, kind (row, card,
cell), field, collection, query, operation, call, grant, work unit,
sandbox, extension point, component.** Revision 14 needed about sixty for a
Chat-class plugin.

## 3. What happens to each product decision

| Revision 14 rule | Goes to |
| --- | --- |
| Entry-view vocabulary (entries, authors, own, reply, mentions, activity) | Records (P3) and a transcript component (P11); mentions and activity are Chat data. |
| Reading position, unread and mention counts | Chat's per-person file plus an index count (P5). |
| Edit window and own-only actions | Chat checks `author == caller` from the stamped record; the 48 hours is Chat's rule. |
| Activity labels (typing, working) | The change feed of an ephemeral appdata channel; Chat's labels. |
| Quotes as copied bytes with cross-item rewrite | References; dropped as a host feature. |
| Attested prefix and suffix with 512-byte caps | Dropped: the host stamps record authors. |
| Chat format tokens and dated folders in the host spec | Chat's own file layout. The host knows only kind and path. |
| Markdown link rewriting on move | Stable IDs; an optional Notes job. |
| Title binding | Frontmatter title field; folder naming is the plugin's. |
| One attempt and one successor per item | The kind-declared concurrency key (P7). |
| One contribution call per attempt | Any number of calls under the run's grant (P10). |
| Send / steer / follow-up vocabulary and the update fence's Send queue | Work inputs in the outbox; "admission closed" is a state (P7). |
| Consultation between agents | A child work unit and a linked file; Chat's rule. |
| 24-hour outage limit | Sandbox deadline plus an operator setting. |
| Consent modes, one-minute wait, request cards | A per-author grant (P4); modes, timing and UI are Chat's. |
| Read-only on loss of a common group | A grant level change on group departure (P1, P4); what Chat shows is Chat's. |
| People lookup within groups | Stays in P1 as the only people query. |
| Marks and swipe caps, header and section caps | The component catalogue's own bounds; one description size budget. |
| Sidebar arrangement, strip, avatars, drag hooks, Hidden entry (my 14.45) | Collections (P6) and components (P11); the arrangement is the sidebar file. |
| Personal layout sections and placements | Personal collections (P6). |
| Confirmation material table | One host challenge shape: effect, targets, audience change (P11). |
| Row-image specifics (portrait note preview) | A kind's card declaration. |
| Search profiles (conversation vs text) | One index with kind-declared field weights (P5). |
| Archive with an execution idle predicate | A plain `archived` field; Chat stops its own work. |
| Fixed retention numbers (Trash 30 days, receipts 7 days) | Operator defaults (see question 1). |
| Alarm missed and grace | The work unit records `due_at` and `ran_at`; "missed" is the plugin's call. |
| Provider marks and `provider_icon` | A publisher-verified icon in the package (P12). |
| Placement choice ("Run on") | Stays a host-drawn choice (secrets never reach the interface); login is "a folder the person picks for the run". |
| Fixed push phrase | Notification plugin text. |

## 4. Security review: how a hostile plugin is contained

| Primitive | Hostile plugin tries | Contained by |
| --- | --- | --- |
| P1 People and spaces | Enumerate people or impersonate a person | Every call carries the host's authenticated person. People are visible only through a shared group. Plugins never see credentials. |
| P2 Files | Write outside its folders, or overwrite a concurrent edit | Path checks; write grant inside the commit; expected-revision writes; history keeps every revision. |
| P3 Records | Forge an author, or rewrite history | The host stamps the author from the caller; records are append-only; edits are new records. |
| P4 Grants | Widen a share silently; feed another person's messages to an agent | Widening needs the host challenge (P11). Agent reads are filtered by per-author grants inside the read. |
| P5 Index | Learn hidden rows from counts, ranks or timing | Queries run as the asker over the asker's grants. Counts are computed over readable rows only. |
| P6 Collections | Use a collection to reach files | Membership confers nothing; every member is rechecked. |
| P7 Work | Run twice, outlive Stop, flood the queue | Request IDs; exact Stop in the outbox; concurrency keys; per-space quota. |
| P8 Sandboxes | Exfiltrate, read the login, escape | No network except P10 calls to declared hosts; only granted mounts; the login folder is mounted only for runs the person picked; deadlines enforced on the machine. |
| P9 Sync and changes | Replay or inject output; receive changes it may not read | Appends carry the run's identity and revision; duplicates apply once; the feed is filtered by current grants. |
| P10 Calls | Use another plugin as a confused deputy | The check is the caller's declared rights ∩ the person's rights ∩ the target's grants, for every hop. |
| P11 Drawing | Spoof host UI, or hide a risky effect | Closed catalogue; sealed frame with no host access; the challenge is drawn by the host only. |
| P12 Packages | Take over an official ID; break a dependency | Signed or operator-pinned publishers; dependencies are checked at install, enable and disable; one validator. |

## 5. The four slices: what is thrown away and what survives

| Slice | Thrown away | Survives |
| --- | --- | --- |
| 1 Registry and spool (`lane/v2-host-registry`) | The spool, relay window, per-frame digests, `WindowFull`, credit and the three rate mechanisms. | Request-ID deduplication; the registry; one credit-style rate limit. |
| 2 Items (`lane/v2-host-items`) | Attested framing, spans as a side table, the viewer-private record, staged-save variants, content schema steps 4–9. | The single per-space writer, revisions, live enable and disable with the disable fence at commit, upload staging with reclaim. Spans become records. |
| 3 Execution (`lane/v2-host-execution`) | Five tables (intents, held Sends, attempts, launch bindings, controls) and the in-memory update gate. | Exact Stop, request IDs before acknowledgement, durable launch binding; all of it folds into one work table with a state column. |
| 4 Harness (`lane/v2-host-harness`) | Relay, eight-entry offset window, `EntryAttached`, the ACK rules, the separate egress socket. | Podman sandbox, per-run login folders, the egress allow-list (as a call), host-set deadline, the outage cutoff tested on a paused clock. |
| Integration and SDK | Per-slice entry points; the separate conformance checker. | The real installer as the one validator; the SDK fake host, `defineOperation` and the build command, regenerated for the new calls. |

## 6. Questions for the owner

Each question stands on its own.

**1. Which limits stay fixed?**
*What:* revision 14 has 312 numbers (sizes, counts, rates, retention). *Why
now:* every primitive needs its bounds stated before code. *Example:* "at
most three marks on a row" is a layout limit; "64 KiB per request" is a
resource limit; "Undo expires after N" is policy. *Options:* (a) layout
limits disappear, resource limits become operator defaults, security
limits stay fixed: little to learn, but each operator can loosen
resources. (b) Keep every number fixed in the contract: predictable, but
heavy to learn and change. *Recommendation:* (a), as the coordinator
proposed, with a short fixed list of security limits (request size, run
deadline maximum, grant expiry maximum).

**2. Where does per-person state live as files?**
*What:* read positions, drafts, mute and pin order must be files under the
owner's rule. *Why now:* it decides how often files are written. *Example:*
reading a busy chat moves the read position every few seconds. *Options:*
(a) one private file per person per app, written at most every few
seconds and on leave: few files, a small loss window. (b) One file per
person per chat: easy to share or delete with the chat, many tiny files.
*Recommendation:* (a), in a visible `People/<me>/` folder that only that
person may read.

**3. How does a record file look on disk?**
*What:* a chat is an append-only list of records, and edits are new
records. *Why now:* it is the file format every append-style app shares.
*Example:* a message is edited twice. *Options:* (a) keep every version
in the file and show the latest: exact history, but the file is less
pleasant to read in a text editor. (b) Rewrite the visible text in place
and keep history in revisions: readable, but authorship must come from
revision history, which is slower to index. *Recommendation:* (a), with
each record as a short Markdown block and IDs hidden by the editor, as
decided for chat files.

**4. Where does an agent's output file live, and who sees it?**
*What:* agent output becomes an ordinary synced file. *Why now:* it
replaces the whole spool and relay design. *Example:* a Claude run writes
200 lines of tool output for a two-line reply. *Options:* (a) in the
chat's folder, granted like the chat: simple, but everyone in the chat
sees the run log. (b) In the harness's own folder, granted to the person
who called it: private, but needs a link from the chat. *Recommendation:*
(b), which matches the existing decision that each run keeps its working
log in its harness's files, linked from the chat.

**5. Is Undo always "go back one revision"?**
*What:* the generic Undo restores the previous revision of what changed.
*Why now:* revision 14 has a different Undo per operation. *Example:* Undo
after moving three notes into a folder. *Options:* (a) generic revert per
file: no per-operation code, but an operation that touched several files
needs them grouped in one change. (b) Plugins supply their own inverse
operation: flexible, but every plugin must write it. *Recommendation:*
(a), with one change ID covering every file an operation touched.

**6. When plugin A calls plugin B, whose rights count?**
*What:* the owner decided "both the plugin's and the person's rights".
*Why now:* it decides whether B can do more than A asked. *Example:* the
composer plugin calls `chat.send`, which writes the chat file. *Options:*
(a) A's declared rights ∩ the person's rights, and B runs with its own
declared rights for the writes it makes: B stays useful, A cannot borrow
more than B exposes. (b) Everything is limited to A's rights: the safest,
but the composer would need write access to chat files itself.
*Recommendation:* (a), which is the usual confused-deputy-safe shape.

**7. Does a missing picked part fall back silently?**
*What:* when a person's picked composer is missing or broken, the host
uses another eligible part. *Why now:* it decides what the person sees.
*Example:* a community composer is uninstalled by a space manager.
*Options:* (a) fall back and show a small notice: the app keeps working,
and the person learns why. (b) Show the error state until the person
picks again: explicit, but the app is broken until they act.
*Recommendation:* (a) for parts the person picked, and (b) for parts that
affect others (AI, notifications), where a silent switch would change what
other people receive.

## 7. What this draft does not cover

Process sharing and memory (being measured on #103), the mockups of the
shell sidebar, the canvas of windows in detail, and the migration of
existing code beyond the table in section 5.
