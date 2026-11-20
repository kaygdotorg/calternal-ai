# Primitive 6: drawing · design section · 2026-11-21

Status: **draft for the owner to grill.** One section of the night's design. It tests
primitive 6 of `docs/proposals/primitives-only-thesis-2026-11-21.md` ("the thesis") and
the drawing parts (P6 Collections, P11 Drawing) of the contract owner's first draft
`docs/proposals/primitives-only-host-2026-11-21.md` on `lane/v2-host-base` ("the host
draft"). Nothing here is decided unless it says "owner decided". Interface work: Claude
lanes only (AGENTS.md).

Area: the closed component catalogue and the sealed frame; how a kind looks; collection
views; the sidebar as a collection of collections; the middle as a canvas of windows;
the right panel; the shell as replaceable parts; the wordmark; forms and actions; drafts
and per-person interface state; native drawing; motion; and what happens to revision
14's entry views, item views, row templates, marks and limits.

Neighbours: Files (records, stable IDs), Questions (the index and collection queries),
People and permissions (grants, consent), Running code (work units, Stop) and Calls (the
call table) are other sections. This section says what drawing needs from each; it does
not redesign them.

## 0. The idea in five lines

1. A plugin never draws. It **declares**: how each of its kinds looks (row, card, cell,
   page), which collections its app starts with, which operations a form or a row action
   calls, and which named **parts** it fills.
2. The host fills declarations with data **as the viewer**, and the client draws them with
   its own controls (Svelte on the web, SwiftUI on Apple platforms).
3. Every list on screen is one of three things: a **collection** (items), a **record
   file** (messages, comments, a log) or a **facet** (the values of a field, such as tags).
4. The shell (switcher card, sidebar, canvas, right panel, composer) is a set of named
   **slots**. A part fills a slot with a declaration; the person (or, for what others
   receive, the space) picks which part.
5. There are no layout limits. Anything that does not fit is shortened, wrapped or moved
   into a menu. The only fixed numbers left protect the renderer and the server.

## 1. The primitive

### 1.1 What is declared, and where it is stored

| Thing | Written by | Stored | Bound or retention |
| --- | --- | --- | --- |
| **Kind look** (row, card, cell, page, record) | plugin author | in the plugin package | lives and dies with the package version |
| **App** (name, icon, kinds, default sidebar, slots it offers) | plugin author | package | package |
| **Part** (fills a named slot) | plugin author | package | package |
| **Collection** (query, hand-added items, views, collection fields) | person, or a plugin's default | a Markdown file in the space (Questions section), or read-only inside a package | userdata: kept until deleted, like a note; size cap (operator default, proposed 256 KiB) |
| **Sidebar** (ordered collections, each shown small) | person; default from the app | a Markdown file (owner decided: a visible file with the collections) | as a collection |
| **Interface state** (last view per collection, sort override, picked parts) | the shell, for the person | one private file per person per space, `People/<me>/interface.md` | one file; size cap (proposed 64 KiB); entries for deleted items are dropped on write |
| **Drafts** (composer text, an unsent form) | the shell, for the person | one private file per person per space, `People/<me>/drafts.md` | removed on send or when the item is trashed; count cap (proposed 500 per person, operator default) |
| **Device state** (sidebar widths, open windows on the canvas, card size step 3/5/7, scroll positions) | the client | on the device only, never on the server | wiped at sign-out |

Why the last row is not a file: it describes one screen, not the person's content. The
owner already decided sidebar width is per device (2026-11-18). Question 4 asks whether
the open windows should follow the person instead.

### 1.2 Host operations of drawing

The host adds few operations of its own. Most drawing reuses Questions (windows), Files
(writes) and Calls (actions). Costs are per request on the request path; "read" means a
file or index read that is not already in memory, "flush" means an `fsync`.

| Operation | Inputs | Result | Failure | Cost |
| --- | --- | --- | --- | --- |
| `view.open` | what to open (app, collection, item, or a part's slot), the space | the checked declaration (from the package cache), the person's sidebar and interface state, the resolved part for each slot | `not_found`, `no_access`, `part_missing` (with the recommended part to turn on) | package declarations: 0 reads (parsed and checked once at install, kept in memory: constant work, gate item 6). Sidebar and interface files: 1 read each the first time, then cached by revision. **0 flushes.** |
| `window` (Questions section) | a collection, a record file or a facet; a range; the fields the looks need | rows with look fields filled as the viewer | `no_access`; a query the index cannot run fast is refused by the Questions section | index only, 0 file reads; 0 flushes. One query per visible list. |
| `look.fields` (part of `window`) | none extra: derived from the kinds present in the window | the fields each kind's look names | an unknown field draws blank, never an error | included in the window's read |
| `state.write` | file (`sidebar`, `interface`, `drafts`), a patch, the expected revision | new revision | `conflict` (another device wrote first: the client merges per entry and retries once) | 1 write, **1 flush**, coalesced: at most one per file per second while the person drags or types, and at once on leave |
| `action` = a call (Calls section) | operation, inputs from value sources | the operation's result | the call's error, drawn in place | the call's own cost; drawing adds none |
| `challenge` | an effect the host knows is risky (widen sharing, let an agent read others' writing, a new network destination, install) | the person's yes or no | `declined`, `expired` | 0 flushes until confirmed; then the effect's own write |
| `frame.open` | a package file (HTML bundle) and the item it is opened on | a sealed frame and a bridge handle | `no_access`, `frame_disabled` | 1 package read (cached); 0 flushes |

Per window request: **0 flushes**. Per sidebar drag: **1 flush** (coalesced). Per typed
draft: **at most 1 flush a second** per open composer. Nothing on the request path takes
an installation-wide lock; sidebar and interface files are per person per space, so two
people never wait on each other.

### 1.3 Kind looks

A kind declares up to five looks. Each look is a small set of **named places**; the
author says which field goes in which place. The renderer decides sizes, fonts, colours,
spacing and motion.

| Look | Places | Used by | If not declared |
| --- | --- | --- | --- |
| **row** | `title`, `detail` (ordered fields), `leading` (icon, avatar or thumbnail), `trailing` (ordered: time, count, dot, icon) | list, compact list, search results, pickers | title + kind icon + `updated` |
| **card** | `image`, `title`, `detail` | gallery, board, strip tooltip, link previews | the row look laid out as a card |
| **cell** | `title`, `leading` | table (first column), calendar entries, mentions inline | the row's `title` and `leading` |
| **page** | one of: `form` (its fields), `editor` (the body), `records` (a record file), `collection`, `frame`, or a `stack` of those | a window on the canvas | `form` over the declared fields |
| **record** (record kinds only) | `body`, `quote`, `attachments`, `status` | the records component | body as plain text |

Rules:

- **A look names fields, never values.** Fields come from the index (frontmatter, records,
  origin stamps) and from host fields every item has (`title`, `kind`, `created`,
  `updated`, `author`, `excerpt`, `trashed`). A look can therefore be drawn **while its
  plugin sleeps**: no wake, no snapshot.
- **Per-person fields** use the prefix `me.` (`me.unread`, `me.pinned`). The Questions
  section computes them from the person's own state, as the person. Nobody else's value is
  ever visible.
- **Order is priority.** When room runs out, `detail` drops from its end, `trailing` keeps
  its first entry and folds the rest, `title` ellipsises. There is no maximum count.
- **Conditions** are one small predicate on a field (`when: "!read"`), nothing more. No
  expressions, no computed text. A plugin that wants computed text writes it into a field.
- **A mixed collection** draws each item with its own kind's look. A kind whose plugin is
  disabled or uninstalled still draws with the default look and a small "plugin off" mark;
  the item is a file and stays visible.
- A kind's look is declared once, by the plugin that owns the kind (gate item 10). Another
  plugin changes how a kind looks only by filling a slot the app declares (1.6).

### 1.4 Collection views (host components driven by a query)

A collection view is a host component. Its input is one collection (or record file, or
facet) and the view's options. The view picks the look; the author writes nothing.

| View | Size | Look used | Options (stored in the collection file) | Web | Mac (SwiftUI) | iPhone (SwiftUI) |
| --- | --- | --- | --- | --- | --- | --- |
| compact list | small | row (one line) | sort, limit shown before "Show more" | `packages/description` `Sidebar.svelte` rows | `List` sidebar style, `Section` | `List` sidebar style |
| strip | small | leading (avatar or icon), title as label | order (hand-added) | horizontal scroller with edge fade (mockup `ChatSidebar.svelte`) | `ScrollView(.horizontal)` with a gradient mask | same |
| facet | small | value and count | field, sort by count or name | rows | `List` rows with `.badge` | same |
| list | big | row (two lines) | sort, group by a field | `ContentList.svelte` | `List` | `List` |
| table | big | cell + columns | columns (fields), sort | `<table>` with sticky header | `Table` | `Table` shows only the first column in compact width ([Apple: Table](https://developer.apple.com/documentation/swiftui/table)); the renderer draws the row look instead |
| board | big | card | group by a choice field, column order | columns that scroll sideways | custom (no system board; written reason: none exists) using `LazyHStack` of `List`s | one column per page, paged sideways |
| gallery | big | card | size (small, medium, large) | grid | `LazyVGrid` | `LazyVGrid` |
| calendar | big | cell | date field, end field | month grid | custom month grid (no system calendar view; written reason) | agenda list by day in compact width |

Every view: windows (never the whole result), one Tab stop per list with arrow keys
(owner decided 2026-11-20), Home/End and Page Up/Down scroll without moving selection
(owner decided), selection is never part of a declaration, every drag has a menu and
keyboard equivalent.

**Mixed kinds in a table or board.** Columns are fields. A field that a kind does not have
draws blank for that item. A board groups by a field; in a collection of mail, notes and
photos no kind shares a "status" field. So a collection may declare **collection fields**:
values that belong to an item's membership in this collection (the mockup's To do / Doing /
Done). They are stored in the collection file next to the member. This is the most
consequential gap found (question 1).

### 1.5 The sidebar, the canvas and the right panel

**The sidebar is a collection of collections** (owner decided), stored as a file. Each
entry is a collection shown small. Example, the person's Chat sidebar:

```markdown
---
kind: sidebar
title: Chat
from: calternal-chat@4
---
- strip [[Pinned chats]]
- [[Groups]]
- [[People]]
- [[Lisbon crew]]
- [[Cals]]
- [[Harnesses]]
- [[Archived]] collapsed
- [[Bots]] hidden
```

Links carry the stable file ID (Files section); the readable text is refreshed on rename.
`[[Groups]]` here points at a collection inside the Chat package (read-only); `[[Lisbon
crew]]` is a collection file the person made.

| The person does | What changes | Cost |
| --- | --- | --- |
| reorder sections | line order | 1 coalesced flush |
| collapse, hide, show | `collapsed` / `hidden` on the line; hidden ones are listed under a "Hidden" entry at the end | same |
| rename a section | `as "Teams"` on the line: renames it in this sidebar only. Renaming the collection itself is "Rename collection" in its menu | same |
| make an own section | a new collection file with no query (hand-added only), linked in | 1 file create + 1 sidebar write |
| drag a row into a section | the item is added to that collection's hand-added list | 1 write to that collection file |
| drag a row onto the strip | added to the strip's collection, at the drop position | same |
| drag a row out of a hand-added section | removed from that collection (Undo restores) | same |
| change a package section's query | "Save a copy": a collection file in the space replaces the line | 1 create + 1 write |
| drop any collection into any sidebar | a new line | 1 write |

Defaults and updates: until the person changes something, the sidebar is the app's
default inside the package (no file at all). The first change writes the person's copy
with `from: <plugin>@<version>`. When the plugin later adds a section to its default, the
shell appends it to the person's copy once, marked new; a section the person hid stays
hidden. No migration code: the rule is "append default entries the file has never listed".

Who sees it: question 2. Recommended: the person's copy lives in `People/<me>/Sidebars/`,
private; "Share sidebar" copies it into the space's collections.

**Items in two sections.** A chat hand-placed in "Lisbon crew" also matches the "People"
query. Revision 14 had `repeat.exclusive`. Here: question 5.

**The middle is a canvas of windows** (owner decided). A window shows one thing: an item
(its page look), a collection (a big view), or an app's home. Opening an item of another
kind from the sidebar opens another window; the sidebar does not change. The host draws
each window's title bar: kind icon, title, the kind's actions in a menu, close, and the
Stop control for any work running for the person in that item (1.8). The arrangement of
windows is question 3.

**The right panel** is a slot with tabs at its foot (ui-shell decided). Its first tab is
the AI chat part (picked per space, since what an agent receives is what others' data
feeds). Further tabs come from the current window's kind (`panels:` in the app) and from
parts filling the app's declared panel slots. Every item kind gets an **inspector** tab
for free: its declared fields as labelled values, editable when the person may write.

### 1.6 The shell as replaceable parts

A **slot** is a named place in the shell or in an app, with a short contract: what it is
given (the current space, app, window item, selection, the person's draft) and which
operations a part in it may call. A **part** is a package entry that fills one slot with a
declaration (catalogue components) or, where nothing else will do, a sealed frame.

| Slot | Offered by | Default part | Who picks (owner decided: by effect) | If the picked part is missing or broken |
| --- | --- | --- | --- | --- |
| `shell.switcher` (Apps and Collections card) | host | the official card (`calternal-shell` package) | person | the official card, with a one-line notice |
| `shell.sidebar` | host | the sidebar file (no code) | person (it is a file) | the app's default sidebar |
| `shell.panel` (tabs) | host | AI chat | person for app panels; space for the AI tab | the tab is absent; a notice suggests the official part |
| `<app>.composer` | the app (Chat) | `calternal-chat-composer` | person | the recommended part with a notice; if it is also missing, an error in place with "Turn on Chat composer" (owner decided: no plain copy inside the core) |
| `<app>.list` (how the app's items are listed) | the app | the kind's own looks | person | the kind's own looks |
| `<app>.<named>` (any slot an app declares) | the app | the app's recommendation | by the slot's declared effect | as above |

**Not replaceable** (host only, drawn in the host's own layer): sign-in; the profile
popover and the Settings and Plugins pages behind it; every challenge and consent request;
the Stop control; the safe-mode switch (all parts off for this session, reachable by a
shortcut and at sign-in). Reason: whatever a part does, the person can always get back to
Settings, turn the part off, and stop a run. The owner decided the card is "replaceable and
movable by a plugin"; this keeps that, and keeps a way back.

**The wordmark morph** is shell behaviour, not a part: the top line shows "calternal" when
nothing is open and morphs (torph, MIT) into the open app's `name` or the open
collection's title (owner decided). A plugin supplies only its name. The face is the
person's Appearance setting. On the Mac it sits below the traffic lights (owner decided).

### 1.7 Forms and actions

- **A form is derived from an operation.** `form("reading.add")` draws one field per
  declared input, with its label, type and required flag from the operation's declaration
  (one place per fact). The author may order or hide fields, nothing else. Values stay on
  the device until submit; a failed submit keeps them.
- **An action** is an operation plus inputs from value sources: a literal, the row's item,
  the window's item, the selection, a form field, the person, a records range. Where it
  appears: `swipe` (leading or trailing), `menu`, `toolbar`, `primary`. The first that fit
  are shown; the rest go into the More menu (no caps).
- **Derived actions:** if an app has a create operation for a collection's kind, the
  collection shows "New" in its header; an item's menu always has the host's Rename, Move,
  Share, Trash.
- **Confirmation** of an effect only the plugin understands ("Delete for everyone?") is a
  declared sentence the host shows in its own sheet. **A challenge** (1.2) is only for
  effects the host itself knows are risky, and no plugin can draw, skip or fake one.
- **Authority does not come from the declaration.** The call table checks the person's
  rights ∩ the plugin's grant on every call. A client that sends a different item ID gets
  only what the person may do anyway. This deletes revision 14's activation contexts,
  15-second leases and choice refs (section 4).

### 1.8 Records, drafts and running work

- **The records component** draws a record file (Files section): records in order, author
  and time from the host's origin stamp, the person's own records on the right, consecutive
  records of one author grouped, day lines, the edited mark, tombstones drawn as nothing,
  quotes as references drawn with the reader's rights (a deleted record shows nothing where
  it is quoted: owner decided), an unread line from `me.seen`, jump to latest, paging both
  ways anchored on a record. It is generic: chats, comments on a note, a mail thread, an
  agent's working log. **It needs no plugin and no snapshot to draw.**
- **Selection in records** gives a range (first and last anchor). An action may take it
  (`{records: selection}`), for example "Ask Claude about these".
- **Seen position:** the component writes `me.seen` for the record file into the person's
  interface file, coalesced (one flush a second at most). Read receipts that others see are
  the plugin's: when the person's switch is on, the plugin appends its own record.
- **Drafts:** the composer component keeps its text on the device at once and writes it to
  `People/<me>/drafts.md` at most once a second and when the window closes. A part filling
  the composer slot reads its own draft through the slot contract.
- **Running work:** a window whose item has work running for the person shows a working
  line from the work unit's state (Running code section) and the run's linked output file;
  Stop is a host control on the window title bar. After a server restart the client
  reconnects and its windows catch up from the revisions it holds; nothing in drawing is
  specific to a restart.

### 1.9 The sealed frame

For what the catalogue cannot say (an agent's HTML page, a mail's HTML body, a chart
library). The frame draws inside one window's content area or one part's area.

| Property | Web | Apple |
| --- | --- | --- |
| No access to the host page | `<iframe sandbox="allow-scripts">` without `allow-same-origin`, served from a separate origin per package | `WKWebView` with a non-persistent `WKWebsiteDataStore`, no cookies shared |
| No network | CSP `default-src 'none'` plus the bundle's own files; `connect-src 'none'` | `WKContentRuleList` blocking every load except the bundle (not sure it covers every path, see 3) |
| One door out | `postMessage` to a bridge that accepts only `call(operation, inputs)` and `read(item it was opened on)`, checked like any call | one `WKScriptMessageHandler` with the same two messages |
| Cannot cover host UI | clipped to its rectangle, opaque, no pop-ups, no top navigation, no fullscreen | same |
| Looks like the app | receives the design tokens (colours, type, motion) as CSS variables | same |
| Keyboard | Escape and the shell shortcuts always reach the host first | same |

### 1.10 Motion as tokens

Declarations carry **no motion**. Each catalogue component states its transitions once,
from the design system's tokens (`docs/ui-design-system.md`: `--dur-fast` 120 ms,
`--dur-base` 220 ms, `--dur-slow` 300 ms, `--dur-sheet` 340 ms on a phone and 190 ms on a
desktop, `--ease-out`, `--ease-sheet`, `--spring`), every transition interruptible, 1 ms
under reduced motion. Native renderers read the same tokens from
`apps/apple/CalternalUI/Sources/CalternalUI/Generated/Tokens.generated.swift`.

| Component event | Order, duration, curve |
| --- | --- |
| row arrives in a list | fades in and rises 6 px, `--dur-base`, `--ease-out` (mockup `motion.ts` `rise`) |
| row moves (reorder, drag settle) | position animates from where it was, `--dur-base`, `--ease-out` |
| section collapses or expands | height, then content fade, `--dur-base`, `--ease-out` |
| row pinned onto the strip | the avatar travels to its slot on `--spring`; the row leaves its section after it lands |
| window opens on the canvas | slides in from where it was opened, `--dur-sheet`, `--ease-sheet`; neighbours make room at the same time |
| wordmark becomes the app name | torph morph over `--dur-slow` |
| switcher card resizes (3, 5, 7 rows) | height on `--spring` following the finger, settles on release |
| keyboard-driven change | no animation (ui-shell interaction rule) |

A sealed frame receives the tokens and is asked to follow them; nobody can check it.

### 1.11 What stays fixed

The owner's direction: layout limits disappear; the interface shortens or wraps instead
of rejecting. What remains protects the renderer or the server:

| Number | Why | Kind | Proposed |
| --- | --- | --- | --- |
| declaration size per package | a renderer must parse it on a phone | security, fixed | 1 MiB |
| nesting depth of a `stack` page | renderer stack safety | security, fixed | 32 |
| live windows per open window | each is a live query | resource, operator default | 32 |
| rows per window page | chosen by the renderer from the viewport, never by the author | none for the author | renderer |
| form value size | the request size | security, fixed (Calls section) | the request cap |
| sidebar, interface and draft file sizes | userdata bound | resource, operator default | 256 / 64 / 256 KiB |

How "shorten or wrap" works where revision 14 rejected:

| Was refused | Now |
| --- | --- |
| more than 3 marks on a row | first `trailing` entry shown, the rest fold into a "+n" or the row's menu |
| more than 3 secondary parts | `detail` drops from its end |
| more than 2 leading / 3 trailing header actions | toolbar shows what fits, the rest in More (system toolbar overflow on Apple) |
| more than 24 children, 200 nodes, 12 form fields | the page scrolls |
| more than one fullSwipe | the first trailing swipe action is the full swipe |
| a 51st choice | the picker searches |
| nesting deeper than 3 | no indent beyond the third level |
| more than 16 binds per app | there are no binds; each visible list is one window, under the live-window quota |

## 2. What plugins can and cannot do

**Plugins can:**

- Say how their kinds look as a row, a card, a table cell and a full page. *Example:* a
  recipe shows its photo on a card and its cooking time on a row.
- Give their app a starting sidebar made of collections. *Example:* Mail starts with
  Inbox, Drafts, Sent and Labels.
- Put buttons, swipes and menu items on their things, each calling one of their
  operations. *Example:* swipe a reading item to mark it read.
- Ask for a form that is built from an operation's inputs. *Example:* "Add link" with URL
  and title fields.
- Replace a part of the shell or of another app, where a slot exists. *Example:* a
  community composer with a different emoji picker.
- Draw anything in a sealed frame, which has no network and can only call operations.
  *Example:* an interactive chart of a budget.
- Show the person's own counts and state without knowing them. *Example:* "3 unread".

**Plugins cannot:**

- Run code in the person's app outside a sealed frame. *Example:* no script in a row.
- Choose sizes, colours, fonts, spacing or animation. *Example:* no red 18-point title.
- See what is drawn on someone's screen. *Example:* a look shows a contact's name, but the
  plugin that declared the look never receives it.
- Load anything from the internet into the interface. *Example:* no remote image or
  tracking pixel; a picture must first be saved as a file.
- Draw a host screen, a password field, a sharing challenge, a consent request or the Stop
  button. *Example:* a plugin cannot show a fake "sign in again".
- Hide Settings, the plugin list or Stop. *Example:* a broken switcher card cannot trap the
  person; the profile popover and safe mode still work.
- Change how another plugin's kind looks except in a slot that app offers and the person
  picks. *Example:* a theme plugin cannot restyle Chat's rows unless Chat offers a list slot.
- Be rejected for drawing too much. *Example:* a row with six badges shows the first and
  "+5" instead of failing install.

## 3. Security

| Attack | What stops it | Where it is checked |
| --- | --- | --- |
| Spoof a host screen (sign-in, challenge, consent, "enter your password") | the catalogue has no secret field and no host mark; host screens are drawn in the shell's own layer above every window, with the host mark | renderer (each platform) |
| Clickjacking: cover a host control with a transparent frame | frames are clipped to their rectangle, opaque, and below the host layer; components are laid out by the renderer, never positioned by the plugin | renderer |
| Read data through a look | looks name fields; the index fills them as the viewer; the drawn values go from host to client and never to the plugin | Questions index; the client talks to the host only |
| Learn hidden members from a collection's count | counts are computed per viewer over readable members only | Questions index |
| Hand-add a file to a shared collection to reach it | membership grants nothing; members the viewer cannot read are not drawn and not counted | Questions index |
| Track the person with remote images or links | declarations hold no URLs except links; links open through the host, which shows the destination; images are only files of the space | installer validator and renderer |
| Exfiltrate from a sealed frame | no network (CSP, sandbox, content rules), only `call` and `read` on its own item through the bridge, checked at the call table | renderer and call table |
| A replacement composer records what the person types | picking a part is the person's (or the space owner's for others-facing slots); the pick screen says "this part sees what you type here"; it may call only the slot's operations within its grant | package install and pick screen; call table |
| Lock the person out with a blank switcher or sidebar | profile popover, Settings, Plugins and safe mode are host-only and always reachable | shell |
| Keep a run going where the person cannot see it | Stop and the working line are host-drawn for every work unit started by or for the person | shell and work table |
| Crash or freeze a renderer with a huge or deep declaration | size and depth caps (1.11) in the one validator at install; the renderer refuses an over-limit declaration on load | installer, renderer |
| Overload the server from a view | one window per visible list, a live-window quota per open window, the page size chosen by the renderer | Questions index (quota) |
| Inject markup through data | text is plain; record and note bodies use the closed inline Markdown subset; no HTML outside a frame | renderer |
| Call operations the declaration did not name | allowed, and harmless: every call is checked as the person ∩ the plugin's grant | call table |
| Read the person's sidebar, drafts or interface state | `People/<me>/` files are readable by that person only; a part reads its own draft through the slot contract | Files grants |
| Pose as another app by name or icon | names unique among installed apps; host names reserved; host pages carry the host mark | installer |
| Drag a row into a frame to smuggle its content | a drop passes only the item's ID; the frame must read it through its own grant | renderer and call table |

**Where I am not sure it holds:**

1. **Native frame isolation.** I have not verified that `WKContentRuleList` blocks every
   network path in `WKWebView` (WebRTC, DNS prefetch, media). A safer fallback is a frame
   with JavaScript but no network permission at the process level; that needs research on
   a device.
2. **Deleting activation contexts.** Revision 14 bound each action to where it was drawn.
   I believe the call check (person ∩ grant) is enough, because a client acting outside a
   declaration only does what the person may do. A guest with title-only access is the
   case to re-check with the People section: the index must refuse fields, not the
   renderer.
3. **Timing and size side channels** in mixed collections (a window over items the viewer
   cannot read takes longer). The Questions section owns this.
4. **Phishing inside the catalogue's own components.** A form titled "Calternal needs your
   Apple ID" with a text field is still possible. The host mark on host screens and the
   absence of a secret field reduce it; they do not remove it.
5. **The slot contract for a composer** must be narrow enough that a malicious composer
   cannot send as the person to a chat other than the open one. That depends on the slot
   giving the open item as a fixed input, which the Calls section must enforce.

## 4. Was and is

| Topic | `dev` today | Revision 14 | This design |
| --- | --- | --- | --- |
| Lists | the web shell's own lists; `packages/description` draws binds | `Bind` (source, filter, sort, window, row template, states), 16 per app | a collection, record file or facet in a host view; no binds |
| How a row looks | per bind: row template with marks (up to 3), icons, actions | per bind; the same item could look different in two lists | per kind, once: row, card, cell, page, record |
| Sidebar sections | fixed by the plugin | `chrome.sidebar.sections`, `Section.repeat` over `layout_sections`, `host.layout.*`, the `sidebar_arrangement` record, drag hooks | entries of the sidebar file; drag = edit a collection's hand-added list |
| Strip | mockup only | `presentation: strip`, `RowAvatarIcon` | the strip view of a collection |
| Chat's messages | `TurnTranscript.svelte` on fixtures | entry views (`snapshot-entry`, overlays, request cards, reading state, `item_view.read`) | the records component over the record file |
| Opening an item while its plugin sleeps | n/a | the plugin stores an item-view snapshot | looks and records draw from the index without the plugin; only a plugin-made thumbnail stays (a file the plugin writes) |
| Actions | `invoke` | activation contexts, resolve, confirm, dispatch, 15-second leases, `ChoiceRef`s | a call with inputs; the call table checks it |
| Forms | `FormNode.svelte` | `form` node, 12 fields | derived from the operation's inputs |
| Confirmation | n/a | `confirmation.material_by_kind` table | host challenge for host-known effects; a declared sentence otherwise |
| Shell parts | ui-shell regions, hard-coded | chrome description: header, footer, panel, extension points | slots and parts, picked by effect, host-only safety screens |
| Two description languages | GenUI (`crates/genui`, `packages/genui`, `apps/apple/.../GenUI`) and the description | both | one catalogue; GenUI's portable program model is superseded (open-questions, 2026-11-19) |
| Limits | some | 312 numbers, about 20 of them layout | 6 numbers, none about layout |

**Deleted:** binds as a list per app; row templates per bind; marks caps; `Section.repeat`,
`layout_sections`, `host.layout.*`, `host.viewer.reorder`, `sidebar_arrangement`, drag
hooks as invokes; entry views, entry overlays, request cards, `item_view.open/read/follow`
and item-view snapshots for record files; activation contexts, view leases, `ChoiceRef`,
`ChoiceOptionsRequest`; `confirmation.material_by_kind`; `requires` feature flags and
`chromeVersions` negotiation (one version until production holds data: owner decided no
compatibility code); persisted signed row records (section 6 of the contract: offline
download is the Files section's full download, and a list on the device is drawn from it);
GenUI as a second language; the Python description verifiers (one validator, in the
installer, per the host draft).

**Code that survives** (paths in `v2-host-integration` unless noted):

| Code | Becomes |
| --- | --- |
| `packages/ui` (tokens, `Selector`, `Switch`, swipe rows, tooltips, `clip-morph.ts`, `listnav.ts`, `roving.ts`, `spring-step.ts`) | unchanged: the web catalogue's controls |
| `packages/description/src/windows/window-store.svelte.ts`, `selection.svelte.ts` | the window engine for every view |
| `packages/description/src/render/RowView.svelte`, `RowImage.svelte`, `RowActions.svelte` | the row look |
| `packages/description/src/render/Sidebar.svelte`, `ContentList.svelte` | compact list and list views |
| `packages/description/src/render/FormNode.svelte` | the derived form |
| `packages/description/src/components/Composer.svelte`, `TurnTranscript.svelte` | seeds of the composer and records components |
| `packages/description/src/invoke/` | simplified to "call with inputs" (no contexts) |
| `apps/mockup` on `lane/threads-ui` (`SwitcherCard`, `Sidebar`, `Section`, `ChatSidebar`, `Middle` with list, table, board, gallery) | the reference for the shell slots and big views; mockup only |
| `apps/apple/CalternalUI` (`Shell/`, `Materials/`, `Foundation/Motion.swift`, `Chrome/ChromeWindowedRows.swift`, `Description/V2RowModel.swift`) | the native renderer's shell, materials, motion and windowed rows |
| `crates/genui`, `packages/genui`, `apps/apple/.../GenUI` | deleted, unless the owner wants agent-made interface without a frame (question 7) |
| `contract/description-v2.schema.json` (3,588 lines), `host-catalogue-v2.json` drawing parts | replaced by one short catalogue schema generated from the SDK types |

## 5. Worked examples

Counts are lines of interface declaration the author writes, in the SDK's TypeScript
(`definePlugin`, `defineKind` in `packages/plugin-sdk/src/author-v2.ts` gain looks).
Operation and file declarations belong to other sections and are not counted, except where
noted. Revision 14's fixtures for comparison (JSON, `contract/fixtures/description-v2/`):
`reading-list-app.json` 285 lines, `notes-app.json` 584, `threads-app.json` 143 and
`threads-nested-catalogue-app.json` 493.

### 5.1 Reading List: 7 interface lines

```ts
export default definePlugin({ id: "reading-list", apps: [{ name: "Reading List", icon: "book" }],
  kinds: { reading: defineKind({
    fields: { url: url(), title: text(), read: bool(false), image: file() },        // Files/Questions
    row:  { title: "title", detail: ["site(url)", "created"], trailing: [dot({ when: "!read" })] },   // 1
    card: { image: "image", title: "title", detail: ["site(url)"] },                                   // 2
  }) },
  operations: {
    add: create("reading", { inputs: ["url", "title"] }),                           // Calls
    markRead: set("reading", { read: true }, { swipe: "trailing", icon: "check" }), // 3 (placement)
  },
  sidebar: [                                                                        // 4
    collection("Unread", { kind: "reading", where: "!read", sort: "-created" }),    // 5
    collection("All", { kind: "reading", sort: "-created" }),                       // 6
  ],                                                                                // 7
});
```

Everything else is derived: the "New" button and its form (from `add`), the cell and page
looks, Rename/Move/Share/Trash in the menu, the inspector, the empty state ("No items"),
search. The image is a file the plugin saves (no remote image in the interface); with no
worker it simply has none. Zero lines of code run to draw it; the plugin never wakes.

### 5.2 Notes: 11 interface lines

```ts
kinds: { notes: defineKind({
  fields: { title: text(), tags: list(text()), snooze_until: date(), thumb: file() },
  row:  { title: "title", detail: ["updated", "excerpt"], leading: thumbnail("thumb") },  // 1
  card: { image: "thumb", title: "title", detail: ["excerpt"] },                         // 2
  page: editor("body"),                                                                  // 3
  panels: [outline("body")],                                                             // 4 (inspector is free)
}) },
sidebar: [                                                                               // 5
  action("New note", "notes.create"),                                                    // 6
  collection("All notes", { kind: "notes", sort: "-updated" }),                          // 7
  facet("Tags", { kind: "notes", by: "tags" }),                                          // 8
  collection("Snoozed", { kind: "notes", where: "snooze_until > now" }),                 // 9
  trash(),                                                                               // 10 (the space's Trash)
],                                                                                       // 11
```

`excerpt` and `updated` are host fields. A tag row opens a collection window filtered by
that tag. History, links and backlinks are Files and Questions; the backlinks list is a
panel `collection("Linked here", { linksTo: "this" })` if wanted (one more line). The
thumbnail is a file the Notes worker writes after an edit; until then the row shows the
host's document miniature.

### 5.3 Chat: 19 interface lines

```ts
kinds: { chats: defineKind({
  records: true,                                                    // a record file (Files)
  fields: { title: text(), people: list(person()), harness: text(), archived: bool(false) },
  row:  { title: "title", leading: avatar("people"), detail: ["last.excerpt"],
          trailing: ["last.at", count("me.unread", { quiet: "me.muted" })] },          // 1-2
  page: records({                                                                     // 3
    composer: slot("composer", { recommended: "calternal-chat-composer", pick: "person" }), // 4
    actions: {                                                                        // 5
      reply:  { op: "chat.reply",  inputs: { quote: "record" } },                     // 6
      edit:   { op: "chat.edit",   when: "own and age < 48h" },                       // 7
      delete: { op: "chat.delete", when: "own", confirm: "Delete for everyone?" },    // 8
      ask:    { op: "chat.call",   inputs: { range: "records.selection" } },          // 9
    },                                                                                // 10
  }),                                                                                 // 11
}) },
sidebar: [                                                                            // 12
  strip("Pinned"),                                                                    // 13
  collection("Groups",    { kind: "chats", where: "people.count > 2 and !archived" }),// 14
  collection("People",    { kind: "chats", where: "people.count = 2 and !archived" }),// 15
  collection("Cals",      { kind: "cals" }),                                          // 16
  collection("Harnesses", { kind: "chats", where: "harness != null and !archived" }), // 17
  collection("Archived",  { kind: "chats", where: "archived", collapsed: true }),     // 18
],                                                                                    // 19
```

How each Chat requirement is drawn:

| Requirement | Drawing |
| --- | --- |
| people and agents in one chat | the records component; author and "own" come from the host's origin stamp, never from the plugin |
| consent | `ask` passes a records range to `chat.call`; the People section's grant creates a per-author request; each other person sees a **host-drawn** consent request in their records view ("ask each time" / "always", one-minute wait: owner decided); the plugin cannot draw or answer it |
| quotes | `reply` passes the record's anchor; the component draws the quoted record with the reader's rights; a deleted one draws nothing |
| delete for everyone | the declared sentence in a host sheet; `chat.delete` appends a tombstone; every reader's component stops drawing it at once |
| edited mark, 48 hours | the component shows "edited" when the record has a newer version; the 48-hour rule is Chat's `when`, and the operation checks it again |
| unread, mute | `me.unread` and `me.muted` from the person's own state; the muted count is quiet (owner decided: Telegram-style bubble) |
| agent run survives a restart | the window shows the host's working line from the work unit and Stop on the title bar; the reply arrives as a record when the run's output is relayed; the client catches up from the revisions it holds |
| the run's working log | a link in the reply opens the harness's own output file in another window (its page is `records`, so the same component draws it) |
| composer | a slot; the official composer part is about 5 lines (`composer({ send: "chat.send", attach: true, mention: "chat.people", draft: true })`) |

**Totals.** Reading List 7, Notes 11, Chat 19 (24 with its composer part), against 285,
584 and 143 to 493 JSON lines in revision 14. The comparison is unfair in one way: JSON
spreads one value over several lines. Counting values instead, revision 14's Reading List
holds about 70 and this one about 25.

## 6. Where the thesis is wrong or incomplete

| Thesis says | Problem | Changed to |
| --- | --- | --- |
| "Collections supply every list." | Messages in a chat, comments on a note and an agent's log are records inside one file, not items. Tags, labels and mailboxes are values of a field. People in a group and installed plugins are host lists. | Three sources for a list: a collection, a record file, a facet. Host lists are drawn only on host pages. |
| "Each kind says how it looks as a row, a card and a cell." | Opening an item needs a fourth look (page); record kinds need a record look. A cell alone makes no table: columns come from the collection. A board over mixed kinds needs a field no kind has. | Five looks; columns and collection fields belong to the collection (question 1). |
| "A chat is a file of records drawn by a list component." | A list of rows cannot do grouping by author, own-side bubbles, anchored paging from the bottom, unread line, quote resolution. | One generic records component (question 6). |
| "The shell itself is made of replaceable parts." | If everything is replaceable, one broken plugin can trap the person or fake a host screen. | Host-only screens (sign-in, Settings and Plugins, challenge, consent, Stop) and safe mode. |
| (silent) how authority flows from the interface | Revision 14's activation contexts and leases existed to tie actions to the screen. | Deleted: the call table is the only check. Re-checked with the People section (3, item 2). |
| (silent) sleeping plugins | Revision 14 needed view snapshots for chats. | Not needed: everything drawn comes from the index and record files. |
| (silent) per-person state | One small file is right, but writing it on every drag or keystroke is not. Some state belongs to a screen, not a person. | Coalesced writes (at most one flush a second); device state never leaves the device. |
| "about fifteen concepts" | Drawing adds four: **look**, **view**, **slot/part**, **action**. Component, form and frame are things the author uses rather than learns. | Counted in the fifteen if "component" is replaced by "look". |
| "A sealed frame covers what the catalogue cannot express." | True on the web; on Apple platforms isolation of a `WKWebView` from the network is not proven. | Marked unproven (3, item 1). |
| (silent) GenUI | A second, older description language with its own interpreter exists in three places. | Deleted unless question 7 keeps it. |

## 7. Questions for the owner

### 1. May a collection have its own fields, such as a status for each thing in it?

- **What:** in the mockup, "Trip to Lisbon" holds mails, notes, photos and chats, and its
  board sorts them into To do, Doing and Done. None of those kinds has a "status". The
  value belongs to the thing *being in this collection*.
- **Why now:** it decides whether boards, tables with extra columns and calendars work for
  mixed collections at all, and it changes the collection file format the Questions section
  is specifying tonight.
- **Example:** you drag the flight confirmation mail to "Done" on the Lisbon board. Where is
  "Done" written? Not into the mail, which may be someone else's.
- **Options:** (a) **collection fields**, stored in the collection file beside each member
  (`- [[Flight confirmation]] status: Done`): works for any kind, including things you may
  only read; costs a join in the index and a larger collection file (a query-matched item
  gets a line once it has a value). (b) **Kind fields only**: a board groups by a field the
  kinds share; mixed boards and extra columns are impossible; nothing new to build.
- **Recommendation:** (a). It is what makes "collections of anything" more than a list,
  and it keeps other people's files untouched.

### 2. When you rearrange an app's sidebar in a shared space, is that yours alone?

- **What:** a sidebar is a file. In a shared space it can be your private copy or the
  space's sidebar that everyone sees.
- **Why now:** it decides where sidebar files live and who may change them; drag, hide and
  rename all write it.
- **Example:** in the family space you hide "Harnesses" and add "Lisbon crew". Does your
  partner's Chat sidebar change?
- **Options:** (a) **yours alone**: the first change writes a private copy in
  `People/<me>/Sidebars/`; "Share sidebar" puts a copy into the space's collections for
  others to add. Costs one file per person per app that they changed. (b) **the space's**:
  one file per app, editable by editors or managers; everyone sees the same sidebar. Costs
  surprise when someone else reorders yours.
- **Recommendation:** (a): what only you see is picked by you (the rule the owner already
  set for parts), and a shared sidebar stays possible on purpose.

### 3. What shape does the canvas of windows have on a desktop?

- **What:** the middle card holds several windows (owner decided). How they sit is open.
- **Why now:** the mockup needs it next, and it decides what "open in another window" does
  on every platform, and what the person keeps between sessions (question 4).
- **Example:** in Notes you open a note, then a chat linked from it, then the Lisbon
  collection.
- **Options:** (a) **a row of columns** that scrolls sideways, each window a column with its
  own width, the newest beside the one it came from (like a paper roll): no overlap, full
  keyboard support, maps to a phone as one window with Back. (b) **free windows** that
  overlap inside the card: familiar from desktops, but hard with keyboard and on iPad, and
  needs a window manager in every renderer. (c) **at most two side by side**, the rest as
  tabs: simplest, least flexible.
- **Recommendation:** (a), mocked up next to (c) so you can compare; (b) costs the most and
  fits the floating-card look least.

### 4. Which interface state follows you to your other devices?

- **What:** some state is about your content (sidebar order, hidden sections, the view you
  last chose for a collection); some is about a screen (sidebar widths, open windows, the
  switcher card's 3/5/7 step, scroll positions).
- **Why now:** synced state is a file and costs writes; device state costs nothing on the
  server. The rule "anything that cannot be rebuilt is a file" would make all of it files.
- **Example:** you open three windows on the Mac, then pick up the iPhone.
- **Options:** (a) **content state synced, screen state on the device** (as you already
  decided for sidebar width): small files, nothing surprising on a phone. (b) **everything
  synced**: the iPhone tries to show three windows it cannot fit, and every resize writes a
  file.
- **Recommendation:** (a), and an explicit "Continue on…" later if you want windows to
  follow you.

### 5. Can one thing appear in two sections of the same sidebar?

- **What:** a section can be a saved question ("chats with one other person") or a list you
  fill by hand ("Lisbon crew"). A chat you drag into "Lisbon crew" still answers the
  question of "People".
- **Why now:** it decides whether dragging a row *moves* it or *copies* it, which people
  feel on every drag.
- **Example:** you drag your chat with Hana into "Lisbon crew". Does it also stay under
  People?
- **Options:** (a) **once per sidebar**: hand-placed beats a saved question, so it leaves
  People; costs an exclusion in the index query (cheap: hand-added lists are short). (b)
  **wherever it matches**: simplest, but the sidebar repeats rows and a drag looks like a
  copy. (c) a per-sidebar switch.
- **Recommendation:** (a), which matches the Telegram-style folders you referenced and
  revision 14's `exclusive` without its machinery.

### 6. Is a "records view" (a timeline of messages) part of the host's catalogue?

- **What:** chats, comments, mail threads and agent logs all need a timeline: authors,
  your own messages on one side, grouping, day lines, unread line, quotes, paging from the
  bottom. A plain list cannot do it.
- **Why now:** without it, Chat cannot be drawn natively and would need a sealed frame
  (web content in the iPhone app), which you rejected for lists ("there is no competing
  with native apps", 2026-11-19).
- **Example:** an agent's reply arrives in a chat with Ada while you are reading older
  messages; the view keeps your place and shows "1 new".
- **Options:** (a) **a generic records component** in the catalogue, with no chat words
  (it knows authors, times, edits, tombstones and references, which the Files section
  already stores): native everywhere, one implementation per platform. (b) **sealed frame
  per plugin**: the host stays smaller, every chat-like plugin writes its own web view.
- **Recommendation:** (a). It is generic (four apps use it) and it is what lets a chat open
  while its plugin sleeps.

### 7. May an agent or plugin make interface outside a sealed frame?

- **What:** the old GenUI language let a plugin ship a small program that a client
  interprets to draw native controls with local state. This design has only declarations
  (no programs) plus the sealed frame.
- **Why now:** GenUI exists in three code bases; keeping it means two description
  languages and two validators.
- **Example:** an agent wants to show a small interactive form ("pick three dates") in a
  chat reply.
- **Options:** (a) **declarations only**: the agent's reply contains a form declaration
  (catalogue components, bound to an operation); anything richer is a frame. One language.
  (b) **keep GenUI** for agent-made interface: more expressive, a second interpreter on
  every platform.
- **Recommendation:** (a): delete GenUI; the form in a reply covers the common case.

### 8. May a sealed frame ever load remote content?

- **What:** a mail's HTML body or an agent's page may reference remote images.
- **Why now:** the frame's isolation rules are written once; Mail (your third app) needs an
  answer.
- **Example:** a newsletter shows a picture from a shop's server, which also tells the shop
  you opened it.
- **Options:** (a) **never**: images missing in HTML mail. (b) **on your tap, per message,
  through a host proxy** that strips cookies and your address. (c) **per sender, remembered**.
- **Recommendation:** (b), then (c) as a setting; never automatic.

## Report: summary and top three questions

**Summary.** Drawing becomes declarations only. A kind says once how it looks (row, card,
cell, page, record); every list is a collection, a record file or a facet, drawn by host
views (compact list, strip, facet; list, table, board, gallery, calendar) that work while
the plugin sleeps. The sidebar is a Markdown file listing collections; every edit (reorder,
collapse, hide, rename, own sections, drag, pin) is a write to that file or to a
collection's hand-added list, coalesced to one flush a second. The shell is slots filled by
parts, picked by effect, with host-only screens (Settings, challenge, consent, Stop, safe
mode) so a part can never trap or fool the person. Forms are derived from operations;
authority comes only from the call table, which deletes activation contexts, leases and
choice refs. Layout limits are gone (six numbers remain, none about layout). Reading List
needs 7 interface lines, Notes 11, Chat 19, against 285, 584 and up to 493 JSON lines in
revision 14.

**Top three questions:** 1 (collection fields for mixed boards), 6 (a generic records view
in the catalogue), 2 (your sidebar is yours alone).

## Simplification opportunities

- Delete GenUI (`crates/genui`, `packages/genui`, the Apple `GenUI/` folder): a second
  description language (question 7).
- Delete the Python description verifiers and `description-v2.schema.json`; generate one
  short catalogue schema from the SDK's TypeScript types and run it in the installer only.
- Delete item-view snapshots for record files; keep thumbnails as ordinary files.
- Derive forms, the inspector, "New" buttons and the empty state instead of declaring them.
- Let the window title bar carry Stop and the working line for every app, instead of each
  app drawing its own.
- `packages/description/src/invoke/` shrinks to one function once activation contexts go.
- The Trash, Hidden and Archived entries are ordinary collections, not special shell rows.
