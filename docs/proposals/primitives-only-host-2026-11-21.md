# A primitives-only host · revision 4 · 2026-11-24

Status: **revision 4: the single statement of the design**, with the
owner's answers from rounds one to nineteen of `docs/morning-2026-11-21.md`
on `dev` (2026-11-21 to 2026-11-23) applied. Where the two disagree, the
morning report wins. What the owner has not decided is in §12 with the
reason. Revision 2 (`502ab2bc`) keeps the option analysis and both reviews;
the section files under [`primitives/`](primitives/) are historical where
this document differs.

Principle (owner, round 9): **harden the space, plugin and VM boundary
rather than make promises the server cannot enforce.** The plugin API is
versatile and powerful; the server is simple, generic, very secure and
opinionated (round 15).

## 0. Three ideas

| Idea | What it means | Built as |
| --- | --- | --- |
| **Files, with questions** | Everything a person keeps is a Markdown file in a space, with its history and a stamp of who wrote it. The server answers questions about the files: lists, search, backlinks, unread counts. | Files, Questions |
| **Calls** | Everything that changes is a call to a named operation, checked once: who is asking, through which plugin, on what. A call runs now, later, in the background or elsewhere (on a machine). People, plugins and agents use the same calls, through the app, the CLI, the API or MCP. | Authority, Running code |
| **Drawing** | Plugins never draw. They declare how their things look and which collections an app starts with; clients draw that with one catalogue of native components. A web view covers what the catalogue cannot draw. | Drawing |

The server knows nothing about chats, notes, mail, agents or providers;
every feature is a plugin built from the same parts a stranger's plugin
gets. Every app stores Markdown on disk, so a person keeps their data
forever with no vendor lock-in (round 15).

## 1. What the server was, and what it is

| Area | Was | Is | Example |
| --- | --- | --- | --- |
| Size | About 160,000 lines of Rust, 17,900 of them app-specific; a 17,200-line contract with about 60 concepts and 312 numbers | A file server with five parts and no app code | Chat's 1,884 lines and 1,317-line test host become about 100 lines |
| Storage | Files plus per-app engines, spans, a viewer record, layout tables | Markdown files; host state holds stamps and keyed hashes, never text | A chat is `Chats/Lisbon crew/chat.md` |
| Spaces | Personal and shared spaces of different kinds | One kind of space; the first one stays personal | Ana creates "Lisbon trip" and invites Lee from her group |
| Changing things | Per-app write paths | Reading is free inside a space; an item changes only through its owning plugin's operations, or directly when its kind is editable outside | Money's ledger changes only through Money |
| Lists | Binds, sources, extractors, search profiles, layout sections | One query form; a collection is a file; a sidebar is a file listing collections | "Unread" is `{ kind: reading, where: { read: false } }` |
| Running things | Jobs, intents, attempts, successors, a spool, a relay | One work table; a box; background jobs through one call | Photo thumbnails are one job over a list of items |
| Agents | A special engine with consent prompts | Plugins that work through the same operations, scoped by their starter | Claude asked in a chat reads the current chat session by default |
| Surfaces | A browser route per component, a separate CLI | One declaration exposed as a CLI subcommand, an API path and an MCP action | `calternal chat send` and the MCP action `calternal-chat.send` are the same call |
| Interface | 87 description shapes, layout caps, entry views | One catalogue of native components plus a web view | A diagram is a catalogue component drawn with Pele |
| App code in the server | Native chat, Notes, providers, automations | None | Notes is a plugin of about 30 lines |

The inventory and the fate of every revision-14 name are in
[was-and-is.md](primitives/was-and-is.md).

## 2. What a plugin can do, and cannot do

| A plugin can | Example |
| --- | --- |
| Read any file in its space that the acting person may read | A summary plugin reads the space's notes |
| Change its own kinds' items through its own operations | Notes edits a note through `calternal-notes` |
| Call other plugins' operations that its manifest lists, to change their items | Mail files an attachment through `calternal-files` |
| Declare kinds (with their looks, and whether they are editable outside or operations only) and the collections its app starts with | Chat declares `chats` as operations only |
| Create files and set fields with no code at all | Reading List's Add form and "Mark read" swipe |
| Keep small private state per person, and a cache the server may delete at any time | Chat's read positions; a mail index |
| Run on a call, at a time, on a repeating schedule or when files change, and start background jobs over many items | Photos makes thumbnails for 2,000 new pictures |
| Run its own program, or the one the person picked, in a hosted sandbox or on a machine a member offered | The Claude harness |
| Offer operations and events, which the server exposes on the CLI, the API and MCP | `calternal reading-list add <url>` |
| Fill slots in other apps, picked by the person (what only they see) or the space (what others receive) | A community composer in Chat |
| Reach listed destinations with credentials the server attaches | A mail plugin fetching mail |
| Draw with the catalogue, and with a web view where the catalogue has nothing | Mail's HTML body |
| Serve visitors of a share link with exactly the link's rights | A shared album lets friends download and upload |

| A plugin cannot | Example |
| --- | --- |
| Read anything the acting person may not, or anything outside its space | A plugin in "Lisbon trip" never sees Ana's personal space |
| Change another plugin's items except through that plugin's operations | A summary plugin cannot rewrite a chat message |
| Reach a destination, use a secret or start a program not listed at install | A notes plugin cannot call a new server after an update without the Owner's yes |
| Hold a secret's value | A mail plugin never sees the mail password |
| Forge who wrote something, or change a file's kind, ID or outside-editing switch | Nobody can post "as Alice" or make chats editable over WebDAV |
| Run code in a client, except in a web view | No JavaScript in a composer |
| Draw or hide a server screen, ask for a password, or skip the passkey confirmation of a risky action | Sharing outside the space always shows the server's sheet |
| Keep running after Stop, or exceed its quotas | A looping trigger is cut off |
| Cost anything while disabled | A disabled plugin has no process, no memory and no work on any path |
| Give a share-link visitor secrets, network or agents | A visitor's upload never starts an agent |

## 3. The rules a plugin follows

1. **Its data is files in the space**, Markdown wherever no other standard
   format exists. What it can rebuild is a cache the server may delete.
2. **Every call goes through the server, as the acting person.** There is no
   other way to reach files, people, the network or other plugins.
3. **It changes only its own kinds' items, through its own operations**;
   other plugins' items change through theirs.
4. **One process per plugin per space**, started on first call, stopped when
   idle; none while disabled, none for a plugin with no code.
5. **What it sends out is declared**: network destinations, secrets and the
   programs it starts are listed at install; an update that adds to the list
   waits for the Owner.
6. **What it offers and calls is declared**: its operations and events, and
   every other plugin's operation it calls.
7. **It names things by ID, not by path.**
8. **Its interface is described** with the catalogue; a web view only where
   the catalogue has nothing.
9. **Risky actions go through the server's confirmation**, with a passkey
   (installing or widening a plugin, sharing outside the space, picking a
   part that sends data out, installing web code; T7).

Authoring has to be very easy (round 15, A22): retries, rights checks and
credentials are handled by the server, and well-written skills shipped
with the SDK steer authors and agents to user-friendly, robust patterns.

## 4. The design

### 4.1 Spaces, people and agents

| | Personal space (the first one) | A space with members |
| --- | --- | --- |
| Members | Only its owner; a chat or file can still be **shared** with others (a share, not membership) | Anyone the Owner invites from their groups; roles reader, editor, manager, owner |
| Private items | Everything is its owner's | None: every member reads every file within their role |
| Agent access | Always available to the owner | A space setting, **on or off**, chosen at creation, told to every member when they join; joining is consent |
| An agent run's scope | Picked by the starter: default the current chat session, ceiling what the starter may read | The same rule |
| Who steers an agent | Only the owner, always | Anyone who may write in the chat |
| "Direct, full access" on the person's own machine | Allowed | Not allowed: sandboxed, or limited to picked folders |
| A run's log and files | The owner's | Readable by every member who can read its chat |
| Installs | The owner | The Owner; a member may offer a machine, accepted by the Owner or a Manager with a warning |

- **Spaces are one kind.** Anyone may create spaces, name them and invite
  people from their groups; the creator is the Owner. The first space takes
  no members.
- **A chat between people** lives in the starter's personal space and is
  shared with its participants. In it the caller of an agent decides what
  the agent reads; there are no consent prompts, and everyone in the chat
  sees the agent and what it was given.
- **A chat session** starts after one quiet hour; it is the default scope of
  an agent run, given automatically after a first-time ask.
- **Agents are reached through operations.** An agent gets no files directly
  by default; it works through operations over MCP, the API or the CLI. A
  folder of a kind that is editable outside may be handed to an agent.
- **A harness that reads a person's sign-in does not update itself:** its
  update waits for that person's yes. Other plugins update themselves; an
  update that adds a destination, secret or program waits for the Owner.

### 4.2 Files and records

- Items are Markdown files; non-Markdown originals stay untouched, with
  standard sidecars such as XMP where a convention exists and app metadata in
  `.calternal/` keyed by stable ID (inherited decisions,
  `docs/inheritance-audit.md`). A file's kind and ID are fixed at create in
  host state.
- **Host state keeps a keyed hash, offset and stamp per record, never the
  text** (D5); it is a database, also for speed (A1). The files are the only
  copy of anyone's words.
- **Each kind is either editable outside or operations only** (one switch,
  decided by the kind, shown at install, never weakened):

  | | Editable outside | Operations only |
  | --- | --- | --- |
  | Kinds | Notes, tasks, calendar, files, photos | Chat, money, mail |
  | In the WebDAV mount | Yes, editable | Absent |
  | A folder handed to an agent | Possible | Not possible |
  | On disk, in exports and backups | Markdown | Markdown |
  | After the plugin is removed | Editable as plain files | Readable; unchangeable until the plugin returns |

- **No outside change to an operations-only kind is accepted at all**, not
  even an appended record (A21); any such change is tampering, detected by
  the keyed hashes and flagged. A body changed on the server's own disk is
  the administrator's problem, restored only from a backup.
- Writes over CalternalDAV or WebDAV to editable kinds are ordinary writes by
  the signed-in person. Detection uses size, change time and inode plus the
  racy-clean rule.
- **Every folder the product creates holds an `AGENTS.md`**, written by the
  server from the kind's declaration: what the folder is for and its on-disk
  format, so an agent that reaches it, even by mistake, knows what it may and
  may not write.
- Records: a header the server writes names the author by person ID; only the
  author edits a record (within the plugin's window, such as Chat's 48 hours);
  the author, the Owner and Managers may delete it (D4); a delete scrubs
  history and search (D1). One file per chat at every size (append 13 ms p50
  up to 256 MiB measured).
- **Links:** the server reads and resolves both standard Markdown links and
  `[[wiki links]]`; new links are written as standard links; converting
  existing wiki links is an optional setting (A11). Renaming rewrites the link
  in every file, in either form (D2), destinations only.
- Mixed boards keep each member's status in the collection's file (D7).

### 4.3 Collections and sidebars

- **A collection is a file**: a saved query plus hand-added items, with big
  views (list, table, board, gallery, calendar) and small views (compact
  list, strip). The word shown to people is not chosen yet.
- **A sidebar is a file listing collections in order.** One collection may
  sit in several sidebars. Customising a sidebar is adding, removing or
  reordering collections and nothing else.
- **A plugin ships collections and a default order**, and no sidebar code.
  The Owner and Managers set a space's default; each member arranges their
  own copy.
- **The card lists sidebars only** (each app's and the person's own). There
  is no Collections tab; a section is added with a + in the sidebar, which
  opens search. Tapping a collection opens its items in the middle card.
- Collections are per space (D6).

### 4.4 Plugins and calls

- **Reading is free inside a space** (within the acting person's rights);
  **an item changes only through its owning plugin's operations** (round
  18), or directly for an editable-outside kind.
- A plugin calls only operations its manifest lists; the called plugin acts
  with its own grant, limited to what the acting person may do (T1). The
  reach set is a later tightening plugins never see.
- **One process per plugin per space**, stopped when idle (about 16 MiB per
  process while in use, nothing idle, 35–51 ms cold start, measured); a
  disabled plugin costs nothing.
- The space's Owner installs an app bundle (I2); each plugin can be replaced
  later. Slots are picked by the person (what only they see) or the space
  (what others receive); a missing part falls back or shows an error
  suggesting the official part.
- Visitors through a share link use the plugin with the link's rights only:
  no secrets, network or agents (T12). The share sheet says that a web-view
  plugin's pages can reach the internet in a browser.

### 4.5 Running code and background jobs

- One per-space work table with a state column; triggers: a call, a time
  (once or repeating), a change.
- **Background jobs:** the program is named at install and has no network
  unless a destination is listed. One call starts a job with a program and a
  list of items. The server queues jobs per space and runs them in the
  space's microVM, interactive work first. A job is keyed by the item's
  hash, so an interrupted one runs again and a finished one is skipped.
  Results are written through the plugin's own operations. Jobs count on the
  same meter as agent hours.
- Agent runs: hosted sandboxes first; a member may offer a machine to the
  space (accepted by the Owner or a Manager, warned; members see whose
  machine each run is on). A run uses three server calls (append to its log,
  read its inbox, end); Stop fences it at once; it ends at its deadline or
  after 24 hours without the server. A message to a running agent is stored
  once, with delivery indicators as in messaging apps (A6).

### 4.6 Drawing

- **One catalogue of native components** for plugins and agents alike,
  including the records view; **anything not in the catalogue is a web
  view** (A3, A4, round 13).
- **Diagrams** are a catalogue component, `calternal-pele`, drawing Mermaid
  with Pele with good themes; usable by every plugin and agent.
- The web view: a plugin ships web code; the space's Owner installs it after
  a plain warning that its pages may reach the internet in a browser (I1).
- **Native features** (the iOS keyboard with synced clipboard, dictation and
  learning gesture typing; on macOS computer use and deep system
  integration) are part of the native app and store their data through
  plugin operations like any client.
- Every surface comes with its motion and interaction designed; sizes come
  from our own studies (rounds 12, 19).

## 5. One declaration, three surfaces

A plugin declares each operation and event once. The server exposes each as
a CLI subcommand (`calternal <plugin> <operation>`, human output by default,
`--json`), an API path (`POST /api/spaces/<space>/calls/<plugin>/<operation>`)
and an MCP action (`<plugin>.<operation>`), three adapters over one route
with the same rights check. Events are a watch, a subscription and an MCP
notification respectively.

- **API tokens are always one person's**, limited to the operations they
  tick, ended when they lose access; there are no space tokens.
- An outside event is an API call made with such a token, so it acts as that
  person.
- **Speed:** an operation through any of the three adapters must be as fast
  as editing the file directly (round 16); this is a build gate (§9).
- **Sync** for clients is fetching from and pushing to the server, and has to
  be very fast (round 15).

## 6. Credentials

| What | How | Proven? |
| --- | --- | --- |
| A plugin's secret | The plugin calls the server's fetch with a named secret; the server attaches it to the request for a listed destination; any service, warned at install (T9) | Designed |
| Mail | IMAP and SMTP are server calls with a named secret | Not built |
| A sandboxed program's credentials | No network; out only through the server's proxy, which adds credentials for listed destinations (round 9) | **Unproven**: HTTPS needs the proxy to terminate TLS with a certificate the sandbox trusts |
| SSH keys | Held by the server, offered as a signing-agent socket; the key never enters the sandbox | **Unproven** over the machine link; a run holding the socket can sign anything |
| Harness sign-in files | The exception: mounted into that harness's runs; its updates wait for the person | Built in the revision-14 harness slice |

## 7. Security

| # | Threat | What contains it |
| --- | --- | --- |
| T1 | A plugin reads beyond the acting person's rights, or outside its space | One check per call; one microVM per space |
| T2 | A plugin changes another plugin's items | Items change only through their owning plugin's operations |
| T3 | A plugin sends data somewhere not listed | No network in its process; fetch, secrets and programs are server calls checked against the install list |
| T4 | Web code sends data out in a browser | Not contained on the web; the Owner accepts this at install, warned; the share sheet says so for visitors |
| T5 | Tampering with an operations-only kind | No outside change accepted; keyed hashes detect disk edits; absent from WebDAV and agents |
| T6 | Forged author or changed kind, ID or switch | Server-held stamps; kind, ID and switch fixed in host state |
| T7 | An agent reads beyond its scope | The starter's scope, ceiling the starter's rights; the space's Agent access off stops all |
| T8 | Someone steers another person's machine | Full-access own-machine runs only in the personal space, steered only by its owner |
| T9 | A machine a member offered reads or forges a run | Accepted by the Owner or a Manager with a warning; members see whose machine it is |
| T10 | A self-updating harness changes the code that reads a sign-in | Its update waits for the person |
| T11 | A run acts after Stop | Fence at the Stop commit; Stop in that run's inbox |
| T12 | Floods: jobs, triggers, calls across spaces | Per-space queues, interactive first; quotas; one meter for jobs and agent hours |
| T13 | A stolen session approves risky actions | Passkey confirmation; revoking a session revokes its picks |
| T14 | A token outlives its person's access | Tokens end with the person's access and cover only ticked operations |
| T15 | A visitor's use reaches secrets, the network or agents | Visitors have only the link's rights |

## 8. Three worked apps

| App | How it is built |
| --- | --- |
| **Reading List** | A `reading` kind (editable outside); "Add link" and "Mark read" declared on the kind; collections "Unread" and "All"; no process; `calternal reading-list add <url>` with no extra code. |
| **Notes** | A `notes` kind, editable outside (in WebDAV, a folder can be handed to an agent); the catalogue's editor; both link forms; rename rewrites every link; tags as a grouped collection; snooze as work at a time. |
| **Chat** | Operations only: absent from WebDAV, no outside change accepted. One record file per chat; a chat between people in the starter's personal space, shared; read position, draft and mute as per-person state; agents scoped to the current chat session by default; replies through fenced calls; steering per §4.1. |

## 9. Build plan

| Step | Goal | State |
| --- | --- | --- |
| 0a, 0b, 1 | Files spike, drawing spike, the first call route with Reading List | **Done** (spikes) |
| 2 | Grow the spike into the test-mode host: hashes-only recovery and tamper detection, racy-clean, a watcher, collections and sidebars as files, one live window | Next |
| 3 | Spaces, sharing, the one check, person tokens; the editable-outside switch with WebDAV exposure | |
| 4 | Running code: plugin box per space (nothing when disabled), work table, triggers, background jobs | |
| 5 | Records in full; operations-only enforcement; `AGENTS.md` per folder from the kind's declaration | |
| 6 | Agent runs in hosted sandboxes and on offered machines; restart test | |
| 7 | **Speed gate:** the same change made through the CLI, MCP and the API, measured against a direct file write on the perf VM; each adapter within the direct write's cost | |
| 8 | Calls between plugins, slots and picks, bundles, the server sheet | |
| 9 | Drawing in full, `calternal-pele`, the web view with its warning | |
| 10 | The app suite (§11), one app at a time, polishing each | |
| 11 | Delete the native engines and v0 models | |

## 10. Decided

| Fork or row | Decision | Round |
| --- | --- | --- |
| D5, A1 | Host state keeps keyed hashes, offsets and stamps, never text; a database | 1, 12 |
| T8 | A chat between people lives in the starter's personal space; sharing it is a share, not membership | 1, 10 |
| I2, T4 | The Owner installs an app bundle; plugins update themselves; a sign-in-reading harness waits for its person | 1, 10 |
| T5 | Hosted first; a member may offer a machine, accepted by the Owner or a Manager with a warning | 1, 2, 10 |
| T1 | The caller lists the operations it calls; the callee acts within the person's rights; the reach set later | 2 |
| T2 | No private items in a shared space; one process per plugin per space | 2 |
| Spaces | One kind; the creator is the Owner; the first space takes no members | 3, 4 |
| Agents | Agent access on or off per space, joining is consent; one scope rule (starter picks, default the current chat session, ceiling the starter's reading); a session starts after one quiet hour; in a chat between people the caller decides what the agent reads | 10, 12, 13 |
| A18 | Personal space: only its owner steers; a space with members: anyone who may write in the chat | 3, 11 |
| T13 | Full-access own-machine runs only in the personal space; picked folders, read-only, otherwise | 5, 11 |
| Run files | Readable by every member who can read the chat | 10 |
| Plugin power | Reading free inside a space; items change only through their owning plugin's operations; sending out listed | 4, 18 |
| Outside editing | One switch per kind (editable outside or operations only), decided by the kind, shown at install; exposed: notes, tasks, calendar, files, photos; operations only: chat, money, mail; stays Markdown; readable, unchangeable without its plugin | 16, 17, 18 |
| Agents and files | Agents work through operations over MCP, API and CLI, as fast as a direct write; a folder of an editable kind may be handed to one | 16, 18 |
| `AGENTS.md` | In every folder the product creates | 17 |
| Background jobs | One call with program and items; queued per space, interactive first; keyed by item hash; results through the plugin's operations; one meter with agent hours | 16, 17 |
| Collections | A collection is a file; a sidebar lists collections in order; one collection in several sidebars; plugins ship collections and a default order; no Collections tab | 18, 19 |
| D1, D2, D3, D4, D6, D7 | Delete scrubs history; rename rewrites every link; plugin caches; Owner and Managers delete; collections per space; mixed boards | 4, 5, 8 |
| T7, T9, T11, T12 | Passkey for risky actions; secrets through the server's fetch; one declaration on three surfaces with person tokens; visitors with the link's rights | 4–7 |
| Credentials | The server's proxy for sandboxed programs; SSH keys as a signing agent; sign-in files the exception | 9 |
| I1, I3 | Web code installed by the Owner after a warning; sideways-scrolling columns | 5, 8 |
| Drawing | One catalogue, web view otherwise; diagrams as the `calternal-pele` component; native features store through plugin operations | 12, 13, 16 |
| A rows | A1–A20 and A22–A25 agreed; A3 and A4 are one; A11 changed (read both link forms, no automatic conversion); A13 with "a disabled plugin costs nothing"; A18 changed (personal space: only its owner steers); A21 changed (no outside change to a chat at all) | 2, 11–15 |
| Authoring | Very easy, through server mechanisms and well-written skills | 15 |

## 11. The app suite: what each needs from the host that the call table does not give yet

The suite the API is tested against (round 15). Five lines each; "nothing"
where the design already covers it.

**Money** (YNAB, Actual Budget; operations only)
1. A change across several files at once (a transfer between two accounts)
   must commit together or not at all; today every write is one file.
2. Sums and group totals in queries (spent per category per month); the
   query engine filters and sorts but does not add up.
3. An exact decimal field type, so amounts never pass through floats.
4. Bank import is the server's fetch plus a background job: nothing new.
5. A reconciled ledger is the operations-only switch: nothing new.

**Workouts** (RP Hypertrophy, MyoAdapt)
1. A chart component in the catalogue (volume, progression).
2. The same sums and averages in queries as Money.
3. Rest timers and reminders are work at a time plus notifications: nothing
   new.
4. Health data from the phone arrives through the native app's operations:
   nothing new.
5. Otherwise nothing.

**Mail** (operations only)
1. IMAP and SMTP server calls (designed, not built), including a long-lived
   connection for new-mail push (IDLE), which the request-and-reply fetch
   does not give.
2. Raw messages and attachments are files; HTML bodies are a web view:
   nothing new.
3. Search over 50,000 bodies is the index: nothing new, but its size and
   speed are unmeasured.
4. Delayed send and snooze are work at a time: nothing new.
5. A thread view across many message files may need the records view to
   draw items from several files.

**Calendar** (Fantastical, BusyCal; editable outside)
1. Recurring events in a date-range query: the index cannot expand a
   repeat rule, so either it learns to, or the plugin supplies computed
   rows.
2. Syncing with outside calendars is fetch plus jobs: nothing new.
3. Time zones in date fields and queries.
4. A calendar view exists in the catalogue's big views: nothing new.
5. Invitations go through Mail's operations: nothing new.

**Tasks** (TickTick, Things 3; editable outside)
1. Queries relative to now ("Today", "Overdue") must be re-evaluated when
   the clock passes a boundary, not only when a file changes.
2. Repeating tasks are repeating work: nothing new.
3. Reminders at an exact time on every device: work plus notifications,
   with delivery timing unmeasured.
4. Subtasks and links are links: nothing new.
5. Otherwise nothing.

**Notes** (Craft, Obsidian; editable outside)
1. Two people editing one note at once needs live merging; writes with an
   expected revision only report a conflict.
2. Both link forms and rename rewriting: designed (A11, D2).
3. Block references to a paragraph need anchors in plain notes, as records
   have.
4. The editor is a catalogue component: nothing new.
5. Backlinks and tags are queries: nothing new.

**Files** (Nextcloud; editable outside)
1. Resumable, chunked upload and ranged download in the call table for
   large files.
2. Previews and thumbnails are background jobs: nothing new.
3. Share links with upload are visitors with the link's rights: nothing
   new.
4. WebDAV is the server's own: nothing new.
5. Version history of binary files: history is designed for text; its cost
   for large binaries is unstated.

**Documents** (Paperless-ngx)
1. OCR is a background job, but heavy jobs may need more memory or CPU
   than the space's microVM; a resource class for jobs is missing.
2. Originals stay untouched with a Markdown sidecar: nothing new.
3. A consumption folder is a change trigger: nothing new.
4. Documents arriving by mail go through Mail's operations: nothing new.
5. Full-text search over OCR text is the index: nothing new.

**Photos** (Immich, PhotoStructure, PhotoPrism; editable outside)
1. The same resumable upload as Files, for phone backup.
2. Thumbnails, video transcoding and face or object recognition are heavy
   jobs: the same missing resource class, possibly a GPU.
3. A timeline over 100,000 items and a map view: catalogue components and
   window speed at that size, unmeasured.
4. EXIF and XMP sidecars are inherited decisions: nothing new.
5. Albums shared to friends are visitors: nothing new.

**Across the suite:** multi-file atomic writes, query aggregates, an exact
decimal type, a time-relative query refresh, resumable upload and ranged
read, job resource classes, long-lived outbound connections for mail, live
merging for notes, and chart, timeline and map components. None of these
needs app knowledge in the server; each is a generic primitive or
component.

## 12. Still open

| Item | Why it is still open |
| --- | --- |
| Per-person private state in a space with members | "No private items in a shared space" (T2) and "reading is free inside a space" leave read positions, drafts, mutes and a member's own sidebar copy unaddressed: are they visible to others, or kept in the personal space? |
| Whether agents are simply plugins | The owner asked (round 10) with a request for a table of personal against shared spaces; §4.1 gives the table, but the question itself was not answered |
| The word for a collection | Not chosen (round 19) |
| How sidebars, collections and sections read to people | The owner said they are still not clear (round 18) before confirming the model in round 19 |
| Native rendering of `calternal-pele` | The component is decided; whether Pele's SVG output draws natively on Apple platforms is unchecked |
| The editable-outside switch for kinds not named | Documents, workouts and any community kind: the kind decides, but no default is stated |
| Space count per account (T10) | Not answered |
| Per-person approval of a plugin's use of their data (T6) | Not answered; may be moot under "reading is free inside a space" |
| Sharing a sidebar | To be designed (A14) |
| Credentials for sandboxed programs and SSH | Decided, not proven (§6) |
| The suite's host needs (§11) | Each is a generic addition the owner has not yet been asked about |
| The install sheet's motion | Not designed (round 19) |
