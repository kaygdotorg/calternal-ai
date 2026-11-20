# A primitives-only host · integrated design · 2026-11-21

Status: **the integrated design for review (#107).** Nothing here is decided
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
| 1 | **Files** | Items are Markdown files in a space. Four writes: replace, set fields, append a record, edit or redact one record. Move, trash, restore and revert one change. History of every file. A kind is a schema over frontmatter (and, for record kinds, a record shape) that a plugin declares. | The host stamps every write with person, plugin and run, where no plugin, file share or editor can change the stamp. Paths stay inside the space. |
| 2 | **Questions** | One index (appdata, rebuildable) over fields, text, links, records and stamps. One query shape with live windows. Lists come from three sources: a **collection** (a file holding a saved query plus hand-added items), a **record file** (the records of one file), or a **facet** (the values of a field). Backlinks, search and the change feed are the same engine. | Every answer, count and rank is computed as the asker, over what the asker may read. A collection grants nothing. |
| 3 | **Authority** | People, groups, spaces, roles and sharing as the existing app settled them (ADR 0058–0060). A plugin's **grant** (kinds, paths, calls, network, secrets, programs, triggers). One **call table** from name to handler for host methods, the outside (network, secrets, notifications) and every plugin's published operations. Consent, dependencies, slots and picks, quotas and the audit trail. | One check per call, at the table: who is asking, through which plugins, on which targets. Agents (anything outbound-capable) read other people's writing only with those people's consent. |
| 4 | **Running code** | One per-space **work** table with a state column. Work runs in a **box** (plugin process, hosted sandbox, or a person's enrolled machine), started by a call, a time or a change. A program talks to the host through two append-only, single-writer files: the **inbox** (host writes) and the **log** (machine writes). | A box has no network, files or rights except its grant. Stop fences the run at once and reaches only that run. Nothing outlives its deadline or the 24-hour rule. |
| 5 | **Drawing** | Plugins declare; they never draw. Kinds declare looks (row, card, cell, page, record). Collections declare views (big: list, table, board, gallery, calendar; small: compact list, strip). The shell is named slots filled by parts. A closed catalogue plus a sealed frame. Host-only screens: sign-in, Settings and Plugins, challenge, consent, Stop, safe mode. | No plugin code runs in a client outside the frame. A plugin cannot spoof or hide a host screen. Drawn values go from the host to the client as the viewer, never to the plugin. |

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
| Host state | History and stamps, receipts, grants, consent, approvals, picks, sign-in, the work table, the audit trail | No | "Lee agreed that Claude may read Lee's messages in Lisbon crew" |
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
| Run on a call, at a time or on a change; run long programs on a machine whose output lands in a file | The Claude harness |
| Publish operations, and call the other plugins' operations that its grant names | The composer calls `calternal-chat/send` |
| Define slots and fill other plugins' slots | A community composer |
| Reach declared hosts with secrets it cannot read; send notifications | A mail plugin fetching from its server |
| Declare operations with no code (`create` from fields, `set` a field) | Reading List's Add and Mark read |
| Show a sealed frame for what the catalogue cannot express | Mail's HTML body |

### 3.2 A plugin cannot

| Cannot | Example |
| --- | --- |
| Read or write outside its grant, even when another plugin asks it to | The composer cannot read chats |
| Name a target it could not reach and was not handed (the designation rule, if question 2 goes that way) | A composer cannot post into a chat it was never shown |
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

## 4. Security

One threat table for the whole design. The per-primitive tables in the
sections give the "checked at" column in detail.

| # | Threat | What contains it | Primitive |
| --- | --- | --- | --- |
| T1 | A plugin reads or writes beyond its grant (path traversal, another kind, an unlisted call) | No ambient authority in the box; every effect is a call checked against the grant, the person's rights and the target, inside the commit; paths resolved beneath the space root | Authority, Files |
| T2 | Confused deputy: plugin A gets B to act on a target A could not reach | Question 2. Recommended: the designation rule plus host-issued handles; strict intersection is the alternative | Authority |
| T3 | Forged author or rewritten history | The host stamps writes in host state; header lines in files are readable copies; record policy; history keeps every change | Files |
| T4 | Agents receive others' writing without consent | Outbound-capable principals mark the call chain; every read in a marked chain withholds records whose author has not consented; consent is host-held and set only from a host prompt | Authority |
| T5 | Laundering: read in one call, send in another | A plugin with network, secrets or programs is marked for all its reads | Authority |
| T6 | Learning hidden rows from counts, ranks, backlinks or the change feed | Queries run as the asker; rows filtered before counting and ranking | Questions |
| T7 | A run keeps acting after Stop; a duplicate Stop hits a newer run | Fence at the Stop commit; Stop lives in that run's inbox | Running code |
| T8 | Replayed or injected run output; a machine claims another's run | Sequence numbers with compare-and-set append; machine key; placement column; a new host epoch after restore ends every run in flight | Running code |
| T9 | A program outlives a dead server or a lost machine | Machine-side deadline and 24-hour rule; host mirror marks the run ended | Running code |
| T10 | Flood: alarms, watches, calls, wake-ups, change loops, disk | Quotas per space and plugin; coalescing; own writes do not trigger own watches; depth limit; record and segment caps | Running code, Files |
| T11 | Exfiltration over the network | Declared destinations only; private addresses refused at resolve, connect and redirect | Authority |
| T12 | Secret theft by echo from a declared destination | Secret bound to one destination; responses containing it refused | Authority |
| T13 | Login theft | Login folders mounted only into runs of their owner, only for approved programs when hosted | Running code |
| T14 | Spoofed host screen or clickjacking | Host screens in the shell's own layer with the host mark; frames clipped, opaque and below the host layer | Drawing |
| T15 | A replacement part records what the person types | Picks belong to the person (or the space, for others-facing slots); the pick screen says what the part sees; the part may call only the slot's operations | Drawing, Authority |
| T16 | Lock-out by a broken shell part | Profile, Settings, Plugins and safe mode are host-only and always reachable | Drawing |
| T17 | A sealed frame exfiltrates what it shows | The frame reads only its own item and what its plugin's operations return; **network isolation does not hold in browsers** (question 9) | Drawing |
| T18 | A hostile member deletes or rewrites others' records | Author-owned records; moderation is question 15 | Files, Authority |
| T19 | A stolen session or modified client approves risky things | Question 16 (passkey for the riskiest confirmations) | Authority |
| T20 | A malicious update | Wider grants wait for approval; same-grant updates are question 6 | Authority |
| T21 | One space starves another | Per-space queues and writers; no installation-wide lock on the request path | Running code, Files |

### 4.1 Where the sections are not sure the design holds

Kept together for the security reviewer, as found:

| From | Doubt |
| --- | --- |
| Authority G1 | A warm plugin process that served Ann can answer Bob from memory (question 5). |
| Authority G2 | Two colluding plugins, an unmarked reader and a marked sender, can pass text through a file. |
| Authority G3 | Plugin-built indexes leak existence through rank and result counts. |
| Authority G4 | Links can carry data in their URL when a person clicks. |
| Authority G5 | Copies into shared folders are not challenged. |
| Authority G6, Files 5, Drawing 3 | Timing is not addressed: a query over many unreadable rows is slower. |
| Authority G7 | Device copies survive removal from a space until the device next signs in. |
| Authority G8, Files 2 | Deleted bytes: history must be scrubbed with the tombstone; backups, offline devices and modified clients keep them. |
| Authority G9 | Consent does not name the machine a run executes on. |
| Authority G10, Running code 19 | Delete for everyone cannot reach an agent's log or a model provider that already received the message. |
| Running code 12 | A machine's owner can read the logs of runs other members started on it (question 7). |
| Running code 13 | An unapproved harness on the caller's own machine can send everything it was given to its author (by design). |
| Running code 14 | Secret echo detection may miss encoded or chunked echoes. |
| Running code | The change-chain depth limit of 4 and the 50 ms machine batching are chosen, not measured; V8 isolates in one process are probably not a security boundary. |
| Drawing 1 | Frame network isolation fails in browsers (WebRTC) and is unproven in `WKWebView`. |
| Drawing 2 | Deleting activation contexts relies on the index refusing fields a guest may not see; recheck title-only guests. |
| Drawing 4 | Phishing inside catalogue components (a form asking for a password) is reduced, not removed. |
| Drawing 5 | The composer slot must give the open item as a fixed input, so a hostile composer cannot post elsewhere. |
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
| Links | Ordinary relative links. A rename commits at once (old paths resolve through an alias); the host rewrites referring links in the background from the link index. | Files, Questions |
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
| Composer | `calternal-chat-composer` (depends on `calternal-chat`) | Fills the composer slot, picked per person; the slot hands it the open chat; it calls `send`. 19 lines for a community composer. |
| People and consent | `calternal-chat` | Participants are a share; calling an agent asks consenting authors through the host's prompt; one minute without an answer withholds that person's messages (owner decided). |
| Agents | `calternal-chat-claude` (a harness) | `run.start` makes a work unit on a machine or hosted sandbox. The run's input is references; reads are consent-filtered. The program reads its inbox and appends to its log. The reply reaches the chat through the harness plugin's change trigger, stamped with the run (question 12). |
| Restart | host | The machine keeps writing its log; when the server returns, sync sends what the other side lacks; a message sent meanwhile is in the inbox exactly once; Stop is in that run's inbox. |
| Notifications | `calternal-chat-notify` | Watches changes and calls `notify`, picked per space; mutes are applied by the host per recipient (question 31). |
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
| C7 | The sealed frame cannot be cut off the network in browsers | **Question 9**; the design states the smaller promise (it reads what you open in it) until answered. |
| C8 | What counts as an "agent": authority says anything outbound-capable; authoring says a declared `agent: true` flag | **Question 3** (with the scope of protected files). Recommended: outbound-capable, so no flag can be skipped. |
| C9 | Agent replies: the files section has the run append to the chat; authoring has the harness plugin's trigger post it | **Question 12.** |
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
Q5), so you see at install which plugins talk to each other.
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

**Recommendation:** (ii) with (c).

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
**Recommendation:** (c), confirmed against the #103 memory numbers. Until
then, interface-only plugins run no process and others start on first
call and stop when idle; no shared runtime per publisher.

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

**Recommendation:** owner approves; updates (c); sources (a).

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
in every consent request. Unapproved harnesses still run only on the
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
**Options:** (a) **promise only "it reads what you open in it"**: the frame
gets its one item plus what its own plugin returns; the install screen says
its pages may reach the internet. (b) No frames on the web until browsers
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
  it keep their copy"**; run files are readable only by the person who
  started the run and kept 30 days.

**Recommendation:** history (a), run files (b). State plainly that backups
keep bytes until they rotate.

### 12. When an agent's reply is written, whose write is it?

*Sources:* authoring Q2; files and running code differ (C9).
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
**Recommendation:** (b).

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
a plugin, sharing outside the space, and "Always" consent**: a few Face ID
prompts a month.
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
**Options:** (a) **the host rewrites only the link destination**, recorded
in history as "link updated after a rename". Cost: Lee's file changes and
shows the new name. (b) Only files the renamer may write are rewritten; the
rest resolve through a permanent alias. Cost: aliases pile up.
**Recommendation:** (a).

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
the slot reaches the picked part** under question 2's rule. (b) Slots are
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

## 10. What this document does not cover

Process-sharing measurements (#103), the shell mockups, the canvas in
detail, and migrating existing code beyond the fate table in
[was-and-is.md](primitives/was-and-is.md).
