# A primitives-only host · revision 5 · 2026-11-24

Status: **revision 5: the single statement of the design**, with edits on
2026-11-27 wherever it contradicted revisions 6 and 7 of the format (marked
"(format r6)"; revision 7 is the authority); for kinds, collections, sidebars, `plugin.md`, places,
calls, events, themes and the space profile, the format is the
authority. Revision 5 was written with the
owner's answers from rounds one to twenty-five of `docs/morning-2026-11-21.md`
on `dev` (2026-11-21 to 2026-11-24) applied. The on-disk format of kinds,
items, collections, sidebars and plugins is specified in
[kind-and-collection-format-2026-11-24.md](kind-and-collection-format-2026-11-24.md). Where this proposal and the
morning report disagree, the morning report wins. What the owner has not decided is in §13 with the
reason. Revision 2 (`502ab2bc`) keeps the option analysis and both reviews;
the section files under [`primitives/`](primitives/) are historical where
this document differs. The 2026-11-28 update records the Chat-first call
surface and build status in §§4.4, 5 and 9, with format revision 10 as
the declaration reference.

Principle (owner, round 9): **harden the space, plugin and VM boundary
rather than make promises the server cannot enforce.** The plugin API is
versatile and powerful; the server is simple, generic, very secure and
opinionated (round 15).

## 0. Three ideas

| Idea | What it means | Built as |
| --- | --- | --- |
| **Files, with questions** | Everything a person keeps is a Markdown file in a space, with its history and a stamp of who wrote it. The server answers questions about the files: lists, search, backlinks, unread counts. | Files, Questions |
| **Calls** | Everything that changes is a call to a named operation, checked once: who is asking, through which plugin, on what. Plugin code starts one way: an operation listens to an event (a call, a time, a schedule, a file change, an outside HTTP request, data on a held connection, or another plugin's committed operation; format r6). People, plugins and agents use the same calls, through the app, the CLI, the API or MCP. | Authority, Running code |
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
| Changing things | Per-app write paths | Collections show anything the person may read; plugin code reads other plugins' kinds only when `reads:` names them; an item changes only through its owning plugin's operations, or directly when its kind is editable outside; another plugin's added fields change only through that plugin (format r6) | Money's ledger changes only through Money |
| Lists | Binds, sources, extractors, search profiles, layout sections | One query form with totals (count, sum, min, max, average by one field) and computed fields; a collection is a file; a sidebar is a file listing collections | "This month" is `where: month(date) = month(today())`, `group: category`, `total: sum(amount)` |
| Running things | Jobs, intents, attempts, successors, a spool, a relay | Events start operations; one work table; background jobs through one call; the server holds long-lived connections | New mail raises an event from the server's IMAP connection |
| A plugin | A TypeScript package with JSON Schemas, a manifest and an interface description | Markdown files (`plugin.md`, `kinds/`, `collections/`, `sidebars/`) plus one `operations.ts` only for what needs code | Reading List is 51 lines of Markdown and 2 lines of TypeScript, against 226 |
| Agents | A special engine with consent prompts | Plugins that work through the same operations, scoped by their starter | Claude asked in a chat reads the current chat session by default |
| Surfaces | A browser route per component, a separate CLI | One declaration exposed as a CLI subcommand, an API path and an MCP action | `calternal chat send` and the MCP action `calternal-chat.send` are the same call |
| Interface | 87 description shapes, layout caps, entry views | One catalogue of native components plus a web view | A diagram is a catalogue component drawn with Pele |
| App code in the server | Native chat, Notes, providers, automations | None | Notes is a plugin of about 30 lines |

The inventory and the fate of every revision-14 name are in
[was-and-is.md](primitives/was-and-is.md).

## 2. What a plugin can do, and cannot do

| A plugin can | Example |
| --- | --- |
| Read its own kinds and the kinds its `plugin.md` names in `reads:`, within the acting person's rights (format r6, C4) | A summary plugin with `reads: calternal-notes.notes` reads the space's notes |
| Change its own kinds' items through its own operations, and add fields, operations, marks and sidebar lines to other plugins' kinds with `extends:` (format r6, M2) | Notes edits a note through `calternal-notes`; Reading List adds "Send to reading list" to mail |
| Call other plugins' operations that its manifest lists, to change their items | Mail files an attachment through `calternal-files` |
| Declare kinds (with their looks, and whether they are editable outside or operations only) and the collections its app starts with | Chat declares `chats` as operations only |
| Add, change and remove items of its kinds with no code at all, on the app, the CLI, the API and MCP | `calternal reading add --url …` |
| Declare computed fields and totals in kind and collection files | A budget's `spent: = sum(from(transactions.category).amount)` |
| Write several changes that commit together or not at all | A transfer between two accounts |
| Keep small private state per person, and a cache the server may delete at any time | Chat's read positions; a mail index |
| Run on a call, at a time, on a repeating schedule or when files change, and start background jobs over many items | Photos makes thumbnails for 2,000 new pictures |
| Run its own program, or the one the person picked, in a hosted sandbox or on a machine a member offered | The Claude harness |
| Offer operations and events, which the server exposes on the CLI, the API and MCP | `calternal reading-list add <url>` |
| Fill host places (panel, palette, search, receive, widgets…) and other apps' slots, picked by the person (what only they see) or the space (what others receive) (format r6, M1) | A community composer in Chat |
| Reach listed hosts, a server a member names (that member confirms it), or the public web without sign-ins; the server signs in for HTTP, IMAP and SMTP and holds open connections | A mail plugin on the server's IMAP connection; a feed reader that "reads public web pages, never signs in as you" |
| Draw with the catalogue, and with a web view where the catalogue has nothing | Mail's HTML body |
| Serve visitors of a share link with exactly the link's rights | A shared album lets friends download and upload |

| A plugin cannot | Example |
| --- | --- |
| Read anything the acting person may not, or anything outside its space | A plugin in "Lisbon trip" never sees Ana's personal space |
| Change another plugin's items except through that plugin's operations, or the fields it added under its own name | A summary plugin cannot rewrite a chat message |
| Reach a destination, use a secret or start a program not listed at install | A notes plugin cannot call a new server after an update without the Owner's yes |
| Hold a secret's value | A mail plugin never sees the mail password |
| Forge who wrote something, or change a file's kind, ID or outside-editing switch | Nobody can post "as Alice" or make chats editable over WebDAV |
| Run code in a client, except in a web view | No JavaScript in a composer |
| Draw or hide a server screen, ask for a password, or skip the passkey confirmation of a risky action | Sharing outside the space always shows the server's sheet |
| Keep running after Stop, or exceed its quotas | A looping trigger is cut off |
| Cost anything while disabled | A disabled plugin has no process, no memory and no work on any path |
| Give a share-link visitor secrets, network or agents | A visitor's upload never starts an agent |

## 3. The rules a plugin follows

1. **It is Markdown files** (`plugin.md`, `kinds/`, `collections/`,
   `sidebars/`) plus one `operations.ts` only for operations that need code;
   a plugin without code is identical to a kind made in the app. Its data is
   files in the space, Markdown wherever no other standard format exists.
   What it can rebuild is a cache the server may delete.
2. **Every call goes through the server, as the acting person.** There is no
   other way to reach files, people, the network or other plugins.
3. **It changes only its own kinds' items, through its own operations**,
   plus the fields it added to other kinds under its own name; other
   plugins' items change through theirs (format r6).
4. **Its code starts only on events** an operation listens to (a call, a
   time, a schedule, a file change, an outside HTTP request on its own web
   address, data on a held connection, or another plugin's committed
   operation: six events, a time including a repeat's occurrence; format
   §8). At most one active worker per plugin and space, stopped on idle
   and recycled when the acting person changes. A disabled or
   description-only plugin has no worker.
5. **What it sends out is declared**, in one of three forms: a fixed list of
   hosts; "a server a member names", which that member confirms on the
   server's sheet; or the public web without sign-ins ("Reads public web
   pages. Never signs in as you."). Secrets and programs are listed too. An
   update that adds to the list waits for the Owner.
6. **What it offers and calls is declared** in operations, events and
   `calls:`. A person-picked slot also grants the one operation its
   selected installed filler declares, for that invocation.
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
| Private items | Everything is its owner's | None: every member reads every file within their role. A member's own state for the space (read positions, drafts, mutes, their sidebar copy) lives in that member's personal space |
| Agent access | Always available to the owner | A space setting, **on or off**, chosen at creation, told to every member when they join; joining is consent |
| An agent run's scope | Picked by the starter: default the current chat session, ceiling what the starter may read | The same rule |
| Who steers an agent | Only the owner, always | Anyone who may write in the chat |
| "Direct, full access" on the person's own machine | Allowed | Not allowed: sandboxed, or limited to picked folders |
| A run's log and files | The owner's | Readable by every member who can read its chat |
| Installs | The owner | Any member may add a description-only plugin; anything with code or sending out is the Owner's. A member may offer a machine, accepted by the Owner or a Manager with a warning |

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

- **Every write is a list of changes that commit together**; a single change
  is a list of one.
- **One standard repeat field** the server understands, the iCalendar repeat
  rule, shared by Calendar, Tasks, Money and Workouts; "Today" and "Overdue"
  are queries the server re-runs at the person's midnight.
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
- Writes over CalDAV, CardDAV (served by the server for kinds with an
  `ical:` or `vcard:` mapping, format r6) or WebDAV to editable kinds are ordinary writes by
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
  views (list, table, board, gallery, calendar, timeline, chart; map comes
  with Photos) and small views (compact list, strip). The word shown to
  people is not chosen yet.
- **Computed fields** are spreadsheet-style formulas, written in a kind file
  (for every view of that kind) or in a collection file (a column of its
  own), including totals through link fields (`sum(tasks.hours)`). A formula
  never writes, never reaches the network, has a bounded cost and no
  cycles; computed values are kept only in the rebuildable cache.
- **Queries give totals**: count, sum, minimum, maximum and average, grouped
  by one field; values carried from period to period are `per day|week|
  month|year` fields with `previous()`, and totals over whole kinds are a
  collection's `totals:` with `all()` (F1, F2; format r6).
- **A form is the kind's `add` line** in its operations list, not a view
  (round 29).
- **A sidebar is a file listing collections in order.** One collection may
  sit in several sidebars. Customising a sidebar is adding, removing or
  reordering its lines: collections, `each:` lines, sections, folder
  shortcuts, and lines other plugins add with `extends:` (format r6 §6).
- **A plugin ships collections and a default order**, and no sidebar code.
  The Owner and Managers set a space's default; each member arranges their
  own copy.
- **The card lists sidebars only** (each app's and the person's own). There
  is no Collections tab; a section is added with a + in the sidebar, which
  opens search. Tapping a collection opens its items in the middle card.
- Collections are per space (D6).

### 4.4 Plugins and calls

- **A plugin is Markdown files** installed into the space's `.calternal/`
  (`kinds/`, `collections/`, `sidebars/`); a plugin with code adds one
  `operations.ts` holding only the operations that need code. The kind
  file's body is the folder's `AGENTS.md`. **Editable-outside kinds get add,
  change and remove without listing them; operations-only kinds have only
  the operations they list** (round 29), on the app, the CLI, the API and
  MCP.
- **Collections show anything the acting person may read; plugin code
  reads another plugin's kinds only when `reads:` names them** (C4, format
  r6); **an item changes only through its owning plugin's operations** (round
  18), or directly for an editable-outside kind.
- A plugin can call operations its manifest lists or the coded operation
  of the installed filler selected by the person for this call. The
  called plugin acts with its own grant, limited by the acting person's
  rights (T1). The host resolves the selection and keeps it local to
  that invocation.
- **At most one active worker per plugin per space**, stopped when idle
  and recycled when the actor changes. The standalone host requires
  the operator's isolated Bubblewrap launcher for installed code.
- The space's Owner installs an app bundle (I2), and any member may add a description-only plugin; each plugin can be replaced
  later. Slots are picked by the person (what only they see) or the space
  (what others receive); a missing part falls back or shows an error
  suggesting the official part.
- Visitors through a share link use the plugin with the link's rights only:
  no secrets, network or agents (T12). The share sheet says that a web-view
  plugin's pages can reach the internet in a browser.

**Calls on the standalone Chat-first host**

| Surface | Available on merged `dev` | Boundary |
| --- | --- | --- |
| `POST /v1/spaces/{space}/call` | `s.declarations`, `s.query`, `s.read`, `s.write`, `s.state`, `s.me`, `s.space`; installed coded operations by qualified name; shell maintenance via `index.rebuild` | Personal-space owner. Browser uses an origin-checked cookie; CLI sessions can use a bearer credential. |
| Isolated `operations.ts` | `s.query`, `s.read`, `s.write`, `s.state`, `s.call`; host-supplied `s.me`, `s.space` and `s.event` context | `on.call` and post-commit `on.change`. The worker protocol also defines `s.run`, `s.run.send` and `s.run.stop`; the merged route refuses them until a scope-aware run adapter is attached. |
| `GET /v1/spaces/{space}/live` | Cookie-authenticated WebSocket for collection/query rows and item records | Owner-only. Reconnect uses the last record anchor. Rights-loss revocation is required before non-owner windows. |

The full fifteen-call design is in [format §8.1](kind-and-collection-format-2026-11-24.md#81-the-fifteen-calls).
The standalone host does not expose `s.fetch`, `s.connect`, `s.notify`,
`s.job`, `s.cache`, `s.share` or `s.outside.mint`.
See the [host call route](../reviews/host-call-route-2026-11-27.md) and
[live protocol](../reviews/live-window-protocol-2026-11-27.md) for payloads.

### 4.5 Running code and background jobs

- **One way to start plugin code: events.** A call, a time, a schedule, a
  file change, an outside HTTP request, data on a held connection and
  another plugin's committed operation are the six events, with a repeat's
  occurrence counted as a time (format r7 §8).
  One per-space work table with a state column.
- **The server holds connections that stay open** (an IMAP mailbox) and
  raises an event when something arrives; the plugin's process may stop
  when idle.
- **No job sizes for now:** the operator sets the space machine's size; it
  is measured again when Photos is built.
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
with the same rights check. The Chat-first HTTP adapter uses
`POST /v1/spaces/{space}/call` with the operation name in the envelope.
Generated CLI and MCP actions are later work for this host.

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
| Sign-ins that are not web requests | The server connects and signs in for HTTP, IMAP and SMTP and gives the plugin the open connection; mail fetching stays in the plugin (round 21) | Not built |
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

The earlier file, records and drawing spikes informed the
[Chat-first slices](chat-first-build-plan-2026-11-27.md). The table records
merged `dev` at `32dc5517` and labels branch evidence.

| Slice | Goal | State |
| --- | --- | --- |
| Spikes | File writes and call route with Reading List, record appends and declaration-driven drawing | Complete as spikes. |
| 0 | Shared format engine for declarations, Markdown items and records, formulas and pure operations | Merged. Reads the two Chat bundles and runs the in-scope fixture vectors. DAV, extensions, themes, profiles, line items and broader repeat/total cases remain pending. |
| 1 | One personal space, local plugin install, Markdown items, atomic writes, rebuildable query index and checked call route | Merged. The host installs the Chat bundle and supports add, change, remove, query and rebuild. Shared rights, external editing and app install approval remain pending. |
| 2 | Ledger-authored records, append receipts and recovery, per-person state | Merged. Two-flush sealed append, verified tail adoption, record changes/redaction and anchor-based unread. Shared-writer recovery needs an authoritative writer source. |
| 3 | Isolated operation workers and slot-selected calls | Merged. Bubblewrap, typed inputs, reaction probe, Chat send/rename and append replay. Durable change delivery and real run-start replay remain pending. |
| 4 | Live windows | Merged. Owner-only cookie-authenticated collection and record subscriptions, anchor reconnect and sign-out socket close. Explicit rights-loss revocation remains pending. |
| 5 | Browser on the real host | Merged host and browser data-client proof for passkey sign-in, declarations, writes and two browser contexts. Browser branch `4d0c500d` adds the screens, shared shell, generated forms and picker; its tests use a disposable filler and a local stream/Stop fixture. |
| 6 | A scoped hosted harness run, streaming, Stop and one completed reply record | Partial on runs branch `293af65a`. Fake-provider Podman service and mediated egress verified there. Host `s.run`, scope checks through Chat, real streaming through the UI and final append remain pending. |
| 7 | One browser-to-harness journey with a scoped run, live reply, Stop and durable result | Pending on the new host. |
| 8 | Agent rules and prompts | After the Chat-first goal. |
| 9 and later | Sharing, broader plugin APIs and app suite, native clients, offline outbox, DAV, jobs, held connections, full install flow, generated CLI and MCP actions, and the speed comparison across adapters | Pending on the new host. Production cutover and retirement of the old engines follow those gates. |

Status sources are the [coordinator report](../morning-2026-11-21.md),
[host core findings](../reviews/host-core-findings-2026-11-27.md),
[worker findings](../reviews/plugin-process-findings-2026-11-27.md) and
[browser findings](../reviews/browser-chat-findings-2026-11-27.md).

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
| Events | One way to start plugin code: six events (a call, a time or a repeat's occurrence, a file change, an outside HTTP request, a held connection, after another plugin's operation) | 21, 29, 33; format r6 |
| Personal state | Read positions, drafts and mutes live in the person's own space; shared spaces keep nothing private | 21 |
| Sign-ins | The server signs in for HTTP, IMAP and SMTP and gives the plugin the open connection; it holds long-lived connections and raises events | 21, 22 |
| Destinations | A fixed list; a server a member names, confirmed by that member on the server's sheet; or the public web without sign-ins, worded "Reads public web pages. Never signs in as you." | 21, 22, 23 |
| Description-only plugins | Any member may add one; code or sending out stays with the Owner | 22 |
| Writes | Every write is a list of changes that commit together | 22 |
| Totals | Count, sum, minimum, maximum and average, grouped by one field | 22 |
| Repeats | One iCalendar repeat field; Today and Overdue re-run at the person's midnight | 22 |
| Jobs | No job sizes for now; measured again with Photos | 22 |
| Plugin format | Markdown files (`plugin.md`, `kinds/`, `collections/`, `sidebars/`) plus one `operations.ts` only for code; a no-code plugin equals a kind made in the app; installed into `.calternal/`; the kind file's body is the folder's `AGENTS.md`; one operations list per kind, with add, change and remove unlisted only on editable-outside kinds | 23, 25, 29 |
| Formulas | Spreadsheet style, in kind files and collection files, with totals through link fields | 23, 24, 25 |
| Views | Timeline and chart now, map with Photos; a form is the kind's create form | 23, 24 |

## 11. The app suite: what each needs from the host

The suite the API is tested against (round 15), walked again after rounds
21–25. "Nothing" means the design and the format cover it.

**Money** (operations only): multi-file writes, totals, the repeat field and
the exact `money` type in the format cover it. Open: totals across accounts
in different currencies (no exchange-rate rule).

**Workouts:** computed volume, weekly totals and a chart view cover it;
sets are records with listed record operations that apply offline (C3). See
`format-stress-test-suite-2026-11-27.md` for this and the six apps below.

**Mail** (operations only): the server signs in to IMAP and SMTP and holds
the IDLE connection, raising an event for new mail. Open: the size and speed
of search over 50,000 bodies is unmeasured, and a thread view that draws
records from several message files.

**Calendar:** the repeat field with server-side expansion in date ranges,
`time` with zones (ADR 0066), calendar and timeline views; time lines in
daily notes are events (`lines:`, C1); the server serves CalDAV for kinds
with an `ical:` mapping (C2). Outside calendar sync is fetch plus events.

**Tasks:** Today and Overdue re-run at the person's midnight; repeating
tasks are the repeat field; computed `late`; a checkbox in any note is a
task whose box follows its status, understood by the shared core (`lines:`,
C1).

**Notes:** two people editing one note and linking to a paragraph are
inherited (ADRs 0045, 0056; §12). Open: how three-way merging meets live
co-editing on the new host (§12, question S5).

**Files:** resumable chunked uploads are the inherited sync route (ADR 0044,
8 MiB chunks); previews are background jobs. Open: history cost for large
binaries.

**Documents:** OCR is a background job writing the body; the original stays
untouched. Open: heavy jobs on the space machine, deferred with job sizes
(round 22).

**Photos:** gallery and timeline now, map with Photos; transcoding and
recognition are background jobs. Open: job size, and window speed over
100,000 items (not now, round 25).

## 12. Inherited sync and conflict decisions on the new host

The old app's accepted decisions (read with git from
`/home/kayg/Developer/calternal` at `origin/dev`) and how each meets the
primitives-only host.

| ADR | Decision | On the new host |
| --- | --- | --- |
| 0021 Saving, offline first | A file is written on the first keystroke; every keystroke goes to local storage first, then to the server with retries; the server flushes after 750 ms of quiet; history per note | **Unchanged** for editable-outside kinds edited in the app. For operations-only kinds the device has no file to write; see question S1 |
| 0044 One sync route | One route and one ordered, replayable change journal; resumable 8 MiB chunk uploads; the CLI, API and MCP use the same journal | **Unchanged.** It is the sync behind §5's three surfaces; the journal is filtered per space and per person's rights. "MCP Events" through signed webhooks is the outward half of round 7's events |
| 0045 Conflicts by kind of data | Text: three-way merge against the last synced version; structured files: merged by record through their format plugin; binaries: both copies kept | **Changes for structured files:** the old format plugins were Rust crates on server and clients (0048); plugins are now TypeScript on the server only. See question S2 |
| 0047 Sync reliability | Silent pushes, catch-up from the journal cursor; an outbox of operation IDs derived from installation, file, base hash and new hash, kept until the server confirms | **Unchanged** for file edits. For coded operations the ID must come from the operation and its input, which is A22's per-gesture key |
| 0048 Format plugins on server and clients | One Rust parser and merge per format, used offline on devices; heavy jobs only on the server | **Changes:** plugin code never runs on devices, so a device cannot run a plugin's merge or its coded operations offline (questions S1 to S3). Heavy jobs on the server carry over as background jobs. The shared Rust engine itself runs in the browser as Wasm (round 33); whether plugin pure modules may run on devices is OPEN (format r7 §17) |
| 0056 Stable block IDs | IDs only for linked blocks, 16 Crockford characters, unique across the library, the server as the authority; delete wins over an offline edit | **Unchanged**, with "the space" in place of the old "Home". Links between spaces are not decided (question S4) |
| 0066 Tasks: time zones and repeats | Repeating task times float; repeating events keep their zone; one-off times keep their offset; Today is asked with the device's date and zone; a server timer at the person's midnight | **Unchanged**, and the format's `time` and `repeat` types follow it. Round 22's "re-run at the person's midnight" is this timer |

### Questions for the owner

**S1. Which changes apply at once on a device with no connection, and which queue?**
*What:* coded operations (a transfer in Money, sending a chat message) run
in the plugin's process on the server. A device without a connection cannot
run them, while a free add, change or remove of an item is a plain file
change the device can make itself.
*Why now:* it decides what every app feels like offline, and what the device
outbox holds.
*Example:* on a plane, you mark three tasks done, add a transaction and
write a chat message.
*Options:* (a) **free add, change and remove and the kind's declared
actions (`set read = yes`) apply at once on the device and sync later;
coded operations queue in the outbox, are shown "not sent yet", and run on
reconnect**; a queued operation that then fails is shown to the person.
Cost: some actions look different offline. (b) Everything queues, nothing
applies until the server answers. Cost: offline use is read-only in feel.
(c) Plugins may declare an offline form of each operation. Cost: a second
implementation of every coded operation.
*Recommendation:* (a). It matches 0021's "local first" for the changes the
device can make itself, and never pretends a coded operation happened.

**S2. How are two offline versions of a structured file merged without format plugins on devices?**
*What:* 0045 merged structured files by record through a Rust format plugin
on both sides. Plugins are now TypeScript on the server only.
*Why now:* tasks, events and transactions are structured files edited on
several devices.
*Example:* your phone sets a task's deadline offline while your laptop
renames the same task.
*Options:* (a) **a generic merge driven by the kind file**: the fields are
declared, so the shared Rust core merges field by field on the server and on
devices, and keeps both copies when the same field changed on both sides.
Cost: no kind-specific merge rules. (b) Merge only on the server with the
plugin's own code; devices keep both versions until they sync. Cost: a plugin
writes merge code, and devices show conflicts until they reconnect.
*Recommendation:* (a). The kind file already says what a field is, so one
merge serves every kind and runs offline.

**S3. Are computed fields and totals available offline?**
*What:* computed values live in the server's rebuildable cache.
*Why now:* a budget or a project's remaining hours shown offline must not
be stale or blank.
*Example:* offline, you add a transaction; does "left" in the category
change?
*Options:* (a) **the shared Rust core evaluates formulas on devices too**,
over the items the device holds; values are marked provisional until the
server confirms them. Cost: the evaluator ships to every client, and totals
over items the device does not hold are incomplete. (b) Offline shows the
last values from the server. Cost: they do not move until reconnect.
*Recommendation:* (a), since formulas are declarations, not plugin code.

**S4. May a link point into another space?**
*What:* block IDs are unique in a space (0056); collections are per space
(D6). A note in one space may link to a note in another.
*Why now:* link rewriting on rename and block links need a rule.
*Example:* your personal journal links to a note in "Lisbon trip".
*Options:* (a) **links across spaces work for people who can read both, and
rename rewrites them**, but collections and formulas never cross spaces.
Cost: the link index spans spaces for one person. (b) Links stay inside a
space; a link to another space is a plain URL that does not follow renames.
Cost: cross-space links break on rename.
*Recommendation:* (a) for links only; it keeps "links follow every rename"
true.

**S5. How does three-way merging meet two people typing in one note at the same time?**
*What:* 0045 merges against the last synced version, which suits edits
made apart. Two people in the same note at once need their keystrokes merged
live.
*Why now:* the owner named it as a next gap (round 25).
*Example:* you and Lee both fix the packing list together.
*Options:* (a) **the old app's live rooms carry over**: while two people
have a note open, edits merge live in a room and the file is saved from it;
three-way merge covers everything else. Cost: a live channel per open note.
(b) Only three-way merge on save. Cost: overlapping edits make conflict
copies.
*Recommendation:* (a), reusing the existing room code.

## 13. Still open

| Item | Why it is still open |
| --- | --- |
| O1–O4 of the suite stress test | Partner sharing across spaces, native Apple surfaces, Workouts' first version, reading rights inside a space (format r7 §17) |
| `refuses:`, plugin pure modules on devices, a binding space-wide look | Format r7 §17 |
| The word for a collection | Not chosen (round 19) |
| Native rendering of `calternal-pele` | The component is decided; drawing Pele's output natively is unchecked |
| Totals across currencies | The format refuses to add different currencies; no exchange-rate rule |
| Space count per account (T10) | Not answered |
| Per-person approval of a plugin's use of their data (T6) | Not answered; partly answered by `reads:` declarations shown on the install sheet (C4, format r6) |
| Sharing a sidebar | To be designed (A14) |
| Credentials for sandboxed programs and SSH | Decided, not proven (§6) |
| Notifications, speed at size, heavy jobs | Deferred by the owner (round 25) |
| The install sheet's motion | Not designed (round 19) |
| What the server's operator can do against a space's Owner | Asked in round 21, not answered |
