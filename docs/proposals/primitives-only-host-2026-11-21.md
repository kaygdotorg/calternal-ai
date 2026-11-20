# A primitives-only host · integrated design · revision 2 · 2026-11-21

Status: **revision 2 of the integrated design (#107)**, after the security
review and the simplicity and authoring review (§13). Nothing here is
decided unless it says "owner decided". It integrates the coordinator's
thesis (`docs/proposals/primitives-only-thesis-2026-11-21.md`) and the
section files under [`primitives/`](primitives/); where they disagree, this
document wins and says why (§11).

| Section | File | Lane |
| --- | --- | --- |
| Files, records, questions, collections | [files-and-questions.md](primitives/files-and-questions.md) | `lane/design-files` |
| People, permissions, calls, threat model | [permissions-and-calls.md](primitives/permissions-and-calls.md) | `lane/design-authority` |
| Running code, machines, agents | [running-code.md](primitives/running-code.md) | `lane/design-run` |
| Drawing, collection views, shell parts | [drawing.md](primitives/drawing.md) | `lane/design-interface` |
| Prior art | [prior-art.md](primitives/prior-art.md) | `lane/design-priorart` |
| The author-facing API and five plugins | [authoring.md](primitives/authoring.md) | `lane/sdk-v2-authoring` (bugs fixed here) |
| Was and is, fate table, code survival | [was-and-is.md](primitives/was-and-is.md) | this lane |

## 0. Three ideas

The design is explained as three ideas. The build keeps five parts.

| Idea | What it means | Built as |
| --- | --- | --- |
| **Files, with questions** | Everything a person keeps is a Markdown file in a space, with its history and a stamp of who wrote it. The host answers questions about the files: lists, search, backlinks, unread counts. | **Files**, **Questions** |
| **Calls** | Everything that happens is a call to a named operation, checked once: who is asking, through which plugin, on what. A call can run now, later or elsewhere (on a machine), and an agent is just code that can send data out, so it only sees writing whose authors agreed. | **Authority**, **Running code** |
| **Drawing** | Plugins never draw. They declare how their things look and which lists an app starts with; clients draw that with native controls. A sealed frame covers the rest. | **Drawing** |

In one sentence: the host is a file server that knows who is asking. It
knows nothing about chats, notes, mail, agents or providers; every feature is
a plugin built from the same parts a stranger's plugin gets.

## 1. What the server was, and what it is

| Area | Was (on `dev` and in revision 14) | Is | Example |
| --- | --- | --- | --- |
| Size | About 160,000 lines of Rust, about 17,900 of them app-specific; a 17,200-line contract with about 60 concepts and 312 numbers | A file server with five parts and no app code | Chat's 1,884 lines and 1,317-line test host become about 100 lines |
| Storage | Files, plus per-app engines, spans, a viewer record, layout tables | Markdown files with four writes; host state for stamps, grants and consent | A chat is `Chats/Lisbon crew/2026-11.md` |
| Lists | Binds, sources, extractors, search profiles, layout sections | One query shape; a collection is a file holding a saved query and hand-added items | "Unread" is `{ kind: reading, where: { read: false } }` |
| Who may do what | Six audiences, activation contexts, leases, five write checks | One check per call at one table; consent per author for anything that can send data out | Claude sees Lee's messages only after Lee says yes |
| Running things | Jobs, intents, attempts, successors, controls, a spool, a relay, acknowledgements | One work table; a box; a run talks through three host calls | A run survives a server restart because its log just syncs |
| Interface | 87 description shapes, layout caps, entry views, request cards | Looks, views, slots, a closed catalogue, a sealed frame, one host sheet | A sidebar section is a collection shown small |
| App code in the host | Native chat, Notes, providers, automations | None | Notes is a plugin of about 30 lines |

The full inventory, the fate of every revision-14 name and what survives of
each crate and branch are in [was-and-is.md](primitives/was-and-is.md).

## 2. What a plugin can do, and cannot do

| A plugin can | Example |
| --- | --- |
| Declare kinds of files with typed fields, and how each looks as a row, card, cell, page and record | Reading List's `reading` kind |
| Create files and set their fields with no code at all, through its kind's write grant | Reading List's Add form and "Mark read" swipe |
| Write files: replace, set fields, append a record, edit or delete its author's own record; read history; undo one change | Notes undoes a change |
| Keep small private state for each person | Chat's read positions, drafts and mutes |
| Ask questions over anything it may read, and ship collections and a default sidebar | Notes' "Linked here"; Chat's Pinned strip |
| Share files with people who share a group with the acting person | Starting a group chat |
| Run on a call, at a time, on a repeating schedule, or when files change | Notes' snooze; a daily summary |
| Run its own program, or the one the person picked, on a machine or hosted sandbox | The Claude harness |
| Publish operations and call other plugins' operations that its grant names | A composer calls `calternal-chat/send` |
| Define slots and fill other plugins' slots | A community composer in Chat |
| Reach declared hosts with secrets it cannot read; send notifications | A mail plugin fetching mail |
| Show a sealed frame for what the catalogue cannot express | Mail's HTML body |

| A plugin cannot | Example |
| --- | --- |
| Read or write outside its grant, even when another plugin asks it to | The composer cannot read chats |
| Name a target in someone else's operation that it could not reach and the person does not have open | A composer cannot post into a chat the person left |
| Forge who wrote something, or change a file's kind or ID | Nobody can post "as Alice" or turn a chat into a note |
| Edit or delete another person's record | Chat cannot let Sam change Lee's words |
| Read another person's private state, or see people outside a shared group | Chat cannot see who muted a chat |
| Give anything that can send data out (an agent, a frame, a plugin with network) writing whose authors did not agree | A summary plugin with a frame sees only consented messages |
| Send input to another person's agent run, or start a program the person did not pick | Lee cannot steer Ana's agent |
| Run code in a client, except in a sealed frame | No JavaScript in a composer |
| Draw a host screen, hide the host's sheet, or ask for a password | Sharing always shows the host's sheet |
| Keep running after Stop, or exceed its storage, compute, call, wake-up and egress quotas | A looping trigger is cut off |
| Reach the network except its declared hosts, or private addresses at all | The harness only reaches its model provider |
| Claim an operation or kind name another plugin owns | A second `chats` kind fails at install |

## 3. Owner decisions this design is bound by

All from `docs/morning-2026-11-20.md` on `dev`, 2026-11-20 ("Change of
focus", "First grilling answers", "Collections and sidebars", and earlier
sections):

- The server is primitives only; app logic is plugin logic. Disabling an
  official plugin that provides important functionality warns the person.
- One plugin may call another plugin's operations.
- Userdata is strictly files. Appdata is rebuildable and uses whatever is
  fastest.
- Drawing is a closed component catalogue plus a sealed frame.
- The host enforces agents reading other people's writing.
- A sidebar section is a collection shown small. A collection is a saved
  query over items of any kind plus hand-added items, with big and small
  views. Each kind says how it looks as a row, card and cell. An app is
  kinds, operations and a default arrangement of collections. A sidebar is a
  collection of collections, stored as a visible file. The middle is a
  canvas of windows.
- Apps are made of several plugins with declared dependencies. Who picks a
  part is split by effect. A missing part falls back, or shows an error that
  suggests the official part.
- Renaming must update every Markdown link at once and cheaply (evening
  decisions).
- A deleted message vanishes with no trace in the interface; editing is
  allowed for 48 hours; read position, draft and mute are per person;
  consent is per conversation ("ask each time" or "always"), and after one
  minute without an answer that person's messages are withheld.
- Spaces by UUID; roles are reader, editor, manager and owner; rusqlite only;
  Markdown files; official plugins are named `calternal-<name>`.

## 4. The five parts

| Part | What it does | The security property it guarantees |
| --- | --- | --- |
| **Files** | Items are Markdown files in a space. Four writes: replace, set fields, append a record, edit or delete one's own record. Move, trash, restore, undo one change, history. A kind is a schema over frontmatter (and for record kinds a record shape). A file's kind and ID are fixed at create in host state. | The host stamps every write (person, plugin, run) in host state, where no plugin, share or editor can change it. A record file accepts outside byte changes only as pure appends of host-formatted records; anything else becomes a conflict copy. Paths stay inside the space. |
| **Questions** | One index (rebuildable) over fields, text, links, records and stamps. One query form with live windows. A collection is a file: a saved query, hand-added items and optional grouping. Backlinks, search, unread counts and the change feed are the same engine. | Every answer, count and rank is computed as the asker, over what the asker may read. A collection grants nothing. |
| **Authority** | People, groups, spaces, roles and sharing as the existing app settled them (ADR 0058–0060). A plugin's grant. One call table from name to handler for host methods, the outside (network, secrets, notifications) and every plugin's operations. The reach set, consent, dependencies, slots and picks, quotas, the audit trail. | One check per call: who is asking, through which plugin, on which target. Anything outbound-capable receives other people's writing only with their consent. |
| **Running code** | One per-space work table with a state column. Work runs in a box (a plugin process, a hosted sandbox, or a person's enrolled machine), started by a call, a time (once or repeating) or a change. A program uses three host calls: append to its log, read its inbox, end. | A box has nothing but its grant. Stop fences a run at once and reaches only that run. Run files are private to the run's actor; only the actor may send input. Nothing outlives its deadline or the 24-hour rule. |
| **Drawing** | Plugins declare looks, views and slot parts; clients draw them natively. A closed catalogue (including a generic records view), a sealed frame, and one host sheet for every decision the host asks of a person. | No plugin code runs in a client outside the frame, and a plugin with a frame counts as outbound-capable. A plugin cannot draw or hide a host screen; its forms carry a header naming it, and there is no password-like field. |

### 4.1 Storage

| Class | What | Rebuildable | Example |
| --- | --- | --- | --- |
| Userdata (files) | Items, collections, sidebars, per-person state, attachments | No: it is the truth | `.people/<uuid>/calternal-chat.md` holds Lee's read positions |
| Host state | Stamps and history, each file's kind and ID, receipts, grants, consent, approvals, picks, sign-in, the work table, the audit trail | No | "Lee agreed that Claude may read Lee's messages in Lisbon crew" |
| Appdata | Index, search, link table, caches, live windows | Yes, from userdata plus host state | Unread counts |

Host state is the same class as `auth.db` and history are today (ADR 0062:
"durable state, not rebuildable"). It is listed for confirmation (A1).

### 4.2 One vocabulary

The sections used different names for the same thing. These are the names
from now on; the authoring prototype is corrected to them.

| Thing | The one name | Not |
| --- | --- | --- |
| Change or remove one's own record | `record.edit`, `record.delete` (delete for everyone; scrubs history and search) | redact, replace, tombstone |
| A row in the work table | **work** | unit, job, intent |
| Work that executes a program | **run** | attempt, execution |
| What a run executes | **program** (a plugin that ships one is a **harness**) | entry, image |
| Where code executes | **box**: a plugin process, a **hosted** sandbox or a **machine** | sandbox, place |
| Code that can send data off the server | **outbound-capable** (people see "can send data out") | agent flag, marked chain |
| A saved list | **collection**; how it is drawn is a **view** (big or small); a collection in a sidebar is a **section** | facet, source, list, window |
| A person's rights / a plugin's rights | **rights** / **grant** | capability, scope |
| An author's yes for outbound-capable code | **consent** | ask, permission |
| Which part fills a slot | **pick** | choice, binding |
| What a cross-plugin call may name | **reach set** | handle, designation token |
| Per-person private values | **state** | viewer record, me namespace |
| A record kind's read position | the state key `read.<file>`; queries use `unread` | position, after_position |

**One query form.** A query is an object, written the same in a manifest, a
collection file's frontmatter and the SDK:
`{ kind: "notes", where: { snooze: null }, sort: "-updated", group: "tags" }`.
`where` maps a field to a value, `{ gt: … }`, `{ in: [...] }` or `null`.
The string shorthand in the authoring prototype is only notation.

### 4.3 The reach set (replaces handles and the designation rule)

When plugin A calls plugin B's operation and names a target (a file, a
record), the target is valid only if:

1. A could reach it itself, by its grant and the person's rights; or
2. it is in the **reach set** now: for a person's session, the items the
   person has open by their own action; for a run, the items named as its
   targets in its start input, until Stop.

Slot targets are not named at all: the host fills them (the composer's
target is the open chat). There are no tokens, keys, expiry or replay
window.

Checked against the security findings:

| Finding | Closed? | Why |
| --- | --- | --- |
| F10: a composer posts into chat A after the person moved to chat B, or from an alarm later | **Closed** | The host fills the slot target; A is no longer open; an alarm has no session, so only rule 1 applies, and a composer has no grant on chats. |
| F9: a run posts into chats beyond its own, or after Stop | **Closed** | Only items named as the run's targets enter its reach set, and the set empties at Stop; replies go through the run's fenced calls. |
| F13: a client or plugin names a program the person did not pick | **Closed** | The host resolves the program from the pick; a plugin may start only its own programs. |

**Left open, stated plainly:**

- "Open" must mean opened by the person's own gesture (a click on a host
  list, a link, a search result). If a plugin could navigate the person to
  an item, that item would enter the reach set. Rule: navigation a plugin
  asks for does not add to the reach set until the person acts in that
  window.
- Items the person has open in other windows or devices are all in the set.
  A plugin other than the slot's part could name any of them in a call it
  makes for the same person. The effect still needs the person's rights and
  the callee's grant, so the residual risk is a wrong but permitted target.
- A run's start input must separate **targets** (where it may act) from
  **reads** (what it is shown); only targets enter the reach set.

### 4.4 Consent at the point of delivery

**Outbound-capable** is computed at install as a closure: a plugin is
outbound-capable if its grant has network, secrets or programs, if it may
start any program, if it calls any outbound-capable operation, or if it
declares a frame. A run is always outbound-capable.

**The rule:** anything the host delivers to an outbound-capable plugin or
its run (file reads, records, query results, search, counts, the change
feed, call results) leaves out other people's writing unless each author
has consented, or is the acting person. There is no chain to carry. For a
plain (non-record) file, every person in its revision stamps counts as an
author. Results carry one `omitted` count instead of a marker per record.

Consent is host-held, set only from the host sheet, per conversation:
"ask each time" or "always". After one minute without an answer the
author's writing is left out (owner decided).

**Two leaks this does not close by itself:**

- **Recall across people (G1, F15).** A plugin process that served Lee may
  answer Ann from memory.
- **Laundering through files (G2).** A plugin that is not outbound-capable
  reads Lee's message for Ann and writes it into a file that an
  outbound-capable plugin later reads as Ann's.

The mechanism for both is owner question T2. The recommended answer is
**one worker process per plugin and acting person**, stopped when idle, with
its memory cost waiting on the #103 measurement. The alternative is **taint
per process**: every write by a process that was handed other people's
writing is stamped as carrying those authors, and is left out for
outbound-capable readers.

### 4.5 Record files

- A record file is a Markdown file of a record kind. Each record begins with
  a header line the host writes (`**Lee** · 2026-11-20 17:02 ^anchor`);
  the stamp lives in host state.
- One host rule, not per-kind knobs: **a record's author may edit or delete
  it** (edit within the plugin's own window, such as Chat's 48 hours).
  Whether managers may also delete others' records is owner question D4.
- Changes from outside (a file share, another editor, a `replace`): a pure
  append of host-formatted records is accepted and stamped with the writer;
  **any other byte change becomes a conflict copy** beside the file. The
  record file stays as it was. No per-record diff is needed: the check is
  "are the old bytes a prefix of the new ones".
- Edit in place: the file shows the latest text; old versions live in
  history, shown to the author. Delete for everyone leaves `*deleted*` and
  scrubs history and search.
- Long files: whether record files are split into monthly segments waits
  for the large-file measurement (question D5).

### 4.6 Runs

A run talks to the host through three calls, all checked like any other:

| Call | Who | What |
| --- | --- | --- |
| `log.append(record, seq)` | the run's machine | Append output or a call request; compare-and-set on the sequence number, so a replay lands once. Call requests in the log are executed as fenced calls of the run. |
| `inbox.read(after)` | the run's machine | Read input, call results and Stop after a sequence number. |
| `end(how)` | the run's machine | Record the end. |

`work.send` (input to a running agent) is accepted only from the run's
actor; anyone who can read the target may Stop it. Run files live under
`.people/<actor>/runs/` and are indexed only for the actor. When the server
is away, the machine keeps writing its log; when it returns, each side
sends what the other lacks. A message sent meanwhile is in the inbox once;
Stop lives in that run's inbox, so it cannot reach a newer run.

### 4.7 Slots and the host sheet

- A **slot** is a named operation signature plus a picker. It declares only
  `effect` (viewer or others, which decides who picks) and `recommended`
  (the official part suggested when none is picked). What the part receives
  and may call is derived from the signature.
- **One host sheet** draws every decision the host asks of a person:
  installing or widening a plugin, consent, sharing (the challenge),
  picking a part, and Stop. One data shape (who asks, what for, the buttons)
  and one implementation per platform.

### 4.8 Testing and checking

- Plugin tests run against **the real host binary in a test mode**: a
  temporary directory, a fake clock and scripted people. There is no second
  fake host to drift from the real one.
- `calternal check` runs the installer's own validator and prints what the
  owner will see: the install sentence and the derived "can send data out"
  word, so an author learns the cost of a grant before shipping.

### 4.9 Drawing, as tested by the spike

The [drawing spike](https://git.kayg.org/kayg/calternal-ai/src/commit/bd880283/docs/reviews/drawing-spike-2026-11-21.md) (`bd880283`) drew Reading List (sidebar, list, board, the Add form,
Mark read) and the Chat sidebar and transcript (own side, authors, agent
mark, grouping, date divider, unread line, a quote, edited, a deleted
record, paging, composer, actions) from the same declarations, with a
Svelte renderer and a SwiftUI renderer in the iPhone Simulator. About 22
lines of declaration for both apps; about 650 lines of generic renderer
per platform; no app-specific drawing code. It holds with these changes,
which are now part of the design:

**Declarations**

| # | Change |
| --- | --- |
| D1 | A kind's row action carries a title, icon, `when` and placement: `actions: { done: { set: { read: true }, title: "Mark read", icon: "check", when: { read: false }, place: "swipe" } }`. These are interface declarations over the kind's write grant, not operations. |
| D2 | A kind's create form has a title: `create: { title: "Add link", inputs: ["url", "title"] }`. |
| D3 | Fields carry a label: `url().label("Link")` (and later translations). |
| D4 | Fields with few values name them: `bool(false).values({ false: "To read", true: "Read" })`; a `choice([...])` field does the same for other types. Boards and group headings show these names. |
| D5 | A collection may `group` by a field; a board needs it. |
| D6 | **A quote is a field of the record** (`quote: link()` to the quoted record's anchor), drawn by the record look's `quote` place, not a link at the start of the body. |
| D7 | An action may set `defaults` (a form's starting values, such as the record's text for Edit) beside fixed `inputs`. |
| D8 | **Choosing an agent's harness is a pick among the parts filling a slot** (`pick: { harness: "calternal-chat-ai/harness" }`), never a typed field. |
| D9 | **A composer part says which of its values feeds each input of the operation it calls**: `inputs: { text: "draft", quote: "quote" }`. The target is filled by the host (§4.3). |
| D10 | Every collection a package declares has an `id`; sidebars link collections by `<plugin>/<id>`. |
| D11 | A look place may list fallbacks, the first non-empty wins: `title: ["title", "people"]` (a one-to-one chat has no title). A list, not an expression. |

**Wire format and generic components**

| # | Change |
| --- | --- |
| W1 | Values carry their type: an instant is `{ "at": "…Z" }`, a person list is `{ "people": [{ id, name, me }] }`; other values are JSON scalars. The client formats times in the viewer's zone. |
| W2 | A window carries exactly the values the looks and their `when` conditions name, keyed by those names (including derived ones such as `site(url)`, `me.unread`), plus the collection's `group` field. This is how a list draws while its plugin sleeps. |
| W3 | A records window carries the viewer's position, the unread count and pages with a cursor for paging from the bottom. |
| W4 | **The host sends quoted records already resolved for the viewer** (`refs: { anchor: { author, excerpt } }`, or `{ gone: true }` for a deleted one), as plain text. The client never fetches another person's record to draw a quote. |
| W5 | **A stamp carries the run's display name and icon** (`run: { name: "Claude", icon: "claude" }`), so "Claude for you" draws without the plugin. |
| W6 | The viewer's picks and a slot's eligible parts are sent as values, for `pick:` inputs. |
| W7 | **A closed icon list**, mapped per platform (the web icon set and SF Symbols); a declaration naming an unknown icon fails at install. |
| W8 | A mention in a body is a Markdown link with the `person:` scheme (`[@Ana](person:p-ana)`); the composer inserts it from its people picker, and the renderer draws it as a mention. |
| W9 | **Order in declarations is meaningful** (field order is form order, action order is menu order). The wire format uses arrays for ordered things, because Swift's JSON decoders lose object key order. |

## 5. Security

One threat table for the whole design. The section files give the detail.

| # | Threat | What contains it | Part |
| --- | --- | --- | --- |
| T1 | A plugin reads or writes beyond its grant | No ambient authority in the box; every effect is a call checked against the grant, the person's rights and the target, inside the commit; paths resolved beneath the space root | Authority, Files |
| T2 | Confused deputy: plugin A gets B to act on a target A could not reach | The reach set (§4.3); the host fills slot targets | Authority |
| T3 | Forged author or rewritten history (replace, share edits, header removal, undo) | Host-held stamps; only the author edits or deletes a record; outside changes that are not pure appends become conflict copies | Files |
| T4 | Changing a file's kind or ID to escape its rules or take over another file's shares | Kind and ID are fixed at create in host state; a mismatching frontmatter copy loses | Files |
| T5 | Anything that can send data out receives others' writing without consent (a run, a program started by another plugin, a frame, a plain file several people wrote) | Outbound-capable closure; consent at the point of delivery; every contributor of a plain file counts | Authority |
| T6 | Recall across people, or laundering through files | Owner question T2: one process per plugin and person, or taint per process | Authority |
| T7 | Learning hidden rows from counts, ranks, backlinks or the change feed | Queries as the asker; ranking without corpus-wide statistics; per-file revision counters; push only when the visible result changes (A17) | Questions |
| T8 | A run acts after Stop, a late Stop hits a newer run, or output after Stop is posted | Fence at the Stop commit; Stop in that run's inbox; the reach set empties at Stop; replies only through fenced calls | Running code |
| T9 | Another person steers someone's agent (and so their login and files) | Only the actor may send input | Running code |
| T10 | The whole space reads a run's log | Run files under `.people/<actor>/runs/` | Running code |
| T11 | A machine's owner forges calls in a run's log | Owner question T5 (hosted sandboxes for shared spaces; otherwise run rights ∩ machine owner's rights) | Running code |
| T12 | Replayed or injected output; a machine claims another's run | Sequence numbers; machine key; placement column; a restore starts a new epoch | Running code |
| T13 | A program outlives a dead server or a lost machine | Machine-side deadline and 24-hour rule; host mirror ends the run | Running code |
| T14 | Floods: alarms, watches, calls, wake-ups, change loops, disk; work spread over many spaces | Quotas per space, per plugin and per account; coalescing; own writes do not fire own watches; depth limit; operator ceiling with fair share | Running code |
| T15 | Exfiltration over the network | Declared destinations only; private addresses refused at resolve, connect and redirect | Authority |
| T16 | A secret yields an equivalent credential | Plugin code never holds the value and may not set other auth headers on its destination; multi-tenant destinations flagged at install (question T9); echo filtering is only a cheap extra | Authority |
| T17 | Login theft, including by a changed entry script in an approved image | Login folders only in their owner's runs; approval binds the plugin digest and the image digest | Running code |
| T18 | A malicious update widens what a plugin can learn | On any update the host recomputes every plugin's derived install sentences; any that grow wait for approval; consent records the harness digest | Authority |
| T19 | A stolen session approves risky things | Passkey question T7; revoking a session revokes the consents and picks it made | Authority |
| T20 | A restore undoes revocations | New epoch: sessions invalidated, standing consents suspended until reconfirmed | Authority |
| T21 | A plugin floods the audit trail to hide its reads | Audit retention per plugin; reads of others' writing by outbound-capable code keep full retention | Authority |
| T22 | Spoofed host screens, clickjacking, phishing forms | One host sheet in the shell's own layer with the host mark; frames clipped and below it; plugin forms carry a header naming the plugin; no password-like field | Drawing |
| T23 | A frame sends out what it shows | A plugin with a frame is outbound-capable, so it receives only consented writing; the promise is "a frame can read what its plugin can read" (question I1) | Drawing |
| T24 | A broken shell part locks the person out | Profile, Settings, Plugins and safe mode are host-only and always reachable | Drawing |
| T25 | One space starves another | Per-space queues and writers; no installation-wide lock on the request path | Running code, Files |

### 5.1 Where the design is not sure it holds

| From | Doubt |
| --- | --- |
| Authority G1, G2, F15 | Recall across people and laundering through files (question T2) |
| Authority G3, G6; Files 5 | Plugin-built indexes and timing leak existence |
| Authority G4 | Links carry data in their URL when a person clicks (accepted) |
| Authority G5 | Copies into shared folders are not challenged |
| Authority G7 | Device copies survive removal from a space until the device signs in |
| Authority G8; Files 2 | Delete for everyone cannot reach backups, offline devices or modified clients |
| Authority G10; Running code 19 | Delete for everyone cannot reach a run that already read the message, or its model provider |
| Running code 13 | An unapproved harness on the caller's own machine can send what it was given to its author (by design) |
| Running code | The change-chain depth of 4 and 50 ms machine batching are chosen, not measured; V8 isolates are probably not a security boundary |
| Drawing 1 | Frames cannot be cut off the network in browsers; `WKWebView` unproven |
| Drawing 2 | Guests with title-only access: the index must refuse fields, not the renderer |
| Drawing 4 | Phishing inside catalogue forms is reduced, not removed |
| Files 1 | Direct disk edits are stamped `outside` and left out for agents, including the owner's own |
| Files 3 | A rename shows the new name in linking files the reader can no longer open |
| §4.3 | The reach set's "opened by the person's own gesture" rule must be enforced by every client |

## 6. Three worked apps

Sizes are from the authoring prototype and the drawing section; they were
not recounted after the fixes.

### 6.1 Reading List: a kind and a sidebar

| Part | How |
| --- | --- |
| Kind | `reading`: `url`, `title`, `read`. Row: title and site; card: title, site, image. |
| Writes | No operations. The kind's write grant lets the interface call the host's create and set: a create form titled "Add link" (D2) and a "Mark read" swipe shown only on unread items (D1). Fields carry labels ("Link", D3). |
| Sidebar | Collections "Unread" (`{ kind: reading, where: { read: false } }`) and "All", which can be drawn as a board grouped by `read` with columns "To read" and "Read" (D4, D5). Drawn on the web and in the iPhone Simulator by the drawing spike. |
| Plugin | `calternal-reading-list`, about 10 lines; no box ever starts. |

### 6.2 Notes

| Part | How |
| --- | --- |
| Kind | `notes`: Markdown with `tags` and `snooze` in frontmatter. |
| Editing | The catalogue's editor bound to the file; writes with an expected revision. |
| Links | Ordinary relative links that follow renames (question D2). |
| Tags | A collection grouped by `tags`; "Linked here" is a backlinks query. |
| Snooze | Work at the snooze time with a key per note; unsnooze stops it by key. |
| History | History and undo of one change. |
| Plugin | `calternal-notes`, about 30 lines. |

### 6.3 Chat

| Part | Plugin | How |
| --- | --- | --- |
| The chat | `calternal-chat` | A record kind in `Chats/<name>/`. Each message is a record; only its author edits (48 hours, Chat's rule; the Edit form starts from the record's text, D7) or deletes it for everyone. A one-to-one chat's row falls back from its title to its people (D11). |
| Unread, draft, mute | `calternal-chat` | Per-person state; `read.<chat>` drives `unread`; `muted.<chat>` makes the host drop pushes for that person. |
| Sidebar | `calternal-chat` | Pinned (a strip view of a personal collection), Groups, People, Cals, Harnesses, Archived; each a collection. |
| Quotes | `calternal-chat` | A `quote` field of the record holding the quoted record's anchor (D6). The host sends it resolved for the viewer (W4); a deleted message shows as gone wherever it is quoted. |
| Composer | `calternal-chat-composer` | Fills the composer slot (picked per person); declares which of its values feeds each input of `send` (D9); inserts mentions as `person:` links (W8); the host fills the target. |
| Agents | `calternal-chat-ai` + `calternal-chat-claude-code` | Outbound-capable, so they receive only consented writing. `chat-ai` starts the harness the person picked from the parts filling its slot (D8, W6); the host asks other authors and resolves the start after the consent freeze, and only then is the prompt posted. The run's target is the chat; its reads are the selected records. Replies are fenced calls from the run, drawn with the run's name and icon from the stamp ("Claude for Ana", W5). Only the actor may steer it; anyone in the chat may Stop it. |
| Notifications | `calternal-chat-notify` | Watches changes, notifies mentioned people; picked per space; the host drops pushes for muted recipients. |
| Restart | host | The log syncs when the server returns; inputs arrive once; Stop reaches the exact run. |

### 6.4 What an author learns

Fifteen concepts: **space, person, file, record, kind (with its looks),
field, collection, query, state, operation, call, grant, work (and runs),
slot, part.** The SDK surface shrinks with the cuts in §13.2: no facets,
no declarative operations, no record-policy knobs, no handles, no request
IDs, no withheld markers and no batching to know about.

## 7. Build plan

Two spikes first, then steps that each end with a test on the running
server. Estimates are rough.

| Step | Goal | Test | Estimate |
| --- | --- | --- | --- |
| 0a | **Files spike**: one hardcoded kind; one file written through the host with a stamp in host state; the actor and request in the call signature from the first call; a directory listing as the query | A test against the real binary in test mode writes a file, reads it from disk and reads its stamp | 1 day |
| 0b | ~~Drawing spike~~ **Done** ([drawing spike](https://git.kayg.org/kayg/calternal-ai/src/commit/bd880283/docs/reviews/drawing-spike-2026-11-21.md)): Reading List and the Chat sidebar and transcript drew from the same declarations on the web and in the iPhone Simulator, with the changes in §4.9 | — | done |
| 0c | **Next drawing step**: what the spike did not draw: live updates while open (a record arriving, "1 new"), the consent sheet, the running-work line and Stop, typing, attachments, mention autocomplete, multi-record selection, web swipe and board drag, iPad and Mac layouts, table, gallery and calendar views, Notes' editor page; then a device check of motion, VoiceOver and keyboard | Each drawn from declarations on web and native; motion checked on a real device | 1 week |
| 1 | Install a real plugin through the one validator; one call route; the index answers a collection | Reading List installs, its Add form creates a file, "Unread" shows it | 2 days |
| 2 | Questions: live windows, collections as files, grouping, backlinks | Edit a file by hand; the window updates; delete the index and it rebuilds the same | 3–4 days |
| 3 | Authority: grants, sharing, the one check, the audit trail | A second person and a guest see only what they may, in results, counts and the feed | 3 days |
| 4 | Running code: plugin box, work table, the three triggers, repeating work | Notes snooze fires once across a restart; unsnooze cancels it; Stop ends a worker | 3–4 days |
| 5 | Record files: append, edit, delete, conflict copies | A share edit that is not an append becomes a conflict copy; delete scrubs history and search | 2–3 days |
| 6 | Runs on machines | Kill the server during a harness run; output arrives once and in order; input sent meanwhile arrives once; Stop reaches the exact run | 4–5 days |
| 7 | Consent at delivery, outbound-capable closure | A frame plugin and a run see only consented authors; a revoke stops further delivery | 2 days |
| 8 | Calls between plugins, the reach set, slots and picks, dependencies, the host sheet | The composer posts only into the open chat; uninstalling a picked part falls back with a notice; disabling official Chat warns | 2–3 days |
| 9 | Drawing in full: catalogue, looks, views, shell slots | Reading List, Notes and Chat draw on web and native | 1–2 weeks |
| 10 | Delete the native engines and v0 models | The workspace builds without them; the three apps pass end to end | 2 days |

**Measurements that gate decisions** (each decides the named question):

| Measurement | Decides |
| --- | --- |
| Append, edit and delete-with-history-scrub on a 64 MiB record file | D5: if cost is flat in file size, there are no segments |
| ~~Plugin process memory and cold start (#103)~~ **Done**: about 16 MiB per process, nothing when idle, 35 to 51 ms cold start cached; shared runtime rejected | T2 (recommendation stands), A23, operator defaults |
| Still unmeasured from #103: the product's in-guest sandbox (seccomp, user separation), the real broker, and several spaces waking at once | Space wake budget and the per-account ceiling (T10) |
| A live window's save path against the 40 ms gate without stored row sets | Step 2's window design |
| The warm path of the one check (target: no disk reads) | Step 3 |
| Change-chain depth and machine batching under load | Running-code limits |
| The kill-the-server run test | Step 6 |
| Drawing from declarations on web and SwiftUI | **Done** by the drawing spike (§4.9); not yet checked on a device |
| Whether V8 isolates can be a boundary | Answered for worker threads by #103: no (a worker thread read files outside its permissions); no design depends on it now |

## 8. Was and is

The table in §1 is the short form. [was-and-is.md](primitives/was-and-is.md)
has the inventory by area with line counts, the fate of every revision-14
operation, host method, route, rule block, manifest field and limit group,
and what survives of each crate and lane branch. Its build plan is
superseded by §7.

## 9. Questions for the owner

For a person with twenty minutes: first confirm a table (A), then the real
forks (B), grouped Trust, Data and Interface. Operator defaults (C) need no
answer.

### A. Confirm: decisions that follow from your own principles

Accepted unless you object.

| # | Decision | Reason |
| --- | --- | --- |
| A1 | Host state (stamps, grants, consent, approvals, picks, sign-in, work) is a store beside the files, backed up with them, with a read-only Markdown copy for people to read. A file's kind and ID are host-state facts. | It is the class `auth.db`, history and sharing already are (ADR 0062); a file a share or plugin can edit cannot hold who agreed to what. |
| A2 | Every plugin, official or not, always runs in the sandbox. | First-party plugins are not special. |
| A3 | A generic records view (authors, times, edits, deletes, quotes) is in the host's catalogue. | Native system controls first. **Shown to work** by the drawing spike: one generic records component per platform drew Chat's transcript on the web and in SwiftUI with no chat words. |
| A4 | GenUI is deleted; agents show forms through declarations, anything richer in a frame. | One description language; closed drawing. |
| A5 | Agent replies are fenced calls from the run, never posted by a trigger on its log. | Nothing a run writes after Stop takes effect. |
| A6 | A message to a running agent is stored once; "arrived" follows the program's own acknowledgement where it has one; the rare helper-crash window is accepted. | Matches bb 0.50.0's "kept, not lost or misapplied". |
| A7 | Edits made directly on the server's disk are stamped `outside` and left out for agents until someone re-saves them in Calternal. | Disk access is administrator trust; it must not consent for anyone. |
| A8 | Layout limits disappear; resource limits become operator defaults; security limits stay fixed (request size, run deadline maximum, grant expiry maximum, frame size and depth, chain depth, consent-ask rate). | "As dumb as possible"; only security needs fixed numbers. |
| A9 | No declarative operations: a kind's write grant lets the interface create files and set fields directly. A slot reaches the picked part; the host resolves the pick. | One way to write; the host never lets a caller name a program. |
| A10 | The host drops a push when the recipient muted the item (in the kind owner's state). | Mute is per person; the sender's plugin cannot read it. |
| A11 | One link form on disk: imported `[[wiki links]]` are converted to standard links on import. | ADR 0024. |
| A12 | Typing indicators wait until Chat needs them. | Nothing short-lived is stored. |
| A13 | A missing picked part falls back with a notice for parts only you see; for parts others receive (AI, notifications) the slot shows an error suggesting the official part. | Already decided: picking is split by effect. |
| A14 | Your changes to an app's sidebar are yours alone: a copy in `.people/<you>/`, shown to you as "My sidebars", that still picks up new default sections (appended with a notice); "Share sidebar" puts a copy into the space. | What only you see is yours; the sidebar stays a visible file. |
| A15 | A hand-placed item appears once per sidebar: dragging a chat into "Lisbon crew" moves it out of People. | A drag then always moves, as folders suggest. |
| A16 | Content state (sidebar order, a collection's chosen view) syncs; screen state (widths, open windows, card size) stays on the device. | Synced state costs writes; you already decided sidebar width is per device. |
| A17 | Search ranks without corpus-wide statistics; revision counters are per file; live updates are pushed only when the viewer's visible result changes. | Queries are answered as the asker. |
| A18 | Who may send input to a run: only its actor; anyone who may read the target may Stop. | The run carries the actor's login and files. |
| A19 | Revoking a session revokes the consents and picks it made; a restore from backup starts a new epoch (sessions invalidated, standing consents suspended). | Revocation must not be undone. |
| A20 | Approval binds a plugin's digest and its program's image digest; the host recomputes every derived install sentence on any update. | The code next to a login is what must be approved. |
| A21 | Record files accept outside changes only as pure appends; anything else becomes a conflict copy. | Only authors change records. |
| A22 | Request IDs are derived by the host; authors never write them. | Retries stay exact without author bookkeeping. |
| A23 | No shared runtime per publisher: the earlier agreement to try one is answered by measurement. | [#103 measurement](https://git.kayg.org/kayg/calternal-ai/src/branch/lane/plugin-memory/docs/measurements/plugin-split-memory-2026-11-20.md): worker threads in one runtime saved only 10 MiB at seven plugins and cost more at one or two; calls took about twice as long; a native crash or the OOM killer took every plugin in the runtime; one plugin wrote a frame the host accepted as another's reply; and in Node 24.21.0 a worker thread could read any file below the process's current directory regardless of its permissions. |

### B. The real forks

Each stands on its own: what it is, why now, a concrete example, what each
option costs, and the recommendation.

#### Trust

**T1. When one plugin calls another, whose rights apply?**
*What:* a composer you installed asks Chat to post your message. Either the
composer itself must be allowed to write chat files ("strict"), or it may
only ask Chat to post, and Chat writes with its own rights, for you, after
the host checks that you may post there and that the target is something
the composer could reach or that you have open ("delegated, with the reach
set").
*Why now:* every app made of parts, and every install screen, depends on it.
*Example:* under strict, a community composer's install screen says "Can
change your chats" and it could rewrite any message. Under delegated, it
says "Can send messages through Chat" and can post only into the chat you
have open.
*Options:* (a) **strict intersection** (the thesis, prior art). There is no
deputy problem. Cost: the less trusted plugin gets more raw power, and
grants grow to everything a plugin touches through others. (b)
**Delegated, with the reach set.** Cost: one host rule; a callee that builds
paths from plain strings is still a buggy deputy, which the SDK
discourages. In both options the caller lists every operation it calls, and
you see that list at install.
*Recommendation:* (b).

**T2. How is one person's data kept from reaching another through a plugin?**
*What:* a plugin process serves everyone in a space. The call check limits
what it fetches, not what it remembers or writes into files.
*Why now:* it decides the memory cost of every plugin, and it closes the
last two consent leaks (recall and laundering).
*Example:* a summary plugin read Lee's private note while serving Lee;
asked by Ann, it repeats a line from it, or it wrote the line into a file
that Ann's agent then reads.
*Options:*
- (a) **One worker process per plugin and acting person**, stopped when
  idle. Airtight for recall. Cost, measured ([#103 measurement](https://git.kayg.org/kayg/calternal-ai/src/branch/lane/plugin-memory/docs/measurements/plugin-split-memory-2026-11-20.md)): about 16 MiB of
  guest memory per active person per code-carrying plugin while in use
  (15 to 17 MiB, about 11 MiB of it Node itself), nothing when idle; a
  stopped process's first call takes 35 to 51 ms p50 with files cached
  and 86 to 157 ms uncached. Laundering through files still needs (b) on
  top, unless the process is never handed others' writing.
  *Worked example:* a five-person space where Chat is active for three
  people. Chat core owns `chats`, so it keeps one shared process: 16 MiB.
  `calternal-chat-ai` and `calternal-chat-notify` read a kind they do not
  own, so each gets one process per active person: 2 × 3 × 16 = 96 MiB.
  Total about 112 MiB while all three are in use, against about 48 MiB
  with one process per plugin; when they go idle, 0. The two idle members
  cost nothing.
- (b) **Taint per process.** Every write by a process that was handed
  other people's writing is stamped as carrying those authors, and is left
  out for outbound-capable readers. Cost: an ordinary Chat process smears
  taint (Sam's message "carries Lee" after it served Lee), so more is left
  out than needed.
- (c) Trust installed plugins (revision 14's rule). Cost: no protection.

*Recommendation:* (a) for every plugin that reads kinds it does not own,
with (b) limited to processes of such plugins. **It stands after the
measurement:** the extra cost is about 16 MiB per active person per such
plugin, only while in use, and the price is one cold start (35 to 51 ms
cached) per person after each idle stop.

**T3. What counts as "an agent", and which writing is protected?**
*What:* the host treats anything outbound-capable as an agent: a run, or a
plugin with network, secrets, programs, a frame, or calls to such plugins.
The rule must also say which files are protected.
*Why now:* it decides whether agents can search a team's notes, and whether
link previews or translators work in chats without consent.
*Example:* in your Family space you ask Claude "find the notes about the
lease"; one note was written by your partner.
*Options:* (a) protect chats only (kinds opt in). Cost: protection depends
on each plugin. (b) Every file another person wrote in. Cost: agents in
group spaces see almost nothing without many prompts. (c) **(b), except in
spaces whose owner turned on "members' agents may read members' writing
here"**, shown to everyone at join, applying only to writing made after a
member saw it. Chats between people and files shared one by one stay
protected.
*Recommendation:* (c), with the outbound-capable closure.

**T4. Who approves plugins, how do updates work, and where may plugins come from?**
*What:* who may install, whether an update with the same rights installs
itself, and which sources are allowed.
*Why now:* it is the line between a bad member and a bad plugin, and
between a private tool and a marketplace.
*Example:* a community composer updates itself; with the same rights it
could start garbling messages.
*Options:* installs by (a) **the space owner only** (managers lose
`plugins.manage`) or (b) managers too. Updates: (a) same-grant updates
install themselves; (b) every version needs approval; (c) **every new
version of an outbound-capable or slot-filling plugin needs approval;
others update by themselves within the same grant.** Sources: (a) **an
operator-pinned list for now**, designed so signed publishers with review
can follow; (b) any signed publisher; (c) anyone.
*Recommendation:* owner approves; updates (c); sources (a).

**T5. Whose machine runs an agent in a shared space?**
*What:* a machine sees everything its programs see, and can forge calls in
a run's log.
*Why now:* it decides whether the space owner can read, or act in, runs
other members started.
*Example:* Ana calls Claude into a chat with Lee in the family space; the
run uses the Mac that Kim (the owner) enrolled.
*Options:* (a) the space's machine; consent sheets say "runs on Kim's
machine", and the run's rights are intersected with Kim's. (b) **Shared-space
runs only on hosted sandboxes.** Cost: hosted compute. (c) Always the
caller's own machine and login. Cost: reverses your decision on
subscriptions.
*Recommendation:* (b), with (a) as an explicit space-owner setting.

**T6. Does each person approve a plugin's use of their own data?**
*What:* the space owner approves a plugin for the space; each person could
also approve its use of their private state and writing.
*Why now:* the security review recommends it; the simplicity review says it
adds a prompt without a fixed risk once the agent rule exists.
*Example:* the owner installs a summarizer; Lee never agreed to it reading
Lee's private notes.
*Options:* (a) owner approval is a ceiling; each person approves on first
use. Cost: a prompt per person per plugin. (b) **Owner approval covers
everyone, and the consent rule protects writing from anything that can send
data out.** Cost: a plugin that cannot send data out may still read and
rearrange your files for others to see, within its grant.
*Recommendation:* (b).

**T7. Should the riskiest confirmations need your passkey?**
*What:* a stolen session or a modified app can press Allow itself.
*Example:* a stolen laptop session approves a plugin with "Any website" and
"Always" consent.
*Options:* (a) a tap is enough. (b) **A passkey for installing or widening
a plugin, sharing outside the space, "Always" consent, picking an
outbound-capable part and enabling frames.** Cost: a few Face ID prompts a
month.
*Recommendation:* (b).

**T8. Where do chats between people live?**
*What:* a chat between people is one file in the starter's space, shared
with the others (owner decided). If that space is a group space, its owner,
managers and readers could read it through their roles.
*Why now:* it decides whether a private chat is private.
*Example:* Ana starts a chat with Lee inside the family space; Kim, the
family space's owner, opens its Chats folder.
*Options:* (a) **in the starter's personal space, never in a group space**,
so only participants hold rights. (b) In any space, with chat files
excluded from role-based access. Cost: an exception in the rights rule.
*Recommendation:* (a).

**T9. What may a plugin do with a secret, and how does mail work?**
*What:* a plugin never holds a secret's value, but it can still use the
secret to mint a new token or write into its own account on the same
service. Mail over IMAP and SMTP cannot work by adding an HTTPS header.
*Example:* a plugin holding your GitHub token calls GitHub's "create token"
endpoint.
*Options:* (a) **restate the promise**: plugin code never holds the value
and may not set other auth headers on its destination; multi-tenant
destinations are shown at install as "can act in your account there"; mail
gets host IMAP and SMTP calls with the same rule. (b) Allow only
single-purpose destinations. Cost: most useful services are multi-tenant.
*Recommendation:* (a).

**T10. Who may create spaces and groups, and how is the host shared?**
*What:* quotas are per space; an account with many spaces multiplies them.
*Example:* one account creates 50 spaces, each running 8 hosted agents.
*Options:* (a) **every account may create spaces and groups up to an
operator limit; every quota is also charged to the account, with an
operator ceiling and fair share.** (b) Only the operator creates spaces.
Cost: people cannot start a group themselves.
*Recommendation:* (a).

**T11. Incoming webhooks: may they drive a plugin, and as whom?**
*What:* an event from outside (a GitHub push, a payment) has no signed-in
person.
*Example:* a plugin watches a repository for new issues.
*Options:* (a) **a host route maps a signed request to one operation,
acting as the person who set it up**, with a per-webhook rate and expiry
when that person loses access. (b) Not in the first version. (c) A
space-level system principal. Cost: rights with no person behind them.
*Recommendation:* (a), after step 8.

**T12. What can visitors through share links, or with title-only access, trigger?**
*Example:* a public link to a reading list; the visitor presses "Mark read".
*Options:* (a) **host writes their rights allow, and no plugin code**;
title-only viewers trigger nothing. (b) Plugin code runs with guest rights.
Cost: plugin code for people outside the space.
*Recommendation:* (a).

**T13. What may a run touch on the actor's own machine?**
*What:* server rights do not bound what a run does on a personal machine.
*Example:* Ana's agent runs with access to `~/work` on her laptop.
*Options:* (a) **only folders the actor picks for the run, read-only by
default, listed in the run's header; the login folder only for the picked
harness.** (b) What the harness asks for. Cost: no boundary beyond the
operating system.
*Recommendation:* (a).

#### Data

**D1. How far does "delete for everyone" reach?**
*Example:* Sam deletes a message with a password in it; Ana's agent had
already read it.
*Options:* history: (a) **scrub the words from history and search**, keep
"deleted by Sam at 17:05", Undo only for a few seconds before the delete is
sent; (b) keep it in history for the author. Run files: (a) blank it in run
logs the host stores; (b) **leave run logs alone and say "agents that
already read it keep their copy"** (run files are private to their actor
and kept 30 days).
*Recommendation:* history (a), run files (b); state that backups keep the
bytes until they rotate.

**D2. How do links follow a rename?**
*What:* you decided renaming updates every Markdown link at once and
cheaply. Some linking files belong to people the renamer cannot write for.
*Why now:* it decides whether the host or a plugin rewrites links.
*Example:* Sam renames a shared note; Lee's private note links to it.
*Options:* (a) **the host rewrites link destinations in every linking
file, never link text**, recorded in history as "link updated after a
rename". Instant everywhere. Cost: a host job, and Lee's file shows the
new name even if Lee can no longer open the target. (b) The host keeps a
permanent alias so every old link still resolves at once, and the Notes
plugin rewrites the links it may write, with its own rights. Cost: aliases
pile up; links in files Notes cannot write show the old path on disk.
*Recommendation:* (a). It is what you asked for (links follow every rename,
as in Obsidian), and with destinations only, a rename can no longer put
words into someone's file.

**D3. Is a plugin allowed its own rebuildable store?**
*What:* userdata is files and appdata is the host's; a plugin has neither
for a cache of its own.
*Example:* a mail plugin's index of 50,000 messages, or an importer's map
from old IDs to new files.
*Options:* (a) **a per-plugin rebuildable store the host may delete at any
time**, bounded by quota, never authoritative. (b) No; such plugins write
files. Cost: caches pollute the person's files.
*Recommendation:* (a).

**D4. Can a space manager delete other people's messages?**
*Example:* someone posts abuse in the family chat.
*Options:* (a) authors only. (b) **managers and the owner may delete (not
edit), shown as "deleted by a manager"**; chats between people (T8) stay
authors only.
*Recommendation:* (b).

**D5. Are long record files split into monthly segments?** *(pending
measurement)*
*What:* a record file could be capped and rolled over monthly, so a long
chat becomes a folder of files.
*Why now:* every record app is written against one answer.
*Example:* Lee quotes a message from last year in a two-year-old chat.
*What removes the question:* if append, edit and delete with history
scrub on a 64 MiB record file cost about the same as on a small one (the
measurement running tonight), there are no segments.
*Options if the cost grows:* (a) the host shows a segmented kind as one
logical file (reads, unread, triggers and sharing span the segments).
(b) Plugins handle segments. Cost: every record app re-implements rollover.
*Recommendation:* no segments if flat; otherwise (a).

**D6. Do collections and sidebars span spaces?**
*What:* a collection is a file in one space; you may want one sidebar
showing chats from your personal space and your family space.
*Example:* "Today" listing tasks from three spaces.
*Options:* (a) **one space for now**; a collection lists only its own
space. (b) A personal collection may query every space you belong to,
each row checked in its own space. Cost: queries across databases and a
harder rights story.
*Recommendation:* (a) now, (b) later if wanted.

**D7. Collection fields (a status for each thing in a collection)?**
*What:* the drawing spike drew a board over one kind grouped by that kind's
own field (Reading List by `read`), which needs nothing new. A board over
mixed kinds needs a value that belongs to the item's place in the
collection, not to the item.
*Example:* you drag a mail to "Done" on the Lisbon board; "Done" cannot be
written into the mail, which may be someone else's.
*Options:* (a) fields stored in the collection file beside each member.
Cost: a join and a larger file. (b) Kind fields only. Cost: no mixed
boards.
*Recommendation:* (a) when a mixed board is built; single-kind boards
already work with (b).

#### Interface

**I1. What does a sealed frame promise?**
*What:* in browsers, WebRTC escapes both the content policy and the frame
sandbox, so a frame cannot be cut off the network.
*Example:* a community budget chart opens your "Taxes 2026" note in a frame.
*Options:* (a) **promise "a frame can read what its plugin can read"**: a
plugin with a frame counts as outbound-capable, so it receives only
consented writing, and its install screen says its pages may reach the
internet. (b) No frames on the web until browsers close the hole. Cost: no
HTML mail or agent pages in browsers. (c) Frames for first-party plugins
only. Cost: against "first-party is not special".
*Recommendation:* (a); remote images in Mail only on your tap, through a
host proxy.

**I2. What does a person install: an app or a plugin?**
*What:* Chat is four plugins (core, composer, agents, notifications) plus
harnesses; four install screens would be noise.
*Example:* installing Chat for the first time.
*Options:* (a) **an app bundle**: one install screen listing the plugins
and their combined rights; each plugin can still be replaced or disabled
on its own later. (b) Each plugin separately. Cost: four screens and
dependency errors.
*Recommendation:* (a).

**I3. What shape does the canvas of windows have on a desktop?** *(for the
mockup; does not block the build)*
*Options:* (a) **a row of columns that scrolls sideways**; (b) free,
overlapping windows (hard on the keyboard and iPad); (c) at most two side
by side, the rest as tabs.
*Recommendation:* (a), mocked up beside (c).

### C. Operator defaults: no answer needed

| Setting | Default |
| --- | --- |
| Run deadline | 24 hours, at most 7 days |
| Missed alarms | Delivered once, late, with both times; grace is the plugin's setting (SDK default one hour) |
| A sleeping laptop | Sleep counts towards the 24-hour rule; the run stops at wake and the app offers resume |
| Updates and reboots under hosted runs | Server updates never wait; reboots wait for idle up to a limit, then mark runs interrupted with a resume offer |
| Process sharing | Plugins never share a process. Interface-only plugins run no process. Others start on first call and stop when idle (measured, #103). |
| Idle stop | 60 s (the bench used 15 s). Longer keeps about 16 MiB per process resident while a person pauses; shorter pays a 35 to 51 ms cold start (86 to 157 ms uncached) more often. 60 s covers the pauses of a conversation without keeping processes for a person who has left. |
| Retention | Trash 30 days, receipts 7 days, run files 30 days, audit per plugin |
| Per-person state writes | Coalesced by the host (about every 5 s); not an author concept |

## 10. Contradictions between the sections, and how they were resolved

| # | Contradiction | Resolution |
| --- | --- | --- |
| C1 | Number of primitives (authority six, thesis seven, my first draft twelve) | Five parts, explained as three ideas (§0) |
| C2 | A third storage class against "userdata strictly files" | Confirm A1 |
| C3 | Plugin calls plugin: delegated (authority) against strict (thesis, prior art) | Fork T1; the reach set replaces the authority section's handles |
| C4 | Lists: one source against three | One source: a collection (grouping replaces facets); a record file is drawn by the records view |
| C5 | Sidebar edits: list of changes (files) against copy (drawing) | Confirm A14 (a copy that still picks up new defaults) |
| C6 | Record edits appended (my first draft) against in place (files) | In place: a deleted message vanishes (owner) |
| C7 | The frame cannot be cut off the network | Fork I1; frames are outbound-capable |
| C8 | Agent: outbound-capable (authority) against an `agent: true` flag (authoring) | Outbound-capable closure; no flag |
| C9 | Agent replies: run appends (files) against trigger posts (authoring) | Confirm A5: fenced calls from the run |
| C10 | Missed alarms: "server never decides" against "a grace with one default" | Operator default (C): delivered late with both times; grace is the plugin's |
| C11 | `.people/` hidden against "a visible file" | Shown to its owner as "My sidebars and settings" |
| C12 | Link rewriting: host (owner decision) against a plugin (simplicity review) | Fork D2, recommending the host with destinations only |
| C13 | Per-person approval: yes (security) against no (simplicity) | Fork T6 |

## 11. Where this document overrides the section files

The section files are left as their lanes wrote them, except
`authoring.md`, whose bugs are fixed in place (a note at its top lists the
changes). Where a section says otherwise, these win: the reach set instead
of handles and designation tokens (§4.3); consent at the point of delivery
instead of marked chains (§4.4); one record rule and conflict copies
instead of record policies and per-record diffs (§4.5); collections with
grouping instead of facets; no declarative operations; one host sheet; the
names in §4.2; the query form in §4.2.

## 12. What this document does not cover

Process-sharing measurements (#103), the shell mockups, the canvas in
detail, and migrating existing code beyond [was-and-is.md](primitives/was-and-is.md).

## 13. The reviews and what changed

### 13.1 Security review

[`docs/reviews/primitives-only-security-review-2026-11-21.md`](https://git.kayg.org/kayg/calternal-ai/src/branch/dev/docs/reviews/primitives-only-security-review-2026-11-21.md)
on `dev`, of `6c432d33`: 21 findings.

| # | Finding | Disposition |
| --- | --- | --- |
| F1 | A frame leaks without counting as outbound-capable | Applied: a frame makes a plugin outbound-capable (§4.4, T23, I1) |
| F2 | Starting a program or calling an outbound-capable plugin escapes consent | Applied: the outbound-capable closure (§4.4, T5) |
| F3 | Run files readable by the whole space | Applied: `.people/<actor>/runs/` (§4.6, T10) |
| F4 | A machine's owner forges calls in a run's log | Fork T5 (run rights ∩ machine owner's rights if a member's machine is allowed) |
| F5 | Anyone can steer someone's run | Applied: only the actor sends input (A18, T9) |
| F6 | Author-owned records bypassed by replace, share edits, header removal, undo | Applied, cheaper as the simplicity review proposed: outside changes that are not pure appends become conflict copies (§4.5, A21, T3) |
| F7 | Kind or ID changed through frontmatter | Applied: host-state facts (A1, T4) |
| F8 | Contributors to plain files unprotected | Applied: every person in the revision stamps counts (§4.4) |
| F9 | Agent replies weaken designation; output after Stop posted by a trigger | Applied: the reach set for run targets, emptied at Stop; replies only through fenced calls (§4.3, A5, T8) |
| F10 | A slot handle replayed into a chat the person left, or from an alarm | Applied: the host fills slot targets; the reach set has no tokens (§4.3) |
| F11 | Approval does not cover the entry code | Applied (A20, T17) |
| F12 | Derived disclosure grows on another plugin's update | Applied (A20, T18) |
| F13 | A client names any installed program | Applied: the host resolves the pick (A9, §4.3) |
| F14 | Rename rewrites link text in others' messages | Applied: destinations only, never link text (D2) |
| F15 | Warm memory leaks across people | Fork T2 |
| F16 | Secrets yield equivalent credentials; mail cannot use header injection | Fork T9; T16 restated |
| F17 | A revoked session's consents and picks survive | Applied (A19) |
| F18 | A restore undoes revocations | Applied (A19, T20) |
| F19 | Audit flood evicts earlier reads | Applied: retention per plugin (T21) |
| F20 | Ranking and counters reveal hidden files | Applied (A17, T7) |
| F21 | Work spread over many spaces starves the host | Fork T10; quotas charged per account (T14) |
| Taint per call | Not enough | Replaced: consent at delivery, plus fork T2 (process per plugin and person, or taint per process) |
| Phishing through forms | Open | Applied: a header naming the plugin on its forms; no password-like field (T22) |
| §4 theatre | Echo scrubbing, `returns` declarations, query-result handles, the push phrase, the frame wording | Echo scrubbing is only a cheap filter (T16); `returns` declarations dropped, and results are typed as references plus plain values; all handles dropped for the reach set; a push goes only to recipients who may read the item; "a frame can read what its plugin can read" (I1) |
| Missing questions | Eleven | Chats between people T8; spaces T10; webhooks T11; visitors T12; run access on a machine T13; send to a run A18; kind and ID A1; session revocation and restore A19; entry code A20; audit retention C |

### 13.2 Simplicity and authoring review

[`docs/reviews/primitives-only-simplicity-review-2026-11-21.md`](https://git.kayg.org/kayg/calternal-ai/src/branch/dev/docs/reviews/primitives-only-simplicity-review-2026-11-21.md)
on `dev`, of `6c432d33`.

| Finding | Disposition |
| --- | --- |
| 60 author names; different names across sections | One vocabulary and one query form (§4.2); `authoring.md` corrected |
| Record policy knobs | Cut: one rule (§4.5); manager deletion is fork D4 |
| Monthly segments | Kept as fork D5, pending the large-file measurement |
| Handles and the designation rule | Replaced by the reach set (§4.3), with what is left open stated |
| Marked chains | Replaced by consent at the point of delivery (§4.4) |
| Withheld markers | Replaced by one `omitted` count |
| Facets | Cut: a collection with grouping |
| Declarative operations | Cut: the kind's write grant (A9); Reading List is a kind and a sidebar |
| Two files per run | Described as three host calls (§4.6) |
| State batching as a concept | Hidden: an operator default (C) |
| Slot fields | Only `effect` and `recommended`; the rest derived (§4.7) |
| Challenge, consent, pick, install and Stop screens | One host sheet (§4.7) |
| Host link rewriting | Not moved: fork D2 presents both, recommending the host with destinations only (the owner's rename decision) |
| Third storage class | Confirm A1 |
| Overlaps between parts | Explained as three ideas (§0); the build keeps five parts |
| `authoring.md` bugs: mute state, read-position key, prompt before consent, mention tokens, recurring work, steer without actor check, request IDs, `calternal-` prefix | Fixed in `authoring.md` (note at its top); `every` for repeating work; the host enforces the actor check |
| A second fake host for tests | Replaced: tests against the real binary in test mode; `calternal check` prints the install sentence and the "can send data out" word (§4.8) |
| Step 1 too large; drawing tested last | Step 0a one-day files spike and step 0b drawing spike; unmeasured costs listed as gating measurements (§7) |
| Taint per process smears Chat | Fork T2 recommends one process per plugin and person, with taint per process as the alternative |
| Missing questions: a plugin's own store, what a person installs, collections across spaces, webhooks | D3, I2, D6, T11 |
| Questions for a twenty-minute read | §9 restructured: Confirm, forks (Trust, Data, Interface), operator defaults |
