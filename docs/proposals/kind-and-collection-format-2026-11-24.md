# The kind and collection format · revision 10

**Revision 10, 2026-11-28** (Chat-first implementation and coordinator
sets 11–16). This is the single statement of the on-disk
format and the plugin call surface; a builder implements from it alone.

It applies, all binding:

- rounds 21 to 33 of `docs/morning-2026-11-21.md`;
- round 34 of the morning report, and its later answers the same day;
- the sixteen "Decided by the coordinator" sets of 2026-11-27;
- the security review of revision 7
  ([docs/reviews/format-rev7-security-review-2026-11-27.md](../reviews/format-rev7-security-review-2026-11-27.md)),
  finding by finding, in Appendix D;
- the implementer review of revision 6
  ([docs/reviews/format-rev6-implementer-review-2026-11-27.md](../reviews/format-rev6-implementer-review-2026-11-27.md)),
  finding by finding, in Appendix C;
- the formulas research, both stress tests and the extension-surface
  audit, for context.

Where they contradict older text, the newer wins. The contradictions
revision 6 resolved are in §19. See §21 for revisions 7–10 and the
closing section for differences between the decisions and the code. Items the
owner has not decided are marked **OPEN** in §17.

The aim (owner, round 33): a very strong, very versatile plugin API that lets
plugins change nearly everything in the spaces where they are installed,
except the core. Definition files are Markdown in the format's own line
grammar; a person can write a sum by hand; an agent writes anything larger.

## Contents

0. Notation
1. Files, parsing and names
2. Kind files
3. Field types and how items write them
4. Formulas and conditions
5. Collection files
6. Sidebar files
7. `plugin.md`
8. Calls, events, the acting person and the write table
9. Outside writes (WebDAV, CalDAV, CardDAV)
10. Agent rules files
11. Theme files
12. The space profile
13. Remote content
14. What is never open
15. A first plugin
16. Reading List, written out in full
17. Round 34 decisions and OPEN items
18. Counts, and two ways to say one thing
19. Contradictions resolved (revision 6)
20. Choices that no decision covers
21. Change list (revisions 7 through 10)
22. Code and specification disagree (after the appendices)

Appendices:

- A. The grammar proved by hand
- B. The four kinds rewritten
- C. The 63 review findings (revision 6)
- D. The 44 security findings (revision 7), the eight partly closed fixes, and the seven attackers

## 0. Notation

In a grammar line, `<x>` is a placeholder, `[x]` is optional, `a|b` means
one of the two, and `x…` means x may repeat, separated by `, `. Literal text
is in `code`. Each rule has an ID (L for lines, N for names, X for
expressions) that Appendix A cites.

## 1. Files, parsing and names

### 1.1 The files of a plugin, and installing

```text
<plugin>/
  plugin.md              who it is, what it reads, sends out, calls, extends and fills
  kinds/<kind>.md        one file per kind
  collections/<id>.md    one file per collection
  sidebars/<id>.md       the app's default sidebar(s)
  templates/<name>.md    starting bodies for new items
  themes/<name>.md       themes (§11)
  profiles/<name>.md     space profiles the Owner may apply (§12)
  assets/                recognised data files such as fonts and raster images
  programs/              example location for bound executable or image files
  frames/                web code drawn in a sealed frame
  operations.ts          operations that need code
```

Every folder and file except `plugin.md` is optional.

- **Installing** copies the plugin to `.calternal/plugins/<plugin>/`, with
  the same layout inside (`plugin.md`, `kinds/`, `collections/` …), so two
  plugins' `collections/all.md` never collide and relative links such as
  `../collections/unread.md` keep working (review 14).
- **Kinds made in the app** belong to the built-in plugin `space`
  (`space.<kind>`), in `.calternal/plugins/space/`.
- **Description-only plugins** have no `operations.ts`, no `frames/`, and
  declare no sign-ins, secrets, programs, outside addresses, `listens`,
  `reads` or `refuses`. `sends out: nothing`, or no `sends out` line, both mean nothing
  is sent (review 53). Any member may add one; everything else is
  installed by the Owner. A plugin with `frames/` is installed by the Owner
  after the web warning (sixth set, I1 (a)).
- **`.calternal/`** is visible and readable over WebDAV and in exports, and
  refused for outside writes (round 29).
- **`format: 1`** is carried by every definition file. The server refuses a
  file whose format, functions or keys it does not know, naming them; it
  never half-reads a newer file.
- **A plugin's identity is its ID plus its source** (Git address or signing
  key). Agent rules, picks and state are keyed by ID plus source (S23).
  - `calternal-` is reserved for plugins on the operator's list.
  - `space`, `home`, `files`, `lines`, `calternal` and `outside` are
    reserved plugin IDs.
  - A plugin installed under a removed plugin's ID from another source
    starts empty, and the old values stay read-only (review 50).
- **Updates** (S4) install by themselves only if every install sentence
  the server derives is unchanged or narrower. Otherwise they wait for
  whoever would have to install the new version (the Owner, if it is no
  longer description-only). The eighth set names the lines that always
  wait: any added `listens`, `reads`, `extends`, `fills`, `outside`,
  notification, destination or sign-in line. Adding or changing a
  `refuses:` rule also waits for the Owner. A changed kind body or
  notification template counts as a changed sentence.
- **Classify assets by content.** The host uses a short allowlist of
  recognised image, font and plain-text types. Everything else, including
  SVG, HTML and any file capable of carrying script, is classified as a
  frame and requires Owner install and the frame warning. An extension or
  a supplied MIME type grants no authority (S14, set 14).
- **Chat-first installation** uses a local bundle folder in the developer
  path. The in-app install path still needs binding of the plugin ID to
  its source and the approval sheet in §1.1a. The server checks definition
  text at install and sends compiled declarations to devices (§8.1).
  Devices draw those declarations without shipping the Unicode install
  checks (set 14).

### 1.1a Text on sheets, and one rule for every free string and path (S23, S43)

- **Every sentence on the install and update sheets is the server's own,**
  derived from declarations. Plugin text (name, body, labels) is drawn
  below it as quoted plain text, without images or links. Names are shown
  with their source and plugin ID.
- **Every free string**, wherever it appears, passes one check in the
  parser:
  - it is NFC;
  - it has no Cc or Cf characters (bidi controls, zero-width characters),
    and no U+2028, U+2029 or U+0085;
  - it has a length limit: name 40, label 40, message 120, body 4 KiB;
  - a name is in a single script (UTS #39);
  - a `name` confusable with a plugin on the operator's list needs that
    plugin's source;
  - a host is shown as an A-label when it is not ASCII.
- **Every path**, in a definition file or a call, is relative and
  normalized, has no `..`, and lies inside its named folder. Served folders
  refuse symlinks, hard links and special files.
- **XMP** is parsed without DTDs or entities, and capped in size.
- **One derived flag, "reaches other people or the outside"** (S9, S16,
  S24, S33).
  - **When it is set:** the operation's plugin declares `sends out`, SMTP
    or HTTP sign-ins or `notifications`, or the operation uses `s.share`,
    or calls an operation that has the flag.
  - **Where it is used:** the server computes it once and uses it for the
    sheet sentence, R1's ask (§10), the risky-install passkey, the proxy
    rule (§13) and reactions (§8.2).
  - **`the public web`:** its sentence is "Can send what it reads to any
    website." A plugin with `the public web` plus `reads`, `listens` or a
    place that gives items is a risky install (passkey) (S24).


### 1.2 The line grammar of definition files

Kind, collection, sidebar, plugin, rules, theme and profile files use this
grammar (fifth set); item files use YAML (§3.2).

```ebnf
file        = "---" NL { header-line } "---" NL body ;
header-line = indent ( entry | section-line | list-item | comment-only | blank ) NL ;
indent      = { "  " } ;                         (* steps of exactly two spaces *)
entry       = key ":" [ " " value ] [ comment ] ;
section-line = value [ comment ] ;               (* only the forms named by the parent *)
list-item   = "- " value [ comment ] ;           (* only where L9 allows *)
key         = name | qualified | place ;         (* §1.3, §1.4, §7.4 *)
value       = part { "," " " part } ;            (* split only at top level *)
part        = word { " " word } ;
word        = quoted | group | bare ;
quoted      = '"' { char - ('"' | "\") | '\"' | "\\" } '"' ;
group       = "(" ... ")" | "[" ... "]" | "<" ... ">" | "`" ... "`" ;  (* balanced; may hold anything *)
bare        = { char - (" " | "," | '"' | "(" | "[" | "<" | "`") }- ;
comment     = " # " { char } | "# " { char } ;   (* L5 *)
```

| Rule | Text |
| --- | --- |
| **L1 Key end** | A key ends at the first `: ` (colon and space) outside quotes and groups, or at a final `:` at the end of the line. A colon not followed by a space (`xmp:Rating`, `calternal-chat:composer`) is part of the word |
| **L2 Splitting** | A value splits into parts at `, ` outside quotes and groups only. Commas inside `( )`, `[ ]`, `< >`, backticks or quotes are text |
| **L3 Quotes** | Inside double quotes, `\"` and `\\` are the only escapes; the quotes are removed from the value. Text needs quotes only when it holds `, ` at top level, starts with a group character, or must keep `# ` |
| **L4 Lists** | `[a, b]` and `a, b` are the same list; `[]` is empty |
| **L5 Comments** | `#` starts a comment only at the start of a line or after a space, **and** only when followed by a space or the end of the line. `#c96f4a` is a value; `# note` is a comment. No comments inside quotes or groups |
| **L6 Nesting** | Children are indented exactly two spaces more than their parent, to any depth the key's grammar allows. A key may have both a value and children (`ical: VEVENT` with mapping lines). A section line without a key is allowed only where its parent grammar names one, such as `checks`, `together`, `operations`, `listens` or the lines under a refusal target |
| **L7 Continuation** | A line indented more than two spaces beyond the previous line continues it, joined with one space |
| **L8 Tabs and blanks** | Tabs are refused at install, naming the line; blank lines are ignored |
| **L9 List items** | `- ` begins a line only in sidebar bodies and their `extends:` lines (§6), and in agent-rule lines (`agent rules:` in §12, the rules body in §10) |
| **L10 Unknown keys** | Refused at install, naming the key and line. Definition files are strict; item files are lenient (§3.2) |

### 1.3 Naming another plugin's things

| Thing | Form | Example |
| --- | --- | --- |
| A plugin | `<plugin>` | `calternal-mail` |
| A kind | `<plugin>.<kind>` | `calternal-mail.messages` |
| An operation | `<plugin>.<kind>.<operation>`, or `<plugin>.<operation>` when the plugin has one operation of that name | `calternal-tasks.done` |
| A collection | `<plugin>:collections/<id>` | `calternal-tasks:collections/loose-ends` |
| A sidebar | `<plugin>:sidebars/<id>` | `calternal-notes:sidebars/notes` |
| A slot | `<plugin>:<slot>` | `calternal-chat:composer` |

- **Inside a plugin's own files** the plugin part is left out: `messages`,
  or `../collections/inbox.md` as a link.
- **In a formula**, and as a key or a `lines:` target, a qualified name or
  one holding `-` or `'` is written in square brackets (N5):
  `[calternal-calendar.events]`, `[plan-days]`.
- **A link type to another plugin's kind** uses the full form:
  `link(calternal-contacts.contacts)`. If that plugin is absent, the value
  shows as plain text.

### 1.4 Names

| Rule | Text |
| --- | --- |
| **N1 IDs** | Plugin, kind, collection, sidebar, slot, theme, profile and setting IDs are `[a-z][a-z0-9-]*`, at most 64 characters, unique within their plugin |
| **N2 Field, operation, state and setting names** | Unicode NFC, case-sensitive, at most 64 characters. They start with a letter, hold letters, digits, `-`, `'` and single spaces between words, and do not end in a space |
| **N3 Reserved words** | A name may not *equal* a reserved word (§1.5). A field name may not contain `and`, `or`, `not` or `in` as a whole word |
| **N4 Formula names** | A formula reads a name as the longest run of words joined by single spaces that holds no operator word (`and or not in`) and does not end in a word directly followed by `(` |
| **N5 Brackets** | In a formula, `[…]` holds a name exactly: a qualified name, or one with `-` or `'`. Round brackets are only for calls, `in (…)` and grouping |
| **N6 Own kinds** | A kind may not share a name with one of its own fields |

### 1.5 Reserved words

| Group | Words |
| --- | --- |
| Formula words | `and`, `or`, `not`, `in`, `yes`, `no`, `true`, `false`, `empty`, `this`, `open item`, `item`, `period`, `records`, `settings`, `state`, `ask` |
| Built-in fields | `id`, `kind`, `created`, `edited`, `author`, `at`, `anchor` |
| Operation words (operation names may not equal them) | `add`, `change`, `remove`, `duplicate`, `set`, `open`, `where`, `timer`, `after`, `within`, `only`, `when`, `code`, `append record`, `change record`, `remove record`, `swipe`, `menu`, `primary`, `mark` |
| Source words (kind IDs may not equal them) | `files`, `lines`, `space`, `home` |
| Credential names (field and input names may not equal them, S40) | `password`, `token`, `secret` |

## 2. Kind files

### 2.1 Every key

| Key | Value | Required | Section |
| --- | --- | --- | --- |
| `kind` | ID (N1) | yes | 2.2 |
| `name` | What people see | yes | 2.2 |
| `format` | `1` | yes | 1.1 |
| `folder` | A pattern | yes | 2.3 |
| `file` | A pattern | no; default `{title}` if the kind has `title`, else the ID | 2.3 |
| `item` | `file`, `folder with <main file>`, `file(<types>)[, paired]` | no; default `file` | 2.4 |
| `editable` | `outside` or `operations`; fixed for the kind's life | yes, except on a hidden kind (always operations) | 2.5 |
| `rights` | `change <role>`, `read <role or group>…` | no; default editor to change, every member to read | 2.6 |
| `progress` | A number field (0 to 1) shown as live progress while an operation runs (O2) | no | 2.6a |
| `fields` | Field lines | yes | 2.7 |
| `records` | Record field lines | no | 2.8 |
| `state` | Per-person keys | no | 2.9 |
| `lines` | Line items in other kinds' notes | no | 2.10 |
| `unique` | One item per value | no | 2.11 |
| `together` | Fields merged as one | no | 2.12 |
| `checks` | Conditions every save meets | no | 2.13 |
| `default where` | A condition applied wherever the kind is listed | no | 2.14 |
| `keep` | How long items live | no | 2.15 |
| `hidden` | `keep <duration>`: a hidden kind for a plugin's durable data | no | 2.15 |
| `search` | Fields searched, in order of weight | no; default title, then body | 2.16 |
| `template` | A template's name, or `none` | no | 2.17 |
| `ical`, `vcard` | The CalDAV or CardDAV mapping | no | 2.18 |
| `row` | Fields on one line | no; default the first text field | 2.19 |
| `page` | Entries on the item's page | no; default every field, then the body | 2.19 |
| `protected` | Fields additions can never push out of view | no; default the title | 2.19 |
| `operations` | Everything that can be done to an item | operations-only kinds: yes | 2.20 |
| body | Plain text for people and agents, written as the folder's `AGENTS.md` | yes | 2.2 |

### 2.2 `kind`, `name` and the body

```markdown
---
kind: reading
name: Reading
format: 1
folder: Reading
editable: outside
fields:
  title: text, required
---
Links saved to read later. One file per link.
```

**Instruction files** (S5; strict default until the owner answers E.2,
§17):

- The server keeps a list of names that harnesses load as instructions:
  `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `.claude/`, `.codex/` and
  `.cursorrules`, extended as harnesses appear.
- In every folder these are written only by the server: over WebDAV, by a
  run, by a plugin or as a mail attachment, any other write under such a
  name gets a new name by rule 0023 (`AGENTS 2.md`).
- The server writes a kind's body only into a folder the kind created, and
  the file opens with one fixed line: "Written from the kind *X* of plugin
  *Y*, added by *Z*. It describes files; it gives no instructions."


### 2.3 `folder:` and `file:` patterns

```text
folder: <segment>[/<segment>]…[, plus marked folders]
file: <text and {<field>[:<format>]}>…
segment = <text> | {<field>[:<format>]}
format  = YYYY | MM | DD | YYYY-MM | YYYY-MM-DD | GGGG-[W]WW
```

```markdown
folder: Photos/{taken:YYYY}/{taken:YYYY-MM-DD}, plus marked folders
file: {date:YYYY-MM-DD} {plan day}
```

- **Names in braces:** `{<field>}` names one of the kind's own fields, and
  a link field gives the linked item's row text. A field in a pattern must
  be required or have a default. Braces hold no dots.
- **What a folder pattern may not do** (review 43, S6; it only narrows):
  - it is relative, with no `..`;
  - it never starts with `.calternal`;
  - it may not equal, contain, or lie inside another installed kind's
    folder (`Notes/Inbox` against `Notes` is a clash), and a clash refuses
    the install.
- **Each value filled into a segment** is one path segment: no `/` or `\`,
  not `.` or `..`, no leading `.`, sanitized by rule 0023. A filled path
  that lands in another kind's folder or in `.calternal/` refuses the
  write.
- **File names** follow inherited 0023: Unicode NFC, unsafe characters
  replaced, and a clash gets " 2".
- **When a pattern field changes**, the server moves the file and rewrites
  every link to it (G6), recorded in history.
  - While a pattern field is in conflict, the file stays at its old path.
  - A rename from a `heading` field is applied when the editor closes, or
    after 30 s idle (review 33).
- **`plus marked folders`** adds folders a person marks for this kind;
  their files are indexed in place and never moved. A marked folder obeys
  the same rules as a pattern, and a file that is already an item keeps
  its kind (S6).

### 2.4 `item:` forms

```text
item: file
item: folder with <main file>
item: file(<media type>…)[, paired]
```

| Form | An item is | Example |
| --- | --- | --- |
| `file` | One Markdown file | a note |
| `folder with <main file>` | A folder whose main file is Markdown; the files beside it belong to the item | `item: folder with message.md` |
| `file(<types>)` | A binary original, never written to. Fields come from embedded metadata (`from metadata`) or from an XMP sidecar in `.calternal/`, keyed by ID (`in sidecar`) (inherited FS1–FS3, PH27) | `item: file(image/*, video/*), paired` |

- **`paired`** groups Live Photos, RAW+JPEG pairs and edited copies as one
  item, removed and restored together.
- **A `file` field** on a Markdown item links to a file stored by inherited
  rule 0028 (Photos or Documents by type).

### 2.5 `editable:`

```markdown
editable: operations
```

| | `outside` | `operations` |
| --- | --- | --- |
| In the WebDAV mount | Yes | Absent |
| Handed to an agent as a folder | Possible | Not possible |
| Built-in `add`, `change`, `remove` | Present unless the operations list names them with a condition or as `code` | Only those listed |
| An outside write | Checked as the built-in operation (§9) | Refused, except through CalDAV or CardDAV as the listed built-ins (§9) |

### 2.6 `rights:` (fifth set; G5; O4, round 34)

```text
rights: change manager|owner
rights: read <role or group>…
rights:
  [change manager|owner]
  [read <role or group>…]
```

```markdown
rights:
  change manager
  read manager, family-adults
```

Use the inline form for one right and the block form for both. Each right
may occur once.

- **`change`** is the least role that may run add, change, remove,
  duplicate and record operations on the kind's items, from any surface,
  including outside writes. An author's own removal stays free (§2.20).
- **`read`** names the roles or groups (by ID, N1) that may read the kind's
  items. One matching role or group is enough on that line. A role
  sets a minimum in the order reader, editor, manager, owner. A named
  group matches by membership. Each applicable read line must pass.
  People outside that audience never learn that such an item exists.
  - Search, collections, totals, counts, sort and group positions,
    formulas, links, backlinks, snippets and plugin search results never
    show an unreadable item, its title or its existence (S42). Filter
    before evaluating conditions, ranking, paging or aggregating. A
    resolved link to an unreadable item gives the same result as a link
    to an absent item. This applies to structured data and all
    host-generated projections.
  - A stored computed value reads only what every reader of its item may
    read; otherwise it is computed per viewer (§4.7).
  - An add, check or `unique:` clash that touches an unreadable item
    answers `not-allowed`, without saying why.
  - The one check enforces it, on every path, devices included: a device
    never receives what its person cannot read. Rights also constrain
  change, exports, sync, subscriptions and share access.
- **Folders** get the same two lines in `space.md` (§12). A folder's rights
  are inherited by every folder and file below it (inherited FB20), and a
  kind's `read` narrows them further, never widens. Paths changed by a
  move must pass the checks at both ends.
- **The words** are the same ones `space.md` uses (§12).

### 2.6a `progress:` (O2, round 34)

```markdown
progress: done fraction
```

This names a number field (0 to 1) of the kind. While an operation on the
item runs, its value is shown as live progress: on a lock screen or Dynamic
Island on iPhone, in the menu bar on Mac. The plugin's code updates the
field like any other. The host subscribes only while the operation is
active and the viewer may read the item. Missing or out-of-range progress
is indeterminate.

### 2.7 `fields:`

```text
<name>: <type>[, required][, default <value>][, heading][, was <old>[|<older>]…][, from metadata][, in sidecar[ <property>]]
<name>: [per day|week|month|year ]= <formula>[, mark]
```

```markdown
fields:
  title: text, required, heading
  read: yes/no, default no
  site: = domain(url)
  newsletter: = contains(from, "newsletter"), mark
  spent: per month = sum(from(transactions.category, month(date) = period).amount)
  rating: number, was stars|score
  taken: time, from metadata
  favourite: yes/no, default no, in sidecar xmp:Rating=5
```

| Flag | Meaning |
| --- | --- |
| `required` | A save without it is refused; a hand edit without it is a Problem |
| `default <value>` | Written when the item is added, or into an outside file once when it is accepted (§9); `today` and `now` are allowed |
| `heading` | Kept in step with the body's first heading; the last change wins, and the heading wins a tie (inherited 0025) |
| `was <old>[\|<older>]…` | Earlier names. An update rewrites every affected file as one background job, recorded in history, before the new version is active (G1, review 28) |
| `from metadata` | Read from the file's embedded metadata; read-only; file items only |
| `in sidecar [<property>]` | Stored in the XMP sidecar, under that property if named; file items only |
| `mark` | On a yes/no computed field: shows the field's name as a label on the item when true (fifth set). In an addition, it is drawn with the adding plugin's icon, never in the host's warning style |

- **Computed fields** start with `=`. Their values live only in the
  rebuildable index, never in the item.
- **`per <period>` fields** hold one value per period (§4.6).
- **Plugin updates:** an added field is automatic; a removed field's
  values stay, unshown; a retype is refused unless every value converts.
  An added field whose key already holds values counts as a retype of
  those values: refused unless every value converts, naming the items
  (review 29).
- **Fields a person adds (G3)** survive updates and work in formulas; the
  plugin ignores them.

### 2.8 `records:`

```text
records:
  <name>: <type>[, required][, default <value>]
  <name>: = <formula>
```

```markdown
records:
  exercise: link(exercises), required
  weight: number kg|lb
  reps: number
  volume: = weight * reps
```

On disk (review 11):

```markdown
Text before the first header is the item's own.

**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9
exercise: "[Bench press](<../Exercises/Bench press.md>)"
weight: 80
reps: 8
Felt heavy today.
```

- **The header** is exactly `**<name>** · <YYYY-MM-DD HH:MM> ^<16
  Crockford characters>`, written by the server, with the time in UTC. `*`
  and `·` in the name are escaped with `\`. The anchor ends at the first
  space. A trailing HTML comment is opaque host data. The format reader
  includes it in the header source span and preserves its bytes when
  editing frontmatter. It accepts untagged headers too.
- **Host append seal.** New appends end the header with a 34-byte
  ` <!--ca1 <22 base64url characters>-->` suffix. The host flushes the
  file and then the ledger. The keyed seal binds the item, position,
  offset, author and record bytes, including the header time and
  anchor. For a keyed gesture, the anchor also binds the request key.
  Only the host verifies the seal.
  On restart the host can adopt a complete verified tail and recover
  its receipt. An unverified, torn or moved tail enters the outside-change
  path. Shared writers still need an authoritative author source for
  recovery.
- **The record's fields** are the consecutive `key: value` lines after the
  header whose key is a declared record field. The first other line starts
  the record's Markdown body, so `Note: felt heavy` is body text when
  `note` is not declared. The body can contain several paragraphs.
- **A record ends** at the next header line, or at a line holding only
  `---`.
- **Built-in record fields:** `author` (from the ledger, never from the
  header), `at` and `anchor`.
- **Authority is the history ledger.** A header line in a record's text is
  escaped with a leading `\` on write. An outside write that adds or
  changes a record not authored by the writer is refused (§9).
- **Reading records:** `records` in the item's own formulas, and
  `<kind>.records` across items in collections and `from()` (C3).

### 2.9 `state:` (fifth set)

```text
state:
  <key>: <type>[, default <value>]
```

```markdown
state:
  read through: record anchor
  muted: yes/no, default no
```

- **What it holds:** per-person keys for this kind's items, stored in the
  person's personal space (round 21).
- **`record anchor`** stores the bare 16-character host anchor, as
  `read through` above. It denotes a ledger position in this item.
  Use `count(records after state.read through)` for unread, including
  when adjacent records have the same timestamp.
- **Writing:** one-liners set them (`mute: set state.muted = yes, menu`).
- **Reading:** formulas and checks read them as `state.<key>`. A formula
  that reads `state.` or a per-person setting is computed per viewer when
  the item is listed, and never stored in the shared index (review 24).
- **Code** reads and writes them through `s.state` (§8.1).

### 2.10 `lines:` (C1)

```text
lines: checkbox|time in <kind>…[, box <field>: `[ ]` <value>, `[/]` <value>, `[x]` <value>, `[-]` <value>]
```

```markdown
lines: checkbox in [calternal-notes.notes], [calternal-calendar.days], box status: `[ ]` "todo", `[/]` "doing", `[x]` "done", `[-]` "cancelled"
lines: time in days
```

The shared core understands exactly two line forms (inherited 0068 rule 1):

- **checkbox:** a Markdown list item with a box. With a child link to an
  item of this kind, the line *is* that item, and its box mirrors the
  field through the mapping. Without `box`, the field is a yes/no named
  `done`.
- **time:** a line starting with `HH:MM - HH:MM`, `HH:MM` or `all day`. It
  fills `start`, `end`, `all day` and `title`, which the kind must have.
  The day comes from the note's `date` field.

Other rules:

- **Targets:** the kinds named may be this plugin's own (bare) or another
  plugin's (in brackets).
- **Plain checkboxes** are listed by `from: lines(checkbox)` (§5.3).
- **One claim per line form and target** (S28): one kind per line form per
  target kind. A second claim refuses the install, as a folder clash does.
- **A box ticked in a note** is checked as the linked item's `change` for
  the writer. If that is refused, the box is put back (S26).
- **The calendar an event belongs to** is stored by the core when the line
  or the item is written, from the line's first top-level area tag (else
  Journal). No formula looks it up (seventh set).
- **Ticking offline** updates the item at once (S1). A box changed over
  WebDAV is compared against its base, and the newest edit wins.

### 2.11 `unique:`

```markdown
unique: date
```

- **One item per value.** An item of a unique kind takes its ID from the
  kind and the value, so two offline adds are the same item and merge
  through the generic merge.
- **`duplicate`** is refused on a unique kind.
- **Restoring** an item over a live one merges into it (review 32).
- **Taken IDs** (S27): an add whose ID is taken gets a new ID, on every
  path. On a unique kind the server derives the ID itself. An add that
  merges into a live item, or a restore over one, is checked as that item's
  `change` for the writer.

### 2.12 `together:`

```text
together:
  <field>, <field>…
```

```markdown
together:
  start, end, all day, time zone
```

- **When two devices changed the group,** both versions are kept: the
  item's value, plus a conflict record in host state, shown on the item
  until a person picks one. The item is listed in Problems (review 34).
- **In `extends:`**, `together:` may group only that plugin's own added
  fields.

### 2.13 `checks:`

```text
checks:
  <condition>[, "<message>"]
```

```markdown
checks:
  end = empty or end >= start, "An event ends after it starts"
```

- **A write that fails** is refused, with the message (else the line).
- **A hand edit that fails** is refused when it came through the server's
  protocols (§9). A file the server finds changed on disk becomes a
  Problem.
- **A merge that fails** keeps both versions.
- **Checks may read** `records.` and `state.`.

### 2.14 `default where:`

```markdown
default where: not hidden
```

- **Where it applies:** collections, search, `each:`, `from()` and `all()`.
- **The way out:** `ignoring default`, in the `from:` line or in the
  function (`from(photos.people, ignoring default)`).
- **Links** to a hidden item still open it (review 35).

### 2.15 `keep:` and `hidden:`

```text
keep: <duration>
hidden: keep <duration>
```

```markdown
keep: 30d
hidden: keep 90d
```

- **`keep`** moves an item to Trash once it has been unchanged for
  `<duration>`, counted from `edited` (review 30). Adding or shortening
  `keep` in an update is listed on the update sheet with the number of
  items it would remove.
- **`hidden: keep <duration>`** makes a hidden kind: no app UI, absent from
  WebDAV and search. It holds a plugin's durable data that is neither a
  person's file nor rebuildable (sync tokens, an outbound queue). Its items
  skip Trash at the end of `keep` (sixth set).
- **A hidden kind is always operations-only**, so `editable:` is not
  written (S13).
- **Only its own plugin reaches a hidden kind.** It never appears in
  another plugin's `reads`, `calls`, `extends`, `listens`, `lines`,
  `from()`, `link()` or collections.
- **A description-only plugin** may not declare a hidden kind.
- **Removing a plugin with hidden kinds:**
  - the remove sheet counts what each holds ("3 messages not sent") and
    offers an export;
  - the items are kept until `keep` runs out;
  - a reinstall offers them to the Owner before the plugin's code sees
    them (review 31).

### 2.16 `search:`

```text
search: <field>…
```

```markdown
search: title, aliases, tags, body
```

- **Order is weight** (G9). Snippets come from the first matching field.
- **Computed fields** are allowed only if they are stored in the index
  (not computed per viewer).
- **A plugin's own search** is `fills: search` (§7.4).

### 2.17 `template:`

```markdown
template: meeting
```

The file `templates/<name>.md` inside the plugin, or `none`. The name cannot
leave `templates/` (review 43).

### 2.18 `ical:` and `vcard:`

```text
ical: VEVENT|VTODO
  <field>: <PROPERTY>[, unless <condition>]
vcard:
  <field>: <PROPERTY>
```

```markdown
ical: VEVENT
  title: SUMMARY
  start: DTSTART
  end: DTEND
  repeat: RRULE
  exceptions: EXDATE
  alert: VALARM
  body: DESCRIPTION, unless source != empty
```

- **What the server serves:** CalDAV (VEVENT and VTODO collections) and
  CardDAV for kinds with a mapping (C2).
- **A client's write** is the kind's built-in add, change or remove (§9).
  If the kind does not list that built-in, the request is refused with
  403.
- **`unless`** applies in both directions: the property is neither sent nor
  read while the condition holds.
- **Unmapped properties and unknown vCard lines** are kept exactly in host
  state, as the item's sent snapshot (inherited 0067), never in the
  Markdown file (review 63).
- **What is served back** (S31) is an allowlist without addresses or URLs:
  - no `ATTENDEE` or `ORGANIZER` unless mapped;
  - no `ATTACH` or `URL`;
  - `VALARM` only as `DISPLAY`;
  - at most 64 KiB per item.
- **Plugin code** never pushes unmapped properties upstream.
- **No scheduling:** the server does no CalDAV scheduling (RFC 6638).
- **Signing in:** DAV uses per-person app passwords, each for one space
  and for DAV only. They are listed, revocable, and end with the person's
  access.

### 2.19 `row:`, `page:` and `protected:`

```text
row: <field>…
page: <entry>…
protected: <field>…
```

`<entry>` is one of:

- a field;
- `body`, `properties`, `image` or `preview`;
- `records(<field>…)`, showing each record's Markdown body and the listed
  record fields;
- `records(from(<kind>.<link field>))`, a records view over several items
  (fifth set);
- `web(<field>)`, an HTML body in a web view (fifth set).

```markdown
row: title, site
page: properties, body
protected: from, outside sender
```

- **Protected fields** are drawn by the host, in a strip outside any
  replacement frame, in a host font that themes cannot change (S34).
- **`web(<field>)`** draws only in a sealed frame on its own opaque origin,
  with scripts, forms, navigation and plugins off and `default-src
  'none'`. Images load only after a tap, through the proxy (§13; S14).
- **Additions** (§7.2) never push protected fields out of view.

### 2.20 `operations:`

```text
add[: <field>…][, phrases]
change[: only <field>…][, when <condition>]
remove[: when <condition>]
duplicate[: when <condition>]
add|change|remove|duplicate: code[, when <condition>]
<name>: set <target> = <value>|ask[, set …][, when <condition>][, <placement>…]
<name>: open where <condition>[, <placement>…]
<name>: timer <duration>|settings.<name>, after <operation>
<name>: code[, when <condition>][, <placement>…]
append record[: <field>…][, when <condition>][, <placement>…]
change record[: <field>…][, within <duration>][, when <condition>]
remove record[: when <condition>]
target    = <field> | state.<key>
placement = swipe | menu | primary
```

| Line | Example |
| --- | --- |
| `add` | `add: url, title, phrases` (the form asks these fields; `phrases` lets the composer fill it) |
| `change` | `change: only title, archived` |
| `remove` | `remove: when not reconciled` |
| `duplicate` | `duplicate` (a copy with a fresh ID; refused on unique kinds) |
| built-in by code | `remove: code` |
| one-liner | `mark read: set read = yes, when not read, swipe, primary` |
| asking | `snooze: set snooze = ask, menu` (the field's own picker) |
| state | `mute: set state.muted = yes, menu` |
| open | `today: open where date = today(), primary` |
| timer | `rest: timer settings.rest, after append record` |
| coded | `reconcile: code, menu` with an optional nested `inputs:` block |
| records | `append record: exercise, weight, reps, primary` · `change record: weight, reps, within 48h` · `remove record` |

A coded operation can declare a nonempty `inputs:` block under its
operation line. Each child has a name and stored field type with the
applicable input flags, such as `required`. Built-in operations use their
own forms above. Calls, slot schemas, refusal checks and generated help
use this declaration. For an item-scoped coded operation, declare
`item: link(<its kind>), required` and pass the target UUID under `item`.
An operation without this input has no event item. Other links are
ordinary inputs, such as a targetless harness receiving a chat UUID.

```markdown
operations:
  rename: code, menu
    inputs:
      item: link(chats), required
      title: text, required
```

- **`set <field> = <value>`** takes a formula; choice values are quoted
  (X3).
- **`open where`** adds the item only when its condition is `<unique field>
  = <value>`, optionally `and`-ed with other equalities, which it also
  sets. Otherwise it opens the first match by the kind's sort, or a list
  (review 15).
- **`change record`** without `within` lets the author change the record
  at any time.
- **`duplicate`** clears `data` fields and other plugins' added fields.
  Records keep their ledger authors in the copy (S30).
- **Removing one's own work** (S22, eighth set): an author may always
  remove their own item or record to Trash. This is a server built-in,
  outside `rights:`, `when` and the operations list. A kind's `remove:
  when` governs only others' items. Restoring from Trash puts back what a
  rule protected. The cost: the author of a reconciled Money transaction
  can remove it, and Trash and history recover it.
- **Offline (S1):** built-ins, one-liners, open, timer and record
  operations apply on the device at once; coded operations queue as "not
  sent yet". The coded operation runs on the server.
- **Agent runs:** the default scope is `session` in the `s.run` options
  (§8.1). There is no formula function for a run scope.
- **Refused on reconnect:** the change is kept as "not accepted" in the
  outbox, with the reason. It is never dropped and never applied; the
  device never decides rights (sixth set).
- **Every line is an operation** on the app, the CLI, the API and MCP, and
  is what agent rules name.

## 3. Field types and how items write them

### 3.1 Every type

| Type | Holds | Written in an item as | Also read from | Example |
| --- | --- | --- | --- | --- |
| `text` | A line | a string | any scalar | `title: text` |
| `long text` | Paragraphs | a block string | any string | `notes: long text` |
| `number` | An exact decimal | a number | a numeric string | `reps: number` |
| `number <unit>[\|<unit>]` | A decimal in a unit, stored in the first unit, shown in the person's choice | a number | `80 kg`, `176 lb` (converted) | `weight: number kg\|lb` |
| `money [<currency>]` | An exact decimal with a currency | `"12.50 EUR"` | `12.50` when the currency is fixed | `amount: money EUR` |
| `yes/no` | True or false | `true` / `false` | `yes` / `no` | `done: yes/no` |
| `date` | A day | `2026-11-27` | — | `due: date` |
| `dates` | Days | a list of dates | — | `exceptions: dates` |
| `time` | A date and time; one-off times keep their offset, repeating task times float, repeating events keep their zone (inherited 0066) | `2026-11-27T09:00:00+01:00` | without offset for floating times | `starts: time` |
| `when` | A day or a time | a date or a time | — | `scheduled: when` |
| `duration` | A length | `"2h"` (units X5) | ISO `PT2H` | `rest: duration` |
| `durations` | Lengths | a list | — | `alert: durations` |
| `choice(<a>\|<b>…)` | One value of a set, in board column order; values may hold spaces | a string | — | `status: choice(todo\|doing\|done)` |
| `tags` | Words; `/` nests | a list of strings without `#` | a comma string | `tags: tags` |
| `url` | An absolute `https:` or `http:` address, or `mailto:` (S15); anything else, in fields and body links alike, is a Problem | a string | — | `url: url` |
| `link(<kind>)`, `links(<kind>)` | Items; `any` allowed | a quoted Markdown link, or a list | a bare path or `[[wiki link]]` | `project: link(projects)` |
| `link(<kind> records)` | One record (fifth set) | a quoted link with `#^<anchor>` | — | `quote: link(chats records)` |
| `person`, `people` | People of the space | the person's ID | — | `owner: person` |
| `repeat` | An iCalendar rule; at most one per kind | `FREQ=WEEKLY;BYDAY=MO` | — | `repeat: repeat` |
| `place` | Latitude, longitude, name | a map `lat`, `lon`, `name` | — | `where: place` |
| `file` | A stored file (inherited 0028) | a quoted Markdown link | — | `scan: file` |
| `colour` | `#rrggbb` only (S15) | `"#c96f4a"` | — | `colour: colour` |
| `data` | A structured value that only the owning plugin's code changes, on every path (outside writes, other plugins, `duplicate`) | a map | — | `source: data` |
| `records(<field>: <type>…)` | Small records in the frontmatter | a list of maps | — | `log: records(done: time, due: date, skipped: date)` |
| `record anchor` | A host record position, used in per-person state (§2.9) | Stored as state | A bare 16-character anchor | `read through: record anchor` |

**Built-in fields** every item has, read-only: `id`, `kind`, `created`,
`edited`. They, and a record's `author` and `at`, come from host state.
Frontmatter keys with those names are ignored, and `at` and every write's
time are the server's time of receipt. A device's clock is kept for display
only, clamped to the last sync, and "newest wins" uses receipt order (S26).

**Every value is drawn as data**, never concatenated into markup or style
(S15).

### 3.2 Item YAML (review 10)

Items use the YAML 1.2 core schema without anchors, aliases, tags or flow
mappings; a file that uses them is a Problem.

- **Links** are written as quoted strings.
- **Added fields** are flat dotted keys (`calternal-reading-list.saved:
  true`), so a person's own key equal to a plugin ID never collides with
  them.
- **Reading is lenient** and nothing is rewritten on read. Unknown keys,
  comments and untouched values survive every edit (inherited 0068).
- **A file whose frontmatter does not parse** is shown and listed in the
  kind's Problems collection, and cannot be changed until fixed.

```markdown
---
title: How Discord stores trillions of messages
url: https://discord.com/blog/how-discord-stores-trillions-of-messages
read: false
added: 2026-11-24
---
```

## 4. Formulas and conditions

One language for computed fields, `where`, `when`, `checks`, `default
where`, `totals`, view columns, chart values and agent-rule conditions.

### 4.1 Values

| Rule | Text |
| --- | --- |
| **X1 Literals** | Numbers `12.5`; money `12.50 EUR`; text in double quotes; `yes`, `no`; dates `2026-11-24`; durations (X5); `empty` |
| **X2 Names** | A field (N4); a linked item's field with one dot, one hop only (`project.name`); a key of a `data` field with a dot (`source.owned`, which is not a hop); records as `records`; `item.<field>` for the record's item in a records collection; `settings.<id>`; `state.<key>`; `period` in `per` fields; `this`, `open item` (X4) |
| **X3 Choice values** | Choice values are text and are always quoted in formulas, including in `set`, `drop:`, `in (…)` and board conditions: `status = "did not happen"` |
| **X4 Scope** | Inside a list function's condition, names are the element's. The outer item is `this.<field>`; `period`, `settings.` and `state.` are global. `this` always means the item the formula belongs to; in a collection's condition, the item open in the window is `open item` (`links to open item`, seventh set). A name the element lacks is an install error |
| **X5 Durations** | Units `s`, `min`, `h`, `d`, `w`, `mo`, `y`, written without a space (`30min`, `6mo`); `m` alone is refused, naming the line |
| **X6 Numbers, units and money** | `number` is base ten, with up to 38 coefficient digits and up to 18 fractional places. Division truncates toward zero at 18 places. `round` rounds ties away from zero. A two-unit number is stored in the first unit; the unit table is the engine's own. Arithmetic across currencies is an error value |

### 4.2 Operators and empty

| Rule | Text |
| --- | --- |
| **X7 Precedence** | Highest first: call and `.`; unary `-`; `* /`; `+ -`; comparisons `= != < <= > >= in`; `not`; `and`; `or`; left to right. `date + duration` is a date; `date - date` is a duration |
| **X8 Empty** | `x = empty` and `x != empty` test for no value (fifth set: no `empty(x)` function). In a condition, empty counts as false; any other comparison with empty is false; arithmetic with empty is empty. `first(a, b, …)` gives the first non-empty value, for "otherwise" |
| **X9 Membership** | `x in ("a", "b")`. `links to <item>` is true when the item links to that item (`links to open item`) |

### 4.3 Every function

| Function | Example |
| --- | --- |
| `if(c, a, b)` | `if(late, "Late", "On time")` |
| `sum(list[, condition])` | `sum(tasks.hours)` |
| `count(list[, condition])`, `count(records after state.<key>)` | `count(depends on, status != "done")`, `count(records after state.read through)` |
| `min(list[, condition])`, `min(a, b)` | `min(available, 0)` |
| `max(list[, condition])`, `max(a, b)` | `max(previous(available), 0)` |
| `average(list[, condition])` | `average(records.reps)` |
| `first(list[, condition])`, `first(a, b, …)` | `first(end, start + 15min)` |
| `last(list)` | `last(log.done)` |
| `from(<kind>.<link field>[, condition][, ignoring default])` | `sum(from(transactions.category).amount)` |
| `from(<kind>.records.<link field>[, condition])` | `max(from(sessions.records.exercise).e1rm)` |
| `all(<kind>[, condition])` (in `totals:`) | `sum(all(categories, group = "Income").spent)` |
| `previous(<field or list>)` (in `per` fields and totals) | `previous(available)` · `sum(previous(all(categories).overspent))` |
| `today()`, `now()` | `due < today()` |
| `days(a, b)` | `days(added, today())` |
| `date(t)` | `date(start) = today()` |
| `week(d)`, `month(d)`, `year(d)` | `month(date) = period` |
| `format(v, pattern)` | `format(taken, "MM-DD")`; patterns are §2.3's date tokens, and `0.00` for numbers |
| `next(repeat[, after: d])` | `next(repeat, after: last(log.done))` |
| `streak(records)` | `streak(log)` |
| `under(tags, "<parent>")` | `under(tags, "area")` |
| `round(x[, n])` | `round(average(records.weight), 1)` |
| `domain(url)` | `domain(url)` |
| `contains(text, part)` | `contains(subject, "invoice")` |

- **`count(records after state.<key>)`** counts visible records strictly
  after the anchor in the current item's ledger order. For a removed
  record the host may supply its retained position as the number of
  visible records at or before it. Empty state or an anchor missing
  from both the visible records and the supplied positions counts all
  visible records.
- **Twenty-five functions** (fifth set: `empty(x)` removed).
- **`week`, `month` and `year`** return the period itself (`2026-11`).
- **New functions** come only from the engine; plugins never ship their
  own.

### 4.4 Limits

- **What a formula may not do:** write anything, reach the network, call a
  plugin, read outside its space, or read what the asking person may not.
- **One link hop** (F3). A computed link counts as the hop.
- **At most 1,000 items** per `from()`, `all()` or link list, counted after
  the condition (F4).
  - The index serves link, period and equality on stored fields.
  - Any other condition is evaluated over at most 10,000 candidates
    (operator default).
  - Past either limit, the value is "too many items" (review 26).
- **A fixed step budget** per item.
- **An index budget per plugin per space** (operator default). Over it,
  that plugin's computed fields show "too costly", and the Owner is told.
  Computed fields added through `extends:` may not use `from()` or `all()`
  (review 52).
- **No cycles;** `previous()` is not a cycle.
- **Bounds within the limits** (S38):
  - dates outside 1900–2200 are a Problem;
  - periods count at most 20 years back (operator default);
  - repeat expansion is capped per view, and `next()` searches at most 10
    years;
  - each query has a step budget per viewer, counted in the plugin's
    budget;
  - an expression nests at most 64 deep, and a formula holds at most
    4 KiB;
  - line items are capped per note;
  - the engine's budgets are the same on every device.

### 4.5 Time

- **`today()`:** formulas with it re-run at midnight. On the server that is
  the space's zone; a device shows its own and marks a difference as not
  yet confirmed (review 24).
- **`now()`** may appear only as one side of a comparison whose other side
  does not contain `now()`. The formula re-runs when that comparison flips.
  Any other `now()` in a stored computed field is refused at install
  (review 25). In `set` it is just the current time.
- **Repeats** are expanded by the engine in calendar views and in
  collections with `expand: repeat`. An item with `series` and
  `occurrence` fields replaces its slot, and a `dates` field mapped to
  EXDATE removes one (inherited 0067).

### 4.6 `per <period>` fields (F1, review 27)

- **Range:** periods run from the period of the kind's oldest item to the
  current one.
- **Display:** a view shows the current period, with a period picker
  wherever a `per` field is shown.
- **Back-dated changes:** recomputation runs as a background job, and the
  values show as not yet confirmed until it finishes.
- **Over the cap:** "too many items" in one period stops that period only,
  and `previous()` of it is empty.

### 4.7 Where values live

- **Stored values** live only in the rebuildable index.
- **Values that depend on the viewer** are computed at query time.
  Per-viewer is transitive: a field that reads one is per viewer.
  Per-viewer fields may not appear in `folder`, `file`, `unique`,
  `together`, `search`, `ical`, `vcard`, the `lines` box mapping or
  notification placeholders (S37).
- **The one Rust engine** runs on the server, on devices and in the browser
  as Wasm (round 33). Formulas and totals run there over the data the
  device may hold. Plugin TypeScript runs on the server (§2.20). A
  calculation outside the formula language needs an engine function.
- **A device's value** is "not yet confirmed" until the server agrees
  (S3).

## 5. Collection files

### 5.1 Every key

| Key | Value | Example |
| --- | --- | --- |
| `collection` | ID | `collection: unread` |
| `title` | Name | `title: Unread` |
| `format` | `1` | `format: 1` |
| `from` | Sources (5.3) | `from: tasks, events` |
| `where` | A condition | `where: not read` |
| `sort` | `<field> [asc\|desc]…`, with `manual` first for a person's own order | `sort: manual, due` |
| `group` | A field or formula; a list field puts the item in each group | `group: month(taken)` |
| `totals` | Unnamed lines over this collection, and named lines over whole kinds | 5.4 |
| `fields` | Computed columns (`= formula`, per viewer allowed), or member fields | `fields:` / `  late: = due < today()` |
| `expand` | `repeat` | `expand: repeat` |
| `drop` | What dropping an item here sets | `drop: set scheduled = today()` |
| `views` | One line per view; the first is the default | 5.2 |
| body | Hand-added items: `- [<title>](<link>)[; <field>: <value>]…` | `- [Mail](<../../Mail/x.md>); status: "Done"` |

When a person changed a collection and the plugin updates it, their version
stays and the new one is offered, as with sidebars (review 17).

### 5.2 Every view

| View | Line form | Example |
| --- | --- | --- |
| list | `list: <column>…` | `list: payee, amount, next(repeat)` |
| table | `table: <column>…` | `table: date, payee, amount` |
| board | `board: <choice or yes/no field>`, or `board: <row condition>, <column condition>` (a matrix) | `board: status` · `board: priority in ("urgent", "high"), urgent` |
| gallery | `gallery: image\|preview\|<file field>, <column>…` | `gallery: image` |
| calendar | `calendar: <time field>[ .. <time field>][, colour <field>]` | `calendar: start .. ends, colour calendar.colour` |
| timeline | `timeline: <date field> .. <date field>` | `timeline: starts .. ends` |
| chart | `chart[ bar\|line]: <total> by <field or formula>` | `chart line: sum(volume) by week(item.date)` |
| map | `map: <place field>` | `map: place` |
| folders | `folders: <column>…` (`from: files` only) | `folders: name, edited, size` |
| compact | `compact: <column>…`, small: a widget, a carousel or a sidebar preview, laid out by its place | `compact: image` |

- **A column** is a field or a formula (fifth set).
- **Dropping into a board group** sets the field, or makes the group's
  conditions true where they are equalities; otherwise it asks.

### 5.3 `from:` sources

```text
from: <kind>…[, ignoring default][, <share modifier>]
<share modifier> = shared with me | ignoring shared
from: <kind>.records
from: lines(checkbox)
from: files
```

- **`<kind>.records`** lists every record of the kind's items, each with
  `item.<field>`. `records` is reserved, so `p.records` is never a kind
  (review 6).
- **`lines(checkbox)`** lists plain checkboxes. Each row has `text`,
  `done`, `note` and `task` (review 17).
- **`files`** lists every file and folder, with `name`, `folder`, `size`,
  `edited`, `type` and `kind`, filtered by the person's rights.
- **Shared collections** (O1, round 34) are the exception to the space
  boundary for collections. A collection shared with a person supplies
  its items to that person's own collections of the same fully qualified
  kind, including a Photos timeline. The source collection's membership
  and the receiving collection's filters both apply. An incoming row
  is never a source for sharing that row onward.
- A kind source in a person's own collection includes local and shared
  items by default. `shared with me` keeps only shared items.
  `ignoring shared` keeps only local items. The two modifiers are
  mutually exclusive. Record, checkbox and file sources are local.
- **Check each item** against the current source rights and eligible
  share before drawing it or counting it. One source item appears once
  even if several shares include it. Eligible look and change grants
  apply to that original item, and edits and removals under a change
  grant go through its own checked operations, Trash and history.
- Filters, columns and direct totals can use the visible rows in the
  merged window. Formula traversal and whole-kind queries stay in the
  space of their evaluation. A share confers no right to read the source
  item's other links or to pass its contents to a plugin or run.

### 5.4 `totals:` (fifth set: `total` merged in)

```text
totals:
  count|sum(<f>)|min(<f>)|max(<f>)|average(<f>)
  <name>: [per day|week|month|year ]= <formula with all()>
```

```markdown
totals:
  count
  sum(amount)
  ready to assign: per month = previous(ready to assign)
      + sum(all(categories, group = "Income").spent)
      - sum(all(categories, group != "Income").assigned)
      + sum(previous(all(categories).overspent))
```

- **Unnamed lines** total this collection's items, per group and for the
  whole list.
- **Named lines** are values over whole kinds, shown in the header.
- **Long formulas** continue per L7.

### 5.5 `sort: manual`, `drop:`, `expand:`

```markdown
sort: manual, pinned desc, due
drop: set scheduled = today()
expand: repeat
```

- **`manual`** keeps each person's order per collection and group, as host
  app data that is backed up and never rebuilt (inherited 0066).
- **`drop:`** applies when an item is dropped onto the collection or its
  sidebar entry.

## 6. Sidebar files

A sidebar's body lists entries in order; customising it means adding,
removing or reordering lines. Each member's copy lives in their personal
space; an update keeps a changed copy and offers the new one (G2).

```text
- [<title>](<relative link to a collection>)
- [<title>](<<plugin>:collections/<id>>)
- each: <kind>[, where <condition>][, sort <field>[ asc|desc]][, under "<heading>"] -> <collection> by <link field>
- each: tags of <kind>[, under "<parent tag>"] -> <collection>
- section: <heading>
- folders: <folder>…
```

```markdown
---
sidebar: tasks
title: Tasks
format: 1
---
- [Today](<../collections/today.md>)
- [Loose ends](<calternal-tasks:collections/loose-ends>)
- each: calendars, where account = empty, under "Calendars" -> week by calendar
- each: tags of tasks -> all-tasks
- section: Collections
- folders: Documents, Photos, Notes
```

- **`each: <kind> … -> <collection> by <link field>`** makes one entry per
  item. Opening an entry shows `<collection>` filtered to items whose
  `<link field>` is that item (review 18).
- **`each: tags of <kind>`** makes one entry per top-level tag, or per
  child of `<parent tag>`. Opening it filters `<collection>` to that tag
  and its children.
- **Links to another plugin's collection** grant nothing.
- **Lines another plugin adds** (§7.2) are drawn after this file's lines,
  and are removed like any line.

## 7. `plugin.md`

### 7.1 Every key

| Key | Value | Example |
| --- | --- | --- |
| `plugin`, `name`, `icon`, `version`, `format` | ID (N1), what people see, a semantic version, `1` | `version: 1.0.0` |
| `needs` | Only for versions and `optional`; everything else is derived from `reads`, `calls`, `extends`, `listens` and link types (sixth set) | `needs: calternal-contacts >=1 optional` |
| `reads` | Other plugins' kinds its code may query, read and get `on.change` for (C4) | `reads: calternal-mail.messages` |
| `calls` | Other plugins' operations its code calls outside a person-picked slot admission (§7.4) | `calls: calternal-files.add` |
| `sends out` | `nothing`, or hosts, `a server you name`, `the public web` | `sends out: caldav.icloud.com, a server you name` |
| `sign-ins` | Accounts the server signs in to; several allowed (G12) | 7.3 |
| `secrets` | Values the Owner enters once, each for one host and one header | `secrets: maps key for api.maptiler.com in X-Api-Key` |
| `programs` | Plugin-local program bindings for runs or jobs | `programs:` / `  ocr: executable programs/ocr` |
| `outside` | Outside addresses, and how each proves itself | 7.5 |
| `extends` | Additions to other plugins' kinds, sidebars and Home | 7.2 |
| `slots`, `fills` | Places it offers, and places it fills | 7.4 |
| `listens` | Other plugins' operations it reacts to after they commit | 7.6 |
| `notifications` | The texts it may send, as templates | 7.7 |
| `settings` | Typed settings (inherited ADR 0055) | 7.8 |
| `refuses` | Conditions that can refuse another plugin's operation | 7.9 |
| body | What it does, shown at install | |

A `programs:` block maps a program ID to `executable <path>` or
`image <path>` inside the installed bundle. The format parser also reads
a bare path as an executable. Install checks that the bound file is
present, shows the binding to the Owner, and requires Owner approval.
Only a bound program can be supplied for execution. Image artifact
loading remains open in the run service.

**Destinations** (review 44, 45; S17, S18; these rules only narrow):

- **Every destination is checked on the connected address after DNS:**
  fixed hosts, `a server you name` and `the public web` alike, and on every
  redirect hop.
  - The check applies to `s.fetch`, `s.connect`, the proxy, CalDAV sync
    and sandbox egress.
  - Loopback, link-local, private, carrier-grade and mesh ranges, and the
    server's own addresses, are never reached.
  - The operator may allow named private hosts.
- **Secrets** (`secrets: <name> for <host> in <header>`) are put only in
  that header, over TLS with a valid certificate. A secret is never put in
  a URL or a body, never carried across a redirect, and scrubbed from
  answers.
- **`a server you name`** has its name and port stored with the sign-in
  when the person confirms it. A change asks that person again.
- **No operation input or field holds a credential** (S40). Sign-ins and
  secrets are entered only on the server's sheet. The install check
  refuses field and input names `password`, `token` and `secret`.

### 7.2 `extends:` (M2)

```text
extends:
  <plugin>.<kind> | any kind with url|file|person|place|time|date:
    fields:
      <field line, §2.7; `mark` allowed>
    operations:
      <operation line, §2.20>
    together:
      <own added fields>
    row: + <field>…
    page: + [section "<heading>": ]<field>…
  <plugin>:sidebars/<id> | home:
    <sidebar line, §6>
```

```markdown
extends:
  [calternal-mail.messages]:
    fields:
      saved: yes/no, default no
      newsletter: = contains(from, "newsletter"), mark
    operations:
      send to reading list: code, menu
    row: + saved
  any kind with url:
    operations:
      save link: code, menu
  calternal-notes:sidebars/notes:
    - [To read](<collections/unread.md>)
  home:
    - [On this day](<collections/on-this-day.md>)
```

- **Names inside the block:** a bare name means the plugin's own added
  field first. Everywhere else an added field is written in full,
  `[calternal-reading-list.saved]` (review 39).
- **Added fields** are stored as flat dotted keys. They are written only by
  the adding plugin's operations, or by a person on an editable-outside
  kind; the owner ignores them. Removing the plugin hides them, and the
  values stay (G1).
- **Added operations** run as the adding plugin, and change the owner's
  fields only through the owner's listed operations.
  - In `extends:`, the only operation lines allowed are `set` on the
    plugin's own added fields, `open where` and `code`.
  - The only placement is `menu` (S29).
  - Operations added by a member's plugin appear only for members who turn
    them on, in menus and in MCP alike.
- **Look additions:**
  - they append at the lowest priority and never cover `protected:`
    fields;
  - they are drawn with the adding plugin's icon;
  - additions from a plugin a member (not the Owner) added are shown only
    to members who turn them on (review 48).
- **`any kind with <type>`** matches stored fields of exactly that type and
  excludes hidden kinds. Matches are re-evaluated when a kind is installed,
  created in the app or updated. Each new match is listed on that sheet,
  or as a notice for app-made kinds, and starts off when it comes from a
  member's plugin (review 40, S36).
- **Agents** get a member's plugin's operations only if they turned them
  on, named `plugin.operation`. Every field value and plugin text reaches
  them labelled as data, in a server-written frame (S39).
- **What additions cannot do:** remove, rename, retype or reorder the
  owner's fields, operations or places.
- **The install sheet** lists every addition by target.

### 7.3 `sign-ins:`

```text
sign-ins:
  <name>: imap|smtp|http|oauth <provider>, per <kind>, for <host>|a server you name
```

```markdown
sign-ins:
  mail account: imap, per accounts, for a server you name
```

Each sign-in is tied to the person who added it and to the item it serves.
The plugin reaches it only through `s.connect` or `s.fetch`, and never sees
its value.

**Who may use it** is OPEN (E.1, §17). Until the owner answers, the strict
default holds: a sign-in is used only in work whose acting person (§8.2) is
the person who added it. Any other use fails with `not-allowed`, and every
use is stamped with who used it.

### 7.4 Places: `slots:` and `fills:` (M1, G13)

| Place | Single or many | Gives | Who picks |
| --- | --- | --- | --- |
| `shell strip` | single | the apps and sidebars | person |
| `shell panel` | many | the open window's item | person; the AI tab the space |
| `palette` | many | the typed text, only for entries the person opens | automatic; the person may hide |
| `search` | many | the query, only in a scope the person picked | person |
| `receive` | many | a file, URL or text the person shares or drops | person |
| `export` | many | the selection | person |
| `kind <kind> panel` | many | the open item | person |
| `kind <kind> page end` | many | the open item | person |
| `kind <kind> list` | single | the rows of a window | person |
| `collection view` | many | the window's rows | person, per collection |
| `widget`, `menu bar` | many | a collection's compact view | person, on the device |
| `<plugin>:<slot>` | as declared | as declared | as declared |

```text
slots:
  <slot>: single|many, gives "<what>", picked by person|space[, recommended <plugin>][, inputs <operation>]
fills:
  <place>: frame <path>|operation <operation>|collection <id>[, as "<label>"]
  receive: url|text|file… -> operation <operation>
```

```markdown
slots:
  composer: single, gives "the draft and the chat", picked by person, recommended calternal-chat, inputs chats.send
fills:
  kind [calternal-mail.messages] panel: frame frames/summary.html
  search: operation search archive, as "Mail archive"
  receive: url, text -> operation add
  calternal-chat:composer: operation send
```

- **`frame <path>`** names a file inside `frames/` (S14).
- **Place keys follow L1:** `kind [calternal-mail.messages] panel` and
  `calternal-chat:composer` are each one key, ending at the first `: `.
- **Typed slot inputs.** `inputs <operation>` references one local
  operation's nonempty typed input block, by unambiguous name or
  `<kind>.<operation>`. Operation fillers use that schema, fixed per
  format version, with no optional extras or negotiation.
- **A place gives only what its row says.** A person can select an
  installed plugin for a person-picked slot in the call envelope,
  `"slot_selection":{"slot":"harness","filler":"my-harness"}`.
  The host resolves that filler to its one declared coded operation
  and supplies its qualified name as `s.event.selected_operation`.
  Under that selection the calling plugin may call that operation
  without a `calls:` entry. The selection doesn't travel to reactions
  or further nested calls. An operation name in ordinary input grants
  no authority.
- **Places that see typed text** say so on the pick screen, and are called
  only when the person opens that entry or scope.
- **A plugin's search** answers only in a scope the person picked, within
  a time budget, in its own section.
- **Errors stay local:** a broken part draws an error in its own place.
- **The first run** is set only in `space.md` (§12; sixth set).

### 7.5 `outside:` (C2, sixth and eighth sets)

```text
outside:
  <name>: [signed <header> with <secret>, ][person, ]methods <METHOD>…
```

```markdown
outside:
  scanner: methods POST
  github: signed X-Hub-Signature-256 with webhook secret, methods POST
  api: person, methods GET POST
```

- **The address:** every outside address is
  `https://outside.<server>/<space>/<plugin>/<name>/<token>/…`. The token
  is 128 bits made by the server, shown once on the sheet and rotatable. A
  plugin may mint addresses below it (`s.outside.mint(item)`), each with
  its own token.
  - Without the token the server answers 404 and wakes nothing.
  - Requests are rate-limited per address, and bodies and answers have a
    size and time bound (operator default) (S1).
- **Proofs:** the token is always required. `signed` adds a signature
  check, and `person` adds a person's token that proves only this address:
  the person ticked it, and it is refused on the API. (`secret` and
  `signed` are one proof now; review simplification.)
- **The server strips every proof** (the address token, the signature
  header and the person's token) and every cookie before the request
  reaches the plugin, so a plugin can never replay them (S2).
- **Cookies and headers** (S25):
  - the app's cookies are `__Host-` and `SameSite=Strict`;
  - the app's API refuses requests whose `Origin` or `Sec-Fetch-Site` is
    the outside origin;
  - outside answers pass only a header allowlist: `Content-Type` from a
    list, `Cache-Control`, `ETag`, and `Location` within the plugin's own
    path;
  - every answer is marked sandboxed;
  - the operator may give outside its own registrable domain, which is
    recommended.
- **The request reaches `on.outside(name)` whole** (method, path below the
  token, allowed headers, body), and the operation answers with a status,
  headers and a body.

### 7.6 `listens:` (M3)

```text
listens:
  after <plugin>.<operation>
```

```markdown
listens:
  after calternal-mail.receive
```

- **Disclosure:** the install sheet says what the event carries ("Sees
  every mail received, with its sender").
- **Never delivered:** credentials. No input can hold one (§7.1).
- **A listener that also sends out** is a risky install, confirmed with a
  passkey (review 51).

### 7.7 `notifications:` (sixth and eighth sets)

```text
notifications:
  <name>: "<at least three words of fixed text, with {<field>} placeholders>"[, actions <operation>…]
```

```markdown
notifications:
  remind: "Time to read {title}"
```

`s.notify` sends only these templates. S20's rules:

- **Fixed text:** a template has at least three words of fixed text, never
  only a placeholder.
- **Placeholders** take only stored fields of the item: never computed,
  `data` or added fields, and never fields the plugin's own code wrote.
- **Each filled value** is cut to 60 characters, with control and format
  characters removed, never turned into a link, and drawn as a quotation.
- **The notification always names the plugin**, and the install sheet
  shows every template.
- **Buttons** (O2, round 34) are operations from the item's own operations
  list, named with `actions`. They are drawn by the device's system
  notification. On a locked device only actions the receiving person
  marked safe are offered. The operation runs as the person who pressed
  it and goes through the usual checks. A plugin cannot mark an action
  safe for someone.
- **Other native surfaces derived from declarations** (O2):
  - share targets come from `fills: receive`;
  - there is one generic Shortcut per verb (add, change, open, run an
    operation), whose options are read from the operations lists;
  - widgets and the menu bar fill `widget` and `menu bar` places with a
    collection;
  - live progress comes from `progress:` (§2.6a).
  - Apple Health and the camera roll use bridges furnished by the
    first-party apps. Generic plugins cannot supply device bridge code.

### 7.8 `settings:`

```text
settings:
  <id>: <type>[, default <value>][, per person|space]
```

```markdown
settings:
  urgent within: duration, default 2d, per person
```

Formulas read settings as `settings.<id>`. A formula that reads a `per
person` setting is computed per viewer (§2.9).

### 7.9 `refuses:` (round 34, later the same day)

```text
refuses:
  [<qualified operation>]:
    <condition>, "<sentence>"
```

```markdown
refuses:
  [calternal-money.transactions.add]:
    amount > 1000 EUR, "Payments over 1,000 EUR need separate approval."
```

- **When it runs:** the engine evaluates the enabled conditions after the
  ordinary rights and input checks and before a handler or a write. It
  uses the same formula limits on the server and the device. The server
  checks again at submission, including after offline work.
- **Scope:** the declaration names an installed operation of another
  plugin in the same space. It sees the authorized operation inputs
  and the fields of the authorized target when there is one. An input
  name takes precedence over a field of the same name. The declaring
  plugin runs no handler and gains no read access or event. The check
  never changes an input.
- **When it holds,** the engine refuses the operation with the usual
  `not-allowed` error, the declaring plugin's name and its quoted
  sentence. If several match, the refusal uses the first in plugin ID
  and file order.
- **What it can never target:** Stop, undo, an author removing their own
  items or records, or leaving a space (§14).
- **Disclosure:** the install sheet shows every line. The Owner must
  approve the declaration and can turn off each condition.

## 8. Calls, events, the acting person and the write table

### 8.1 The fifteen calls

**Errors:** one caller-safe shape, `{ code, message, next }`, with ten
codes: `not-allowed`, `not-found`, `invalid`, `conflict`, `too-large`,
`limit`, `unreachable`, `needs-sign-in`, `stopped`, `timeout`.

| Call | Input | Output | Errors | Bound |
| --- | --- | --- | --- | --- |
| `s.declarations()` | `{}` | `{space, plugins}`: compiled reachable kinds, fields, operations and inputs, collections, sidebars, slots and fills | `not-allowed` | installed catalog |
| `s.query(q)` | `{ from, where, sort, group, totals, fields, cursor }` or a qualified `collection` with pagination | `{ rows, totals, cursor }`; rows include viewer `state` and operation-name booleans in `when` | `not-allowed`, `invalid` | 200 rows a page |
| `s.read(item)` | An item ID | `{ id, kind, fields, computed, state, when, edited, version, body, records }` | `not-found`, `not-allowed` | 32 MiB |
| `s.write(changes, { preview? })` | A list of `add(kind, fields, body?)`, `change(item, fields?, body?, base?)`, `remove(item)`, `move(item, folder?, file?)`, `append record(item, record)`, `change record(item, anchor, record)`, `remove record(item, anchor)` | `{changes:[{id, version, anchor?, link?}]}` in input order; with `preview`, the plan for a confirm sheet | `invalid`, `conflict`, `not-allowed`, `too-large`, `limit` | 1,000 changes, 16 MiB; all or nothing |
| `s.fetch(req, { secret?, sign-in? })` | An HTTP request to a declared destination | `{ status, headers, body }` | `not-allowed`, `unreachable`, `needs-sign-in`, `too-large`, `timeout` | 32 MiB, 60 s |
| `s.connect(sign-in, item)` | An IMAP or SMTP sign-in for an item | The connection the server holds | `needs-sign-in`, `unreachable` | 1 per sign-in and item |
| `s.notify(people, { item, template })` | People who may read the item; a declared template | `accepted`, whatever each person's mute (S21) | `not-allowed`, `limit` | 100 recipients |
| `s.job(program, items, input?)` | A declared program | A job ID; results come back through the plugin's operations | `not-allowed`, `limit` | 10,000 items |
| `s.run(program, { scope, targets })` | A declared program and target items, with `scope: session` by default (see below) | A handle `{ id, send(input), stop(), status }`; `send` only by the run's actor, `stop` by anyone who may read the target | `not-allowed`, `limit`, `stopped` | the run's quota |
| `s.call("<plugin>.<operation>", input)` | A listed or slot-selected operation | Its output | `not-allowed`, and its own errors | depth 8 |
| `s.state` | `get(item, key)`, `set(item, key, value)` for keys declared in `state:` | This person's value | `invalid`, `too-large` | 64 KiB per person and plugin |
| `s.cache` | `get`, `set`, `delete`, `file(name)` | Rebuildable data; may be deleted at any time | `limit` | per-plugin quota |
| `s.share(item, people)` | Who | Only in `on.call` from a live gesture. Opens the server's share sheet, starting at reader and listing only people the person can already see; returns when the person confirms or cancels (review 23, S41) | `not-allowed`, `stopped` | 100 people |
| `s.outside.mint(item)` | An item of its own kinds | A new outside address below a declared one, with its own token (S1) | `not-allowed`, `limit` | per-plugin quota |
| `s.me`, `s.space`, `s.event` | — | The acting person and their role; the space; `{ kind, key, item, before?, request?, selected_operation? }` | — | — |

In `s.write`, a record carries its declared fields and its Markdown body.
The host supplies its author, receipt time and anchor. A committed append
receipt supplies a canonical Markdown record link, for example
`[Record](<Chats/A%20chat/chat.md#^0123456789abcdef>)`. Consumers resolve the
anchor in the authorized space if the path changes. Chat sends and starts
in one coded call under one outer request key. It appends the full
message, then calls the selected filler with `{chat: item, prompt: link}`.
Retries with that same key reuse the append receipt and the nested call's
derived key. The append remains committed if start fails. This provides
one gesture and a retryable sequence across two effects.

**Browser and live delivery.** The call route is
`POST /v1/spaces/{space}/call`. See the [host call contract](../reviews/host-call-route-2026-11-27.md)
for the request envelope and the implemented input subset. Both query
rows and read metadata carry `state` and `when`. Clients draw the
host-evaluated operation conditions. The host checks authorization again
on execution. The browser uses a Secure, HttpOnly, SameSite=Strict
`__Host-calternal_session` cookie with `Path=/` and no Domain. The
WebSocket checks the configured origin and authenticates from that cookie
at upgrade. The existing private admin flow issues a fresh installation's
first-owner code. The development test supplies its disposable code by pipe.

Live query and record windows apply authorized reads for the receiving
person before sending. Private state invalidates only that person's
window. Losing access must send an explicit revocation so a client
closes the affected window and drops its data. Until that exists, live
windows stay owner-only. Session sign-out already closes its socket.
See the [live protocol](../reviews/live-window-protocol-2026-11-27.md).

**Retention.** Request receipts default to 7 days
(`CALTERNAL_RECEIPT_RETENTION_DAYS`); after expiry a retry is a new request.
Trash defaults to 30 days. The host checks for expired Trash on
access. The action log is pruned with receipts for this slice.

**Session scope.** With `scope: session`, the host projects each explicit
target's readable fields and the records in its current session. Use
the host's record order and receipt times. A gap of at least one hour
ends a session, including a gap between the last record and the run's
start. An item with no current records still has its permitted fields.
The next record starts a new session. The quiet-hour threshold is the
operator default.

The target set and starting boundary are fixed when the run starts.
The person's rights and agent rules remain ceilings at every call.
Writes are fenced to record operations on those targets that the
harness may call. A record or field link gives no additional run scope.
The Chat-first build uses this scope only.

### 8.2 The six events, and who acts

| Event | Fires when | Receives | Returns |
| --- | --- | --- | --- |
| `on.call` | The operation is run | The form's fields or the input | An output or an error |
| `on.time(<kind>.<field>)` | A `time`, `date` or `when` field arrives (a date at 09:00 in the acting person's zone), or a `repeat` field's occurrence | The item, and the occurrence | nothing |
| `on.change(<kind or collection>)` | Items of its own kinds or of `reads` kinds change; for a collection, when an item enters, leaves or changes | The item, and `s.event.before` | nothing |
| `on.outside(<name>)` | A request with the token reaches a declared address | `{ method, path, headers, body }` | `{ status, headers, body }` |
| `on.connection(<sign-in>)` | Data arrives on a held connection | The item and what arrived | nothing |
| `on.after(<plugin>.<operation>)` | A `listens` operation has committed | `{ input, result }`, filtered to what the acting person may read | nothing; it may call operations |

**One "who acts" rule** (S9, S10, eighth set; replacing per-event cases):

- **The person who last directed work at the item acts:**
  - for a gesture or a call (`on.call`), the person who made it;
  - for a change (`on.change`, `on.after`), the person who made the change
    or ran the operation;
  - for a time (`on.time`), the person who last changed the item, stamped
    in history (the arming person);
  - for a connection (`on.connection`), the sign-in's person;
  - for an outside request with `person`, that person;
  - for the space profile's first run, the Owner.
- **Rights are re-read when the work runs.** If the acting person is no
  longer a member or has lost the rights, the work does not run, and the
  item is listed in the plugin's Problems.
- **A run's context travels along:** work a run causes carries the run
  through every event it raises, and each operation in the chain is
  checked against that run's agent rules (§10).
- **A reaction the person did not direct at this plugin**:
  - cannot use `s.run` or `s.share`;
  - reads `s.state` only for its own kinds;
  - may call an operation that reaches other people (§1.1a) only when the
    event came from that person's own gesture, not a run's.
- **The sheet says** "Acts as whoever changes *X*: then may *calls list*".
- **With no person** (an outside request without `person`), a handler
  reads and writes only the plugin's own kinds. `s.call`, `s.run`,
  `s.share`, `s.connect` and `s.notify` are refused, and `s.fetch` reaches
  only declared hosts (S3).
- **Timing:** a missed time fires once on restart; a repeat fires at most
  every 5 minutes (operator default).

**Delivery:**

- **One path:** `on.change` and `on.after` are raised by the call route
  after commit. An isolated install probe discovers the `on.change`
  selector names. The host validates and stores names, not handler
  code from the probe, and reuses those registrations. Prefix a
  selector with `collection:` to distinguish it from a kind.
- **At least once, with a key.** Each write is keyed as (event key, index
  of the write within the run), so a replayed handler that writes in the
  same order replays exactly (review 21).
- **Coalescing:** changes are coalesced per item.
- **Loops:** call depth travels with the event key across deliveries. A
  plugin never receives `on.after` or `on.change` for a commit its own code
  caused, directly or through `s.call`. An event at depth 8 is not
  delivered, and is shown on the plugin's Problems (review 36).
- **Processes:** at most one active worker per plugin and space.
  The worker stops on idle and is recycled when the acting person
  changes, so cached results cannot cross between people. Installed
  code stays off without an OS sandbox with filesystem and network
  isolation, resource bounds and process-family cleanup.
- **Worker frames:** one UTF-8 JSON value per line, at most 64 MiB
  excluding LF. Oversized callback results return `too-large` in a
  bounded error frame. Oversized or invalid inbound worker frames
  fail the invocation. Chunked results are deferred.

### 8.3 What `s.write` may touch (review 22, S11, S13)

| Target | Allowed changes |
| --- | --- |
| The plugin's own kinds | Every change form; `data` fields only here |
| Another plugin's editable-outside kind | Only through that kind's built-ins named in `calls:` (`calls: calternal-notes.notes.change`; the sheet says "Can change your notes"); `reads:` grants reading only (C4) |
| Another plugin's operations-only kind | Only the plugin's own added keys (§7.2) |
| Another plugin's hidden kind | Nothing |
| `move` | Only within the item's own kind's folder tree; refused for patterned kinds unless the pattern fields change with it |
| Any other file | Nothing |

**`s.call` designation** (S12): an item named in `s.call`'s input counts
only if the caller could read it itself (its own kinds, `reads:`), or
received it from a place, an event or an earlier result. Otherwise the call
fails `not-found` before the callee runs. Items in the output of kinds the
caller cannot read return as IDs only.

## 9. Outside writes (WebDAV, CalDAV, CardDAV)

Review 12, 13, 33, 38, 47 and 49, and the sixth set.

- **Checked as an operation:** an outside write is diffed field by field
  and checked as the built-in `add`, `change` or `remove`. That covers
  their `only` and `when`, `rights:`, `checks:`, `data` fields and records.
  A failure refuses the whole request with the reason (HTTP 403 or 409).
  A Problem is only a file the server finds changed on disk outside its
  own protocols.
- **`data` fields** are changed only by the owning plugin's code, on every
  path.
- **Kind and exposure follow the item's ID** in host state, never its path
  (S7). The `files` built-in moves an item only within its kind's folder
  tree, and operations-only items are moved only by their plugin's
  operations.
- **Records:** adding, changing or *removing* a record not authored by the
  writer is refused (Managers may remove, inherited D4). So is reordering
  records, and changing one outside its window. The page always draws the
  author from the ledger (S44).
- **Which kind a new file belongs to:** the kind whose folder pattern
  matches it most deeply. When two kinds match the same path, the `file:`
  pattern decides; a full tie was already refused at install.
  - A taken `id:` gets a new ID.
  - A file's `kind:` line is ignored.
  - Defaults are written into the file once, when it is accepted, as one
    recorded write.
- **Moves:** items are addressed by ID.
  - A PUT to a path that moved is applied to the item that was there, with
    a redirect to its new path.
  - An outside MOVE of a patterned file is refused.
  - An outside MOVE of an unpatterned file changes only its path.
- **PUT without `If-Match`:** on an item with records, it is merged
  three-way against the version the client last fetched, and record
  operations are replayed by anchor. An operation whose anchor is gone
  becomes a Problem, with its values.
- **CalDAV and CardDAV writes** are the kind's listed built-ins (§2.18).
- **`.calternal/`** refuses every outside write.

## 10. Agent rules files

`Agents/rules.md` holds a person's rules; `.calternal/agent-rules.md` holds
a space's. A person's rules only narrow the space's (R2).

```text
- allow|ask|deny <operation>[ on <kind>][ in <plugin>:collections/<id>][ where <condition>][ outside this chat][ for <harness>]
- allow|ask|deny add|change|remove in <folder>/[ for <harness>]
- allow|ask|deny fetch <host>[ for <harness>]
```

```markdown
---
rules: agents
format: 1
---
- ask calternal-mail.send
- deny calternal-money.* on transactions where amount > 500 EUR
- allow change in Notes/Work/
- deny fetch github.com for calternal-chat-codex
```

- **`in`:** a collection is written `<plugin>:collections/<id>`, and a
  folder ends in `/` (review 19).
- **Who may change the rules files** (S8): `Agents/rules.md` and
  `.calternal/agent-rules.md` change only through the server's own sheet
  (Settings, or the "Always allow" prompt). An operation, a run, a plugin
  or an outside write never changes them; they are host screens for §14.
- **Matching** (S32): a rule matches if its `on`, `in` and `where` hold
  before *or* after the change. A move is checked as `remove` at its source
  and `add` at its destination.
- **Reaching other people** (S33) uses the derived flag of §1.1a. R1's
  "ask" for it is a floor that only the person's own "Always allow" lifts,
  for their own runs. A space's rules may hold `allow` only for operations
  that do not reach other people.
- **The condition** runs to ` outside this chat`, ` for ` or the end of the
  line, and may not contain those words outside quotes.
- **Precedence:** deny, then ask, then allow; the most restrictive match
  wins.
- **Fixed ceilings:** the starter's rights, the run's scope, the space's
  Agent access switch, and the server's risky actions (passkey).
- **Defaults (R1):** in the personal space everything is free except
  reaching other people. In a space with members, removing and changing
  others' items also ask.
- **"Always allow"** writes one line with `for <harness>` (R3) and never
  expires (R6). When it was added and last used is host state.
- **No reviewing model** decides (R4).
- **Harnesses:** a direct run on the person's own machine uses the
  harness's own permissions (R5). Our harnesses run permissive inside our
  sandbox (R7).

## 11. Theme files (M4)

```text
theme: <id>
name: <name>
format: 1
palette:
  background|text|accent|link|success|muted: "<colour>"
fonts:
  interface|chat|code|wordmark: <assets/ file>|<a font on the server's list>
icons: <assets/ folder: one SVG per host icon name>
sounds:
  <host sound name>: <assets/ file>
motion: <host preset>
wallpaper: <assets/ file>
window: opaque|frosted
for:
  <plugin>: [accent "<colour>"][, density compact|regular]
```

```markdown
---
theme: dusk
name: Dusk
format: 1
palette:
  background: "#1f1f1f"
  text: "#f7f7f7"
  accent: "#c96f4a"
fonts:
  interface: Google Sans Flex
  code: assets/fonts/cascadia.woff2
icons: assets/icons/
motion: calm
wallpaper: assets/dusk.avif
window: frosted
for:
  calternal-money: accent "#2f7d5b", density compact
---
```

- **Palette:** named roles, not positions. A role left out keeps the
  default (review 62).
- **Fonts:** a file in `assets/`, or a font on the server's list; a theme
  never downloads from elsewhere.
- **What a theme cannot set:** CSS, positions or sizes.
- **Guarantees:** a contrast floor; reduced motion always wins; host
  screens are not themed.
- **Never themeable** (S34): the warning and danger colours, and the icons
  in protected strips and host warnings. Protected fields, addresses and
  amounts are drawn in a host font.
- **Assets:** theme SVGs are reduced to paths, with no script,
  `foreignObject`, `href` or `style`. Fonts with contextual substitutions
  across letters are refused.
- **Assets** pass the font, wallpaper and sound checks.

## 12. The space profile, `.calternal/space.md` (M5)

```text
space profile: <id>
format: 1
strip: <plugin>…
picks:
  <place>: <plugin>
rights:
  [<plugin>.<kind>]: <right>
  [<folder>/]: <right>
accent: <colour>
icon: <asset path>
agent rules:
  - <rule line, §10>
first run:
  <plugin>: operation <operation>
theme: <theme>

<right> = change manager|owner | read <role or group>… | change manager|owner, read <role or group>…
```

```markdown
---
space profile: family-money
format: 1
strip: calternal-money, calternal-tasks
picks:
  shell panel: calternal-chat-claude-code
rights:
  [calternal-money.transactions]: change manager
  [Documents/Taxes/]: read manager, family-adults
accent: "#c96f4a"
icon: assets/family.png
agent rules:
  - ask remove on calternal-money.transactions
first run:
  calternal-money: operation setup
theme: dusk
---
```

- **Who writes it:** the Owner, or an applied profile a plugin ships. The
  install sheet lists every line.
- **It only tightens:** it raises roles and narrows rules, never widens
  them, and installs nothing itself.
- **`first run`** runs once, when the Owner applies the profile, as the
  Owner. It is the only first-run declaration.
- **Not tightening** (S35):
  - a pick of a part that sends out needs the passkey (T7);
  - an updated profile is offered, never re-applied;
  - the sheet says "tightens rights and rules; sets defaults".
- **A space's look** (round 34, as recommended): `theme` is a default each
  member may change for themselves. `accent` and `icon` are the space's own
  and bind everyone, so a member always knows which space they are in.
  The accent is a `colour` (§3.1). The icon is a checked image path
  under the profile's owning `assets/` directory (§1.1a). Neither
  changes protected host surfaces.
- **Folder rights** follow §2.6, and are inherited downward.

## 13. Remote content (sixth and eighth sets; S16)

- **Only stored values** load, never computed or added fields, and only as
  images or previews, through the server's proxy, without cookies.
- **Who wrote the value:** the proxy loads a value only if a person, or an
  outside sender, wrote it. A value written by plugin code loads only when
  its host is in that plugin's `sends out`.
- **Every hop**, redirects included, passes §7.1's address rules.
- **What it fetches:** only `http` or `https` on ports 80 and 443, and only
  image types. There is a size and time cap, and no headers are forwarded.
- **The cache** is per space. Plugin code cannot reach the proxy.
- **Mail bodies** load nothing remote until the person taps (inherited
  I1).
- **Frames** reach the network only under their own origin's rules and the
  web warning (§1.1).
- **Run egress** is through the host proxy alone, to hostnames declared
  in the harness's `sends out:`. The run sandbox has no direct network
  route. The proxy resolves names, refuses private and internal addresses,
  dials the checked address and logs only the hostname and byte counts.
  The first run-service implementation mediates CONNECT on port 443
  through a per-run socket.

## 14. What is never open

No key, place, extension or profile reaches these:

- **Host screens:** sign-in, passkeys, recovery, the profile popover,
  Settings and Plugins pages, consent requests, safe mode, and Stop.
- **The one call check** and its table.
- **Another plugin's things:** its inputs before its operation; removing,
  renaming or retyping its fields or operations.
- **A file's identity:** its kind, ID, author stamp and outside-editing
  switch.
- **Other spaces,** except eligible shared collection rows in §5.3.
- **Sync:** the route, the journal and the merge.
- **Secrets and reach:** secrets' values, the network list and programs.
- **The client:** code outside a frame, and CSS.
- **Free notification text.**
- **The share sheet**, and any widening of sharing.
- **Host-reserved shortcuts.**
- **Another person's private state.**
- **A person's own way out:** nothing may refuse Stop, undo, deleting
  one's own items or records to Trash (§2.20), or leaving a space.
- **The agent rules files** (§10).
- **Instruction files** (§2.2).
- **The canvas of windows**, for now.

## 15. A first plugin

A first plugin needs these names and nothing else, the 37 that Reading List
(§16) uses. Everything else in this document can wait until an app needs it.

| Where | Names |
| --- | --- |
| `plugin.md` | `plugin`, `name`, `version`, `icon`, `format`, `notifications` |
| Kind | `kind`, `name`, `folder`, `editable`, `outside`, `fields`, `row`, `search`, `operations` |
| Field types and flags | `text`, `url`, `yes/no`, `date`, `time`, `required`, `default`, `today` |
| Formulas | `=`, `domain()`, `not` |
| Operations | `add`, `set`, `when`, `swipe`, `primary` |
| Collection | `collection`, `title`, `from`, `where`, `sort`, `desc`, `views`, `list`, `board` |
| Sidebar | `sidebar`, `- [title](<link>)` |
| Code (optional) | `on.time`, `s.notify` |

## 16. Reading List, written out in full

```markdown
<!-- plugin.md -->
---
plugin: calternal-reading-list
name: Reading List
version: 2.0.0
icon: book
format: 1
notifications:
  remind: "Time to read {title}"
---
Save links to read later, with an optional reminder.
```

```markdown
<!-- kinds/reading.md -->
---
kind: reading
name: Reading
format: 1
folder: Reading
editable: outside
fields:
  title: text, required
  url: url, required
  read: yes/no, default no
  added: date, default today
  remind: time
  site: = domain(url)
row: title, site
search: title, site, body
operations:
  add: url, title
  mark read: set read = yes, when not read, swipe, primary
---
Links saved to read later. One file per link.
```

```markdown
<!-- collections/unread.md -->
---
collection: unread
title: Unread
format: 1
from: reading
where: not read
sort: added desc
views:
  list: title, site
---
```

```markdown
<!-- collections/all.md -->
---
collection: all
title: All
format: 1
from: reading
sort: added desc
views:
  list: title, site
  board: read
---
```

```markdown
<!-- sidebars/reading.md -->
---
sidebar: reading
title: Reading List
format: 1
---
- [Unread](<../collections/unread.md>)
- [All](<../collections/all.md>)
```

```ts
// operations.ts: the only code, for the reminder; it acts as whoever last changed the item
export const remind = on.time("reading.remind", async (item, s) =>
  s.notify([s.me], { item, template: "remind" }));
```

- **Change and remove** are not listed, because the kind is editable
  outside.
- **Without `remind` and `notifications`,** it is a description-only plugin
  any member may add.
- **To appear in the system share sheet,** it adds `fills: receive: url ->
  operation add`.

## 17. Decided in round 34, and what stays OPEN

**Decided (round 34 and its later answers, 2026-11-27):**

| Item | Answer | Written in |
| --- | --- | --- |
| **O1** shared items across spaces | A shared collection's items join the other person's own collections of the same kind; the share's right decides look or change; the one exception to S4 | §5.3 |
| **O2** iPhone and Mac surfaces | As recommended: notification buttons from the operations list, share targets, one generic Shortcut per verb, a declared progress value; device bridges first-party only | §2.6a, §7.7 |
| **O4** reading rights inside a space | Yes: `rights: read …` on a kind or folder; nothing reveals an unreadable item, counts included | §2.6, §12 |
| A space's look | As recommended: the theme is a default; the space's accent and icon bind | §12 |
| `refuses:` | Yes, as a declared condition only | §7.9 |
| Plugin code on devices | Not now. Formulas and totals run in the core on the device; a TypeScript operation runs on the server and queues offline; what formulas cannot say becomes a new built-in function | §2.20, §4.7 |
| `session()` | A run scope, not a formula function | §2.20, §8.1 |

**OPEN (the owner's), with the strict defaults in force:**

| Item | The question | Default now, and the one-paragraph change |
| --- | --- | --- |
| **O3** Workouts' first version | Log only, or with progression? | The choice determines whether Workouts ships progression code |
| **E.1** whose sign-in another member's action may use | May Ana's action use Lee's mail account in a shared space? | **Default (strict):** a sign-in is used only by its own person (§7.3). **If the owner chooses (a):** §7.3 gains a sign-in sheet switch, "Members of *space* may send and read with this account". Every use is stamped, and the person can turn it off |
| **E.2** who writes instruction files (`AGENTS.md`, `CLAUDE.md` …) | May members write instructions that every agent in the folder follows? | **Default (strict):** only the server writes them (§2.2). **If the owner chooses (b):** anyone with write on the folder may, and the server prefixes "These instructions were written by *person*" |

## 18. Counts, and two ways to say one thing

| Surface | Revision 10 |
| --- | --- |
| Line grammar | 10 line rules, 6 name rules and 9 expression rules |
| Reserved words | 51 in five groups, plus 6 reserved plugin IDs |
| Kind keys | 28 including body and separate `ical` and `vcard` keys |
| Field flags | 7 |
| Operation line forms | 12 (3 in `extends:`), with one new nested key, `inputs`, on coded operations |
| Field types | 25 entries including `record anchor`, plus 4 built-in item fields |
| Functions | 25, with a new form of `count` |
| Collection keys | 13 including body |
| Views | 10 |
| Sidebar line forms | 6 |
| `plugin.md` keys | 21 including body; `programs` is an existing key with named child bindings; `inputs <operation>` is a slot clause |
| Outside proofs | 2 beyond the mandatory token |
| Host places | 12 |
| Theme keys | 11; 6 palette roles |
| Profile keys | 10 |
| Agent rule line forms | 3 |
| Calls | 15 including `s.declarations`, counting `s.me`, `s.space` and `s.event` together as the context row |
| Events | 6 |
| Error codes | 10 |

The call count is the design surface in §8.1. See the
[host call contract](../reviews/host-call-route-2026-11-27.md) for the
standalone host's smaller implemented surface.

**Still two ways to say one thing** (kept apart by the fifth set):

- `sign-ins:` and `secrets:`;
- `row:` and the `list` view;
- `timeline` and `calendar: a .. b`;
- `keep:` and `hidden: keep`;
- `from: files` and `folders:` lines, beside kinds' `folder:` patterns.

## 19. Contradictions resolved (revision 6)

Revision 6's twenty-five resolutions stand:

- **Operations:** one operations list; built-ins unlisted only on
  editable-outside kinds.
- **Dropped keys:** `card` and `cell`.
- **Formulas:** the function list; the cap counted after the condition.
- **Items:** file items with an XMP sidecar.
- **Reach:** `.calternal/` readable but refused for outside writes;
  `reads:` for code; `extends:` for additions; sidebar line forms.
- **Runtime:** the engine as Wasm; `on.outside` as a full request; `per`
  fields and `totals`; thirteen calls.
- **Superseded stress-test forms:** `available: code` gone; search by
  order; `sign-ins:`; the `add` line as the form.
- **Other:** daily notes in `Calendar/`; the Inbox expressible; places,
  look and `listens`.

Revision 7 adds six, all from the sixth set:

- six events again;
- the outside address on its own web address;
- frames make a plugin Owner-installed;
- `s.notify` takes templates, not text;
- `share` gone from `add`;
- `first run` only in `space.md`.

## 20. Choices that no decision covers (for the coordinator)

1. **L5: `#` starts a comment only when a space follows**, so a colour such
   as `#c96f4a` needs no quotes. This is simpler than the review's rule,
   which would make every colour a quoted value.
2. **N3: names may not contain the four operator words.** The review would
   refuse names containing *any* reserved word, which loses natural names
   such as `item count` or `when due`. As a result, Workouts' `reps in
   reserve` is renamed `reserve`.
3. **One form for built-in arguments, `remove: when …`.** The review would
   allow both `remove, when` and `remove: when`.
4. **Placements may be combined** (`swipe, primary`), except in `extends:`, which allows `menu` only (S29).
5. **Kind formulas that read `state.` or `per person` settings are computed
   per viewer**, instead of refused as the review proposed. This is more
   versatile; the cost is that those fields have no index.
6. **Moving any file is the person's built-in on `from: files`.** The
   review proposed a `moves files` line; with this choice, Files needs no
   code and no new key.
7. **Boards on yes/no fields** are allowed (two columns), so §16's `board:
   read` stands.
8. **Unmapped CalDAV properties live in host state**, as the sent snapshot
   (inherited 0067), never in the Markdown file.
9. **Palette roles:** six named roles. Warning and danger are the host's
   own and cannot be themed (S34).
10. **`assets/`** is the plugin folder for theme files.

## 21. Change list

### Revision 10 (Chat-first implementation, sets 11–16)

- Anchor state and ledger-order unread, typed coded inputs and slots, and
  one send-and-start gesture with a host receipt link.
- Bound programs, conservative assets, scoped slot admission, isolated
  handler discovery and worker limits.
- Host record seal, exact numbers, compiled declarations, viewer-aware row
  metadata and cookie-authenticated live windows.
- Local developer installation, retention, private operator setup, live
  revocation and mediated run egress.

### Revision 9 (round 34)

- Added the shared-collection exception and kind and folder read rights.
- Settled native surfaces, space appearance, and declarative refusals.
- Kept TypeScript operations on the server and specified `session` as the
  default run scope. E.1 and E.2 keep their strict defaults.


### Revision 8 (security review of revision 7, eighth set)

- **New §1.1a:**
  - text on sheets;
  - one rule for free strings and one for paths;
  - XMP without DTDs;
  - the derived flag "reaches other people or the outside";
  - reserved plugin IDs;
  - updates that wait.
- **Instruction files** are written only by the server (§2.2; E.2 strict
  default).
- **Folder patterns** may not nest in either direction; filled segment
  values and marked folders are checked (§2.3).
- **Lines:** one claim per line form, ticked boxes checked as `change`, and
  the calendar link stored by the core (§2.10).
- **IDs:** taken IDs and merging adds (§2.11).
- **Hidden kinds** are operations-only and reachable only by their own
  plugin (§2.15).
- **CalDAV:** an allowlist of served properties, no scheduling, DAV app
  passwords (§2.18).
- **`web()`** draws in a sealed frame; protected text uses a host font
  (§2.19).
- **`duplicate`** clears bindings; an author may always remove their own
  work (§2.20, §14).
- **Values:** `url` and `colour` values are checked, built-in fields come
  from host state, receipt time is used, and values are drawn as data
  (§3.1).
- **`open item`** replaces the second meaning of `this` (§4.1, seventh
  set).
- **Cost bounds** within the limits; per-viewer values are transitive and
  kept out of shared places (§4.4, §4.7).
- **Destinations:** every destination is checked after DNS on every hop;
  secrets only in their header; `a server you name` is bound when
  confirmed; no credential inputs (§7.1).
- **Sign-ins** are used only by their person (§7.3; E.1 strict default).
- **`extends:` operations** are limited to three line forms and `menu`;
  matches re-evaluated; agents see values as data (§7.2).
- **Outside addresses** carry a server token, have proofs stripped, cookies
  and headers locked down, and `secret` merged into the token (§7.5).
- **Notification templates** need fixed words; placeholders take stored
  values only (§7.7).
- **One "who acts" rule:**
  - timers act as the arming person;
  - rights are re-read when work runs;
  - a run's context travels along;
  - no-person handlers are limited (§8.2).
- **`s.write`** writes another plugin's kind only through `calls:` of its
  built-ins; `s.call` has a designation rule; `s.notify` returns
  `accepted`; `s.share` needs a live gesture; `s.outside.mint` is new (§8.1,
  §8.3).
- **Outside writes:** kind by ID; records cannot be removed by others (§9).
- **Agent rules files** are server-only; rules match before or after a
  change; moves count twice; R1's floor (§10).
- **Themes:** warnings are not themeable; SVG and font limits (§11).
- **Profiles:** pick passkey, `first run` on apply, updates offered (§12).
- **The remote-content proxy** has its own rules (§13).
- **OPEN:** E.1 and E.2 added; O4 gains S42's leaks (§17).

### Revision 7 (implementer review of revision 6, fifth and sixth sets)


- **Grammar:** the EBNF line grammar with L1–L10, name rules N1–N6,
  reserved words, and expression rules X1–X9 (review 1–9; fifth set).
- **Files and identity:**
  - install paths per plugin, and the `space` plugin for kinds made in the
    app;
  - plugin identity by source;
  - frames make a plugin Owner-installed;
  - the description-only rule restated.
- **Kinds:**
  - `rights:` replaces `change:`; new `state:`, `hidden: keep` and the
    `mark` flag;
  - fields: several `was` names, added-field retypes, `keep` counted from
    `edited`;
  - identity and merging: `unique` IDs, `together` conflicts, the reach of
    `default where`, folder pattern limits;
  - records: record blocks defined, and `lines:` notation;
  - pages: `page:` gains `web()` and records over several items;
    `protected` drawn by the host;
  - the CalDAV mapping's directions;
  - `open where`, and the "not accepted" state for refused offline work.
- **Types:** the "written as" column, the YAML subset, a link to a record,
  and units.
- **Formulas:**
  - quoted choice values, precedence, `x = empty`, `first()` as
    "otherwise";
  - duration units, `previous()` over lists;
  - limits: `now()`, candidate counts, index budgets;
  - per-viewer values and the period range.
- **Collections:**
  - `totals` absorbs `total`; formula columns; member field syntax;
  - boards with two conditions replace `matrix`; `compact` absorbs
    `strip`;
  - grouping by list fields, and how updates treat a changed collection.
- **Sidebars:** `each: … -> <collection> by <link>`, and tag entries.
- **`plugin.md`:**
  - `needs` derived; `search` gone;
  - `secrets … for <host>`, and the destination limits;
  - `notifications:`;
  - `extends:` naming and visibility rules; `listens` disclosures;
  - `outside` on its own web address.
- **Runtime:**
  - six events, each with its acting person;
  - write keys and loop depth;
  - the `s.write` table;
  - `s.share` opens the sheet, and `s.state` is the view of `state:`.
- **New sections:** outside writes (§9), remote content (§13), a first
  plugin (§15), and Appendices A to C.

## Appendix A. The grammar proved by hand

### A.1 The review's twenty-two inputs

| # | Input | Revision 7 | Rule |
| --- | --- | --- | --- |
| H1 | `due, soft: date` | Refused: a comma is not a name character | N2 |
| H2 | `ratio a:b: number` | Refused: L1 reads the key `ratio a:b`, and `:` is not a name character | L1, N2 |
| H3 | `quoted: = contains(subject, "say \"hi\""), mark` | Parses; the text is `say "hi"` | L3 |
| H4 | `done: set status = "done", when count(depends on, open) = 0, swipe` | Three parts: `set …`, `when count(depends on, open) = 0`, `swipe`. The comma inside `count( )` is text | L2 |
| H5 | `note late: set note = "late, when overdue", when overdue, menu` | Three parts; the quoted comma is text | L2, L3 |
| H6 | `kind: files` | Refused: `files` is a source word | §1.5 |
| H7 | `kind: records` | Refused: `records` is a formula word | §1.5 |
| H8 | `extends:` / `[calternal-mail.messages]:` / `fields:` / `saved: yes/no` | Four levels, in two-space steps | L6 |
| H9 | `ical: VEVENT` with children | A key with both a value and children | L6 |
| H10 | A header look-alike typed over WebDAV | A new record not authored by the writer is refused; the page draws authors from the ledger | §2.8, §9 |
| H11 | `Note: felt heavy` after a header | Text, because `Note` is not a declared record field | §2.8 |
| H12 | `größe` against a hand-written `Größe` | Different keys (case-sensitive, NFC); `Größe` is a person's own field | N2, G3 |
| H13 | A person's key `calternal-reading-list: yes`, beside an added field | No collision: added fields are flat dotted keys | §3.2 |
| H14 | `colour: colour, default #c96f4a` | The default is `#c96f4a`: no space follows `#` | L5 |
| H15 | `status: choice(did not happen)` and `set status = "did not happen"` | Parses: inside `choice( )` the value is text (a group), and in the formula it is quoted | L2, X3 |
| H16 | `board: priority in ("urgent", "high"), urgent` | The row condition is `priority in (…)`; the column condition is `urgent`, the computed field | X3, §5.2 |
| H17 | `calternal-chat:composer: frame frames/composer.html` | The key is `calternal-chat:composer`: the first `: ` follows `composer` | L1 |
| H18 | `sum(from([calternal-calendar.events].task, logged).length)` | Parses; the hyphenated name is bracketed | N5 |
| H19 | `keep: 6m` | Refused, naming the line: `m` alone is not a unit | X5 |
| H20 | `- allow change in today` | Refused: a folder ends in `/`, and a collection is written `<plugin>:collections/<id>` | §10 |
| H21 | No `read:` key, and `where: not read` | `read` is empty, which counts as false, so `not read` is true: the item is unread | X8 |
| H22 | `default 2m`, and `set remind = now() + 1mo` | `2m` is refused (write `2min`). The `set` line is allowed: `mo` is a unit, and the `now()` limits apply only to stored computed fields | X5, §4.5 |

### A.2 The four parse checks, line by line

Each line of §16 and Appendix B is listed with the rule that parses it. Rows
in **bold** failed in revision 6.

**Reading List** (§16). Every header line is `key: value` (L1) or a child
line (L6). The lines that matter:

| Line | Rule |
| --- | --- |
| `mark read: set read = yes, when not read, swipe, primary` | L2 splits it into four parts; one-liner (§2.20); placements combine (choice 4) |
| **`board: read`** | §5.2: boards accept yes/no fields (choice 7) |
| **`s.me` in `on.time`** | §8.2: `on.time` acts as the arming person (eighth set) |
| **`sends out: nothing`** | §1.1: `nothing` and no line mean the same. The line is dropped here, and the plugin has code anyway |
| `search: title, site, body` | §2.16: `site` is a stored computed field |
| `notifications:` / `remind: "Time to read {title}"` | §7.7 |

**Calendar `events`** (B.1):

| Line | Rule |
| --- | --- |
| **`calendar: link(calendars)`** (was `own calendar`, a link mixed with a tag) | §2.10: the core stores the calendar when the line or item is written (seventh set); no formula |
| **`ends: = first(end, start + if(all day, 1d, 15min))`** | X8, X5 |
| **`did not happen: set status = "did not happen", when not logged, menu`** | X3: quoted choice value |
| **`ical: VEVENT` with children** | L6 |
| **`change: only title, body, calendar, when source = empty or source.owned`** | X2: a dot into a `data` field reads a key; X8 |
| **the `today` collection's `date(start) <= today()`** | §4.3 `date()` (the suite wrote `day()`) |
| **`people: links(calternal-contacts.contacts)`** | §1.3: the full form for another plugin's kind |
| `checks:` / `end = empty or end >= start, "An event ends after it starts"` | §2.13, X8, L2 |
| `alert: durations` | §3.1 |
| `logged: = ends <= now()` | §4.5: `now()` on one side of a comparison |
| `lines: time in days` | §2.10: an own kind written bare |

**Tasks `tasks`** (B.2):

| Line | Rule |
| --- | --- |
| **`board: priority in ("urgent", "high"), urgent`** (Today's matrix) | X3, §5.2 |
| **`add: title, scheduled, due, priority, tags, phrases`** | §2.20. The suite's `line in today` is removed: where a new task's line goes is the core's C1 behaviour, not a flag |
| **`done: set status = "done", set completed = now(), when is open, swipe, primary`** | X3; choice 4 |
| **`urgent: = is open and days(today(), first date) <= settings.urgent within`** | `days(a, b)` with two arguments, compared with a duration setting; per viewer (choice 5) |
| **`time spent: = sum(from([calternal-calendar.events].task, logged).length)`** | N5. The suite's Calendar `events` now has `task` and `length` (fixed there) |
| **`from: lines(checkbox)` with `where: task = empty`** | §5.3: `where` is its own key; the rows have `task` |
| **`each: tags of tasks -> all-tasks`** | §6 |
| `lines: checkbox in [calternal-notes.notes], [calternal-calendar.days], box status: …` | §2.10, L2: the commas inside backticks and quotes stay in their parts |
| `is open: = status in ("todo", "doing")` | X9; `open` alone is an operation word, so the field is `is open` |

**Workouts `sessions`** (B.3):

| Line | Rule |
| --- | --- |
| **`file: {date:YYYY-MM-DD} {plan day}`** | §2.3: no dot in braces; `plan day` is now `required` |
| **`chart line: sum(volume) by week(item.date)`** | X2: `item.` in a records collection |
| **`change record: weight, reps, reserve, within 12h`** | §2.20: the window is stated; `reps in reserve` renamed (N3, choice 2) |
| `records:` block | §2.8 |
| `today: open where date = today(), primary` | §2.20, open |

## Appendix B. The four kinds rewritten

### B.1 Calendar `events`

```markdown
---
kind: events
name: Event
format: 1
folder: Calendar
file: {title}
editable: outside
lines: time in days
fields:
  title: text, required, heading
  start: when, required
  end: when
  all day: yes/no, default no
  time zone: text
  location: text
  people: links(calternal-contacts.contacts)
  calendar: link(calendars)                 # stored by the core from the first area tag (seventh set)
  task: link(calternal-tasks.tasks)
  tags: tags
  status: choice(did not happen)
  repeat: repeat
  exceptions: dates
  series: link(events)
  occurrence: when
  alert: durations
  source: data
  remote description: long text
  ends: = first(end, start + if(all day, 1d, 15min))
  length: = ends - start
  logged: = ends <= now()
together:
  start, end, all day, time zone
checks:
  end = empty or end >= start, "An event ends after it starts"
row: start, title
page: start, end, location, people, calendar, alert, body
search: title, location, body
ical: VEVENT
  title: SUMMARY
  start: DTSTART
  end: DTEND
  location: LOCATION
  repeat: RRULE
  exceptions: EXDATE
  alert: VALARM
  body: DESCRIPTION, unless source != empty
operations:
  add: title, start, end, calendar, phrases
  change: only title, body, calendar, when source = empty or source.owned
  remove
  did not happen: set status = "did not happen", when not logged, menu
  skip: code
  move occurrence: code
  move all future: code
---
An event: one per file, or a time line in a daily note until it gets its own
note.
```

### B.2 Tasks `tasks`

```markdown
---
kind: tasks
name: Task
format: 1
folder: Tasks
file: {title}
editable: outside
lines: checkbox in [calternal-notes.notes], [calternal-calendar.days], box status: `[ ]` "todo", `[/]` "doing", `[x]` "done", `[-]` "cancelled"
fields:
  title: text, required, heading
  status: choice(todo|doing|done|cancelled|someday), default todo
  scheduled: when
  due: when
  priority: choice(urgent|high|medium|low)
  assignee: link(calternal-contacts.contacts)
  tags: tags
  pinned: yes/no, default no
  depends on: links(tasks)
  waiting on: text
  repeat: repeat
  repeat from: choice(schedule|completion), default schedule
  completed: time
  log: records(done: time, due: date, skipped: date)
  is open: = status in ("todo", "doing")
  waiting: = count(depends on, status in ("todo", "doing")) > 0 or waiting on != empty
  first date: = first(due, scheduled)
  overdue: = is open and first date < today()
  urgent: = is open and days(today(), first date) <= settings.urgent within
  next: = next(repeat, after: if(repeat from = "completion", last(log.done), scheduled))
  streak: = streak(log)
  time spent: = sum(from([calternal-calendar.events].task, logged).length)
together:
  repeat, repeat from
row: title, first date, priority
search: title, tags, body
operations:
  add: title, scheduled, due, priority, tags, phrases
  change
  remove: code
  done: set status = "done", set completed = now(), when is open, swipe, primary
  won't do: set status = "cancelled", when is open, menu
  schedule: set scheduled = ask, menu
  skip: code, when repeat != empty
  track: code
  duplicate
---
One task per note. The box on every line that links here follows `status`.
```

`time spent` reads another plugin's kind, so `plugin.md` says `reads:
calternal-calendar.events`.

### B.3 Workouts `sessions`

```markdown
---
kind: sessions
name: Session
format: 1
folder: Workouts/Sessions
file: {date:YYYY-MM-DD} {plan day}
editable: outside
fields:
  date: date, default today
  plan day: link(plan-days), required
  done: yes/no, default no
  pump: choice(Low|Moderate|Great)
  soreness: choice(None|Healed early|Healed on time|Still sore)
  volume: = sum(records.volume)
records:
  exercise: link(exercises), required
  weight: number kg|lb
  reps: number
  reserve: number
  planned: yes/no, default no
  volume: = weight * reps
  e1rm: = weight * (1 + reps / 30)
row: date, plan day, volume
page: records(exercise, weight, reps, reserve), body
operations:
  add: plan day, date
  today: open where date = today(), primary
  append record: exercise, weight, reps, reserve, primary
  change record: weight, reps, reserve, within 12h
  remove record
  rest: timer settings.rest, after append record
  finish: set done = yes, when not done, primary
---
One session per file; each set is a record in the body.
```

## Appendix C. The 63 review findings

| # | Finding | Fixed in | Note |
| --- | --- | --- | --- |
| 1 | Splitting a line | §1.2 L1, L2 | |
| 2 | Nesting, value and children, continuation | §1.2 L6, L7, L8 | |
| 3 | Quotes, escapes, comments, lists | §1.2 L3, L4, L5, L9 | L5 is simpler than the review's rule (choice 1) |
| 4 | No name grammar | §1.4 N2–N5 | N3 bans only operator words inside names (choice 2) |
| 5 | No reserved words | §1.5 | |
| 6 | Dotted names, scope | §1.4 N6; §4.1 X2, X4 | |
| 7 | Choice values | §4.1 X3 | |
| 8 | Precedence, empty, "otherwise" | §4.2 X7, X8 | |
| 9 | Units, durations, money | §4.1 X5, X6; §4.3 `format` | |
| 10 | Item YAML | §3.1 "written as"; §3.2 | |
| 11 | Record blocks | §2.8 | |
| 12 | Outside edits as `change` | §9 | |
| 13 | Which kind a new file belongs to | §9 | |
| 14 | ID and install-path collisions | §1.1; §1.4 N1 | |
| 15 | Operation lines | §2.20 | One argument form, not both (choice 3); placements combine (choice 4) |
| 16 | `lines:` notation | §2.10 | |
| 17 | Collection details | §5.1, §5.3; §4.2 X9 | |
| 18 | `each:` default collection | §6 | |
| 19 | Agent rule lines | §10 | |
| 20 | The acting person | §8.2 | Revision 8: one "who acts" rule; a timer acts as the arming person, with rights re-read when it fires (eighth set, S10) |
| 21 | Write keys | §8.2 | |
| 22 | What `s.write` may touch | §8.3 | No `moves files` line (choice 6) |
| 23 | `s.share` | §8.1 | |
| 24 | Per-viewer values | §2.9, §4.5, §4.7 | Computed per viewer instead of refused (choice 5) |
| 25 | `now()` | §4.5 | |
| 26 | The cap and the index | §4.4 | |
| 27 | `per` fields | §4.6; `previous()` over lists in §4.3 | |
| 28 | `was` | §2.7 | |
| 29 | Added field over existing values | §2.7 | |
| 30 | `keep` counted from `created` | §2.15 | |
| 31 | Removing a plugin with hidden data | §2.15 | |
| 32 | `unique` offline | §2.11 | |
| 33 | Patterned moves | §2.3, §9 | |
| 34 | `together` and conflicts | §2.12 | |
| 35 | Where `default where` applies | §2.14 | |
| 36 | `on.after` loops | §8.2 | |
| 37 | Refused offline work | §2.20 | Sixth set |
| 38 | Records against a whole-file PUT | §9 | |
| 39 | Added and owner field names | §7.2 | |
| 40 | `any kind with` | §7.2 | |
| 41 | Outside on the app's address | §7.5 | Sixth set: its own web address |
| 42 | Description-only with frames | §1.1 | Sixth set |
| 43 | Folder, template and body reach | §2.2, §2.3, §2.17 | |
| 44 | Secrets tied to destinations | §7.1 | |
| 45 | Private ranges | §7.1 | |
| 46 | Remote loads | §13 | Sixth set |
| 47 | Record authorship | §2.8, §9 | |
| 48 | Imitating warnings | §2.7 `mark`, §2.19, §7.2 | |
| 49 | `data` over WebDAV | §9 | |
| 50 | Plugin identity | §1.1 | |
| 51 | `on.after` contents | §7.6 | |
| 52 | Unbounded compute | §4.4 | |
| 53 | Reading List does not install | §16, §1.1, §2.16, §8.2 | Yes/no boards allowed (choice 7) |
| 54 | Calendar `events` | Appendix B.1; suite test fixed | |
| 55 | Tasks `tasks` | Appendix B.2; suite test fixed | |
| 56 | Workouts `sessions` | Appendix B.3; suite test fixed | `reps in reserve` renamed `reserve` |
| 57 | Overall parse summary | Appendix A | |
| 58 | 227 names for authors | §15 | |
| 59 | 290 names with code | §15 | Not reduced; §15 states the subset |
| 60 | `share` twice | §2.20, §7.4 | Sixth set: `share` gone |
| 61 | `first run` twice | §7.4, §12 | Sixth set |
| 62 | Theme palette and assets | §11 | Named roles and `assets/` (choices 9, 10) |
| 63 | CalDAV directions | §2.18 | Unmapped properties in host state (choice 8) |

**Totals:** 63 findings, 63 fixed, 0 not taken. Seven are fixed differently
from the review's smallest fix: 3, 4, 15, 20, 22, 24 and 53 (see §20 and the
notes).

## Appendix D. The security review of revision 7

### D.1 The eight partly closed fixes, now closed at the harm

| # | The harm | How revision 8 stops it, not only the found case |
| --- | --- | --- |
| 41 | A plugin's outside traffic acting as the app, or trading on its name | Every proof and cookie is stripped before delivery, so nothing replayable reaches the plugin. The app's cookies are `__Host-` and `SameSite=Strict`, and its API refuses requests from the outside origin, so same-site requests do nothing. Answers pass only a header allowlist (`Location` only inside the plugin's path), so redirects and downloads cannot borrow the operator's name. A server token makes the address really secret (§7.5) |
| 42 | Plugin HTML running without the Owner's yes | Any HTML or script anywhere in a plugin counts as `frames/`, `frame <path>` must be inside `frames/`, and `web()` draws only in a sealed frame with no scripts or network (§1.1, §7.4, §2.19). There is no other way for HTML to reach a renderer |
| 43 | Something other than the server steering agents or capturing another kind's files | Instruction-file names are written only by the server, by any route (§2.2). Folder patterns may not nest either way; filled values are single segments; marked folders obey the same rules; kind and exposure follow the ID, not the path (§2.3, §9). One path rule applies to every path (§1.1a) |
| 44 | A secret leaving through its own host or a redirect | Secrets go only into their declared header, over valid TLS, never into URLs or bodies, never across redirects, and are scrubbed from answers. `a server you name` is bound when it is confirmed (§7.1) |
| 45 | Reaching the inside | Every destination, fixed hosts included, is checked after DNS on every hop, for every route that opens a connection (fetch, connect, proxy, CalDAV sync, sandbox egress) (§7.1) |
| 46 | Data leaving through loads nobody declared | The proxy loads only values a person or an outside sender wrote. A value written by code loads only when its host is in `sends out`, every hop is checked, the proxy serves images only, and plugin code cannot reach it (§13) |
| 50 | A plugin passing for another | Sheet sentences are the server's own; plugin text is quoted. Names are single-script and checked for confusables against the operator's list, and shown with source and ID. More IDs are reserved, and rules, picks and state are keyed by ID plus source (§1.1, §1.1a) |
| 51 | Credentials reaching a listener | No input or field can hold a credential: sign-ins and secrets are entered only on the server's sheet, and the names `password`, `token` and `secret` are refused (§7.1, §1.5). So there is nothing to filter out of `on.after` |

### D.2 The seven attackers: what now stops each attempt that got through

| Attacker | Attempts, and the rule that stops each |
| --- | --- |
| **B.1 Member adding description-only plugins** | 1a capture files: no nesting either way, §2.3 · 1b filled folder: one-segment values, §2.3 · 1c HTML page: HTML counts as frames, `web()` sealed, §1.1, §2.19 · 1d script link: `url` schemes, §3.1 · 1e claim lines: one claim per line form, §2.10 · 1f instruction as a name: free-string rule, and agents see data in a frame, §1.1a, §7.2 · 1g mark others' folders: marked folders obey the pattern rules, §2.3 |
| **B.2 Malicious update** | 2a widen: updates wait unless narrower, §1.1 · 2b harvest tokens: no credential inputs, proofs stripped, §7.1, §7.5 · 2c DNS rebinding: checked after DNS on every hop, fixed hosts too, §7.1 · 2d leak a secret: header-only secrets, §7.1 · 2e send with `nothing`: proxy loads code-written values only for declared hosts, §13 · 2f who muted: `s.notify` returns `accepted`, §8.1 · 2g free text: template rules, §7.7 |
| **B.3 Outsider** | 3a call a webhook: the token, §7.5 · 3b reflect data: no-person handlers touch only own kinds, §8.2 · 3c lock-screen text: placeholders only from stored values, quoted, cut, §7.7 · 3d invitation: no CalDAV scheduling, served-property allowlist, §2.18 · 3e attachment `AGENTS.md`: renamed, §2.2 · 3f XMP entities: no DTDs, §1.1a |
| **B.4 Agent run with hostile content** | 4a rewrite its rules: rules files are server-only, §10 · 4b act through a reaction: the run's context travels with its events, §8.2 · 4c leave a scoped rule: rules match before or after, moves count twice, §10 · 4d plant instructions: instruction files are server-only, §2.2 · 4e escape the folder: one path rule, no symlinks, §1.1a · 4f follow instructions in data: values reach agents as labelled data, §7.2 |
| **B.5 Owner against a member** | 5a repoint a sign-in: bound on confirmation, §7.1 · 5b use Lee's sign-in: only its person (E.1 default), §7.3 · 5c lift Lee's ask: R1's floor; space rules cannot allow reaching people, §10 · 5d stop own deletes: authors always remove their own, §2.20 · 5e route open items: sending picks need the passkey, §12 |
| **B.6 Plugin against plugin** | 6a hidden queue: hidden kinds reachable only by their plugin, §2.15 · 6b read without `reads:`: `s.call` designation rule, §8.3 · 6c write through `reads:`: writes only through `calls:` of built-ins, §8.3 · 6d built-ins on operations-only kinds: `extends:` allows only own-field `set`, `open where` and `code`, §7.2 · 6e take the swipe: `extends:` placements are `menu` only, §7.2 |
| **B.7 Modified client** | 7a record time: server receipt time, §3.1 · 7b taken ID: new ID, checked as `change`, §2.11 · 7c tick a box: checked as `change`, box put back, §2.10 · 7d forge `edited`: built-ins come from host state, §3.1 |

### D.3 S1 to S44

| # | Finding | Fixed in |
| --- | --- | --- |
| S1 | The outside address holds no secret | §7.5 (token, rate and size limits, `s.outside.mint`) |
| S2 | The `person` proof reaches the plugin | §7.5 (stripped; address-only token) |
| S3 | No-person events have no read or call rule | §8.2 |
| S4 | Which updates wait | §1.1 |
| S5 | Instruction files | §2.2 (strict default; E.2 OPEN in §17) |
| S6 | Nested patterns, filled values, marked folders | §2.3 |
| S7 | Moves change kind and exposure | §9 |
| S8 | Agents write their own rules | §10 |
| S9 | Reactions carry an agent past its rules | §8.2 |
| S10 | A timer acts as the item's author | §8.2 (the arming person; rights re-read) |
| S11 | `reads:` grants writes | §8.3 |
| S12 | `s.call` designation | §8.3 |
| S13 | Hidden kinds reachable | §2.15, §8.3 |
| S14 | HTML without frames | §1.1, §2.19, §7.4 |
| S15 | `url` schemes and `colour` text | §3.1 |
| S16 | The proxy | §13 |
| S17 | Fixed hosts and private ranges | §7.1 |
| S18 | Where a secret goes; binding a named server | §7.1 |
| S19 | Who may use a sign-in | §7.3 (strict default; E.1 OPEN in §17) |
| S20 | Templates carry free text | §7.7 |
| S21 | `s.notify` reveals mutes | §8.1 |
| S22 | Refusing removal of one's own items | §2.20, §14 |
| S23 | Sheet text and identity | §1.1, §1.1a |
| S24 | "The public web" can send | §1.1a |
| S25 | Same-site outside origin; plugin headers | §7.5 |
| S26 | Client-chosen times, bases, built-ins | §2.10, §3.1 |
| S27 | Adds on existing items | §2.11 |
| S28 | Two claims on one line form | §2.10 |
| S29 | `extends:` operations and placements | §7.2 |
| S30 | `duplicate` copies bindings | §2.20 |
| S31 | CalDAV unmapped properties, scheduling, credentials | §2.18 |
| S32 | Rule conditions and folders stepped around | §10 |
| S33 | "Reaching other people" | §1.1a, §10 |
| S34 | Themes recolour warnings | §11, §2.19 |
| S35 | Non-tightening profile keys | §12 |
| S36 | `any kind with` drifts | §7.2 |
| S37 | Per-viewer values in shared places | §4.7 |
| S38 | Cost within the limits | §4.4 |
| S39 | Names and values as instructions | §7.2 |
| S40 | Credential inputs | §7.1, §1.5 |
| S41 | `s.share` pre-fills and needs no gesture | §8.1 |
| S42 | Leaks once read rights exist | §2.6, §4.7, §5.3, §12 |
| S43 | Paths, links and parsers | §1.1a |
| S44 | Removing another person's record over WebDAV | §9 |

**Totals:** 44 findings, 44 fixed, 0 not taken. Two (S5, S19) are fixed by
the strict default, with the owner's question kept OPEN.

### D.4 The review's simplification opportunities

All eight are taken, since each only removes:

- one "who acts" rule (§8.2);
- one derived "reaches other people or the outside" flag (§1.1a);
- one rule for free strings and one for paths (§1.1a);
- `secret` and `signed` as one proof, the token (§7.5);
- `hidden` implies operations-only (§2.15);
- writes to another plugin's kind through `calls:` of its built-ins
  (§8.3);
- no "skipped (muted)" result (§8.1);
- one `data` rule on every path (§3.1).


## 22. Code and specification disagree

This list compares the coordinator's decisions with `dev` at
`32dc5517` and marks branch-only evidence separately.

| Area | Decision | Observed code or evidence |
| --- | --- | --- |
| Typed slots and harness start | Slot input schemas come from typed operations. The harness starts from the new message link. | The engine validates `inputs <operation>` when present. Chat's two slots omit it. The bundled harness has no typed `start` block and its handler consumes only `chat`; Chat already sends `prompt` from the receipt. The host call report's send envelope also still shows `chat` where merged code requires `item`. |
| Send and start replay | One request key covers append and start, with no duplicate effects on retry. | Subkeys and append replay exist. A mock proved one start and the merged host proved append replay after a refused start. The real run start remains unconnected to the merged host, so replay with a real run is unproven. |
| Program and asset admission | Owner approval for bound plugin-local programs and frame-capable content; recognised assets may stay data. | The parser still accepts a flat program ID list and the merged Claude Code fixture uses it. The host refuses programs, destinations and assets/frames. Lane D binds an executable on its branch, and its runner refuses image bindings. The shared content classifier and in-app install approval flow remain pending. |
| Compiled declarations | The host ships checked declarations to devices. | The JSON projection is live in the browser. The proposed shared Rust browser engine build without install checks is still unmeasured. The projection splits validated sidebar/collection/page strings and omits some declared presentation details, including fill labels. |
| Worker isolation and reactions | Isolated code with CPU and memory bounds and process-family cleanup. Committed reactions are replayable. | The merged launcher is opt-in Bubblewrap with namespaces, a six-second deadline and a Node heap limit. A whole-worker CPU and memory quota is unproven. The install probe and actor recycling are implemented. Change delivery is post-commit in memory and can be lost on host stop. |
| First owner and access loss | Private operator issuance, and an explicit content-revocation frame before non-owner live access. | Passkey enrolment uses inherited auth and a disposable setup code passed by pipe in the browser test. Standalone operator issuance through the private admin flow has no end-to-end proof. The personal-space host admits only the owner and closes sockets on session revocation; it has no per-window permission-loss frame or client purge contract. |
| Run egress | Every run uses the declared-host proxy. | The fake-provider tests on lane D (`293af65a`) exercise proxy-only access and process-family Stop. The merged host still refuses the harness manifest, has no wired run service and has no real provider proof. |
| Broader format and rights | The full proposal covers shared spaces and collections, broader formula and operation forms, and DAV and other surfaces. | This host admits one owner per personal space. Shared grants and shared-writer tail recovery are absent. Conditional coded dispatch and path-link rewrite support are partial. The engine findings list deferred grammar and repeat/total cases. Browser row conditions do not establish those execution paths. |

Sources are the [coordinator's sets 11–16](../morning-2026-11-21.md#decided-by-the-coordinator-eleventh-set-the-four-gaps-chat-found-the-owner-can-overturn-any),
[format engine findings](../reviews/format-engine-findings-2026-11-27.md),
[host core findings](../reviews/host-core-findings-2026-11-27.md),
[worker findings](../reviews/plugin-process-findings-2026-11-27.md),
and [browser findings](../reviews/browser-chat-findings-2026-11-27.md).
