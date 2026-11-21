# A primitives-only host · revision 3 · 2026-11-21

Status: **revision 3, with the owner's answers applied** (rounds one to
eight of `docs/morning-2026-11-21.md` on `dev`, 2026-11-21). What the owner
decided is written below as the design. What is still open is listed in
§11, each with the reason. Revision 2 (`502ab2bc`) holds the full reasoning
behind each option and the two reviews; the section files under
[`primitives/`](primitives/) hold the detail of each part and are historical
where this document differs.

## 0. Three ideas

| Idea | What it means | Built as |
| --- | --- | --- |
| **Files, with questions** | Everything a person keeps is a Markdown file in a space, with its history and a stamp of who wrote it. The server answers questions about the files: lists, search, backlinks, unread counts. | Files, Questions |
| **Calls** | Everything that happens is a call to a named operation, checked once: who is asking, through which plugin, on what. A call runs now, later or elsewhere (on a machine). Code that can send data out sees other people's writing only as the space's agent-access level allows. | Authority, Running code |
| **Drawing** | Plugins never draw. They declare how their things look and which collections an app starts with; clients draw that with native controls. Web code is the exception, installed by the space's Owner after a warning. | Drawing |

The server is a file server that knows who is asking. It knows nothing about
chats, notes, mail, agents or providers; every feature is a plugin built from
the same parts a stranger's plugin gets.

## 1. What the server was, and what it is

| Area | Was | Is | Example |
| --- | --- | --- | --- |
| Size | About 160,000 lines of Rust, 17,900 of them app-specific; a 17,200-line contract with about 60 concepts and 312 numbers | A file server with five parts and no app code | Chat's 1,884 lines and 1,317-line test host become about 100 lines |
| Storage | Files plus per-app engines, spans, a viewer record, layout tables | Markdown files; host state holds stamps and keyed hashes, never text | A chat is `Chats/Lisbon crew/chat.md` in the starter's personal space |
| Spaces | Personal and shared spaces of different kinds | One kind of space; the first one stays personal | Ana creates "Lisbon trip" and invites Lee from her group |
| Lists | Binds, sources, extractors, search profiles, layout sections | One query form; a collection is a file holding a saved query and hand-added items | "Unread" is `{ kind: reading, where: { read: false } }` |
| Who may do what | Six audiences, activation contexts, leases, five write checks | One check per call; a plugin may touch any file the person may; sending out is listed at install | A composer posts through Chat only because its manifest lists `calternal-chat/send` |
| Running things | Jobs, intents, attempts, successors, a spool, a relay, acknowledgements | One work table; a box; a run talks through three server calls | A run survives a server restart because its log just syncs |
| Surfaces | A browser route per component, a separate CLI | One declaration exposed as a CLI subcommand, an API path and an MCP action | `calternal chat send`, `POST …/calls/calternal-chat/send` and the MCP action `calternal-chat.send` are the same call |
| Interface | 87 description shapes, layout caps, entry views, request cards | Looks, views, slots, a closed catalogue, web code with a warning, one server sheet | A sidebar lists collections; tapping one opens it in the middle |
| App code in the server | Native chat, Notes, providers, automations | None | Notes is a plugin of about 30 lines |

The inventory and the fate of every revision-14 name are in
[was-and-is.md](primitives/was-and-is.md).

## 2. What a plugin can do, and cannot do

| A plugin can | Example |
| --- | --- |
| Read and change any file in its space that the acting person may | Notes edits a note the person may edit |
| Declare kinds, how each looks as a row, card, cell, page and record, and the collections its app starts with | Reading List's `reading` kind and its "Unread" collection |
| Create files and set fields with no code at all | Reading List's Add form and "Mark read" swipe |
| Keep small private state per person, and a cache the server may delete at any time | Chat's read positions; a mail index |
| Run on a call, at a time, on a repeating schedule or when files change | Notes' snooze |
| Run its own program, or the one the person picked, in a hosted sandbox or on a machine a member offered | The Claude harness |
| Call another plugin's operations that its manifest lists | A composer calls `calternal-chat/send` |
| Offer operations and events, which the server exposes on the CLI, the API and MCP | `calternal reading-list add <url>` |
| Fill slots in other apps, picked by the person (what only they see) or the space (what others receive) | A community composer in Chat |
| Reach listed destinations with secrets the server attaches | A mail plugin fetching mail |
| Ship web code for what the catalogue cannot draw, after the Owner accepts a warning | Mail's HTML body |
| Serve visitors of a share link with exactly the link's rights | A shared album lets friends download and upload |

| A plugin cannot | Example |
| --- | --- |
| Touch any file the acting person may not, or anything outside its space | A plugin in "Lisbon trip" never sees Ana's personal space |
| Reach any destination, use any secret or start any program not listed at install | A notes plugin cannot call out to a new server after an update without the Owner's yes |
| Call an operation its manifest does not list | A composer cannot call `calternal-notes/delete` |
| Hold a secret's value | A mail plugin never sees the mail password |
| Forge who wrote something, or change a file's kind or ID | Nobody can post "as Alice" |
| Edit another person's record; delete one unless the person is the Owner or a Manager | A plugin cannot let Sam change Lee's words |
| Give code that sends data out more of other people's writing than the space's agent-access level allows | In a space set to "off", no agent reads anyone's writing |
| Run code in a client, except its web code | No JavaScript in a composer |
| Draw or hide a server screen, ask for a password, or skip the passkey confirmation of a risky action | Sharing outside the space always shows the server's sheet |
| Keep running after Stop, or exceed its quotas | A looping trigger is cut off |
| Give a share-link visitor secrets, network or agents | A visitor's upload never starts an agent |

## 3. The rules a plugin follows

1. **Its files are files in the space.** Everything it keeps that cannot be
   rebuilt is a Markdown file in the space; its cache may be deleted by the
   server at any time.
2. **Every call goes through the server, as the acting person.** The plugin
   has no other way to reach files, people, the network or other plugins.
3. **One process per plugin per space.** It starts on first call and stops
   when idle; a plugin with no code runs no process.
4. **What it sends out is declared.** Network destinations, secrets and the
   programs it starts are listed at install; an update that adds to the list
   waits for the Owner.
5. **What it offers and calls is declared.** Its operations and events, and
   every other plugin's operation it calls.
6. **It names things by ID, not by path.** Files, records and people are
   referenced by their stable IDs; paths are for people.
7. **Its interface is described**, drawn by the clients; web code only where
   description cannot express it.
8. **Risky actions go through the server's confirmation**, with a passkey
   where the action is risky (installing or widening a plugin, sharing
   outside the space, an agent level that reads others' writing).

## 4. The design

### 4.1 Spaces and people

- **Spaces are one kind.** Anyone creates as many spaces as they want, names
  them and invites anyone in their own groups; the creator is the Owner.
  Roles are reader, editor, manager and owner (ADR 0058).
- **The first space stays personal:** it takes no members. A chat between
  people lives there and is shared with its participants (T8); like every
  Markdown file it has history.
- **No private items in a shared space.** Every member may read every file
  of a space within their role; privacy means a separate space (T2).
- **Agent access is chosen when a space is created**, at one of three
  levels: **off**; **only what a person hands the agent**; **the whole
  space**. Members see the level when they join.
- **Collections are per space.** A sidebar lists collections; tapping one
  opens its items in the middle card. The sidebar's own file is not a row in
  the Collections list. The Owner and Managers set a space's default sidebar;
  each member arranges their own copy, which still picks up new default
  sections.

### 4.2 Files and records

- Items are Markdown files. Four writes: replace, set fields, append a
  record, edit or delete a record. Move, trash, restore, undo one change,
  history. A file's kind and ID are fixed at create in host state.
- **Host state keeps a keyed hash, offset and stamp per record, never the
  text** (D5). The files are the only copy of anyone's words.
- A record begins with a header the server writes, naming the author by
  person ID with the display name as link text; renderers draw the name from
  the stamp. Offsets count from the end of the frontmatter, so a title
  change moves no record. Records shorter than the delete marker are padded.
- **Only a record's author edits it** (within the plugin's own window, such
  as Chat's 48 hours). **The author, the Owner and Managers may delete it**
  (D4); a delete scrubs the words from history and search (D1).
- One file per chat, at every size (measured: append 13 ms p50 from 1 to
  256 MiB). Shrinking changes are written in place; only growing an old
  record rewrites the rest, which a kind may bound with a file-size cap.
- **Outside changes.** Writes over CalternalDAV or WebDAV are signed in, so
  they are ordinary writes by that person. A pure append of records is
  accepted. Any other change to a record file is saved as a conflict copy
  with a new ID, headers are put back from their stamps, and a changed body
  is flagged `tampered` and left out for agents. A body changed on the
  server's own disk is the administrator's problem: detected and flagged,
  restored only from a backup (D5).
- Change detection uses size, change time and inode, plus git's racy-clean
  rule (a file changed no earlier than the server's last write is
  re-hashed). Crash recovery rolls a failed append back, rolls a delete
  forward, and keeps a short-lived copy of an edit's new text until it is
  flushed.
- **Renaming rewrites the link in every file** at once (D2), destinations
  only, never link text.
- **Mixed boards** keep each member's status in the collection's file beside
  the member (D7); per-item metadata an app adds lives in `.calternal/`
  keyed by stable ID (the old app's FS1–FS6).
- A plugin may keep a rebuildable cache the server may delete at any time
  (D3).

### 4.3 Plugins and calls

- **Files free, sending out listed.** A plugin may read and change any file
  in its space that the acting person may. Network destinations, secrets
  and programs are listed at install and enforced by the server: the
  plugin's process has no network of its own, and fetches, secrets and
  program starts are server calls checked against the list.
- **Calls between plugins:** a plugin may call an operation its manifest
  lists; the called plugin acts with its own grant, limited to what the
  acting person may do (T1). Limiting a part further to what the person has
  open (the reach set) is a later tightening that plugins never see.
- **One process per plugin per space**, started on first call, stopped when
  idle (T2; measured at about 16 MiB per process, nothing when idle, 35 to
  51 ms cold start). Plugins never share a process.
- **Installing:** the space's Owner installs; a person installs an app
  bundle (one screen listing its plugins and their combined lists), and each
  plugin can be replaced later (I2, T4). Plugins update themselves; an
  update that adds a destination, secret or program waits for the Owner.
- **Slots and picks:** a part fills a slot; the person picks what only they
  see, the space picks what others receive. A missing part falls back with a
  notice, or the slot shows an error suggesting the official part. Official
  plugins are marked as such.
- **Visitors** through a share link use the plugin with exactly the link's
  rights: no secrets, no network, no agents (T12).

### 4.4 Agents

- Anything that can send data out (a run, or a plugin with listed
  destinations, secrets, programs or web code) receives other people's
  writing only as the space's agent-access level allows.
- **Agents run in hosted sandboxes first.** A member may offer their own
  machine to the whole space; members see whose machine a run is on (T5).
- **On a person's own machine** a run sees only the folders picked for it,
  read-only by default, unless the run is "direct" with "full access", which
  has what the harness asks for (T13).
- **Steering:** in a space with other members, anyone who may write in the
  chat may steer an agent there by replying to it; in a space of your own, a
  per-chat switch decides (A18).
- A run talks to the server through three calls (append to its log, read its
  inbox, end); Stop fences it at once and reaches only that run; it ends at
  its deadline or after 24 hours without the server.

### 4.5 Drawing

- Kinds declare looks; collections declare big and small views; the shell is
  slots filled by parts; one server sheet carries every decision the server
  asks of a person. A generic records view draws any record file (shown on
  the web and in SwiftUI by the drawing spike, with its changes SD1–SD11 and
  SW1–SW9 from revision 2 §4.9).
- **Web code (I1):** a plugin may ship web code. The space's Owner installs
  it after a plain warning that its pages may reach the internet in a
  browser; for those plugins the network list is not a guarantee on the web.
  The server's operator does not review plugins.
- Canvas: sideways-scrolling columns (I3), with snapping to window edges and
  a fade at the edge still to design.

## 5. One declaration, three surfaces

A plugin declares each operation and event once. The server exposes each on
three surfaces, with the same rights check on all of them:

| Surface | Operation | Event |
| --- | --- | --- |
| CLI | `calternal <plugin> <operation> …` (human output by default, `--json`) | `calternal <plugin> watch <event>` |
| API | `POST /api/spaces/<space>/calls/<plugin>/<operation>` | a subscription returning the event stream |
| MCP | the action `<plugin>.<operation>` | an MCP notification |

- **API tokens are always one person's.** A token is limited to the
  operations its person ticks and ends when the person loses access. There
  are no space tokens.
- **An outside event** (a GitHub push, a payment) is an API call made with
  such a token, so it acts as that person.
- Every call, on every surface, carries the person, the plugin, the request
  key and the space, and is checked at the one table.

## 6. Credentials

| What | How | Proven? |
| --- | --- | --- |
| A plugin's secret | The plugin calls the server's fetch with a named secret; the server attaches it to the request for a listed destination. Any service is allowed, with a warning at install for services where the secret can act in an account. | The fetch path is designed; not built for the primitives host |
| Mail | IMAP and SMTP are server calls with a named secret, under the same rule | Not built |
| A sandboxed program's credentials | The program has no network; it goes out only through the server, which adds credentials for listed destinations | **Unproven**: adding credentials to HTTPS needs the proxy to terminate TLS with a certificate the sandbox trusts |
| SSH keys | Held by the server and offered to the sandbox as a signing-agent socket; the key never enters the sandbox | **Unproven**: the socket must reach the run over the machine link; while a run holds the socket it can sign anything |
| Harness sign-in files | The stated exception: the person's login folder for a harness is mounted into that harness's runs, and the program can read it | Built in the revision-14 harness slice |

## 7. Security

| # | Threat | What contains it |
| --- | --- | --- |
| T1 | A plugin reads or writes beyond the acting person's rights, or outside its space | One check per call inside the commit; one microVM per space; paths resolved beneath the space root |
| T2 | A plugin sends data somewhere not listed | No network in the plugin's process; fetch, secrets and programs are server calls checked against the install list |
| T3 | Web code sends data out in a browser | Not contained on the web: the Owner accepts this at install, warned |
| T4 | Code that sends data out reads others' writing beyond the agent-access level | Applied at the point of delivery to anything that can send data out |
| T5 | Forged author or rewritten history | Server-held stamps and keyed hashes; only authors edit; outside changes become conflict copies and `tampered` marks |
| T6 | Changing a file's kind or ID | Fixed at create in host state |
| T7 | A run acts after Stop, or a Stop reaches a newer run | Fence at the Stop commit; Stop lives in that run's inbox |
| T8 | A machine a member offered forges calls in a run's log | Members see whose machine a run is on (open: see §11) |
| T9 | A self-updating plugin changes the code next to a login | Open: see §11 |
| T10 | Floods across spaces or accounts | Quotas per space, plugin and account, with an operator ceiling |
| T11 | A stolen session approves risky actions | Passkey confirmation (T7 answer); revoking a session revokes its consents and picks |
| T12 | A token outlives its person's access | Tokens end when the person loses access and cover only ticked operations |
| T13 | A visitor's use reaches secrets, the network or agents | Visitors have only the link's rights |
| T14 | A modified client claims a different plugin for a no-code action | The server takes the plugin from the declaration it served; the grant still bounds the action |
| T15 | Ranking or counters reveal what a reader cannot see | Queries as the asker; per-file counters; no corpus-wide ranking statistics |

Still not sure: links carry data in their URL when clicked; copies into
shared folders are not challenged; delete for everyone cannot reach
backups, offline devices or a model provider that already read a message;
timing is not addressed; phishing inside catalogue forms is reduced, not
removed.

## 8. Three worked apps

| App | How it is built |
| --- | --- |
| **Reading List** (`calternal-reading-list`, about 10 lines, no process) | A `reading` kind with `url`, `title`, `read`; "Add link" and "Mark read" declared on the kind; collections "Unread" and "All" (a board grouped by `read`). Exposed as `calternal reading-list add <url>` with no extra code. |
| **Notes** (`calternal-notes`, about 30 lines) | A `notes` kind; the catalogue's editor; links rewritten in every file on rename; tags as a grouped collection; snooze as work at a time, cancelled by key. |
| **Chat** (`calternal-chat` with `-composer`, `-ai`, `-notify` and harnesses, installed as one bundle) | One record file per chat; a chat between people in the starter's personal space; read position, draft and mute as per-person state; quotes as a record field resolved by the server; a composer slot picked per person; agents start the picked harness in a hosted sandbox, see writing as the space's agent level allows, reply through fenced calls, and can be steered by anyone who may write in the chat; notifications skip muted people. |

## 9. Build plan

| Step | Goal | State |
| --- | --- | --- |
| 0a, 0b | Files spike; drawing spike on the web and iPhone Simulator | **Done** |
| 1 | One call route, `authorize` and audit, files and records, query, Reading List as data | **Done as a spike** (`lane/spike-host`, 39016818); sign-in, sharing, consent, quotas, recovery, watcher, collections as files, live windows stubbed |
| 2 | Grow the spike into the test-mode host: hashes-only recovery and the `tampered` mark, racy-clean, offsets from the frontmatter, a watcher, collections as files, one live window; move `Folder::open_rw`, one frontmatter module and `FileId` down | Next (4–5 days) |
| 3 | Spaces of one kind, the personal first space, grants, sharing, the one check, the audit trail, person tokens | |
| 4 | Running code: plugin box per space, work table, triggers | |
| 5 | Records in full: edit, delete with scrub, conflict copies | |
| 6 | Runs in hosted sandboxes and on offered machines; restart test | |
| 7 | Agent-access levels at the point of delivery | |
| 8 | Calls between plugins, slots and picks, bundles, dependencies, the server sheet | |
| 9 | The three surfaces: CLI, API and MCP from one declaration | |
| 10 | Drawing in full, then web code with its warning | |
| 11 | Delete the native engines and v0 models | |

## 10. Decided

All dated 2026-11-21 (rounds of `docs/morning-2026-11-21.md`).

| Fork | Decision | Round |
| --- | --- | --- |
| D5 | Host state keeps keyed hashes, offsets and stamps, never text | 1 |
| T8 | A chat between people lives in the starter's personal space | 1 |
| I2 | A person installs an app bundle | 1 |
| T4 | The space's Owner installs; plugins update themselves | 1 |
| T5 | Hosted sandboxes first; a member may offer their machine to the space | 1, 2 |
| T1 | The caller lists the operations it calls; the callee acts with its own grant within the person's rights; the reach set comes later | 2 |
| T2 | No private items in a shared space; one process per plugin per space | 2 |
| A14, A15, A19 | Your sidebar is yours (sharing it to be designed); a drag moves; revoking a session revokes its consents and picks, a restore starts a new epoch | 2 |
| A7 | DAV writes are ordinary signed-in writes; raw disk edits are the administrator's problem | 2 |
| Spaces | One kind; the creator is the Owner; anyone may create spaces | 3 |
| T3 | Agent access is chosen when a space is created: off, what a person hands the agent, or the whole space | 3, 6 |
| A18 | Anyone who may write in the chat may steer an agent in a shared space; a per-chat switch in your own | 3 |
| Plugin power | Files free, sending out listed | 4 |
| First space | Stays personal, takes no members | 4 |
| Default sidebar | Only the Owner and Managers replace it | 4 |
| D6 | Collections are per space | 4 |
| T7 | Passkey confirmation for risky actions | 4 |
| D4 | The Owner and Managers may delete, not edit, others' messages | 4 |
| D1 | Delete for everyone scrubs history and search | 5 |
| D2 | The server rewrites the link in every file on rename | 5 |
| T13 | A run on your machine sees picked folders, read-only, unless "direct" with "full access" | 5 |
| D3 | A plugin may keep a cache the server may delete | 5 |
| T12 | Visitors use plugins with the link's rights only: no secrets, network or agents | 5, 6 |
| I3 | Sideways-scrolling columns | 5 |
| T9 | Any service, warned at install; the server holds and attaches secrets | 6, 7 |
| T11 | One declaration exposed on the CLI, API and MCP; tokens are one person's, limited to ticked operations; outside events are API calls with such a token | 6, 7 |
| I1 | The space's Owner installs web code after a warning; the operator reviews nothing; official plugins are marked | 8 |
| D7 | Mixed boards: status in the collection file beside each member | 8 |

## 11. Still open

| Item | Why it is still open |
| --- | --- |
| How the three agent-access levels meet per-author consent | "Only what a person hands the agent" may or may not still need each other author's yes (the earlier decision: ask each time or always, one minute); the morning report records the levels but not their wording or this interaction |
| A chat between people in a first space that takes no members | T8 puts such chats in the starter's personal space, round 4 says that space takes no members; it holds only if a per-file share to a non-member is allowed |
| Private per-person files in shared spaces | Round 2 allows no private items in a shared space, but per-person state (read positions, drafts), a member's sidebar copy and run files private to their actor are private items |
| Steering an agent that has "full access" on its starter's machine | A18 lets any writer in the chat steer it; with T13's full access that means steering someone else's machine and login |
| Plugins update themselves (T4) against approval of code next to a login (A20) | A20 was explained but not decided; a self-updating harness changes the code that reads a person's sign-in files |
| Who accepts a member's offered machine, and what it can see | Assumed the Owner or a Manager; a machine's owner can read and forge what runs on it (security review F4) |
| Web code for visitors | T12 gives visitors no network, but web code can reach the internet in a browser regardless (I1) |
| The space count per account (T10) | Assumed against the creator's account and an operator limit; not answered |
| Per-person approval of a plugin's use of their data (T6) | Not answered |
| Table A rows A1–A6, A8–A13, A16, A17, A20–A25 | Not yet confirmed one by one |
| Sharing a sidebar | To be designed (A14) |
| Credentials for sandboxed programs and SSH | Designed, not proven (§6) |
| Canvas edges | Snapping and the fade at the edge to design (I3) |
