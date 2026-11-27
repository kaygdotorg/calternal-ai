# What a plugin can and cannot change · 2026-11-27

Status: **proposal, paper only.** An audit of the plugin extension surface
against the owner's goal of 2026-11-27: "i want a very strong, very
versatile plugin API that allows plugins to modify nearly everything
(except core; nearly) for the spaces they own!", on a server that stays
simple, generic, dumb, extremely secure and opinionated (AGENTS.md, "Who
decides what"). Nothing here is decided. Section 4 separates what the
coordinator can decide from the few questions for the owner. Lane issue:
[#110](https://git.kayg.org/kayg/calternal-ai/issues/110).

Read for this audit: `docs/morning-2026-11-21.md` (all rounds, mainly 24 to
33), [primitives-only-host-2026-11-21.md](primitives-only-host-2026-11-21.md)
(revision 5), [primitives/](primitives/) (mainly `drawing.md`),
[kind-and-collection-format-2026-11-24.md](kind-and-collection-format-2026-11-24.md),
[host-gaps-2026-11-24.md](host-gaps-2026-11-24.md),
[format-stress-test-money-mail-chat-2026-11-24.md](format-stress-test-money-mail-chat-2026-11-24.md),
`docs/plugin-api.md`, `docs/ui-shell.md`, `docs/open-questions.md`,
`docs/inheritance-audit.md`, `docs/composer.md`. Prior art was read with
Firecrawl on 2026-11-27 (§6).

**Citations** are `[file line]`, with these short names:

| Short | File |
| --- | --- |
| MR | `docs/morning-2026-11-21.md` |
| P | `docs/proposals/primitives-only-host-2026-11-21.md` |
| D | `docs/proposals/primitives/drawing.md` |
| F | `docs/proposals/kind-and-collection-format-2026-11-24.md` |
| H | `docs/proposals/host-gaps-2026-11-24.md` |
| X | `docs/proposals/format-stress-test-money-mail-chat-2026-11-24.md` (its §4 table, by item number) |
| Au | `docs/proposals/primitives/authoring.md` |
| A | `docs/plugin-api.md` (the revision-14 API: inherited intent, not yet carried into the format) |
| S | `docs/ui-shell.md` |
| Q | `docs/open-questions.md` |
| I | `docs/inheritance-audit.md` |
| C | `docs/composer.md` |

## 0. The answer in eight lines

1. Today a plugin can already change most of **its own** app: kinds,
   fields, checks, operations, looks, pages, collections, totals, sidebars,
   search, templates, events, jobs, sign-ins, harnesses and skills.
2. Since 2026-11-27 it can also **add** fields, operations, collections and
   sidebar lines to another plugin's kind, and fill slots that app offers.
3. The "no" rows fall into five groups, and five generic mechanisms cover
   nearly all of them (§2): **places** (slots everywhere, decided by the
   host instead of hoped for from each app), **extensions of another
   kind** (one `extends:` block), **reactions** (after an operation, and a
   declared refusal before it), **themes as tokens**, and **space
   profiles** (per-space defaults a plugin can ship).
4. None of the five adds an app word to the server. Together they add
   one constant table of host places, one key column to the call check,
   one event, one file kind (themes) and one file per space.
5. Closed for good: changing another plugin's inputs, and anything that
   touches sign-in, consent, the call check, Stop, sync or another space
   (§3). Waiting for the owner: plugin code on the device (O1).
6. Of the 82 rows in §1: **34 yes, 24 partly, 24 no**. Of the 24 no rows,
   six stay closed for good, one (the canvas) waits for its own design, and
   17 open with a §2 mechanism, a format line or owner question O1.
7. Plugin code on the device would close the last real gap (offline coded
   operations, custom field types, plugin formula functions, custom merge).
   It is affordable only as pure functions in an interpreter with no host
   functions at all, never as browser JavaScript and never for drawing;
   Apple's current rules allow downloaded JavaScript plug-ins under
   conditions the sealed frame already brings us under (§5.1, §6.2).
8. Four questions for the owner (§4.2); the rest the coordinator can
   decide (§4.1).

## 1. Everything in a space a plugin might want to change

"Today" means the design as decided on `dev` at `f8c91151` (nothing of it
is built beyond spikes). **Yes**: designed and decided. **Partly**: some of
it, or only in the revision-14 API and not yet in the format. **No**: no
way at all. "Mechanism" names how: a **declaration** (a line in a
Markdown file), a **slot** (G13), an **operation** (code that runs on an
event), or a **frame** (the sealed web view). §2 numbers (M1 to M5) point
to the mechanisms of section 2; "format line" means a missing key that
needs no new mechanism.

### 1.1 Its own kinds

| # | What | Today | Mechanism and source | If not: the smallest generic way | What stays closed, and why |
| --- | --- | --- | --- | --- | --- |
| 1 | Declare kinds | Yes | declaration, a kind file [F 40-75], [P 57] | | A kind ID is unique in the space [H 92-97]; `editable` is fixed for life [F 68], [MR 1213]: tampering (T5 [P 319]) |
| 2 | Fields | Yes | `fields:` [F 69]; updates by fixed rules, no migration code (G1) [MR 1220] | | |
| 3 | Field types | Partly | a closed list of 16 types [F 82-99] | **Named types**, a format line in `plugin.md`: a base type plus checks and a display hint (`isbn: text, check: …, shown as: code`). A type with its own editor needs O1 or a frame | The stored base type stays from the closed list: the merge (S2 [MR 1200]), the index and formulas must understand every value |
| 4 | Computed fields | Yes | formulas in kind and collection files [MR 1190], [F 183-223] | | Bounded cost, one hop, no cycles [F 210-216] |
| 5 | Its own formula functions | No | the engine's one list; a plugin asks by issue [MR 1298] | O1 (pure functions on the device), or a new engine function by issue | The engine list is one for every app until O1 is answered |
| 6 | Rules beyond types | Yes | `checks:` lines [H 57-71], [MR 1235] | | |
| 7 | Fields that move together | Yes | `together:` [MR 1235] | | |
| 8 | How its items merge | Partly | one generic field merge from the kind file [MR 1200], [P 475-492] | A merge function needs O1 | The sync route and its journal (ADR 0044 [P 446]) |
| 9 | Operations | Yes | one operations list [MR 1234]; `operations.ts` [F 18-28]; events [MR 1156], [H 218-221] | | |
| 10 | Who may change its items | Yes | `change: manager` (G5) [MR 1282] | | A kind can only raise the role, never lower it below editor |
| 11 | Templates; an item that is a folder | Yes | `template:`, `item: folder with …` (G10) [MR 1260], [F 589] | | |
| 12 | What search reads and how it weighs | Yes | `search:` (G9) [MR 1259] | | Ranking across kinds is the host's [H 129-131] |
| 13 | Looks: row, card, cell, page, record | Yes | declaration [D 77-110], [F 71] | | Sizes, colours, fonts, spacing and motion are the renderer's [D 396] |
| 14 | An item page with its own content | Yes | page `form`, `editor`, `records`, `collection`, `frame` or a `stack` [D 88]; frame for the rest [D 294-323] | | The frame's network hole is told at install [D 312-323] |
| 15 | Views of a collection | Partly | a closed set: list, table, board, gallery, calendar, timeline, chart, compact, strip; map with Photos [F 141-147], [MR 1236] | **M1** host place `collection view`: a frame that receives the window's rows as the viewer, read-only, and may call its plugin's operations. A new native view only by joining the catalogue | The catalogue itself: each native view is written once per platform |
| 16 | Collections and totals | Yes | collection files [F 116-147]; `totals:` with `all()` (F2) [MR 1286] | | |
| 17 | Sidebars and their default order | Yes | sidebar files [F 149-162], [MR 1130] | | Customising stays add, remove, reorder [MR 1131] |
| 18 | Right-panel tabs for its kinds | Yes | `panels:` [D 208-212] | | The AI tab is the space's pick [D 225] |
| 19 | Where its actions show (swipe, menu, toolbar, primary) | Yes | declaration [D 255-258] | | Overflow goes to More, never refused [D 361-372] |
| 20 | A confirmation sentence | Yes | declared, shown in the host's sheet [D 262-264] | | Challenges for host-known risks are host-only [D 263-264] |
| 21 | Its own settings, for a person and for the space | Partly | typed settings [A 182-217], inherited ADR 0055 [I 80]; not yet in the format | A `settings:` block in `plugin.md`; the Settings pane is derived (format line) | The Settings frame and the host's panes [D 237-239] |
| 22 | Per-person state | Partly | `s.state`, 64 KiB [H 214]; declared keys were invented in [X 10] | A `state:` block in the kind (format line) | Another person's state [D 428] |
| 23 | Empty states | Partly | derived "No items" [D 525-528] | An `empty:` line on a collection: one sentence and one operation (format line) | |
| 24 | Notifications | Partly | `s.notify` [H 210], declared phrases only [Au 606-608, 626-627] | A notification carries the item's declared operations as its actions (format line) | Free text: it leaks onto lock screens and enables phishing |
| 25 | App name, icon, badge | Partly | name; icon from the host set or one bounded SVG path [S 267]; apps in the strip [MR 1271]; no badge | A `badge:` line naming one collection whose count is shown (format line) | Icons are never images or HTML [S 267] |
| 26 | CLI, API and MCP for its operations | Yes | derived from one declaration [P 283-290] | | Tokens are always one person's [P 292] |
| 27 | Background jobs, events, held connections | Yes | [MR 1100-1108], [MR 1156], [MR 1169] | | Quotas, one meter [P 326] |
| 28 | Network, secrets, sign-ins | Yes | declared in `plugin.md` [P 97-101]; several sign-ins (G12) [MR 1268] | | A secret's value; a destination not listed (T3 [P 317]) |
| 29 | Import: open a file type, receive from the share sheet, accept a drop | No | an operation can write items, but nothing routes a file or a share to it | **M1** many-place `receive`: typed input (file type, URL, text) to one operation | The person picks the target every time unless they set a default |
| 30 | Export in its own format | Partly | every kind is Markdown in every export [MR 1095] | **M1** many-place `export`: an operation that turns a selection into a file | The generic Markdown export is always there |
| 31 | Talk to outside servers | Yes | HTTP, IMAP, SMTP by the server [MR 1159]; destinations [P 97-101] | | Private addresses [Q 94] |
| 32 | Answer outside requests (a CalDAV client, an ActivityPub inbox, a webhook that needs a reply) | Partly | `on.outside` with a person's token or a secret address [H 219-220], [MR 1231]; no reply body, no protocol path | Widen `on.outside` to return a bounded reply under the plugin's own path: owner question **O4** | The server's own routes and sign-in |
| 33 | Change how sync works | No | one route and journal [P 446] | None: stays closed | Data loss and divergence between devices (§3) |
| 34 | Coded operations that work offline | No | coded operations queue, "not sent yet" (S1) [MR 1199] | O1 | |

### 1.2 Another plugin's kinds

| # | What | Today | Mechanism and source | If not: the smallest generic way | What stays closed, and why |
| --- | --- | --- | --- | --- | --- |
| 35 | Read its items | Yes | reading is free inside a space, within the person's rights [MR 1120], [P 54] | | Outside the space (T1 [P 315]) |
| 36 | Change its items | Yes, through its operations | [MR 1120], [P 73], T2 [P 316] | | Direct writes to its fields |
| 37 | Add fields to its kind | Yes | decided 2026-11-27 [MR 1295] | Written as **M2** | The owning plugin's own fields |
| 38 | Add operations to its kind (menu, swipe) | Yes | decided 2026-11-27 [MR 1296] | Written as **M2** | The added operation changes the item only through the owner's operations [MR 1296] |
| 39 | Add collections, views and totals over its kind | Yes | [MR 1297] | | |
| 40 | Add a collection to its sidebar | Yes | G14 [MR 1284] | | The person can remove the line [MR 1284] |
| 41 | Fill a slot it offers | Yes | G13, declared in `plugin.md` [MR 1283], [D 214-235] | | A plugin never changes what another part put in a slot (no `wrap`) [D 232-233] |
| 42 | Show its added fields on the other kind's row, card or page | No | a kind's look is declared once, by its owner [D 109-110] | **M2** look additions: appended at the lowest priority | Places the owning kind marks protected (a sender, a warning) |
| 43 | Replace how the other kind's items are listed | Partly | only through `<app>.list` if the app offers it, picked by the person [D 227], [D 405-406] | **M1** host place `kind list` on every kind | Protected places are drawn whatever the replacement says |
| 44 | Add a panel tab or a page section to its items | Partly | only through panel slots the app declares [D 210] | **M1** host places `kind panel` and `kind page end` on every kind | |
| 45 | Add a rule to its kind | No | `checks:` belong to the kind file [H 64] | **M2** marks: a formula that flags, never refuses; refusing is **M3**, owner question **O2** | |
| 46 | React after one of its operations | Partly | `on.change(kind)` sees file changes [H 219], not which operation ran or its result | **M3** `on.after(plugin.operation)` | The listener cannot change the result |
| 47 | Act before one of its operations: refuse it | No | | **M3** declared refusal (`refuses:`), formula only: owner question **O2** | Code that blocks, latency coupling (§5.2) |
| 48 | Act before one of its operations: change its inputs | No | | None | Stays closed: the author's stamp would sign words they did not write (§3, §5.2) |
| 49 | Remove, rename or retype its fields or operations | No | | None | Stays closed: breaks the owning plugin; G1's rules are the owner's [MR 1220] |
| 50 | Add a formula function for its kind | No | [MR 1298] | O1 | |
| 51 | Add to its folder's `AGENTS.md` | No | written from the kind file [P 179-182] | Derived: added fields get a generated section "Fields added by …" (no mechanism) | |
| 52 | Offer an action on every kind that has a certain field ("Save link" on anything with a URL) | No | additions name one kind [MR 1296] | **M2** target `any kind with <field type>` | |

### 1.3 The shell

| # | What | Today | Mechanism and source | If not: the smallest generic way | What stays closed, and why |
| --- | --- | --- | --- | --- | --- |
| 53 | Appear in the app strip | Yes | an app in `plugin.md`; the strip holds apps and the person's own sidebars [MR 1264], [MR 1271], [S 267] | | Order is the person's [S 238] |
| 54 | Replace the strip or the switcher | Yes | slot `shell.switcher`, picked by the person [D 223] | Renamed **M1** single place `shell strip` | Safe mode and the profile popover stay reachable [D 237-243] |
| 55 | The sidebar's content | Yes | the sidebar file [D 224], [F 149-162] | | |
| 56 | The sidebar's header (wordmark) | No | shell behaviour; a plugin supplies only its name [D 244-247] | **M4** a theme sets the wordmark face | |
| 57 | Items in "Needs you" | Yes | a waiting work unit's question [D 201-206] | | Consent requests there are host-drawn [D 205-206] |
| 58 | A window's title bar | Partly | host-drawn: icon, title, the kind's actions, close, Stop [D 194-198] | Actions only, as today | Stop, close and the title: spoofing another item, hiding a run |
| 59 | The toolbar | Partly | `toolbar` actions [D 257]; buttons only | A catalogue toolbar item bound to a field or a view switch (catalogue, no new mechanism) | |
| 60 | The right panel | Yes | slot `shell.panel` [D 225] | | |
| 61 | An app's composer | Yes | slot `<app>.composer`, picked by the person [D 226], with a narrow contract [D 448-450] | | "This part sees what you type here" on the pick screen [D 421] |
| 62 | The command list (palette) | Partly | `palette.commands` in the revision-14 API [A 502-512]; derived from declarations [I 80]; not in the format | **M1** many-place `palette`: every operation is listed; a plugin adds phrases (`words:`) | The palette's own ranking and host commands |
| 63 | Keyboard shortcuts | No | a file per account, set by the person [C 27], [S 141-142] | A `key:` default on an operation line; the person rebinds (format line) | Host-reserved chords (Escape, safe mode, search, Stop): trapping |
| 64 | Search results | Partly | items of every kind by `search:` [MR 1259] | **M1** many-place `search`: an operation returns rows, called only when the person picks that scope ("Search the mail server") | Ranking across sources stays the host's; never called on every keystroke (it would hand every keystroke to a plugin with network) |
| 65 | Notifications from another app | No | | Through **M3** (`on.after`) plus its own `s.notify` | |
| 66 | Settings panes | Partly | its own pane derived from its settings [I 80] | Row 21 | Settings, Plugins and the profile popover are host-only [D 237-239] |
| 67 | First open of an app in a space (setup) | No | | **M5** space profile plus **M1** single place `first run` (a form derived from one operation) | |
| 68 | Account sign-up, sign-in, passkeys | No | host only [D 237] | None | Stays closed: credential phishing (§3) |
| 69 | The canvas of windows | No | host (sideways columns, I3) [P 380]; once meant to become a shell plugin [Q 72] | Not now: options in Settings, Appearance | Window management in every renderer is the costliest thing to open |
| 70 | The share sheet | No | the host's sheet [P 78] | None | Stays closed: widening sharing without the host's sheet (§3) |
| 71 | Stop, the Plugins page, safe mode, challenges, consent | No | host only [D 237-243], [D 401-404] | None | Stays closed: trapping and spoofing (§3) |

### 1.4 Look

| # | What | Today | Mechanism and source | If not: the smallest generic way | What stays closed, and why |
| --- | --- | --- | --- | --- | --- |
| 72 | A theme (colours, materials) | Partly | decided: plugins ship themes; tokens, never CSS [Q 87]; palettes of eight colours plus rules [`docs/ui-backgrounds-and-themes.md` 25]; not in the format | **M4** theme files | No CSS: positioning, hiding host marks |
| 73 | How another plugin's rows look | No | [D 405-406] | **M4** tokens scoped to an app or kind (accent, density), and **M2** look additions | Layout and protected places |
| 74 | Icons | Partly | the host set or one SVG path [S 267] | **M4** an icon map (bounded SVG paths per host icon name) | Images and scripts as icons |
| 75 | Fonts | Partly | the person picks from catalogues or uploads [S 270-289] | **M4** a theme names families; bundled files pass the font validator [S 282-287] | |
| 76 | Sounds and haptics | No | nothing for sounds; haptics are the shell's [`docs/description-contract.md` 409] | **M4** sound tokens: one short file per host sound event | Haptic patterns: host only |
| 77 | Motion | No | declarations carry no motion; tokens are the host's [D 325-345] | **M4** a theme picks one of the host's motion presets | Curves per component; reduced motion always wins |
| 78 | Wallpapers and the window material | Partly | the person's settings [S 97-104], [MR 1264] | **M4** a theme ships wallpapers and a default material | |

### 1.5 A space's defaults and rules

| # | What | Today | Mechanism and source | If not: the smallest generic way | What stays closed, and why |
| --- | --- | --- | --- | --- | --- |
| 79 | The space's default sidebars, look and strip | Partly | sidebars: the Owner and Managers set the default [P 212]; look and strip: nothing | **M5**, and owner question **O3** for the look | What only one person sees stays theirs [D 656-657] |
| 80 | Rights and agent rules for the space | Partly | a kind raises its own role (G5) [MR 1282]; rules are a ceiling people narrow (R2) [MR 1246] | **M5** a profile may raise a role or narrow a rule, never the reverse | Widening a right or loosening a rule (§3) |

### 1.6 Agents, harnesses, skills, automations

| # | What | Today | Mechanism and source | If not: the smallest generic way | What stays closed, and why |
| --- | --- | --- | --- | --- | --- |
| 81 | Harnesses, skills, automations, operations as agent tools | Yes | agents are plugins [MR 1212], [MR 1224], G4 [MR 1223]; the skills box [F 578-611]; `calternal-automations` [MR 1230-1232]; MCP [P 285] | | The agent rules engine: a plugin may not loosen the person's or space's rules (R1 to R7 [MR 1243-1251]) |

### 1.7 Native surfaces

| # | What | Today | Mechanism and source | If not: the smallest generic way | What stays closed, and why |
| --- | --- | --- | --- | --- | --- |
| 82 | Widgets, Live Activities, the Mac menu bar, the share extension, Shortcuts, Spotlight, the keyboard | No | native features are the app's and store through plugin operations [P 276-279], [MR 1085] | **M1** many-places `widget` and `menu bar` (a collection in its compact view, operations as buttons, drawn by the native app from synced data); the share extension is row 29's `receive`; Spotlight indexes every kind from its `search:` line with no plugin part; Shortcuts gets one generic "Run an operation" action | Code in an app extension; anything on the lock screen the person did not add |

**Count:** 82 rows: **yes 34, partly 24, no 24.** Of the 24 no rows, six
stay closed for good (33 sync, 48 changing another plugin's inputs, 49
removing its fields, 68 sign-in, 70 the share sheet, 71 Stop and the host
screens; §3), one waits for its own design (69 the canvas), and 17 open
with M1 to M5, a format line or O1 (5, 29, 34, 42, 45, 47, 50, 51, 52, 56,
63, 65, 67, 73, 76, 77, 82).

## 2. Five generic mechanisms

Each is one mechanism in the server, named without an app word, and each
reuses a part the design already has.

### M1. Places: slots everywhere, defined by the host

**Problem.** Today a plugin can add to another app only where that app
thought to offer a slot (G13 [MR 1283]). Most "no" rows in the shell and on
other kinds' pages are places no app will think to offer.

**Shape.** The host defines a fixed list of places, the same in every
space; apps keep declaring their own (G13). A place is **single** (one
part fills it: the strip, a composer) or **many** (each fill adds an entry:
panel tabs, palette entries, share targets). Every place states what it
gives (the open item, the selection, the typed text), which operations a
part in it may call, and who picks (by effect: the person for what only
they see, the space for what others receive [D 221-228]).

| Host place | Single or many | Gives | Who picks |
| --- | --- | --- | --- |
| `shell strip` | single | the apps and sidebars | person |
| `shell panel` | many | the open window's item | person; the AI tab the space |
| `palette` | many | the typed text, only for entries the person opens | automatic (operations), the person may hide |
| `search` | many | the query, only when the person picks that scope | person |
| `receive` | many | a file, URL or text the person shares or drops | person, each time or as a default |
| `export` | many | the selection | person |
| `first run` | single per app | nothing | the space (M5) |
| `kind <kind> panel` | many | the open item | person |
| `kind <kind> page end` | many | the open item | person |
| `kind <kind> list` | single | the rows of a window | person |
| `collection view` | many | the window's rows | person, per collection |
| `widget`, `menu bar` | many | a collection's compact view | person, on the device |

```markdown
<!-- acme-summaries/plugin.md -->
---
plugin: acme-summaries
fills:
  - kind calternal-mail.messages panel: frame panels/summary.html
  - search: operation search-archive, as "Mail archive"
  - receive: url, text -> operation calternal-reading-list.add
---
```

**Example.** A summaries plugin adds a "Summary" tab to every mail without
Mail offering a slot; a Reading List plugin becomes a target of the share
sheet and of a drop onto the window.

**Security.** A place gives only what its row says, filled as the viewer;
a part calls only its own plugin's operations and what its `calls:` lists,
checked at the one table [D 265-268]. Places that see what the person types
(palette, search, composer) say so on the pick screen [D 421] and are
called only when the person opens that entry or scope, so no plugin with
network receives every keystroke. A broken part draws an error in its own
place only [D 234-235]. Protected places of a kind (M2) are drawn whatever
a `kind list` replacement says.

**What it adds to the server.** A constant table of host place names,
built once (gate item 6), next to the slot picks the server already keeps
(step 8 of [P 351]). Drawing is the client's. The names are the shell's own
nouns (kind, collection, panel, palette), not app words.

### M2. Extending another plugin's kind: one `extends:` block

**Problem.** The 2026-11-27 decisions allow added fields, operations,
collections and sidebar lines [MR 1295-1297], [MR 1284], but say nothing
about showing an added field, writing it on an operations-only kind,
flagging items, or targeting "anything with a URL".

**Shape.**

```markdown
<!-- calternal-reading-list/plugin.md -->
---
plugin: calternal-reading-list
extends:
  calternal-mail.messages:
    fields:
      saved: yes/no, default no
    operations:
      send to reading list: code, menu
    row: + saved
    page: + section "Reading": saved
    marks:
      newsletter: = contains(from, "newsletter")
  any kind with url:
    operations:
      save link: add reading (url = url, title = title), menu
---
```

The owning kind may name **protected** places, which an addition can never
push out of view (`protected: from, outside sender` on Mail's row).

- An added field is stored in the item's frontmatter under the adding
  plugin's name (`calternal-reading-list.saved: yes`), and only the adding
  plugin's operations, or a person on an editable-outside kind, may write
  it. This holds on an operations-only kind too: the owner's own fields
  still change only through the owner [MR 1120].
- Look additions are appended at the lowest priority (`detail` from its
  end, `trailing` after the owner's), so the owner's places drop last
  [D 102-103].
- A mark is a formula that shows a label; it never refuses (that is M3).
- `any kind with <type>` resolves at install against the kinds present and
  again when a kind is installed later.
- The install sheet lists every addition by target ("Adds a 'Saved' field
  and 'Send to Reading List' to Mail messages"); the person can turn each
  off; removing the plugin hides them and keeps the values (G1 [MR 1220]).

**Security.** Additions cannot remove, rename, retype or reorder the
owner's fields, operations or places (row 49). The added operation runs as
the adding plugin, so it changes the owner's fields only through the
owner's operations [MR 1296]. A field write is stamped with the adding
plugin, so history shows who set it. The real risk is confusion (an added
badge that imitates a warning); protected places and the install sheet are
the answer.

**What it adds to the server.** One more column in the call check: a write
by a plugin to another plugin's kind is allowed only for keys under its own
name. The check already knows the plugin and the kind; the key prefix is
one comparison per changed key. The index learns kinds by field type, once
per install.

### M3. Reactions: after an operation, and a declared refusal before it

**Problem.** `on.change` sees that a file changed, not which operation ran
with which inputs and result; nothing can stop another app's operation.

**Shape.**

```markdown
<!-- acme-budget-guard/plugin.md -->
---
plugin: acme-budget-guard
listens:
  - after calternal-money.add
refuses:
  calternal-money.add:
    - when amount > 1000 EUR and not account.joint:
        "Payments over 1,000 EUR go through the joint account"
---
```

```ts
// operations.ts
export const flagLarge = on.after("calternal-money.add", async (call, s) =>
  s.notify(s.space.managers, { item: call.result.item, text: "large payment" }));
```

- **`on.after`** is an event like the six that exist [H 218-221]: delivered
  at least once after the operation committed, with its declared inputs
  and result filtered to what the acting person may read, run as the
  listener within the acting person's rights. It cannot change the result;
  it may call operations, including the owner's own undo. Depth 8 and "its
  own writes do not fire itself" apply [H 213], [H 223-226].
- **`refuses:`** (only if the owner says yes to O2) is a condition in the
  formula language, evaluated by the shared engine before the operation
  runs, on the server and on the device, with the formula limits
  [F 210-216]. The refusal shows the plugin's name and sentence. It never
  changes an input and never runs code. It cannot target Stop, undo,
  removing one's own items or records, or leaving a space: a person can
  always stop, undo and delete what is theirs.

**Example.** A household guard refuses large payments from a personal
account; a spam plugin moves new mail to Junk after `calternal-mail.receive`.

**Security.** After-reactions see nothing the listener could not read
anyway (reading is free in the space [P 54]). A refusal is data, not code:
it cannot leak (a formula has no network), cannot be slow beyond the step
budget, and runs the same offline, so an offline change and its replay
agree. The only harm is refusing too much, which the Owner installed after
seeing the sentence and can turn off.

**What it adds to the server.** One event type, raised by the call route
after commit (the route already writes an audit row per call). The engine
already evaluates `checks:`; `refuses:` attaches more checks to an
operation from a second source.

### M4. Themes as tokens

**Problem.** Themes, icons, fonts, sounds and motion were decided as
"tokens, never CSS" [Q 87] but have no file, and nothing lets a theme give
one app its own accent.

**Shape.** `themes/<name>.md` in a plugin; installed into
`.calternal/themes/`; picked by the person (and given as a default by M5).

```markdown
---
theme: dusk
name: Dusk
palette: "#1f1f1f #f7f7f7 #c96f4a #5b7f9e #d9a441 #6f8f5b #a65e7c #8a8a8a"
fonts:
  interface: Google Sans Flex
  code: Cascadia Code
  wordmark: files/fonts/wordmark.woff2
icons: files/icons/          # one SVG path per host icon name
sounds:
  sent: files/sounds/sent.m4a
motion: calm                 # one of the host's presets
wallpaper: files/dusk.avif
window: frosted
for:
  calternal-money: accent "#2f7d5b", density compact
---
```

**Security.** No CSS, no positions, no sizes: a theme cannot move, cover
or hide anything. The renderer keeps a contrast floor, so no theme can make
text or a warning unreadable; reduced motion always wins; the host mark on
host screens is not themed [D 414]. Assets pass the font validator
[S 282-287], the wallpaper pipeline [S 97-104] and a new, equally small
sound check (type by magic number, a few seconds, a few hundred KiB).

**What it adds to the server.** One file kind and stored assets, served
like fonts and wallpapers already are [S 282-287]. Tokens are applied on
the device.

### M5. Space profiles: defaults a plugin can ship

**Problem.** A space has no file for its own defaults: look, strip, the
picks for places others see, rights and rules. A plugin cannot ship "a
family budget" set up the right way, and nothing runs when an app first
opens in a space.

**Shape.** `.calternal/space.md`, written by the Owner or by applying a
profile a plugin ships (`profiles/<name>.md`).

```markdown
---
space profile: family-money
theme: dusk                     # a default; each member may pick their own
strip: calternal-money, calternal-tasks
picks:
  shell panel: calternal-chat-claude-code
rights:
  calternal-money.transactions: change manager
agent rules:
  - ask before remove in calternal-money
first run:
  calternal-money: operation calternal-money.setup
---
```

**Security.** Applying a profile is the Owner's yes on the install sheet,
which lists every line. A profile only raises roles and narrows rules
(R2 [MR 1246]); it can never widen. What only one person sees stays theirs
to change. A profile names plugins; it never installs code by itself (the
install sheet does).

**What it adds to the server.** One file per space, read at space open and
cached by revision, like a sidebar. The rights and rules it holds feed the
check and rule engine that exist.

## 3. What is never open, and the attack each closure prevents

| Never open | The attack it prevents |
| --- | --- |
| Sign-in, passkeys, recovery, the profile popover, Settings and Plugins pages, challenges, consent requests, Stop, safe mode [D 237-243] | A fake "sign in again" that steals a passkey ceremony or a password; a broken or hostile part that traps the person with no way to turn it off |
| The one call check and its table [D 265-268], [P 90-91] | A plugin that adds an allow rule and acts beyond the person's rights (privilege escalation); M2 and M5 may only narrow |
| Changing another plugin's inputs before an operation (row 48) | Forged authorship: Lee's stamp on words a plugin wrote; a money transfer redirected to another account |
| Removing, renaming or retyping another plugin's fields or operations (row 49) | Silent data loss and a broken owner plugin after any update |
| A file's kind, ID, author stamp and outside-editing switch [P 76], T5, T6 [P 319-320] | Turning a chat into an editable note to rewrite history; posting as someone else |
| Another space: reading, collections, formulas [P 72], S4 [MR 1202] | A plugin installed in a shared space reading the person's private space |
| The sync route, the journal, the merge (row 33) | Divergent devices and lost changes; a merge that drops a field everyone else keeps |
| Secrets' values, the network list, programs [P 74-75] | Exfiltration to an undeclared server; a new destination slipped into an update |
| Code in the client outside a frame, and CSS (rows 72, 73) | Clickjacking, covering the host's controls, hiding warnings |
| Free notification text [Au 626-627] | A message body shown on a locked phone; phishing in a push |
| The share sheet and any widening of sharing [P 78] | A plugin quietly sharing an item outside the space |
| Host-reserved shortcuts (row 63) | A part that takes Escape or the safe-mode chord and traps the person |
| Another person's private state [D 428] | Learning who muted whom, reading someone's drafts |
| Refusing Stop, undo, deleting one's own items, leaving a space (M3) | Holding a person's data or a running agent hostage |
| The canvas of windows (row 69), for now | Not an attack: a window manager in every renderer is the costliest thing to open; reopen with a design |

## 4. Open points

### 4.1 For the coordinator (detail; the most versatile choice recommended)

1. **Write M1 to M5 into the format** as proposed, and add the format lines
   of rows 3, 21, 22, 23, 24, 25, 51, 62, 63. Recommendation: all of them;
   each is one key with no new server part.
2. **Host place list** (M1): start with the twelve places in the table;
   more are added only by the same test as server features (useful to two
   unrelated apps, no app word [MR 1210]).
3. **Protected places** (M2): Mail and Chat mark their sender and any
   outside-sender warning protected; every kind's `title` is protected by
   default.
4. **Where added field values live** (M2): in the item's frontmatter under
   the adding plugin's name, not in a side file, so exports and outside
   readers see them; recommended over a side file because one place per
   fact wins and the merge already handles frontmatter keys.
5. **`any kind with <type>`** (M2): allowed for `url`, `file`, `person`,
   `place`, `time` and `date` only; enough for "save link", "open in maps",
   "add to calendar".
6. **`on.after` payload** (M3): the operation's declared inputs and its
   result, filtered as the acting person; never the plugin's private state.
7. **Search providers** (row 64): only on a picked scope, with a time
   budget per answer (the 3 s tap rule [Q 61]); results ranked within their
   own section, never mixed into the host's ranking.
8. **Themes scoped to an app** (M4): accent and density only.
9. **Notification actions** (row 24): the item's declared operations,
   performed with the same call check; on a locked device only those the
   person marked safe.
10. **Shortcuts** (row 63): a plugin's default chord loses to the person's
    and to the host's reserved list, and the conflict is shown in Settings.
11. **Native surfaces** (row 82): build `widget` and `menu bar` with the
    native shell lane; Spotlight from `search:` needs no plugin part.
12. **Record each of these in the morning report** under "Decided by the
    coordinator" with this document as the reason (AGENTS.md).

### 4.2 For the owner (high level, at most six)

**O1. May plugin code run on your devices, as pure functions with no
access to anything?**

- *What:* today a plugin's code runs only on the server; devices draw its
  declarations and frames. A *pure module* would be a small part of a
  plugin (plain JavaScript functions: compute a value, check or format a
  field, merge two versions, preview an operation's changes) that the
  shared Rust core runs inside its own interpreter, with no network, no
  files, no clock beyond what it is handed and a fixed step budget.
- *Why now:* it decides whether coded operations ever work offline (S1
  queues them [MR 1199]), whether plugins may add formula functions (held
  back by [MR 1298]), custom field types (row 3) and custom merges (row 8).
- *Example:* on a plane you split a 90 EUR dinner three ways in Money.
  Without a module the split waits as "not sent yet"; with one, the three
  transactions appear at once and are confirmed on landing.
- *Options:* (a) **pure modules inside the core's interpreter, never for
  drawing**: costs about a megabyte in the app and in the web download,
  interpreted speed, and one more thing to keep safe; Apple allows
  downloaded JavaScript plug-ins under conditions the sealed frame already
  brings us under (§5.1). (b) **No device code,
  as now**: costs offline coded operations, plugin formula functions,
  custom types and merges for good. (c) Modules as browser JavaScript or
  Wasm: rejected, the browser gives them network (§5.1).
- *Recommendation:* (a), started with formula functions and field checks
  only, since those are pure by nature and the engine already runs on the
  device as Wasm [MR 1289].

**O2. May one plugin stop another plugin's action before it happens?**

- *What:* a plugin could add a rule to another app's action that refuses
  it, with a sentence ("Payments over 1,000 EUR go through the joint
  account").
- *Why now:* it decides whether policy plugins (household rules, spam
  rules, compliance) can exist, and it is a new kind of power between
  plugins.
- *Example:* the guard above, installed in the family space, refuses a
  large payment Ana enters from her personal account.
- *Options:* (a) **yes, as a declared condition only**, no code, never
  changing what was entered, named in the refusal, never able to refuse
  Stop, undo or deleting your own things: costs one more way for an
  installed plugin to get in the way. (b) **No, only after**: a plugin can
  react and undo afterwards; costs policies that act after the fact (the
  payment exists for a moment, others may have been notified). (c) Code
  that refuses: costs every action waiting on another plugin's process,
  and offline changes that look done and are refused later.
- *Recommendation:* (a). It gives the power without coupling apps' speed
  or safety to each other.

**O3. Whose is the look of a space: the space's, or each person's?**

- *What:* themes, wallpaper, the strip and the shell's layout can be set
  for a whole space or chosen by each member.
- *Why now:* M4 and M5 need to know who wins.
- *Example:* Ana's family space uses a warm theme she picked; Lee prefers
  dark and his own wallpaper.
- *Options:* (a) **each person's, with a space default**: the space sets
  what a new member sees first and one accent colour that tells members
  which space they are in; every member can change their own look. (b)
  **The space's**: one look for all; costs members' own preferences. (c)
  Only each person's: costs a family or team any shared identity.
- *Recommendation:* (a), the rule you set for slots: what only you see,
  you pick; the space provides the start.

**O4. May a plugin answer requests from outside the server?**

- *What:* today a plugin can receive an outside call (a webhook) but
  cannot reply with content or speak a protocol. Answering would let a
  plugin serve CalDAV to Apple Calendar, an ActivityPub inbox, or a
  webhook that needs a reply.
- *Why now:* it decides whether Calendar and Tasks can sync with the
  phone's own apps through standard protocols, and it puts a plugin on the
  internet.
- *Example:* your iPhone's Calendar app subscribes to your Calternal
  calendar over CalDAV.
- *Options:* (a) **yes, under the plugin's own path in that space, only
  with a person's token or a secret address, through declared operations,
  with a bounded reply**, shown at install as "Answers requests from
  outside at …": costs an internet-facing door per plugin. (b) **No**:
  costs standard-protocol sync with other apps.
- *Recommendation:* (a), since the token rules already make every outside
  call act as one person [P 292-295].

## 5. The three points examined

### 5.1 (a) Plugin code on the device

What it would unlock: offline coded operations, plugin formula functions,
custom field types and editors, custom merges. What it would not: native
custom views. Drawing from code is GenUI again, which was deleted for
being a second language [D 725-738]; a custom view stays a frame or a
catalogue addition.

**Web.** Plugin JavaScript run as browser JavaScript inherits the browser's
network: a sandboxed iframe still opens WebRTC connections that CSP does
not cover [D 312-323], and a Web Worker keeps `fetch` and the page's
storage unless it is wrapped again. ShadowRealm is not shippable (TC39
stage 2.7), and Hardened JavaScript (SES) says itself that it cannot stop
endless loops or memory exhaustion (§6). The cheaper and stronger form is
a JavaScript interpreter compiled into the shared Rust core (which already
ships to the browser as Wasm, [MR 1289]) with no host functions at all: the
module can only compute on what it is handed. Figma moved to exactly this
(QuickJS compiled to Wasm) after escapes were found in its first sandbox
(§6). Costs: download size (Figma calls it "medium-sized"; my estimate of
about a megabyte is not measured), no JIT, no browser devtools for
authors, and running the same interpreter on the server (not in the
plugin's Node process) so the device and the server agree.

**iOS.** Apple's current rules (§6.2) forbid downloaded code that changes
the app's features (2.5.2) but name "HTML5 and JavaScript … plug-ins" as
software an app may offer (4.7), and the developer agreement allows
downloaded *interpreted* code that keeps the app's advertised purpose,
bypasses no OS security and makes no storefront (DPLA 3.3.1 B). Two
consequences:

1. **The sealed frame already puts the native app under 4.7.** A frame is
   plugin HTML and JavaScript running on the device today. So the
   conditions apply whether or not O1 is answered yes: an index of every
   plugin with universal links (4.7.4), a way to report a plugin and block
   abuse (4.7.1), age flags (4.7.5), no native APIs exposed without
   Apple's permission (4.7.2), and explicit consent "in each instance"
   before a plugin gets data or a privacy permission (4.7.3). The install
   sheet is a consent; whether Apple reads "each instance" as each install
   or each use is not known. The index must not look like a store (3.2.2
   (i), DPLA (c)): the operator's suggested list (G7 [MR 1257]) is the
   closest thing we have and needs checking against that line.
2. **A pure module adds little exposure on top of the frame**: it is
   JavaScript, interpreted, with no native API at all. The one doubt is
   the interpreter: older versions of the agreement asked for Apple's own
   WebKit or JavaScriptCore; the current text quoted in §6.2 does not. If
   App Review objects, the same module runs in a bare `JSContext`
   (JavaScriptCore with nothing injected has no network and no timers),
   at the price of two engines whose results must agree; the module would
   then be forbidden dates, `Intl` and random numbers it was not handed.

**Security.** A module without I/O cannot leak; what it returns is drawn or
written through the same checks. The risks are an interpreter escape (a
memory-safe interpreter inside Wasm on the web, inside the Rust core on
iOS) and endless loops (the step budget).

### 5.2 (b) Reacting before another plugin's operation

Before-hooks are what makes Emacs advice and bb's `on(op, hook)` powerful
(§6, [D 232-233]), and what makes them fragile: one hook's latency becomes
every caller's, one crash becomes another app's failure, the order of
several hooks becomes behaviour, and offline changes the device already
applied are refused later on replay (S1 [MR 1199]). Changing inputs is
worse: the author's stamp then signs what a plugin wrote. Hence M3: after
in code, before only as a declared condition the engine evaluates on both
sides, and owner question O2.

### 5.3 (c) Whose is a space's look

Per-person state already lives in the person's own space [MR 1157]; the
person picks what only they see [D 221-228]; the device keeps what
describes one screen [D 49-53]. A space look forced on members contradicts
all three. The one shared element worth keeping is identity: a space accent
and icon, so a member always knows which space they are in. Hence O3 (a).

## 6. Prior art: what others allow that we do not, and what it costs them

Read with Firecrawl on 2026-11-27. The earlier study
([primitives/prior-art.md](primitives/prior-art.md)) covered Obsidian, VS
Code, bb, Figma, Raycast and Home Assistant for the host as a whole; this
pass asks only how each lets extensions change the host.

### 6.1 Six hosts

| Host | What its extensions may change that ours may not | What it costs them | What we take |
| --- | --- | --- | --- |
| **Obsidian** ([Plugin API](https://docs.obsidian.md/Reference/TypeScript+API/Plugin), [Plugin security](https://help.obsidian.md/plugin-security)) | Anything: commands, ribbon, status bar, settings tabs, custom views, editor extensions, Markdown post-processors, protocol and file-extension handlers; in practice plugins patch the app's and each other's code (common practice, not documented) | "Obsidian cannot reliably restrict plugins to specific permissions or access levels"; manual review only for popular or flagged plugins. Elastic reported a 2026 campaign that turned a synced vault's plugin settings into a malware launcher ([REF6598](https://www.elastic.co/security-labs/threat-command/phantom-in-the-vault)) | Handlers for file types and protocols (M1 `receive`); never synced plugin settings that run code |
| **VS Code** ([contribution points](https://code.visualstudio.com/api/references/contribution-points), [capabilities](https://code.visualstudio.com/api/extension-capabilities/overview), [runtime security](https://code.visualstudio.com/docs/configure/extensions/extension-runtime-security)) | Keybindings, menus, views, custom editors, themes, icon themes, product icon themes, colours, language features, configuration defaults; code with full Node access. But "Extensions have no access to the DOM of VS Code UI … cannot … apply custom CSS", so VS Code can change its own UI without breaking them | No permission model: an extension "can read and write files … make network requests, run external processes". GlassWorm (2025-10) poisoned OpenVSX and marketplace extensions ([Truesec](https://www.truesec.com/hub/blog/glassworm-self-propagating-vscode-extension)); a 2026 wave used `extensionPack` and `extensionDependencies` to pull in loaders later ([CSA](https://labs.cloudsecurityalliance.org/research/csa-research-note-glassworm-open-vsx-transitive-dependency-a/)) | Contribution points as host-owned places (M1); themes and icon themes as tokens (M4); no DOM, no CSS. A dependency (`needs:`) must never install code the install sheet did not show |
| **Raycast** ([security](https://developers.raycast.com/information/security), [store](https://developers.raycast.com/basics/prepare-an-extension-for-store)) | Commands with a closed set of native components, file and network access without a sandbox | "Not further sandboxed as far as policies for file I/O, networking"; safety rests on open source and review; a binary downloaded after review could be swapped, so the guidelines demand hash checks | The closed component set drawn natively (our catalogue); review is a second layer only |
| **Emacs** ([advice combinators](https://www.gnu.org/software/emacs/manual/html_node/elisp/Advice-Combinators.html), [advising named functions](https://www.gnu.org/software/emacs/manual/html_node/elisp/Advising-Named-Functions.html)) | Wrap any function `:before`, `:after`, `:around`, `:override`, `:filter-args`, `:filter-return`; every hook, keymap and variable is global | The manual: advice "can be a source of bugs, since existing callers … may assume the old behavior"; "If you are writing code for release … try to avoid including advice"; "It is generally cleaner to create a new hook in `foo` … than to have `bar` put advice in `foo`" | Named places the host owns, not general interception: the strongest argument for M3's shape (after-events and declared refusals, no `:around`, no `:filter-args`) |
| **Home Assistant** ([manifest](https://developers.home-assistant.io/docs/creating_integration_manifest), [blocking operations](https://contributors.home-assistant.io/core/internals/blocking-operations/)) | Arbitrary Python in the core process: entities, services, config flows; a custom integration with a built-in's name replaces it (common knowledge, not found on the page) | "A blocking call in the event loop stalls all of Home Assistant"; it only detects and logs most of them; every custom integration loads with a stability warning | A plugin never replaces another's name (kind and operation IDs are unique [H 92]); one process per plugin per space [P 232] |
| **Notion Workers and Coda (Superhuman) Packs** ([Notion capabilities](https://developers.notion.com/reference/capabilities), [Workers](https://developers.notion.com/workers/get-started/overview.md), [Coda building blocks](https://coda.io/packs/build/latest/connectors/), [column formats](https://coda.io/packs/build/latest/connectors/column-formats/), [fetcher](https://coda.io/packs/build/latest/connectors/features/fetcher/)) | Notion Workers: sync tables, agent tools, webhooks, hosted in "a sandboxed Node.js environment". Coda Packs: formulas, actions, sync tables, cards, column formats ("just metadata" pointing at a formula), run on Coda's servers with no `fetch` except a declared fetcher | Coda: "Packs don't ship their own UI", one network domain by default, many npm libraries break; Notion: integrations cannot change Notion's interface at all | The closest match to our design (server-side code, declared domains, host-held sign-ins, "a Pack only receives the parameters … asked for"). Coda's column format is our named field type (row 3) and look addition (M2); we go further with places and frames |

What none of them has that we keep: one call check for every change, a
declared network list enforced outside the plugin, and host screens no
plugin can draw.

### 6.2 Apple's rules on downloaded code

Read from
[App Store Review Guidelines](https://developer.apple.com/app-store/review/guidelines/)
("Last Updated: October 16, 2026", checked by scraping the page myself) and
the [Apple Developer Program License Agreement](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/)
(read by the research pass; the agreement's own version date was not
found).

- **2.5.2:** "Apps should be self-contained in their bundles, and may not
  read or write data outside the designated container area, nor may they
  download, install, or execute code which introduces or changes features
  or functionality of the app, including other apps."
- **4.7:** "Apps may offer certain software that is not embedded in the
  binary, specifically HTML5 and JavaScript mini apps and mini games,
  streaming games, chatbots, and plug-ins. … You are responsible for all
  such software offered in your app, including ensuring that such software
  complies with these Guidelines and all applicable laws."
- **4.7.1:** such software must follow the privacy rules of 5.1; "include a
  method for filtering objectionable material, a mechanism to report
  content and timely responses to concerns, and the ability to block
  abusive users"; and follow 3.1 for digital goods.
- **4.7.2:** "Your app may not extend or expose native platform APIs or
  technologies to the software without prior permission from Apple."
- **4.7.3:** "Your app may not share data or privacy permissions to any
  individual software offered in your app without explicit user consent in
  each instance."
- **4.7.4:** "You must provide an index of software and metadata available
  in your app. It must include universal links that lead to all of the
  software offered in your app."
- **4.7.5:** "Your app must provide a way for users to identify software
  that exceeds the app's age rating, and use an age restriction mechanism
  based on verified or declared age to limit access by underage users."
- **3.2.2 (i), unacceptable:** "Creating an interface for displaying
  third-party apps, extensions, or plug-ins similar to the App Store or as
  a general-interest collection."
- **DPLA 3.3.1 B** (the clause once numbered 3.3.2): "an Application may
  not download or install executable code. Interpreted code may be
  downloaded to an Application but only so long as such code: (a) does
  not change the primary purpose of the Application … (b) does not bypass
  signing, sandbox, or other security features of the OS; and (c) for
  Applications distributed on the App Store, does not create a store or
  storefront for other Applications."

My reading, not Apple's: 4.7 carves JavaScript plug-ins out of 2.5.2's ban;
Wasm is named nowhere, so a plugin module should be JavaScript, not Wasm
(which matches AGENTS.md: plugins are never Wasm).

## 7. What I could not verify

- Whether Apple reads 4.7.3's "each instance" as each install or each use,
  and whether the operator's suggested plugin list counts as a storefront
  under 3.2.2 (i). Only App Review can answer; ask before the native app
  ships frames.
- Whether the current developer agreement still asks for Apple's own
  JavaScript engine anywhere (older versions did); the quoted clause does
  not.
- The size and speed of a JavaScript interpreter compiled into the Rust
  core as Wasm; "about a megabyte" is an estimate. It needs a measurement
  on the dev VM before O1 is decided, like the engine's (issue #108).
- That Obsidian plugins patch each other's code and that a Home Assistant
  custom integration replaces a built-in of the same name: common
  practice, not found in their documentation.
- Whether a generic "Run an operation" action in Shortcuts, with options
  filled at run time, passes App Review and feels right (row 82).
- Nothing in this document was built or measured; every server cost is
  read from the design, not from code.

## Simplification opportunities

- **G13 slots, panel slots, `<app>.list` and the shell's slots become one
  table of places (M1)**, host places and app places in the same shape;
  `drawing.md` §1.6 and the `slots:` line invented in [X] collapse into it.
- **The 2026-11-27 decisions and G14 become one `extends:` block (M2)**
  instead of four separate rules (fields, operations, collections, sidebar
  lines).
- **`on.change` and `on.after` could share one delivery path**: both are
  "something committed", keyed, at least once, coalesced per item.
- **`refuses:` reuses `checks:`**: one list of conditions per operation,
  whatever plugin wrote the line, evaluated by the one engine.
- **The format still shows `actions:` and `create:`** in
  [kind-and-collection-format-2026-11-24.md](kind-and-collection-format-2026-11-24.md)
  §1.2 and its examples, though round 29 folded both into the operations
  list [MR 1234], [MR 1239]; the format document should be rewritten once,
  with M1 to M5 and the format lines of §4.1.
- **`docs/plugin-api.md` describes the revision-14 API** (settings,
  palette, diagnostics); its still-wanted parts (typed settings, palette
  phrases) should move into the format and the file be marked historical.
- **Themes, fonts and wallpapers share one asset path** (validated,
  stored once, served by the server); M4's sound check should be the same
  code with another magic-number list, not a fourth pipeline.
