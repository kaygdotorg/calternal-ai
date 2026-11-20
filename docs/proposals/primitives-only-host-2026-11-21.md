# A primitives-only host · integrated design · 2026-11-21

Status: **revision 2 of the integrated design (#107)**, with the security
review's fixes applied (§11). Nothing here is decided
unless it says "owner decided". It integrates the coordinator's thesis
(`docs/proposals/primitives-only-thesis-2026-11-21.md`) and six section files
under [`primitives/`](primitives/):

| Section | File | Lane |
| --- | --- | --- |
| Files, records, questions, collections | [files-and-questions.md](primitives/files-and-questions.md) | `lane/design-files` |
| People, permissions, calls, threat model | [permissions-and-calls.md](primitives/permissions-and-calls.md) | `lane/design-authority` |
| Running code, machines, agents | [running-code.md](primitives/running-code.md) | `lane/design-run` |
| Drawing, collection views, shell parts | [drawing.md](primitives/drawing.md) | `lane/design-interface` |
| Prior art | [prior-art.md](primitives/prior-art.md) | `lane/design-priorart` |
| The author-facing API and four plugins | [authoring.md](primitives/authoring.md) | `lane/sdk-v2-authoring` |
| Was and is, the fate table, the build plan | [was-and-is.md](primitives/was-and-is.md) | this lane |

The sections hold the detail: operations, costs in reads and flushes, and
attack tables. This document is the whole picture, meant to be read top to
bottom. Where it and a section disagree, this document records the
resolution and the reason (§8).

## 0. In one paragraph

The host is a file server that knows who is asking. It stores files and
their history; it answers questions about them; it decides, at one table,
whether a call may happen; it runs plugin code and agent programs in boxes;
and it hands interface declarations to clients, which draw them with native
controls. It knows nothing about chats, notes, mail, agents or providers.
Everything a person would call a feature is a plugin, built from the same
five primitives a stranger's plugin gets.

## 1. Owner decisions this design is bound by

From `docs/morning-2026-11-20.md` on `dev` ("Change of focus", "First
grilling answers", "Collections and sidebars"), all 2026-11-20:

- The server is primitives only; app logic is plugin logic. Disabling an
  official plugin that provides important functionality warns the person.
- One plugin may call another plugin's operations.
- Userdata is strictly files. Appdata is rebuildable and uses whatever is
  fastest.
- Drawing is a closed component catalogue plus a sealed frame.
- The host enforces agents reading other people's writing.
- A sidebar section is a collection shown small. A collection is a saved
  query over items of any kind plus hand-added items, with big and small
  views. Each kind says how it looks as a row, card and cell. An app is as
  dumb as possible: kinds, operations and a default arrangement of
  collections. A sidebar is a collection of collections, stored as a
  visible file. The middle is a canvas of windows.
- Apps are made of several plugins with declared dependencies. Who picks a
  part is split by effect. A missing part falls back, or shows an error
  that suggests the official part.
- Earlier, still binding: a deleted message vanishes from the conversation
  with no trace in the interface; editing is allowed for 48 hours; read
  position, draft and mute are per person; consent is asked per
  conversation ("ask each time" or "always"), and after one minute without
  an answer the person's messages are withheld; spaces by UUID; roles are
  reader, editor, manager and owner; rusqlite only; Markdown files.

## 2. The primitives

Five primitives. The thesis had seven; my first draft had twelve.

| # | Primitive | What it does | The security property it guarantees |
| --- | --- | --- | --- |
| 1 | **Files** | Items are Markdown files in a space. Four writes: replace, set fields, append a record, edit or redact one record. Move, trash, restore and revert one change. History of every file. A kind is a schema over frontmatter (and, for record kinds, a record shape) that a plugin declares. A file's **kind and ID are host-state facts fixed at create**; the frontmatter holds read-only copies. | The host stamps every write with person, plugin and run, where no plugin, file share or editor can change the stamp. On record kinds **every byte change** (replace, share or editor edit, revert, restore) is diffed per record and checked against the kind's policy; a refused change becomes a conflict copy. Paths stay inside the space. |
| 2 | **Questions** | One index (appdata, rebuildable) over fields, text, links, records and stamps. One query shape with live windows. Lists come from three sources: a **collection** (a file holding a saved query plus hand-added items), a **record file** (the records of one file), or a **facet** (the values of a field). Backlinks, search and the change feed are the same engine. | Every answer, count and rank is computed as the asker, over what the asker may read. A collection grants nothing. |
| 3 | **Authority** | People, groups, spaces, roles and sharing as the existing app settled them (ADR 0058–0060). A plugin's **grant** (kinds, paths, calls, network, secrets, programs, triggers). One **call table** from name to handler for host methods, the outside (network, secrets, notifications) and every plugin's published operations. Consent, dependencies, slots and picks, quotas and the audit trail. | One check per call, at the table: who is asking, through which plugins, on which targets. Agents read other people's writing only with those people's consent, where an agent is anything **outbound-capable**: the closure, computed at install, over network, secrets, programs, permission to start any program, calls to any outbound-capable operation, and frames. Taint is per worker process: other people's writing returned to a process marks every later write it makes. |
| 4 | **Running code** | One per-space **work** table with a state column. Work runs in a **box** (plugin process, hosted sandbox, or a person's enrolled machine), started by a call, a time or a change. A program talks to the host through two append-only, single-writer files: the **inbox** (host writes) and the **log** (machine writes). | A box has no network, files or rights except its grant. Stop fences the run at once and reaches only that run, and the run-scoped handles it was given end at Stop. Run files are private to the run's actor. Only the actor may send input to a run. A plugin starts only its own programs, or the program the person picked. |
| 5 | **Drawing** | Plugins declare; they never draw. Kinds declare looks (row, card, cell, page, record). Collections declare views (big: list, table, board, gallery, calendar; small: compact list, strip). The shell is named slots filled by parts. A closed catalogue plus a sealed frame. Host-only screens: sign-in, Settings and Plugins, challenge, consent, Stop, safe mode. | No plugin code runs in a client outside the frame, and a plugin that declares a frame is outbound-capable. A plugin cannot spoof or hide a host screen; every plugin form carries a non-removable header naming the plugin, and the catalogue has no password-like field. Drawn values go from the host to the client as the viewer, never to the plugin. |

### 2.1 What was merged or cut, and why

| Change | Reason |
| --- | --- |
| Thesis 3 (people and permissions) and 5 (calls) **merged into Authority** | The permission check exists only at the call table, and people and groups are data the check reads (authority section). |
| Thesis 7 (the outside) **folded into Authority's call table** | Network, secrets and notifications are three host methods with a grant (running-code section, prior-art Q6). Their threat (exfiltration) is covered by the outbound-capable rule. |
| My draft's Records **folded into Files** | Append is one of four writes, not a second idea. |
| My draft's Collections **folded into Questions** | A collection is a built-in kind plus the query engine; nothing else (files section). |
| My draft's Sync and changes **folded into Files and Running code** | Run sync is two single-writer files (running-code section). Device sync is file revisions. The change feed is a live query. A general two-way sync with merges would bring back the relay's problems. |
| My draft's Packages **folded into Authority** | Dependencies, slots and picks are grants and checks (authority 1.8). |
| My draft's Work and Sandboxes **merged into Running code** | One table, one box, three triggers. |

### 2.2 Storage

Three classes, not two. **This contradicts the owner's rule** ("userdata
strictly files, appdata rebuildable") and is question 1.

| Class | What | Rebuildable | Example |
| --- | --- | --- | --- |
| Userdata (files) | Items, collections, sidebars, per-person state, attachments | No: it is the truth | `Chats/Lisbon crew/2026-11.md`, `.people/<uuid>/calternal-chat.md` |
| Host state | History and stamps, each file's kind and ID, receipts, grants, consent, approvals, picks, sign-in, the work table, the audit trail | No | "Lee agreed that Claude may read Lee's messages in Lisbon crew" |
| Appdata | Index, search, link table, caches, live window state | Yes, from userdata plus host state | Unread counts |

## 3. What plugins can and cannot do

### 3.1 A plugin can

| Can | Example |
| --- | --- |
| Declare kinds with typed fields, and say how each looks as a row, card, cell, page and record | Reading List's `reading` kind |
| Create, replace and set fields on files of kinds it may write; append records; edit or redact records under the kind's policy (`author`, `author-or-manager`, `writers`); read history and revert one change | Notes undoes a change |
| Keep small private state per person (`state.get` and `state.set`) | Chat's read positions, drafts and mutes |
| Ask questions over anything it may read, and ship collections with big and small views, plus a default sidebar | Notes' "Linked here"; Chat's Pinned strip |
| Share files with people who share a group with the acting person | A new group chat |
| Ask authors, through the host's prompt, to let an agent read their writing | Calling Claude into a chat |
| Run on a call, at a time or on a change; run its own long programs (or the one the person picked) on a machine whose output lands in a file private to the actor | The Claude harness |
| Publish operations, and call the other plugins' operations that its grant names | The composer calls `calternal-chat/send` |
| Define slots and fill other plugins' slots | A community composer |
| Reach declared hosts with secrets it cannot read; send notifications | A mail plugin fetching from its server |
| Declare operations with no code (`create` from fields, `set` a field) | Reading List's Add and Mark read |
| Show a sealed frame for what the catalogue cannot express (which makes the plugin outbound-capable) | Mail's HTML body |

### 3.2 A plugin cannot

| Cannot | Example |
| --- | --- |
| Read or write outside its grant, even when another plugin asks it to | The composer cannot read chats |
| Choose a slot's target: the host fills it (the open chat for a composer); name any other target it could not reach and was not handed by the host (designation rule, question 2) | A composer cannot post into a chat the person left |
| Forge who wrote something | Nobody can post "as Alice" |
| Edit or delete another author's record, unless the kind's policy and the person's role allow it | Chat cannot let Sam edit Lee's message |
| Read another person's private state, or see people outside a shared group | Chat cannot see who muted a chat |
| Give an agent, or any outbound-capable code, writing whose author has not agreed | The harness gets "withheld" markers |
| Run code in a client, except in a sealed frame | No JavaScript in the composer |
| Draw outside the catalogue, or draw or hide a host screen | Sharing always shows the host's challenge |
| Keep running after Stop, or exceed its storage, compute, call, wake-up and egress quotas | A looping trigger is cut off |
| Reach any network address except its declared hosts; reach private addresses at all | The harness only reaches its model provider |
| Start a process or load native code itself; a program is a host-started work unit with mounts and a deadline | A plugin cannot spawn a shell |
| Register an operation or kind name another plugin owns | A second `chats` kind fails at install |
| Change a file's kind or ID, by any write path | Editing `kind: notes` into a chat file over the share has no effect |
| Send input to another person's run, or start another plugin's program | Lee cannot steer Ana's agent; Chat starts only the harness the person picked |
| Escape consent by starting a program, calling an outbound-capable plugin, or showing a frame | A "summary" part with a frame is filtered like an agent |
| Rewrite the words of links in files the renamer cannot write | Renaming a note never changes text in Lee's messages |

## 4. Security

One threat table for the whole design. The per-primitive tables in the
sections give the "checked at" column in detail.

| # | Threat | What contains it | Primitive |
| --- | --- | --- | --- |
| T1 | A plugin reads or writes beyond its grant (path traversal, another kind, an unlisted call) | No ambient authority in the box; every effect is a call checked against the grant, the person's rights and the target, inside the commit; paths resolved beneath the space root | Authority, Files |
| T2 | Confused deputy: plugin A gets B to act on a target A could not reach | Question 2. Recommended: the designation rule; the host fills slot targets itself; handles only from typed returns and run inputs (run-scoped, ending at Stop); strict intersection is the alternative | Authority |
| T3 | Forged author or rewritten history, including by replace-writes, share edits, header removal and revert | The host stamps writes in host state; header lines are readable copies; every byte change to a record kind is checked against the kind policy per record; history keeps every change | Files |
| T3b | Changing a file's kind or ID to escape its policy or take over another file's shares | Kind and ID are host-state facts fixed at create; a mismatching frontmatter copy loses | Files |
| T4 | Agents receive others' writing without consent (through a run, a program started by another plugin, a frame, or a plain file several people wrote) | Outbound-capable is the install-time closure over network, secrets, programs, starting programs, calls to outbound-capable operations, and frames; every read by an outbound-capable plugin withholds writing whose authors have not consented; for a non-record file every person in its revision stamps must consent; consent is host-held and set only from a host prompt | Authority |
| T5 | Laundering through files, titles, names or collections across calls | Taint per worker process: the other authors returned to a process are stamped as `carries` on every write it makes, and tainted rows are withheld from outbound-capable reads | Authority |
| T6 | Learning hidden rows from counts, ranks, backlinks or the change feed | Queries run as the asker; rows filtered before counting and ranking | Questions |
| T7 | A run keeps acting after Stop; a duplicate Stop hits a newer run; output written after Stop is posted by a trigger | Fence at the Stop commit; Stop lives in that run's inbox; replies go only through the run's fenced calls with run-scoped handles, never through a trigger on the log | Running code |
| T7b | Another person steers someone's agent (and so their login and local files) | Only the run's actor may send input; others may only Stop | Running code |
| T7c | Everyone in the space reads a run's log, with consented messages and the actor's files | Run files live under `.people/<actor>/runs/` and are indexed only for the actor | Running code, Files |
| T7d | A machine's owner forges calls in a run's log | Question 7: shared-space runs on hosted sandboxes; if a member's machine is allowed, run rights are intersected with the machine owner's rights and consent prompts name the machine and harness | Running code |
| T8 | Replayed or injected run output; a machine claims another's run | Sequence numbers with compare-and-set append; machine key; placement column; a new host epoch after restore ends every run in flight | Running code |
| T9 | A program outlives a dead server or a lost machine | Machine-side deadline and 24-hour rule; host mirror marks the run ended | Running code |
| T10 | Flood: alarms, watches, calls, wake-ups, change loops, disk | Quotas per space and plugin; coalescing; own writes do not trigger own watches; depth limit; record and segment caps | Running code, Files |
| T11 | Exfiltration over the network | Declared destinations only; private addresses refused at resolve, connect and redirect | Authority |
| T12 | A plugin obtains an equivalent credential through a secret (mints a token, writes to its own tenant) | Restated guarantee: plugin code never holds the secret value, and may not set other auth headers on a secret's destination; multi-tenant destinations are flagged at install (question 35). Echo filtering is a cheap extra, not a guarantee | Authority |
| T13 | Login theft, including by an updated entry script inside an approved image | Login folders mounted only into runs of their owner; approval binds the plugin digest and the image digest, and any change re-approves before hosted placement or login mounts | Running code |
| T14 | Spoofed host screen, clickjacking, phishing through plugin forms | Host screens in the shell's own layer with the host mark; frames clipped, opaque and below the host layer; every plugin form has a non-removable header naming the plugin; no password-like field | Drawing |
| T15 | A replacement part records what the person types, or posts somewhere else | Picks belong to the person (or the space, for others-facing slots); the pick screen says what the part sees; the host fills the slot's target, and the part supplies only content | Drawing, Authority |
| T16 | Lock-out by a broken shell part | Profile, Settings, Plugins and safe mode are host-only and always reachable | Drawing |
| T17 | A sealed frame exfiltrates what it shows | Network isolation does not hold in browsers, so a plugin that declares a frame is outbound-capable and its reads are consent-filtered; the promise is "a frame can read what its plugin can read" (question 9) | Drawing |
| T18 | A hostile member deletes or rewrites others' records | Author-owned records; moderation is question 15 | Files, Authority |
| T19 | A stolen session or modified client approves risky things | Question 16 (passkey for the riskiest confirmations); revoking a session also revokes the consents and picks it made | Authority |
| T20 | A malicious update | Approval binds plugin and image digests; on any update the host recomputes every plugin's derived install sentences (including "can read X through B") and any that grow wait for approval; consent records the harness digest | Authority |
| T20b | A restore from backup undoes revocations | A restore starts a new epoch: sessions invalidated, the handle key rotated, standing consents suspended until reconfirmed | Authority |
| T20c | A plugin floods the audit trail to evict records of its earlier reads | Audit retention per plugin; marked reads of other people's writing keep their full retention | Authority |
| T20d | Rank, link resolution or version counters reveal hidden files | Per-file revision counters; push only when the viewer's visible result changes; ranking without corpus statistics, or the leak documented (question 36) | Questions |
| T21 | One space starves another, or work spread over many spaces starves the host | Per-space queues and writers; no installation-wide lock on the request path; buckets also charged to the account, with an operator ceiling and fair share (question 37) | Running code, Files |

### 4.1 Where the sections are not sure the design holds

Kept together for the security reviewer, as found:

| From | Doubt |
| --- | --- |
| Authority G1 | A warm plugin process that served Ann can answer Bob from memory (question 5). |
| Authority G2 | Laundering across calls: replaced by taint per worker process (T5); the remaining channel is a link the person clicks. |
| Authority G3 | Plugin-built indexes leak existence through rank and result counts. |
| Authority G4 | Links can carry data in their URL when a person clicks. |
| Authority G5 | Copies into shared folders are not challenged. |
| Authority G6, Files 5, Drawing 3 | Timing is not addressed: a query over many unreadable rows is slower. |
| Authority G7 | Device copies survive removal from a space until the device next signs in. |
| Authority G8, Files 2 | Deleted bytes: history must be scrubbed with the tombstone; backups, offline devices and modified clients keep them. |
| Authority G9 | Consent does not name the machine: closed, consent prompts name the machine and the harness (T7d). |
| Authority G10, Running code 19 | Delete for everyone cannot reach an agent's log or a model provider that already received the message. |
| Running code 12 | A machine's owner can read the logs of runs other members started on it (question 7). |
| Running code 13 | An unapproved harness on the caller's own machine can send everything it was given to its author (by design). |
| Running code 14 | Secret echo detection misses encoded or chunked echoes: it is now only a cheap filter, and the guarantee is restated (T12, question 35). |
| Running code | The change-chain depth limit of 4 and the 50 ms machine batching are chosen, not measured; V8 isolates in one process are probably not a security boundary. |
| Drawing 1 | Frame network isolation fails in browsers (WebRTC) and is unproven in `WKWebView`. |
| Drawing 2 | Deleting activation contexts relies on the index refusing fields a guest may not see; recheck title-only guests. |
| Drawing 4 | Phishing inside catalogue components (a form asking for a password) is reduced, not removed. |
| Drawing 5 | The composer slot must give the open item as a fixed input: resolved, the host fills slot targets (F10). |
| Files 1 | Direct disk edits are stamped `outside`; the owner's own server-side edits become withheld from agents. |
| Files 3, 4 | A rename rewrites links in files the renamer cannot write, and reveals the new name (question 18). |
| Files 6 | Two plugins declaring the same kind name must be refused at install. |
| Prior art | Every host method (index, change feed, export) must apply the same check; slot picks must not let a plugin steal a role; grant and deadline must travel with an offline run. |

## 5. Three worked apps

Code sizes are from the authoring section's paper prototypes; interface
line counts are from the drawing section.

### 5.1 Reading List: no code

| Part | How | Primitive |
| --- | --- | --- |
| Kind | `reading`: frontmatter `url`, `title`, `read`. Row: title and site; card: title, site, image. | Files, Drawing |
| Operations | `add` and `mark-read` are declarative (`create`, `set`): no worker, no box. | Authority |
| Collections | "Unread" (`read = false`, newest first) and "All", shown small in the sidebar. | Questions |
| Size | 11 lines (today 226); 7 interface lines (today 285 JSON lines). | |

### 5.2 Notes

| Part | How | Primitive |
| --- | --- | --- |
| Kind | `notes`: Markdown files; `tags` and `snooze_until` in frontmatter. | Files |
| Editing | The catalogue's editor bound to the file; a write with an expected revision. | Files, Drawing |
| Links | Ordinary relative links. A rename commits at once (old paths resolve through an alias); the host rewrites referring link destinations in the background from the link index, and never rewrites link text in files the renamer cannot write. | Files, Questions |
| Tags | A facet. "Linked here" is a backlinks query. | Questions |
| Snooze | A work unit due at `snooze_until` that clears the field. | Running code |
| History | `history` and `revert` of one change. | Files |
| Size | 36 lines plus 11 interface lines (today a 67-line package plus the native engine). | |

### 5.3 Chat (several plugins)

| Part | Plugin | How |
| --- | --- | --- |
| The chat file | `calternal-chat` | A record kind in `Chats/<name>/`, segmented by month (question 20). Each message is a record with a host-written header (`**Lee** · 2026-11-20 17:02 ^anchor`); the stamp lives in host state. Edit rewrites the record in place (old versions in history, shown to the author); delete for everyone redacts it (`*deleted*`), scrubbing history and search. |
| Unread, read position, draft, mute | `calternal-chat` | `state.set` per person, buffered to one write per 5 s; the index answers "records after my position". |
| Sidebar | `calternal-chat` | Default arrangement: Pinned (strip), Groups, People, Cals, Harnesses, Archived; each section a collection. Pins are a personal collection in the person's order. |
| Quotes | `calternal-chat` | A link to the record's anchor, drawn with the reader's rights; a deleted record shows nothing wherever it is quoted. |
| Composer | `calternal-chat-composer` (depends on `calternal-chat`) | Fills the composer slot, picked per person. The part supplies only the text; the host fills the target (the open chat) when it calls `send`. 19 lines for a community composer. |
| People and consent | `calternal-chat` | Participants are a share; calling an agent asks consenting authors through the host's prompt; one minute without an answer withholds that person's messages (owner decided). |
| Agents | `calternal-chat-ai` and `calternal-chat-claude` (a harness) | Both are outbound-capable (the AI part may start programs), so all their reads are consent-filtered. The host resolves the program from the person's pick and makes a work unit on a hosted sandbox or the actor's machine. The run's input is references, with run-scoped handles for the chat; its files live in `.people/<actor>/runs/`. The program reads its inbox and appends to its log. The reply is a fenced call from the run (`chat.reply` through its handle), stamped with the run; nothing it writes after Stop takes effect. Only the actor may send it more input. |
| Restart | host | The machine keeps writing its log; when the server returns, sync sends what the other side lacks; a message sent meanwhile is in the inbox exactly once; Stop is in that run's inbox. |
| Notifications | `calternal-chat-notify` | Watches changes and calls `notify`, picked per space; a push goes only to recipients who may read the item, and mutes are applied by the host per recipient (question 31). |
| Size | Chat core 104 lines (today 1,884 plus a 1,317-line test host); 19 interface lines. | |

### 5.4 What a plain author learns

About fifteen concepts: **space, person, file, record, kind, field, look,
collection, query, operation, call, grant, work, slot (part), state.**
Revision 14 needed about sixty for a Chat-class plugin.

## 6. Was and is

[primitives/was-and-is.md](primitives/was-and-is.md) has the whole
inventory. The host on `dev` is about 160,000 lines of Rust, and the audit
on #78 found about 17,900 of them app-specific. On top of that, revision 14
is about 17,200 lines of contract with 312 numbers. The file also gives the
fate of every revision-14 operation, host method, route, rule block,
manifest field and limit group, and what survives of each crate and lane
branch.

| Area | Was | Is |
| --- | --- | --- |
| Storage | Files plus per-item engines, spans, viewer records, layout tables | Files with four writes; host state for stamps and grants |
| Lists | Binds, sources, extractors, search profiles, layout sections | One query shape, three list sources |
| Authority | Six audiences, activation contexts, leases, five write checks | One call table, one check, consent per author |
| Execution | Jobs, intents, attempts, successors, controls, spool, relay, ACKs | One work table, a box, two files per run |
| Interface | 87 description shapes, layout caps, entry views, request cards | Looks, views, slots, catalogue, frame |
| App code in the host | Native chat, Notes, providers, automations | None |

## 7. Build plan

From [was-and-is.md §5](primitives/was-and-is.md#5-build-plan), adjusted to
the five primitives. Each step ends with a test on the running server.

| Step | Goal | Test that proves it | Estimate |
| --- | --- | --- | --- |
| 1 | **One plugin does one real thing**: install Reading List through the one validator; call `add` over the one call route; the file appears; a query returns it. | End-to-end test with the real server binary: install, call, read the file on disk, get the row from the query. | 2–3 days |
| 2 | Questions: index rebuilt from files and host state; collections as files; live windows; facets | Edit a file by hand; the window updates. Delete the index; it rebuilds to the same answers. | 3–4 days |
| 3 | Authority: grants, sharing, one check, audit | A second person and a guest see only what they are granted, in queries, counts and the change feed; an ungranted call is refused. | 3 days |
| 4 | Running code: plugin box, work table, three triggers | Notes snooze fires once across a restart; Stop ends a worker. | 3–4 days |
| 5 | Files: records, stamps, edit, redact, segments | A person and a plugin append; forged headers are escaped; redact removes the words from history and search. | 2–3 days |
| 6 | Programs on machines: inbox and log | Kill the server during a harness run; output arrives once and in order; a message sent meanwhile arrives once; Stop reaches the exact run. | 4–5 days |
| 7 | Consent and marked chains | A run sees only consented authors' records; a revoke fences further reads; the one-minute default withholds. | 2 days |
| 8 | Plugin calls plugin, slots and picks, dependencies | The composer posts only into the open chat; uninstalling a picked part falls back with a notice; disabling official Chat warns. | 2–3 days |
| 9 | Drawing: catalogue, looks, views, shell slots, records view | Reading List, Notes and Chat draw on web and native from declarations only. | 1–2 weeks (interface lanes) |
| 10 | Delete the native engines and v0 models | The workspace builds without them; the three apps pass end to end. | 2 days |

After step 1, steps 2–8 can run two or three at a time, since each adds one
part behind the same call route.

## 8. Contradictions between the sections, and how they are resolved

| # | Contradiction | Resolution |
| --- | --- | --- |
| C1 | Number of primitives: authority six; running code and prior art fold the outside into calls; the thesis seven; my draft twelve | **Five** (§2.1). The merges follow the sections' own arguments; no owner decision fixes the count. |
| C2 | A third storage class (files, authority and running code all found one) against "userdata strictly files, appdata rebuildable" | **Question 1** (first rank); the design assumes option (a) until answered. |
| C3 | Plugin calls plugin: authority recommends delegated operation with the designation rule; the thesis and prior art (R1) recommend strict intersection | **Question 2.** The owner's words "may call another plugin's operations" fit both. |
| C4 | Lists: the thesis says collections supply every list; drawing says three sources | **Resolved for drawing's three sources** (collection, record file, facet): all three are the same query engine (files section), so nothing new is learned. |
| C5 | A person's sidebar edits: files says a list of changes over the default; drawing says a private copy that still picks up new default sections | **Question 13.** Both keep the sidebar a visible file; they differ on updates. |
| C6 | Record edits: my first draft appended versions; the files section edits in place | **Resolved for in place.** The owner decided a deleted message vanishes with no trace and that the product is an interface over readable files; appended versions keep deleted words in the file. |
| C7 | The sealed frame cannot be cut off the network in browsers | **Question 9**; a plugin that declares a frame is outbound-capable, and the promise is "a frame can read what its plugin can read" (security review F1). |
| C8 | What counts as an "agent": authority says anything outbound-capable; authoring says a declared `agent: true` flag | **Question 3** (with the scope of protected files). Recommended: outbound-capable, so no flag can be skipped. |
| C9 | Agent replies: the files section has the run append to the chat; authoring has the harness plugin's trigger post it | **Resolved by the security review (F9):** neither as written; the run replies through its fenced calls with run-scoped handles (question 12). |
| C10 | Missed alarms: "the server never decides" (third set) against "a grace period, with one default" (evening) | **Resolved:** the host delivers once late with both times; grace is a plugin setting with an SDK default (running-code Q7). Both records hold together. |
| C11 | Per-person state folder: hidden `.people/` (files) against "a visible file" for sidebars (owner) | **Resolved:** `.people/<uuid>/` is shown to its owner in Files as "My sidebars and settings"; hidden from everyone else. |

## 9. Questions for the owner

All questions from every section, de-duplicated, most consequential first.
Each stands on its own. Where several sections asked the same question, the
sources are named.

### 1. Is host-held state an exception to "userdata is strictly files"?

*Sources:* files Q1, authority Q3, running code Q5.
**What:** some things can neither be rebuilt nor safely be files: who wrote
each message (the stamp), who may see a file, which plugins are approved,
who agreed that an agent may read their messages, which part a person
picked, sign-in, and the list of running and scheduled work. A file can be
changed by a file share, a text editor or a plugin.
**Why now:** it decides where the permission check reads from, the file
format of every append-style app, and what a backup restore must bring
back together.
**Example:** a plugin with write access to `Settings/` edits `consent.md` to
say Lee agreed; or Lee edits a chat file over the share and changes Sam's
name on a message to their own.
**Options:**
- (a) **A host state class beside the files** (as `auth.db`, history and
  sharing already are today), backed up with userdata, with a read-only
  Markdown copy the host writes for people to read. Safe and fast, and an
  append stays at two flushes. Cost: a third storage class that is neither
  a file nor rebuildable; a restore must restore both together.
- (b) **Files only the host may write**, refused for every other writer.
  One rule. Cost: sync, shares and backups must special-case them; a restore
  from an old backup can bring back revoked consent.
- (c) **Signed markers inside the files.** Files are the whole truth. Cost:
  noise in every file; any outside edit breaks the signature.

**Recommendation:** (a). History and sharing are already stored this way
(ADR 0062: "durable state, not rebuildable").

### 2. When one plugin calls another, whose rights apply?

*Sources:* authority Q1, prior art R1 and Q5, the thesis.
**What:** a composer you installed asks Chat to post your message. Either
the composer must itself be allowed to write chat files ("strict"), or it
may only ask Chat to post, and Chat writes with its own rights, for you,
after the host checks that you may post there ("delegated").
**Why now:** every split app (Chat's parts, Mail saving to Notes) and every
install screen depends on it.
**Example:** under strict, a community composer shows "Can change your
chats" and could rewrite any message directly. Under delegated, it shows
"Can send messages through Chat" and can post only where you are, into the
chat it was shown.
**Options:**
- (a) **Strict intersection** (thesis, prior art): the callee does only what
  the caller, the callee and the person may all do. No deputy problem.
  Cost: the less trusted plugin gets more raw power; grants grow to
  everything a plugin touches through others.
- (b) **Delegated operation with the designation rule** (authority): the
  caller needs the right to call that operation; you need the rights it
  uses; the callee acts under its own grant; anything the caller names must
  be something it could reach or was handed by the host. Cost: one more
  rule, one token check per named item; a callee that builds paths from
  plain strings is still a buggy deputy, which the SDK and validator
  discourage.

In both, the caller's manifest lists every operation it calls (prior art
Q5), so you see at install which plugins talk to each other. Under (b) the
host fills slot targets itself, handles exist only for typed returns and
run inputs (run-scoped, ending at Stop), and results are typed as
references plus plain values so the host can check what a callee reveals
(security review).
**Recommendation:** (b). It gives less power to the less trusted plugin, and
the host checks the one rule in one place.

### 3. Which writing does the agent-consent rule protect, and what counts as an agent?

*Sources:* authority Q2, authoring Q5.
**What:** the host cannot tell an agent from other code. Either plugins
declare themselves agents, or anything that can send data off the server
(a running program, or a plugin with network, secrets or programs) counts.
The rule must also say which files it protects.
**Why now:** it decides whether agents can search a team's notes, and
whether link previews or translators work in chats without consent.
**Example:** in your Family space you ask Claude "find the notes about the
lease"; one of them was written by your partner.
**Options for "agent":**
- (i) A declared `agent: true` flag. Simple. Cost: a plugin can leave it
  off unless the installer forces it.
- (ii) **Anything outbound-capable.** Airtight. Cost: link-preview and
  translation plugins also need consent in chats.

**Options for scope:**
- (a) Chats only (kinds opt in). Cost: protection depends on each plugin.
- (b) Every file another person wrote in. Cost: agents in group spaces see
  almost nothing without many prompts.
- (c) **(b), except in spaces whose owner turned on "members' agents may
  read members' writing here"**, shown to everyone when they join. Chats
  between people and files shared one by one stay protected.

**Recommendation:** (ii) with (c). "Outbound-capable" is the install-time
closure over network, secrets, programs, permission to start any program,
calls to any outbound-capable operation, and frames (security review F1,
F2). The space switch applies only to writing made after each member has
seen it.

### 4. May any plugin ever run outside the sandbox?

*Source:* prior art Q1.
**What:** some systems run plugins with the full rights of the program;
others run them in a box that has only what it was handed.
**Why now:** one trusted tier puts an exception into every security rule.
**Example:** the Claude harness: a box with mounts, or trusted code on the
server?
**Options:** (a) **every plugin, official or not, always in the sandbox**.
Cost: some things need a host method written first. (b) A trusted tier
for official plugins. Cost: a second security story; a replaced official
plugin gets full power. (c) A trusted switch for the operator only, for
debugging. Cost: it can be left on.
**Recommendation:** (a). It is your rule that first-party plugins are not
special.

### 5. May a plugin remember one person's data and show it to another?

*Sources:* authority Q4; running code Q6 (process sharing, waiting for #103).
**What:** a plugin's process serves everyone in a space. The call check
limits what it fetches per call, not what it remembers.
**Why now:** it decides the memory cost of every plugin.
**Example:** a summary plugin read Lee's private note while serving Lee;
asked by Ann, it repeats a line from it.
**Options:** (a) trust installed plugins (revision 14's rule): no cost.
(b) One process per person for plugins that read others' kinds: airtight,
memory grows with active people. (c) **(a) for plugins with no network,
secrets or programs; (b) for outbound-capable ones.**
**Recommendation:** the security review (F15) says (b) for **every** plugin
that reads kinds it does not own, not only outbound-capable ones, because
an unmarked summary plugin can still answer Ann with Lee's text. That is
the safe answer; its cost is memory, to be confirmed against the #103
numbers. Until then, interface-only plugins run no process and others start
on first call and stop when idle; no shared runtime per publisher.

### 6. Who approves plugins, and how are updates and publishers handled?

*Sources:* authority Q6, prior art Q2 and Q3.
**What:** who may install a plugin, whether an update with the same rights
installs by itself, and where plugins may come from.
**Why now:** it is the line between a bad member and a bad plugin, and
between a private tool and a marketplace.
**Example:** a community composer updates itself. Asking for a new network
destination is blocked until you approve; but with the same rights it could
start garbling messages.
**Options:**
- Installs: (a) **the space owner approves installs and every wider
  grant**, and managers lose `plugins.manage`; (b) managers may install as
  today.
- Updates: (a) same-grant updates apply automatically for signed
  publishers; (b) every new version needs approval; (c) **(a) for
  publishers you marked trusted, (b) for the rest.**
- Sources: (a) **operator-pinned list only for now**, designed so signed
  publishers with review can follow; (b) any signed publisher; (c) anyone
  with an account.

**Recommendation:** owner approves; sources (a). For updates, the security
review narrows (c): every new digest of a plugin that is outbound-capable
or fills a slot needs approval; others may update by themselves within the
same grant. Approval binds the plugin digest and the image digest (F11),
and the host recomputes every plugin's derived install sentences on any
update (F12).

### 7. Whose machine runs an agent in a shared space, and who may read the run's log?

*Source:* running code Q2.
**What:** a machine sees everything its programs see, including other
members' consented messages.
**Why now:** it decides whether the space owner can read runs that other
members started.
**Example:** Ana calls Claude into a chat with Lee in the family space; the
run uses the Mac that Kim (the owner) enrolled; Kim can read the log there.
**Options:** (a) the space's machine, with every consent screen saying "runs
on Kim's machine". (b) **Shared-space runs only on hosted sandboxes**, never
on a member's machine. Cost: hosted compute. (c) Always the caller's own
machine and login. Cost: reverses your decision on subscriptions.
**Recommendation:** (b), with (a) as an explicit space-owner setting shown
in every consent request. Under (a) the run's rights are intersected with
the machine owner's rights, because a machine can forge calls in the log
(security review F4), and consent prompts name the machine and the harness
and whether it is approved. Unapproved harnesses still run only on the
caller's own machine.

### 8. Does each person approve a plugin's use of their own data?

*Source:* prior art Q4.
**What:** the owner approves a plugin for the space; each person may also
approve its use of their own private state and writing.
**Why now:** it decides whether `.people/<me>/` and your writing are open
to any plugin the owner chose.
**Example:** the owner installs a summarizer; Lee never agreed to it reading
Lee's private notes.
**Options:** (a) **owner approval is a ceiling; each person approves on
first use**, drawn by the host. Cost: one prompt per person per plugin.
(b) Owner approval covers everyone.
**Recommendation:** (a): it is the agent rule applied to every plugin.

### 9. What does a sealed frame promise, given that browsers cannot cut it off the network?

*Source:* drawing Q8.
**What:** a frame runs a plugin's own web code. In browsers WebRTC escapes
both the content policy and the frame sandbox.
**Why now:** it decides what a frame may read and how Mail shows HTML mail.
**Example:** a community budget chart opens your "Taxes 2026" note in a
frame; a hostile version could send the numbers out through WebRTC.
**Options:** (a) **promise only "a frame can read what its plugin can
read"** (the earlier wording "what you open in it" overstated it): a
plugin that declares a frame counts as outbound-capable, so its reads are
consent-filtered, and the install screen says its pages may reach the
internet. (b) No frames on the web until browsers
close the hole. Cost: no HTML mail or agent pages in browsers. (c) Frames
for first-party plugins only. Cost: against "first-party is not special".
Remote images in Mail: never automatic; per message on your tap, through a
host proxy.
**Recommendation:** (a).

### 10. Is a timeline of records part of the host's catalogue?

*Source:* drawing Q6.
**What:** chats, comments, mail threads and agent logs need a timeline:
authors, your own messages on one side, day lines, an unread line, quotes,
paging from the bottom. A plain list cannot do it.
**Why now:** without it, Chat cannot be drawn natively.
**Example:** an agent's reply arrives while you are reading older messages;
the view keeps your place and shows "1 new".
**Options:** (a) **a generic records view** with no chat words (authors,
times, edits, tombstones, references), native everywhere. (b) A sealed frame
per plugin. Cost: web content in the iPhone app for every chat-like app.
**Recommendation:** (a).

### 11. How far does "delete for everyone" reach?

*Sources:* files Q4, running code Q4.
**What:** history keeps every version, and agent runs keep their own copies
of what they read.
**Why now:** without scrubbing, "restore the previous version" brings a
deleted message back.
**Example:** Sam deletes a message containing a password; Ana's agent run
had already read it.
**Options:**
- History: (a) **scrub the bytes from history and search**, keep "deleted
  by Sam at 17:05"; Undo only within a few seconds, before the redact is
  sent. (b) Keep it in history for the author only.
- Run files: (a) also blank it in run logs the host stores. (b) **Leave run
  logs as they are, and say in the delete dialog "agents that already read
  it keep their copy"**; run files live under `.people/<actor>/runs/`,
  readable and indexed only for the person who started the run, kept 30
  days.

**Recommendation:** history (a), run files (b). State plainly that backups
keep bytes until they rotate.

### 12. When an agent's reply is written, whose write is it? (resolved by the security review)

*Sources:* authoring Q2; files and running code differ (C9). **Resolved:**
neither option as written. The run replies through its own fenced calls,
using run-scoped handles for the chat that the host mints at `work.start`
and ends at Stop (F9). The options below are kept for the record.
**What:** something must append the agent's answer to the chat with a stamp
saying "Claude, for Lee".
**Why now:** it decides whether a sandboxed program holds write access to a
shared file.
**Example:** Lee calls Claude into a group chat; its reply appears under
Lee's message.
**Options:** (a) the run appends to the chat itself, with a grant on that
one chat for its lifetime. Exact stamp; a sandbox writes a shared file.
(b) **The run writes only its log; the harness plugin's change trigger posts
the reply**, and the host carries the cause so the stamp names the run. The
program touches nothing shared; triggers must carry causes.
**Recommendation (superseded):** (b).

### 13. Your changes to an app's sidebar: yours alone, and a copy or a list of changes?

*Sources:* files Q6, drawing Q2 (C5).
**What:** an app ships a default sidebar; you reorder, hide and rename its
sections. In a shared space this is either your own sidebar or the space's,
and it is stored either as a full copy or as a list of changes over the
default.
**Why now:** it decides where sidebar files live and what happens when the
app's default changes in an update.
**Example:** you hide "Harnesses" in the family space; later Chat adds a
"Bots" section.
**Options:** (a) **yours alone, as a copy** in `.people/<you>/` that still
picks up new default sections, appended at the end with a notice; "Share
sidebar" puts a copy into the space. (b) Yours alone, as a list of changes.
Cost: merging rules, and the file is harder to read. (c) The space's
sidebar, shared by everyone. Cost: someone else reorders yours.
**Recommendation:** (a): what only you see is yours, and the file stays a
plain readable list.

### 14. May a collection have its own fields, such as a status for each thing in it?

*Source:* drawing Q1.
**What:** a board over mails, notes and chats sorts them into To do, Doing
and Done, but none of those kinds has a status.
**Why now:** it decides whether mixed boards and tables work at all, and the
collection file format.
**Example:** you drag the flight confirmation mail to Done on the Lisbon
board. Where is "Done" written? Not into the mail, which may be someone
else's.
**Options:** (a) **collection fields** stored in the collection file beside
each member. Cost: a join in the index and a larger file. (b) Kind fields
only. Cost: no mixed boards.
**Recommendation:** (a).

### 15. Can a space manager delete other people's messages?

*Source:* authority Q5.
**What:** records are author-owned. A moderation right would let a role
delete anyone's.
**Why now:** it is a host rule; otherwise any plugin could do it.
**Example:** someone posts abuse in the family chat.
**Options:** (a) authors only; nobody can clean up. (b) **managers and the
owner may delete (not edit), shown as "deleted by a manager"**; a chat
between people outside any space role stays (a).
**Recommendation:** (b).

### 16. Should the riskiest confirmations need your passkey?

*Source:* authority Q7.
**What:** a stolen session or a modified app can press Allow itself.
**Why now:** it decides the host prompt's design.
**Example:** a stolen laptop session approves a plugin with "Any website"
and "Always" consent in every chat.
**Options:** (a) a tap is enough. (b) **A passkey for installing or widening
a plugin, sharing outside the space, "Always" consent, picking an
outbound-capable part and enabling frames**: a few Face ID prompts a month.
In both, revoking a session also revokes the consents and picks it made
(F17).
**Recommendation:** (b).

### 17. What does "exactly once" mean for a message to a running agent?

*Source:* running code Q1.
**What:** a message goes through your phone, the server and the machine; it
is stored once and never applied twice, and ends "arrived" or "did not
arrive".
**Why now:** it decides how much the machine must prove.
**Example:** the machine's helper crashes between writing "arrived" and
handing the message to Claude, while Claude keeps running.
**Options:** (a) **accept that window**, and mark "arrived" after the
program's own acknowledgement where it has one. (b) Require every program
to acknowledge every input. Cost: excludes some community harnesses.
**Recommendation:** (a).

### 18. When a rename rewrites links in files the renamer cannot edit, is that allowed?

*Source:* files Q3.
**What:** some files that link to the renamed one belong to other people.
**Why now:** it decides whether links always follow renames.
**Example:** Sam renames a shared note; Lee's private note links to it.
**Options:** (a) the host rewrites the link destination in every linking
file, recorded in history as "link updated after a rename". Cost: Lee's
file changes and shows the new name. (b) **Only files the renamer may write
are rewritten; the rest resolve through a permanent alias.** Cost: aliases
pile up.
In both, link text is never rewritten in files the renamer cannot write,
since a rename could otherwise put words into Lee's messages (F14).
**Recommendation:** (b), as the security review recommends.

### 19. Who is the author of bytes changed outside Calternal, and may agents read them?

*Source:* files Q5.
**What:** files can be changed over the file share (the person is known) or
directly on the server's disk (nobody is known).
**Why now:** it decides what an agent sees.
**Example:** the owner fixes a typo in a chat with Vim on the server.
**Options:** (a) **share edits stamped with the person; disk edits
`outside` and withheld from agents** until re-saved in Calternal. (b) Disk
edits stamped with the space owner. Cost: a backup or sync tool could
consent on the owner's behalf.
**Recommendation:** (a).

### 20. Does the host hide the segments of a long record file?

*Source:* authoring Q1.
**What:** a record file is capped (proposed 4 MiB) and rolls over monthly,
so a long chat is a folder of files.
**Why now:** every record app is written against one answer.
**Example:** Lee quotes a message from last year; the unread count spans
two months.
**Options:** (a) **the host presents a segmented kind as one logical file**
(append to the current segment; reads, positions, triggers and sharing span
all). (b) Plugins see segments. Cost: every record app re-implements
rollover.
**Recommendation:** (a).

### 21. Which limits stay fixed?

*Sources:* the coordinator, my first draft, drawing §1.11.
**What:** revision 14 has 312 numbers.
**Why now:** every primitive needs its bounds before code.
**Example:** "three marks on a row" is layout; "64 KiB per request" is a
resource; "a run lasts at most 7 days" is security.
**Options:** (a) **layout limits disappear; resource limits become operator
defaults; security limits stay fixed** (request size, run deadline maximum,
grant expiry maximum, frame size and depth). (b) Every number stays fixed in
the contract.
**Recommendation:** (a).

### 22. Do operations without code exist?

*Source:* authoring Q3.
**What:** `create` and `set` let an app declare "add a file of this kind" or
"set this field" with no worker.
**Why now:** it decides whether a small app needs a box at all.
**Example:** Reading List's Add and Mark read.
**Options:** (a) **yes, exactly two declarative forms**. Cost: the host
interprets a tiny template language. (b) Every operation is a handler.
**Recommendation:** (a).

### 23. How does an app call the part a person picked?

*Source:* authoring Q4.
**What:** a slot is a named place another plugin can fill, such as Chat's
harness.
**Why now:** agents, composers and notifications depend on it.
**Example:** Chat starts whichever harness Alice picked.
**Options:** (a) **a slot declares an operation signature, and a call to
the slot reaches the picked part** under question 2's rule; the host
resolves the pick and fills the slot's target, so a caller cannot name a
program or part the person did not pick (F13). (b) Slots are
drawing-only; behaviour goes through operations named by convention.
**Recommendation:** (a).

### 24. Can one thing appear in two sections of the same sidebar?

*Source:* drawing Q5.
**What:** a section is a saved question or a hand-made list; a chat dragged
into "Lisbon crew" still answers "People".
**Why now:** it decides whether a drag moves or copies.
**Example:** you drag your chat with Hana into "Lisbon crew".
**Options:** (a) **once per sidebar**: hand-placed wins, so it leaves
People. (b) Wherever it matches (Telegram's folders). (c) A per-sidebar
switch.
**Recommendation:** (a).

### 25. What shape does the canvas of windows have on a desktop?

*Source:* drawing Q3.
**What:** the middle card holds several windows; how they sit is open.
**Why now:** the mockup needs it next.
**Example:** you open a note, then a chat linked from it, then the Lisbon
collection.
**Options:** (a) **a row of columns that scrolls sideways**. (b) Free,
overlapping windows. Cost: hard on the keyboard and iPad. (c) At most two
side by side, the rest as tabs.
**Recommendation:** (a), mocked up beside (c).

### 26. Which interface state follows you to your other devices?

*Source:* drawing Q4.
**What:** some state is about content (sidebar order, a collection's
chosen view); some is about a screen (widths, open windows, the card's
size).
**Why now:** synced state is a file and costs writes.
**Example:** you open three windows on the Mac, then pick up the iPhone.
**Options:** (a) **content state synced, screen state on the device**. (b)
Everything synced.
**Recommendation:** (a).

### 27. Should automatic updates wait for hosted runs, and may the host reboot under them?

*Source:* running code Q3.
**What:** runs on people's machines survive any server update; hosted
sandboxes do not survive a reboot.
**Example:** a kernel update during a long hosted Codex run.
**Options:** (a) **updates never wait; reboots wait for idle up to a limit,
then mark runs interrupted with a resume offer.** (b) Snapshot and resume
hosted sandboxes. Cost: new, unmeasured work.
**Recommendation:** (a) now.

### 28. Default run deadline, and missed alarms

*Source:* running code Q7 (C10).
**What:** a run needs a default deadline (machines accept up to 7 days), and
a missed alarm needs a rule.
**Example:** a 07:00 daily summary when the server was down until 11:00.
**Options:** (a) **the host delivers the alarm once, late, with both times;
grace is the plugin's setting with an SDK default of one hour; the default
run deadline is 24 hours, the maximum 7 days.** (b) The host applies a grace
and drops late alarms.
**Recommendation:** (a).

### 29. A laptop that sleeps longer than 24 hours

*Source:* running code Q8.
**What:** a run stops after 24 hours without the server; a sleeping laptop
is also without the server.
**Example:** Ana closes the lid on Friday during a run and opens it on
Monday.
**Options:** (a) **count sleep**: the run stops at wake and Chat offers
resume. (b) Do not count sleep. Cost: a run can outlive 24 hours of wall
time.
**Recommendation:** (a).

### 30. Keep GenUI for agent-made interface?

*Source:* drawing Q7.
**What:** the old GenUI language lets a plugin ship a small program that
clients interpret.
**Why now:** keeping it means two description languages.
**Example:** an agent wants to show "pick three dates" in a reply.
**Options:** (a) **declarations only** (a form in the reply; anything richer
is a frame); delete GenUI. (b) Keep GenUI.
**Recommendation:** (a).

### 31. How does a notification plugin honour another person's mute?

*Source:* authoring §3 item 2.
**What:** mutes are per-person private state, which the plugin sending a
notification cannot read.
**Example:** Lee muted "Lisbon crew"; Sam's message triggers a push.
**Options:** (a) **the host drops a notification when the recipient's
state for that plugin says muted**: one host rule, nothing disclosed.
(b) Mutes go into a shared file. Cost: everyone sees who muted whom.
**Recommendation:** (a).

### 32. Obsidian-style `[[wiki links]]` in imported files

*Source:* files Q7.
**What:** Calternal writes only standard links (ADR 0024); imported vaults
use `[[Note]]`.
**Example:** importing a 3,000-note vault.
**Options:** (a) **index and rewrite both forms, write only standard
links**. (b) Convert on import. (c) Ignore wiki links.
**Recommendation:** (a).

### 33. Typing and "working" indicators

*Source:* authoring Q6.
**What:** short-lived state that must not be stored.
**Example:** "Lee is typing".
**Options:** (a) a per-file ephemeral channel with a size and rate bound.
(b) **Leave it out of the first version.**
**Recommendation:** (b), then (a) when Chat needs it.

### 34. Does a missing picked part fall back silently?

*Source:* my first draft.
**What:** when your picked composer is missing or broken, the host uses
another eligible part.
**Example:** a manager uninstalls the community composer you picked.
**Options:** (a) **fall back and show a small notice**, for parts only you
see. (b) **Show the error state**, for parts that affect others (AI,
notifications), so a silent switch never changes what others receive.
**Recommendation:** (a) for per-person slots, (b) for per-space slots.

### Questions added by the security review

These are numbered after the others; the order is revisited after the
simplicity review.

### 35. What may a plugin do with a secret, and how does mail work?

*Source:* security review F16 and check (i).
**What:** "a plugin can use a secret but never read it" is true for the
value, but a plugin can still call the service's "create token" endpoint
or send data into its own account on the same service. Mail over IMAP and
SMTP cannot work by the host adding an HTTPS header.
**Why now:** it decides what the install screen may promise, and whether a
mail plugin can exist.
**Example:** a plugin holds your GitHub token for `api.github.com`; it calls
the endpoint that mints a new token and keeps that one.
**Options:** (a) **restate the guarantee**: plugin code never holds the
value; it may not set other auth headers on a secret's destination;
multi-tenant destinations (GitHub, Google, Anthropic) are flagged at
install as "can act in your account there". Mail gets host methods for
IMAP and SMTP with the secret, behind the same rule. (b) Keep the stronger
claim and allow only single-purpose destinations. Cost: most useful APIs
are multi-tenant.
**Recommendation:** (a).

### 36. May search ranking or counters reveal that hidden files exist?

*Source:* security review F20.
**What:** full-text ranking uses statistics over the whole space, so hidden
documents shift the order of visible ones; link resolution and global
version counters can show that a hidden file exists or changed.
**Why now:** it fixes the index design in step 2.
**Example:** Ann searches "divorce" in the family space; the ranking of her
own results shifts because of Lee's private notes.
**Options:** (a) **rank without corpus-wide statistics, use per-file
revision counters, and push live updates only when the viewer's visible
result changes**: a little worse ranking. (b) Keep corpus ranking and
document the leak. Cost: an existence oracle for terms.
**Recommendation:** (a).

### 37. Who may create spaces and groups, and how is the host shared between accounts?

*Source:* security review F21 and its missing questions.
**What:** quotas are per space; one account with many spaces can multiply
them and starve the host.
**Why now:** it decides admission control before running code ships.
**Example:** an account creates 50 spaces, each running 8 hosted agents.
**Options:** (a) **charge every bucket to the account as well, with an
operator ceiling and fair share across accounts**; space and group creation
allowed for every account up to an operator limit. (b) Only the operator
creates spaces. Cost: people cannot start a group themselves.
**Recommendation:** (a).

### 38. Where do person-to-person chats live, and can space roles read them?

*Source:* security review, missing questions.
**What:** a chat between people is one file in the starter's space, shared
with the others (owner decided). Rights are `max(space role, shares)`, so a
manager or reader of that space could read it.
**Why now:** it decides whether a private chat is private.
**Example:** Ana starts a chat with Lee inside the family space; Kim, the
family space's owner, browses its Chats folder.
**Options:** (a) **person-to-person chats live in the starter's personal
space**, never in a group space, so only the participants hold rights.
(b) They may live in a group space, and the chat kind's files are excluded
from role-based access (shares only). Cost: a per-kind exception to the
rights function.
**Recommendation:** (a).

### 39. Which person does a webhook act as, and what bounds it?

*Source:* security review, missing questions.
**What:** a webhook arrives from outside with no signed-in person.
**Why now:** a call needs a person for the one check.
**Example:** a GitHub webhook for a repository a plugin watches.
**Options:** (a) **the person who set up the webhook**, with a signature
check against a secret held by the host, a per-webhook rate quota and
expiry when that person loses access. (b) A space-level "system" principal.
Cost: a principal with no person behind it, whose rights must be defined.
**Recommendation:** (a).

### 40. What can visitors through share links, or with title-only access, trigger?

*Source:* security review, missing questions.
**What:** a guest who opened a share link, or who sees only a title, may
press a button that calls plugin code.
**Why now:** it decides whether guests can make plugins work for them.
**Example:** a public link to a reading list; the visitor presses "Mark
read".
**Options:** (a) **guests may run only declarative operations on the shared
file, and no plugin code**; title-only viewers may trigger nothing. (b)
Guests run plugin code with guest rights. Cost: plugin code runs for
people outside the space, under a weak identity.
**Recommendation:** (a).

### 41. Is a run's access to the actor's own machine bounded?

*Source:* security review, missing questions (with F5).
**What:** on the actor's own machine a run may mount a working folder and
the person's login. Server rights do not bound what it does there.
**Why now:** it decides what "the run's rights" mean on a personal machine.
**Example:** Ana's agent runs with access to `~/work` on her laptop.
**Options:** (a) **only folders the actor picks for the run, read-only by
default, listed in the run's header**; the login folder only for the
picked harness. (b) The run sees what the harness asks for. Cost: no
boundary beyond the operating system.
**Recommendation:** (a).

## 11. Security review and what changed

The independent security review of `6c432d33`
([`docs/reviews/primitives-only-security-review-2026-11-21.md`](https://git.kayg.org/kayg/calternal-ai/src/branch/dev/docs/reviews/primitives-only-security-review-2026-11-21.md)
on `dev`) found 21 problems. Revision 2 handles them as follows. "Applied"
means the rule is now in §2–§5; "question" means it is a real choice for the
owner. The section files under `primitives/` are left as their lanes wrote
them; where they disagree, this document wins.

| # | Finding | Disposition |
| --- | --- | --- |
| F1 | A plugin with a frame leaks without counting as outbound-capable | Applied: declaring a frame makes a plugin outbound-capable (§2, T4, T17, question 9 restated) |
| F2 | Starting a program, or calling an outbound-capable plugin, escapes consent | Applied: outbound-capable is the install-time closure (§2, T4, question 3) |
| F3 | Run files readable by the whole space | Applied: `.people/<actor>/runs/`, indexed only for the actor (§2, T7c, Chat, question 11) |
| F4 | The machine owner can forge calls in a run's log | Question 7 sharpened: hosted sandboxes for shared spaces; otherwise run rights ∩ machine owner's rights, and prompts name the machine (T7d) |
| F5 | Anyone can steer someone's run | Applied: only the actor may send input (§2, T7b, can't-list) |
| F6 | Author-owned records bypassed by replace, share edits, header removal, revert | Applied: one policy check per record on every byte change; refused changes become conflict copies (§2, T3) |
| F7 | Kind or ID changed by editing frontmatter | Applied: kind and ID are host-state facts fixed at create (§2, §2.2, T3b) |
| F8 | Contributors to plain files unprotected | Applied: consent from every person in the file's revision stamps (T4) |
| F9 | Agent replies weaken designation, and post after Stop through a trigger | Applied: run-scoped handles minted at `work.start`, ending at Stop; replies only through fenced calls (T7, Chat, question 12 resolved) |
| F10 | A slot handle replayed into a chat the person left, or from an alarm | Applied: the host fills slot targets; slot handles removed (T2, T15, question 2) |
| F11 | Approval does not cover the entry code next to the image | Applied: approval binds plugin and image digests (T13, question 6) |
| F12 | Derived disclosure grows on another plugin's update | Applied: derived install sentences recomputed on every update; consent records the harness digest (T20, question 6) |
| F13 | A client names any installed program | Applied: a plugin starts only its own programs or the picked one; the host resolves the pick (§2, question 23) |
| F14 | Rename rewrites link text inside others' messages | Applied: never rewrite link text in files the renamer cannot write; question 18 now recommends (b) |
| F15 | Warm memory leaks across people even for unmarked plugins | Question 5 sharpened: the review recommends one process per person for every plugin that reads kinds it does not own |
| F16 | A secret lets a plugin mint equivalent credentials; mail cannot work by header injection | New question 35; T12 restated |
| F17 | A revoked session's consents and picks survive | Applied: revoking a session revokes them (T19, question 16) |
| F18 | A restore brings back revoked sessions and consents | Applied: a restore starts a new epoch (T20b) |
| F19 | Audit flood evicts records of earlier reads | Applied: retention per plugin; marked reads of others' writing keep full retention (T20c) |
| F20 | Ranking, link resolution and counters reveal hidden files | New question 36; T20d |
| F21 | Work spread over many spaces starves the host | New question 37; T21 |
| Checks (b), (d), (g) | Taint per call is not enough; phishing through catalogue forms | Applied: taint per worker process replaces taint per call (T5); plugin forms carry a non-removable plugin header and the catalogue has no password-like field (T14) |
| Missing questions | Person-to-person chats, webhooks, visitors, run access on the actor's machine | New questions 38–41. Who may send to a run, kind and ID edits, session revocation, restore, entry-code approval and audit retention are applied above instead of asked |

What the review called theatre or over-built (its §4):

| Item | Change |
| --- | --- |
| Echo scrubbing of secrets | Kept only as a cheap filter; it guarantees nothing (T12) |
| `returns` declarations on free-form results | Dropped as a guarantee: results are typed as references plus plain values, so the host can check them (question 2); the install screen no longer claims to state everything a plugin can learn |
| Handles for query-result rows | Dropped: designation already passes for targets the plugin can reach; slot handles are dropped too (F10). Handles remain only for typed returns and run inputs |
| Taint per call | Replaced by taint per worker process |
| "Declared phrases only" in pushes | Restated: a push goes only to recipients who may read the item; its title is plugin-controlled text |
| "A frame reads what you open in it" | Restated: "a frame can read what its plugin can read" (question 9) |

Order of work, as the review recommends: step 1 may start once F1, F2 and
F7 are in, which they now are. Settle F6 and F8 before step 5; F3, F4, F5,
F9, question 7 and question 12 before steps 6 and 7; F10, F12 and F13
before step 8; F1 before any frame ships in step 9.

## 10. What this document does not cover

Process-sharing measurements (#103), the shell mockups, the canvas in
detail, and migrating existing code beyond the fate table in
[was-and-is.md](primitives/was-and-is.md).
