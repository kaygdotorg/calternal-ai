# The kind and collection format · revision 6

**Revision 6, 2026-11-27.** This is the single statement of the on-disk
format and the plugin call surface. A builder implements from this document
alone, without reading the morning report.

It applies:

- rounds 21 to 33 of `docs/morning-2026-11-21.md`;
- the three "Decided by the coordinator" sets of 2026-11-27, with
  [extension-surface-2026-11-27.md](extension-surface-2026-11-27.md) for
  the third (M1 to M5);
- the formulas research
  ([formulas-research-2026-11-24.md](formulas-research-2026-11-24.md));
- both stress tests
  ([Money, Mail and Chat](format-stress-test-money-mail-chat-2026-11-24.md)
  and [the rest of the suite](format-stress-test-suite-2026-11-27.md)).

Where those sources contradict older text, the newer wins. Every
contradiction this revision resolved is listed in §15. The design it serves
is [primitives-only-host-2026-11-21.md](primitives-only-host-2026-11-21.md).
Items the owner has not decided are marked **OPEN** in §13, with what each
answer would change here. Details that came from the stress tests but are not
yet recorded as decided are marked **PENDING** in §14.

The owner's requirements: Markdown with frontmatter, never JSON; a person can
write a sum by hand; an agent can write anything larger on request; a no-code
plugin and a kind made in the app are the same files; plugins may modify
nearly everything in the spaces where they are installed, except the core
(round 33).

## Contents

0. Notation
1. Files, names and parsing
2. Kind files
3. Field types and built-in fields
4. Formulas and conditions
5. Collection files
6. Sidebar files
7. `plugin.md`
8. The thirteen calls and seven events
9. Agent rules files
10. Theme files
11. The space profile, `.calternal/space.md`
12. What is never open
13. OPEN items
14. PENDING details
15. Contradictions resolved
16. Choices this revision makes that no decision covers
17. Reading List, written out in full
18. Counts, and two ways to say one thing
19. Change list from revision 5

## 0. Notation

In a grammar line, `<x>` is a placeholder, `[x]` is optional, `a|b` means
one of the two, and `…` means the item may repeat, separated by commas. Every
grammar line has an example right after it. Words in `code` are literal.

## 1. Files, names and parsing

### 1.1 The files of a plugin

```text
calternal-reading-list/
  plugin.md              who it is, what it reads, sends out, calls, adds and offers
  kinds/<kind>.md        one file per kind
  collections/<id>.md    one file per collection
  sidebars/<id>.md       the app's default sidebar(s)
  templates/<name>.md    starting bodies for new items (optional)
  themes/<name>.md       themes (§10, optional)
  profiles/<name>.md     space profiles the Owner may apply (§11, optional)
  frames/                web code for frames and panels (optional)
  operations.ts          only for operations that need code (optional)
```

- Installing copies the Markdown files into the space:
  `.calternal/kinds/`, `.calternal/collections/`, `.calternal/sidebars/`,
  `.calternal/templates/`, `.calternal/themes/`, and `.calternal/plugins/<plugin>.md`.
  A profile is copied only when the Owner applies it (§11).
- "New kind" in the app writes the same files.
- A plugin without `operations.ts` and without `sends out`, `sign-ins`,
  `secrets` or `programs` is a **description-only plugin**. Any member may
  add one; anything else is installed by the Owner (round 22).
- **`.calternal/` is visible and readable** over WebDAV and in exports, but
  refused for outside writes. Its files change only through the app's
  editors, installs and updates (round 29).
- Items live in the kind's folder (§2.3), one per file or folder.
- The kind file's body is written into that folder as its `AGENTS.md`.
- Every definition file (kind, collection, sidebar, `plugin.md`, agent
  rules) carries `format: 1`. A server refuses at install a file whose
  format or functions it does not know, naming them ("this plugin needs
  format 2; update the server"). It never half-reads a newer file.

### 1.2 Naming another plugin's things

One form, used alike in `needs`, `reads`, `calls`, `extends`, `fills`,
`listens`, sidebars, `from`, link types and agent rules (coordinator, second set):

| Thing | Form | Example |
| --- | --- | --- |
| A plugin | `<plugin>` | `calternal-mail` |
| A kind | `<plugin>.<kind>` | `calternal-mail.messages` |
| An operation | `<plugin>.<operation>`, or `<plugin>.<kind>.<operation>` when two kinds share a name | `calternal-tasks.done` |
| A collection | `<plugin>:collections/<id>` | `calternal-tasks:collections/loose-ends` |
| A sidebar | `<plugin>:sidebars/<id>` | `calternal-notes:sidebars/notes` |

Inside a plugin's own files the plugin part is left out: `messages`,
`collections/inbox.md` as a relative link.

### 1.3 How definition files are parsed

Item files use ordinary YAML frontmatter, so other tools read them. **Kind,
collection, sidebar, plugin and rules files use the line grammar of this
document**, not YAML. The older examples' bare lines (`add`, `change`) under
a key are not valid YAML, and an exact grammar is easier to check (§16,
choice 1). The rules:

- A file is `---`, header lines, `---`, then a Markdown body.
- A header line is `<key>: <value>`, or a key alone followed by indented
  lines (two spaces) that belong to it.
- An indented line is `<name>: <line>` or a bare `<line>`, whichever the
  key's grammar says.
- `#` begins a comment to the end of the line, except inside double quotes.
- Text values with commas, colons or `#` are written in double quotes.
- Unknown keys are refused at install, with the key named. People's files
  are lenient (§2.18); definition files are strict.

### 1.4 An item

```markdown
---
title: How Discord stores trillions of messages
url: https://discord.com/blog/how-discord-stores-trillions-of-messages
read: no
added: 2026-11-24
---
The person's own notes.
```

- The server adds `id:` and `kind:` as read-only copies; host state holds
  the truth.
- Computed fields are never written into the item.
- Unknown keys, comments and untouched values survive every edit (inherited
  0068).
- Reading is lenient and nothing is rewritten on read. A file whose
  frontmatter does not parse is shown, but cannot be changed until fixed,
  and it is listed in the kind's **Problems** collection with the reason
  (host-gaps 2.1).

## 2. Kind files

### 2.1 Every key

| Key | Value | Required | Section |
| --- | --- | --- | --- |
| `kind` | The kind's ID, unique in the space | yes | 2.2 |
| `name` | What people see | yes | 2.2 |
| `format` | `1` | yes | 1.1 |
| `folder` | Where items live, a pattern | yes | 2.3 |
| `file` | The item's file name, a pattern | no; default `{title}`, else the ID | 2.3 |
| `item` | `file` (default), `folder with <main file>`, or `file(<media types>)[, paired]` | no | 2.4 |
| `editable` | `outside` or `operations`; fixed for the kind's life | yes (round 27: no default) | 2.5 |
| `change` | `manager` or `owner`: the least role that may add, change or remove items (G5) | no; default editor | 2.6 |
| `fields` | One line per field | yes | 2.7 |
| `records` | One line per field of each record | no | 2.8 |
| `lines` | Line items in other kinds' notes | no | 2.9 |
| `unique` | A field with one item per value | no | 2.10 |
| `together` | Groups of fields merged as one value | no | 2.11 |
| `checks` | Conditions every save must meet | no | 2.12 |
| `default where` | A condition every collection over this kind applies unless it says `ignoring default` | no | 2.13 |
| `keep` | How long items live | required when `shown: no` | 2.14 |
| `shown` | `no` for a hidden kind holding a plugin's durable data | no; default yes | 2.14 |
| `search` | Fields searched, in order of weight | no; default title, then body | 2.15 |
| `template` | A file in the plugin's `templates/`, or `none` | no | 2.16 |
| `ical`, `vcard` | The iCalendar or vCard mapping the server serves over CalDAV or CardDAV | no | 2.17 |
| `row` | Fields shown on one line (lists, search results, links) | no; default the first text field | 2.18 |
| `page` | What an item's page shows, in order | no; default every field, then the body | 2.18 |
| `protected` | Fields on the row and page that another plugin's additions can never push out of view (M2) | no; default the title | 2.18 |
| `operations` | Everything that can be done to an item | operations-only kinds: yes | 2.19 |
| body | Plain text for people and agents; written as the folder's `AGENTS.md` | yes | 2.2 |

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

### 2.3 `folder:` and `file:` patterns

```text
folder: <path>[/{<field>[:<format>]}]…[, plus marked folders]
file: <text and {<field>[:<format>]}>…
```

`<format>` is one of `YYYY`, `MM`, `DD`, `YYYY-MM`, `YYYY-MM-DD` or
`GGGG-[W]WW` (the ISO week, `2026-W48`). A field used in a pattern must be
required or have a default.

```markdown
folder: Photos/{taken:YYYY}/{taken:YYYY-MM-DD}, plus marked folders
file: {date:YYYY-MM-DD}
```

- Names follow inherited 0023: Unicode NFC, unsafe characters replaced, and
  a clash gets " 2".
- When a field in a pattern changes, the server moves the file and
  rewrites every link to it (G6), recorded in history.
- `plus marked folders` adds folders a person marks for this kind ("Show in
  Photos", inherited PR1). Their files are indexed where they are and never
  moved.

### 2.4 `item:` forms

```text
item: file
item: folder with <main file>
item: file(<media type>…)[, paired]
```

| Form | An item is | Example |
| --- | --- | --- |
| `file` (default) | One Markdown file | a note |
| `folder with <main file>` | A folder whose main file is Markdown; other files beside it belong to the item | `item: folder with message.md` (Mail: the raw `.eml` beside it) |
| `file(<types>)` | A binary original, never written to. Fields come from its embedded metadata (`from metadata`) or from Calternal's sidecar in `.calternal/`, keyed by ID (`in sidecar`). The sidecar format is XMP (inherited FS1–FS3, PH27, PH-B) | `item: file(image/*, video/*), paired` |

`paired` groups files that belong together (Live Photo, RAW+JPEG, edited
copies) into one item. They are removed and restored together (inherited
PH5, PR11). Attachments of Markdown items follow inherited 0028: a `file`
field links to a file stored in Photos or Documents by type, never a copy in
the item (G10).

### 2.5 `editable:`

| | `outside` | `operations` |
| --- | --- | --- |
| In the WebDAV mount | Yes, editable | Absent |
| A folder handed to an agent | Possible | Not possible |
| Built-in `add`, `change`, `remove` | Present unless the operations list removes them or gives them a condition | Only those listed |
| An outside change | An ordinary write by the signed-in person, checked by `checks:` | Refused; a disk edit is tampering, detected by keyed hashes |

```markdown
editable: operations
```

### 2.6 `change:` (the role line)

```text
change: manager|owner
```

```markdown
change: manager
```

The server enforces it on every add, change, remove and record operation,
from any surface, including outside edits (G5). It is a kind-level key and
must not be confused with the built-in `change` operation (§2.19, and §18).

### 2.7 `fields:`

```text
<name>: <type>[, required][, default <value>][, heading][, was <old name>][, from metadata][, in sidecar[ <property>]]
<name>: [per day|week|month|year ]= <formula>
```

```markdown
fields:
  title: text, required, heading
  read: yes/no, default no
  site: = domain(url)
  spent: per month = sum(from(transactions.category, month(date) = period).amount)
  rating: number, was stars
  taken: time, from metadata
  favourite: yes/no, default no, in sidecar xmp:Rating=5
```

| Flag | Meaning |
| --- | --- |
| `required` | A save without it is refused; a hand edit without it is a Problem |
| `default <value>` | Filled at add; `today` and `now` are allowed |
| `heading` | Kept in step with the body's first heading, last change winning, the heading winning a tie (inherited 0025) |
| `was <old name>` | A rename across a plugin update; the server rewrites the key. These lines are the kind's version history (G1) |
| `from metadata` | Read from the file's embedded metadata (EXIF, PDF); read-only (file items only) |
| `in sidecar [<property>]` | Stored in the item's `.calternal/` sidecar, under the named XMP property if given (file items only) |

- **Computed fields** start with `=` (§4). They are never stored in the
  item, only in the rebuildable index.
- **`per <period>` fields** hold one value per period for each item:
  `period` is the period being computed, and `previous(<field>)` is the
  same field one period earlier (F1, coordinator).
- **Plugin updates (G1):** an added field is automatic; a rename needs
  `was`; a removed field's values stay in the files, unshown; a retype is
  refused unless every value converts. There is no migration code.
- **Fields a person adds (G3)** survive updates, work in collections and
  formulas, and the plugin ignores them.

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

- A record is a block in the item's body. It starts with a header line the
  server writes, `**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9` (display
  name, UTC time, a 16-character anchor as in ADR 0056), and ends where the
  next header begins.
- The authority for who wrote a record is the history ledger, not the
  header. A line in a record body that looks like a header is escaped with
  `\`.
- A record's fields are `key: value` lines right after its header, then its
  text (§16, choice 4).
- Only the author changes a record, within the window the operation states;
  the author, the Owner and Managers may remove one; removing scrubs
  history and search (inherited D1, D4).
- Records are read in formulas as `records` (`sum(records.volume)`), across
  items as `<kind>.records` (§5.3, C3).

### 2.9 `lines:` (line items, C1)

```text
lines: checkbox|time in <kind>…[, box <field>: [ ] <value>, [/] <value>, [x] <value>, [-] <value>]
```

```markdown
lines: checkbox in calternal-notes.notes, calternal-calendar.days, box status: [ ] todo, [/] doing, [x] done, [-] cancelled
lines: time in calternal-calendar.days
```

The shared Rust core understands exactly two line forms, on the server and
on every device (inherited 0068 rule 1):

- **checkbox:** a Markdown list item with a box. With a child link to an
  item of this kind, the line *is* that item, and its box mirrors `<field>`
  through the mapping. Without such a link it is a plain checkbox, listed
  by `from: lines(checkbox)`.
- **time:** a line starting with `HH:MM - HH:MM`, `HH:MM` or `all day`. It
  fills the kind's `start`, `end`, `all day` and `title` fields, which the
  kind must have (checked at install). The day comes from the note's
  `date`. A start-only line shows as 15 minutes long without writing an end.

Ticking a box offline updates the item at once (S1, C1). A box changed over
WebDAV or in source mode is compared against its base, and the newest edit
wins (inherited 0066). No other line forms exist without a new decision.

### 2.10 `unique:`

```markdown
unique: date
```

One item per value. The listed form `open where <condition>` (§2.19) opens
it, adding it if missing.

### 2.11 `together:`

```text
together:
  <field>, <field>…
```

```markdown
together:
  start, end, all day, time zone
```

The generic merge (S2) treats each group as one value: when two devices
changed the group, both versions are kept.

### 2.12 `checks:`

```text
checks:
  <condition>
```

```markdown
checks:
  empty(end) or end >= start
```

- A write that fails a check is refused, with the line as the message.
- A hand edit that fails becomes a Problem.
- A merge whose result fails a check keeps both versions.
- Checks may read records: `records.<field>` (PENDING, §14 P7).

### 2.13 `default where:`

```markdown
default where: not hidden
```

Every collection over the kind applies it unless its `from:` says
`ignoring default`.

### 2.14 `keep:` and `shown:`

```text
keep: <duration>
shown: no
```

```markdown
shown: no
keep: 90d
```

- `keep` moves items to Trash once they are older than `<duration>`, from
  `created`.
- A `shown: no` kind has no app UI, is absent from WebDAV and search, and
  is how a plugin keeps durable data that is neither a person's file nor
  rebuildable: sync tokens, an outbound queue, sent snapshots. It must
  state `keep`. Everything stored has a bound (review checklist item 7).

### 2.15 `search:`

```text
search: <field>…
```

```markdown
search: title, aliases, tags, body
```

Fields in order of weight (G9), with `body` allowed. Snippets come from the
first matching field. A plugin's own search (semantic, faces) is a
`search:` operation in `plugin.md` (§7), merged with this one.

### 2.16 `template:`

```markdown
template: templates/meeting.md
```

The starting body of a new item, or `none`. A person may pick another
template at add (G10).

### 2.17 `ical:` and `vcard:`

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
  body: DESCRIPTION, unless not empty(source)
```

- The server serves CalDAV (VEVENT and VTODO collections) and CardDAV for
  every kind with a mapping (C2 part 1). A client's write is an ordinary
  write by the signed-in person, so checks and the role line apply.
- Unmapped properties and unknown vCard lines are kept exactly (inherited
  0063, 0068).
- The protocol details (discovery, sync tokens, ETags, app passwords,
  Apple's MOVE and DELETE) are the server's, from inherited 0064 and 0067.

### 2.18 `row:` and `page:`

```text
row: <field>…
page: <entry>…
```

`<entry>` is a field, or one of `body`, `properties` (the fields table,
shown only when the item has any), `image`, `preview`, `records(<field>…)`,
or `web(<field>)` (PENDING P3).

```markdown
row: title, site
page: properties, body
protected: from, outside sender
```

`protected:` names fields that additions from other plugins (§7.2) can never
push out of view, whatever a replacement of the kind's list (a `kind <kind>
list` place, §7.5) draws. The title is protected by default.

### 2.19 `operations:`

One list is the whole answer to what can be done to an item (round 29).

```text
add[: <field>…][, phrases][, share]
change[: only <field>…][, when <condition>]
remove[, when <condition>]
duplicate[, when <condition>]
<built-in>: code
<name>: set <field> = <value>|ask|<formula>[, set …][, when <condition>][, swipe|menu|primary]
<name>: open where <condition>[, swipe|menu|primary]
<name>: timer <duration>|settings.<name>, after <operation>
<name>: code[, when <condition>][, swipe|menu|primary]
append record[: <field>…][, when <condition>][, primary]
change record[: <field>…][, within <duration>][, when <condition>]
remove record[, when <condition>]
```

| Line | Example | Meaning |
| --- | --- | --- |
| `add` | `add: url, title, share` | Items can be added; the form asks these fields in order; `phrases` lets the composer fill it from text; `share` offers it in the system share sheet |
| `change` | `change: only title, archived` | Items can be changed; `only` limits the fields |
| `remove` | `remove: when not reconciled` | To Trash (30 days, inherited 0073) |
| `duplicate` | `duplicate` | A copy with a fresh ID and identity (inherited FB19) |
| built-in by code | `remove: code` | The built-in's name, done by `operations.ts` |
| one-liner | `mark read: set read = yes, when not read, swipe` | Sets fields; `ask` shows the field's picker (`snooze: set snooze = ask, menu`); the placement word puts it on a swipe, in the menu, or as the primary button |
| open | `today: open where date = today(), primary` | Opens the matching item; with `unique`, adds it if missing |
| timer | `rest: timer settings.rest, after append record` | A countdown on the device after another operation, with a local notification |
| coded | `reconcile: code, menu` | Done by `operations.ts` |
| record operations | `append record: exercise, weight, reps, primary` · `change record: weight, reps, within 48h` · `remove record` | C3 |

- **Offline (S1):** built-ins, one-liners, open, timer and record
  operations apply at once on the device and sync later. Coded operations
  queue in the outbox, shown as "not sent yet", and run on reconnect.
- **One declaration, every surface:** each line is an operation on the app,
  the CLI (`calternal reading add --url …`), the API and MCP, and is what
  agent rules name (§9).
- **Another plugin's added fields, operations and marks** sit in that
  plugin's `plugin.md` (§7.2, `extends`), never in this file.

## 3. Field types and built-in fields

| Type | Holds | Example |
| --- | --- | --- |
| `text` | A line of text | `title: text` |
| `long text` | Paragraphs | `notes: long text` |
| `number` | A number | `reps: number` |
| `number <unit>[\|<unit>]` | A number in a unit; with two units, shown in the person's choice (a setting) | `weight: number kg\|lb` |
| `money [<currency>]` | An exact decimal with a currency, never a float | `amount: money EUR` |
| `yes/no` | True or false | `done: yes/no` |
| `date` | A day | `due: date` |
| `dates` | A list of days | `exceptions: dates` |
| `time` | A date and time. One-off times keep their offset, repeating task times float, and repeating events keep their zone (inherited 0066) | `starts: time` |
| `when` | A day or a time, as the person gives it | `scheduled: when` |
| `duration` | A length of time (`7d`, `2h`, `30m`) | `rest: duration` |
| `durations` | A list of lengths (relative alerts) | `alert: durations` |
| `choice(<a>\|<b>…)` | One of a fixed set, in board column order | `status: choice(To do\|Doing\|Done)` |
| `tags` | A list of words; `/` nests | `tags: tags` |
| `url` | A web address | `url: url` |
| `link(<kind>)`, `links(<kind>)` | One or several items, stored as Markdown links; `<kind>` may be another plugin's (`link(contacts)` becomes plain text if that plugin is absent) or `any` | `project: link(projects)` |
| `person`, `people` | One or several people of the space | `owner: person` |
| `repeat` | An iCalendar repeat rule; at most one per kind; the server expands it | `repeat: repeat` |
| `place` | A location (latitude, longitude, name) | `where: place` |
| `file` | An attached file stored by inherited rule 0028 | `scan: file` |
| `colour` | A colour | `colour: colour` |
| `data` | A structured value only the plugin's code changes, shown read-only (a sync binding) | `source: data` |
| `records(<field>: <type>…)` | A list of small records in the frontmatter | `log: records(done: time, due: date, skipped: date)` |

**Built-in fields** every item has, read-only: `id`, `kind`, `created` and
`edited` (inherited 0025).

## 4. Formulas and conditions

One language for computed fields, `where`, `when`, `checks`, `default
where`, `totals`, chart values and agent-rule conditions, written in
spreadsheet style (round 24).

### 4.1 Values and operators

- **Values:** numbers (`12.5`), money (`12.50 EUR`), text in double
  quotes, `yes`, `no`, dates (`2026-11-24`), durations (`7d`, `2h`,
  `30m`), `empty`.
- **Operators:** `+ - * /`, `= != < <= > >=`, `and`, `or`, `not`, `in
  (<a>, <b>…)`. `date + duration` is a date; `date - date` is a
  duration.
- **Names:**
  - a field by its name (`read`);
  - a linked item's field with a dot, one hop only (`project.name`, F3);
  - records as `records`;
  - a plugin setting as `settings.<name>`;
  - in a `per` field, `period`;
  - in a collection condition, `this` (the open item: `links to this`).

### 4.2 Every function

| Function | Example |
| --- | --- |
| `if(c, a, b)` | `if(late, "Late", "On time")` |
| `sum(list[, condition])` | `sum(tasks.hours)` |
| `count(list[, condition])` | `count(depends on, open)` |
| `min(list[, condition])`, `min(a, b)` | `min(available, 0)` |
| `max(list[, condition])`, `max(a, b)` | `max(previous(available), 0)` |
| `average(list[, condition])` | `average(records.reps)` |
| `first(list[, condition])` | `first(under(tags, "area"))` |
| `last(list)` | `last(log.done)` |
| `from(<kind>.<link field>[, condition])` | `sum(from(transactions.category).amount)` |
| `from(<kind>.records.<link field>[, condition])` | `max(from(sessions.records.exercise).e1rm)` |
| `all(<kind>[, condition])` (in `totals:` only) | `sum(all(categories, group = "Income").spent)` |
| `previous(<field>)` (in `per` fields only) | `previous(available)` |
| `today()`, `now()` | `due < today()` |
| `days(a, b)` | `days(added, today())` |
| `date(t)` | `date(start) = today()` |
| `week(d)`, `month(d)`, `year(d)` | `month(date) = period` |
| `format(v, pattern)` | `format(taken, "MM-DD") = format(today(), "MM-DD")` |
| `next(repeat[, after: d])` | `next(repeat, after: last(log.done))` |
| `streak(records)` | `streak(log)` |
| `under(tags, "<parent>")` | `under(tags, "area")` |
| `round(x[, n])` | `round(average(records.weight), 1)` |
| `domain(url)` | `domain(url)` |
| `contains(text, part)` | `contains(subject, "invoice")` |
| `empty(x)` | `empty(repeat)` |

Twenty-six functions, counting `min`, `max`, `today`, `now`, `week`,
`month` and `year` once each.

`week`, `month` and `year` return the period itself (`2026-11`), so they
compare with `period` and group charts. `streak` counts consecutive done
occurrences in a log: a skip does not break it and a miss does (inherited
0066). Functions the engine gains stay in the engine; a plugin asks for one
by issue and never ships its own (coordinator, first set).

### 4.3 Limits and where values live

- **What a formula may not do:** write anything, reach the network, call a
  plugin, read outside its space, or read what the asking person may not.
- **One link hop** (F3). A second hop is an extra computed field on the
  item in between (`in inbox: = mailbox.role = "Inbox"` on messages, then
  `count(from(messages.thread, in inbox)) > 0` on threads).
- **At most 1,000 items** per `from()`, `all()` or link list, **counted
  after the condition**, answered from an index on the link field and the
  date's period, never by reading every linked item (F4). Over the cap the
  value is "too many items".
- **A fixed step budget** per item.
- **No cycles:** computed fields may use other computed fields but never in
  a cycle; `previous()` is not a cycle. The server refuses a file with a
  cycle and names the fields.
- **Re-evaluation:** formulas with `today()` re-run at the person's
  midnight. Formulas with `now()` re-run at their next change time (an
  event's end), never by polling (inherited 0067). `today()` is each
  device's own date and zone.
- **Repeats** are expanded by the engine in calendar views and wherever a
  collection says `expand: repeat`. An item with `series` and `occurrence`
  fields replaces that occurrence, and a `dates` field mapped to EXDATE
  removes one (inherited 0067 CA15).
- **Where values live:** only in the rebuildable index, never in files. The
  one Rust engine computes them on the server, on devices and in the
  browser as Wasm (round 33). A device's value is marked "not yet
  confirmed" until the server agrees (S3).

## 5. Collection files

### 5.1 Every key

| Key | Value | Example |
| --- | --- | --- |
| `collection` | ID | `collection: unread` |
| `title` | Name | `title: Unread` |
| `format` | `1` | `format: 1` |
| `from` | One or several kinds; `files`; `<kind>.records`; `lines(checkbox)`; optionally `ignoring default`; absent means hand-added items only | `from: tasks, events` |
| `where` | A condition | `where: not read` |
| `sort` | `<field> [asc\|desc]…`, with `manual` first for a person's own order | `sort: manual, due` |
| `group` | One field or formula to group by | `group: month(taken)` |
| `total` | `count`, `sum(f)`, `min(f)`, `max(f)` or `average(f)` over this collection's items, per group and for the whole | `total: count, sum(amount)` |
| `totals` | Named values over whole kinds, with `all()`, optionally `per <period>` (F2) | see 5.4 |
| `fields` | Computed columns (`= formula`), or member fields stored beside each member in the body (a board's own status, inherited D7) | `fields:` / `  late: = due < today()` |
| `expand` | `repeat`: list each occurrence in date order | `expand: repeat` |
| `drop` | What dropping an item onto this collection sets | `drop: set scheduled = today()` |
| `views` | One line per view; the first is the default | 5.2 |
| body | Hand-added items: links, each optionally followed by member fields | `- [Mail](<…>) status: Done` |

### 5.2 Every view

| View | Line form | Example |
| --- | --- | --- |
| list | `list: <field>…` | `list: title, site` |
| table | `table: <field>…` | `table: date, payee, amount` |
| board | `board: <choice field>` (dropping into a column sets the field) | `board: status` |
| gallery | `gallery: <image field\|preview>, <field>…` | `gallery: image` |
| calendar | `calendar: <time field>[ .. <time field>][, colour <field>]` | `calendar: start .. ends, colour own calendar.colour` |
| timeline | `timeline: <date field> .. <date field>` | `timeline: starts .. ends` |
| chart | `chart[ bar\|line]: <total> by <field or formula>` | `chart line: sum(volume) by week(date)` |
| map | `map: <place field>` | `map: place` |
| matrix | `matrix: <row condition>, <column condition>` | `matrix: priority in (urgent, high), urgent` |
| folders | `folders: <field>…` (drill-in with breadcrumbs; `from: files` only) | `folders: name, edited, size` |
| compact | `compact: <field>…` (small: a widget, a sidebar preview) | `compact: start, title` |
| strip | `strip: <field>…` (small: a carousel) | `strip: image` |

A view line may use a formula as a column (`list: payee, next(repeat)`;
PENDING P1).

### 5.3 `from:` sources

```markdown
from: tasks
from: photos, ignoring default
from: sessions.records
from: lines(checkbox)
from: files
```

- `<kind>.records` lists the records of every item of the kind, each with
  its item's fields reachable as `item.<field>` (C3).
- `lines(checkbox)` lists plain checkboxes in any note (C1).
- `files` is every file and folder in the space, with the built-in fields
  `name`, `folder`, `size`, `edited`, `type` and `kind`, filtered by the
  person's rights.
- Collections never cross spaces (S4; OPEN O1 may change this).

### 5.4 `totals:` with `all()`

```text
totals:
  <name>: [per day|week|month|year ]= <formula with all()>
```

```markdown
totals:
  ready to assign: per month = previous(ready to assign)
      + sum(all(categories, group = "Income").spent)
      - sum(all(categories, group != "Income").assigned)
      + sum(all(categories).previous(overspent))
```

A formula may continue on indented lines. `all()` obeys the 1,000 cap. A
total shows in the collection's header and may be named in a sidebar
line.

### 5.5 `sort: manual`, `drop:`, `expand:`

```markdown
sort: manual, pinned desc, due
drop: set scheduled = today()
expand: repeat
```

- **`sort: manual`** keeps each person's own order per collection and
  group. The ranks are host app data, backed up and never rebuilt from the
  files (inherited 0066 `order.db`). Losing them loses only the manual
  order.
- **`drop:`** sets fields when an item is dropped onto this collection or
  onto its sidebar entry.
- Placing a collection in another plugin's sidebar or on Home is an
  `extends:` line in `plugin.md` (§7.2, G14, M2).

## 6. Sidebar files

A sidebar is a collection of collections. Its body lists entries in order;
customising it means adding, removing or reordering lines. Each member's
copy lives in their personal space. When the plugin updates a sidebar a
person changed, the person's version stays and the new one is offered
(G2).

```text
- [<title>](<relative path to a collection>)
- [<title>](<plugin>:collections/<id>)
- each: <kind>[, where <condition>][, sort <field>[ asc|desc]][, under "<heading>"]
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
- each: calendars, where empty(account), under "Calendars"
- section: Collections
- folders: Documents, Photos, Notes
```

- **`each:`** makes one entry per item of a kind (a calendar, a person, a
  type). Opening an entry shows that kind's default collection filtered to
  the item.
- **`section:`** heads an area a person fills with a +.
- **`folders:`** gives shortcuts into folders (Files).
- A link to another plugin's collection grants nothing: collections only
  show what the person may read.
- Lines another plugin adds to this sidebar come from its `extends:` block
  (§7.2). They are drawn after this file's lines, and the person removes
  them like any line (G14).

## 7. `plugin.md`

### 7.1 Every key

| Key | Value | Example |
| --- | --- | --- |
| `plugin` | ID, `calternal-<name>` for official ones | `plugin: calternal-photos` |
| `name`, `icon`, `version` | What people see; semantic version | `version: 1.0.0` |
| `format` | `1` | `format: 1` |
| `needs` | Plugins it depends on, with versions; `optional` allowed | `needs: [calternal-notes >=1, calternal-contacts >=1 optional]` |
| `reads` | Other plugins' kinds its code may query, read and listen to (C4) | `reads: calternal-mail.messages` |
| `calls` | Other plugins' operations its code calls | `calls: [calternal-files.add]` |
| `sends out` | `nothing`, or a list of hosts, `a server you name`, `the public web` | `sends out: caldav.icloud.com, a server you name` |
| `sign-ins` | Accounts the server signs in to for it; several allowed, each tied to the person and the item it serves (G12) | 7.3 |
| `secrets` | Fixed secrets the Owner enters (an API key) | `secrets: maps key` |
| `programs` | Programs it runs in the sandbox or in jobs | `programs: ffmpeg, ocr (PP-OCR)` |
| `outside` | Outside addresses and how each proves itself | 7.4 |
| `extends` | What it adds to other plugins' kinds, sidebars and Home | 7.2 |
| `slots` | Places it offers for others to fill | 7.5 |
| `fills` | Host places and others' slots it fills | 7.5 |
| `listens` | Other plugins' operations it reacts to after they commit | 7.7 |
| `settings` | Typed settings with defaults (inherited ADR 0055) | 7.6 |
| `search` | Shorthand for `fills: search: operation <name>` | `search: search photos` |
| body | What it does, in a sentence or two, shown at install | |

### 7.2 `extends:` (M2, replacing the first set's separate rules and G14's lines)

```text
extends:
  <plugin>.<kind> | any kind with url|file|person|place|time|date:
    fields:
      <field line as in §2.7>
    operations:
      <operation line as in §2.19>
    row: + <field>…
    page: + [section "<heading>": ]<field>…
    marks:
      <label>: = <condition>
  <plugin>:sidebars/<id> | home:
    <sidebar line as in §6>
```

```markdown
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
      save link: code, menu
  calternal-notes:sidebars/notes:
    - [To read](<collections/unread.md>)
  home:
    - [On this day](<collections/on-this-day.md>)
```

- **Added fields** are stored in the item's frontmatter under the adding
  plugin's name (`calternal-reading-list.saved: yes`). Only the adding
  plugin's operations write them, or a person on an editable-outside kind,
  and this holds on operations-only kinds too. The owning plugin ignores
  them. When the adding plugin is removed they leave the view and the
  values stay (G1).
- **Added operations** run as the adding plugin. They change the owner's
  fields only through the owner's listed operations.
- **`row: +` and `page: +`** append at the lowest priority, so the owner's
  places drop last, and they never cover `protected:` fields (§2.18).
- **A mark** is a condition that shows a label on the item. It never
  refuses anything.
- **`any kind with <type>`** resolves at install against the kinds present,
  and again when a kind is installed later.
- **Sidebar and Home lines** are drawn after the owner's lines. The person
  removes them like any line, or uses Reset (G14; Home, coordinator second
  set).
- **What additions cannot do:** remove, rename, retype or reorder the
  owner's fields, operations or places (§12).
- **The install sheet** lists every addition by target ("Adds a 'Saved'
  field and 'Send to Reading List' to Mail messages"), and the person can
  turn each off.

### 7.3 `sign-ins:` (G12)

```text
sign-ins:
  <name>: imap|smtp|http|oauth <provider>, per <kind>[, for <host or "a server you name">]
```

```markdown
sign-ins:
  mail account: imap, per accounts, for a server you name
  mail sending: smtp, per accounts, for a server you name
```

- The server stores each sign-in, tied to the person who added it and to
  the item it serves.
- A plugin reaches it only through `s.connect` or `s.fetch(…, { sign-in
  })`, and never sees its value.

### 7.4 `outside:` (C2 part 2; round 29)

```text
outside:
  <name>: secret|signed <header> with <secret>|person[, methods <METHOD>…]
```

```markdown
outside:
  scanner: secret, methods POST
  github: signed X-Hub-Signature-256 with webhook secret
  api: person, methods GET POST PUT DELETE
```

- Each name is an address under the plugin's own path,
  `/outside/<space>/<plugin>/<name>/…` (path shape for the build). It is
  never the server's root and never `/.well-known/`.
- How each request proves itself:
  - `secret`: the address itself is secret;
  - `signed`: the sender also signs, and the server checks the signature;
  - `person`: a person's token.
- The request reaches `on.outside(name)` whole, and the operation answers
  with a status, headers and a body. So a plugin can serve any protocol or
  API itself.

### 7.5 Places: `fills:` and `slots:` (M1 and G13, one table)

A **place** is somewhere a part can be drawn or called. The host defines a
fixed list every space has; an app adds its own with `slots:`. Both have the
same shape:

| Place | Single or many | Gives | Who picks |
| --- | --- | --- | --- |
| `shell strip` | single | the apps and sidebars | person |
| `shell panel` | many | the open window's item | person; the AI tab the space |
| `palette` | many | the typed text, only for entries the person opens | automatic; the person may hide one |
| `search` | many | the query, only in a scope the person picked | person |
| `receive` | many | a file, URL or text the person shares or drops | person, each time or as a default |
| `export` | many | the selection | person |
| `first run` | single per app | nothing | the space (§11) |
| `kind <kind> panel` | many | the open item | person |
| `kind <kind> page end` | many | the open item | person |
| `kind <kind> list` | single | the rows of a window | person |
| `collection view` | many | the window's rows | person, per collection |
| `widget`, `menu bar` | many | a collection's compact view | person, on the device |
| `<plugin>:<slot>` (an app's own, G13) | as declared | as declared | as declared |

```text
slots:
  <slot>: single|many, gives <what>, picked by person|space[, recommended <plugin>]
fills:
  <place>: frame <path>|operation <operation>|collection <id>[, as "<label>"]
  receive: <type>… -> operation <operation>
```

```markdown
slots:
  harness: single, gives the chat and the run's scope, picked by person, recommended calternal-chat-claude-code
fills:
  kind calternal-mail.messages panel: frame frames/summary.html
  search: operation search archive, as "Mail archive"
  receive: url, text -> operation calternal-reading-list.add
  calternal-chat:composer: frame frames/composer.html
```

- **Who picks:** the person, for what only they see; the space, for what
  others receive.
- **A place gives only what its row says**, filled as the viewer. A part
  calls only its own plugin's operations and what its `calls:` lists.
- **Places that see typed text** (palette, search, a composer) say so on
  the pick screen, and are called only when the person opens that entry or
  scope.
- **Errors stay local:** a broken part draws an error in its own place
  only. A missing part falls back to the recommended one, or suggests the
  official part.
- **A plugin's search** (`search:` in §7.1 is the same as `fills: search`)
  answers only in a scope the person picked, within a time budget, in its
  own section (coordinator, third set).
- **New host places** pass the same test as a server feature: useful to
  two unrelated apps, and describable without an app's name.

### 7.6 `settings:` (inherited ADR 0055)

```text
settings:
  <name>: <type>[, default <value>][, per person|space]
```

```markdown
settings:
  urgent within: duration, default 2d, per person
```

Settings have stable IDs and appear in Settings and the palette from the
declaration. Formulas read them as `settings.<name>`.

### 7.7 `listens:` (M3, the after half)

```text
listens:
  after <plugin>.<operation>
```

```markdown
listens:
  after calternal-mail.receive
```

The listed operation's commits reach `on.after` (§8.2). Declaring it shows
on the install sheet ("Reacts after Mail receives a message").

### 7.8 Example

```markdown
---
plugin: calternal-documents
name: Documents
version: 1.0.0
icon: doc
format: 1
needs: [calternal-mail >=1 optional]
reads: calternal-mail.messages
sends out: nothing
programs: ocr (PP-OCR), pdf (pdfium), classifier
search: search documents
outside:
  scanner: secret, methods POST
extends:
  calternal-mail.messages:
    operations:
      file to documents: code, menu, when count(attachments) > 0
calls: []
---
Scans and PDFs, read and sorted for you. The originals are never changed.
```

## 8. The thirteen calls and seven events

`operations.ts` exports operations, each listening to one event. Every call
goes through the server as the acting person (`s.me`), within the plugin's
declarations. Bounds are operator defaults unless marked fixed.

**Errors.** Every call fails with one caller-safe error shape, `{ code,
message, next }`: a plain message and the next useful step (review checklist
item 1: one shared type). The codes:

| Code | When |
| --- | --- |
| `not-allowed` | Rights, role line, `reads`, `calls` or `sends out` |
| `not-found` | No such item, kind or operation |
| `invalid` | A field type, `checks:` line or form, with the line |
| `conflict` | The item changed since the base the write named |
| `too-large` | Over a size bound |
| `limit` | Over a count, rate or quota bound |
| `unreachable` | A destination or connection is down |
| `needs-sign-in` | A sign-in is missing or expired |
| `stopped` | A run or job was stopped |
| `timeout` | Over a time bound |

### 8.1 Calls

| Call | Input | Output | Errors | Bound |
| --- | --- | --- | --- | --- |
| `s.query(q)` | `{ from, where, sort, group, total, fields, cursor }` in collection-file terms | `{ rows, totals, cursor }` | `not-allowed`, `invalid` | 200 rows a page |
| `s.read(item)` | An item ID | `{ id, kind, fields, computed, body, records }` | `not-found`, `not-allowed` | 32 MiB per item |
| `s.write(changes, { preview?, key? })` | A list of `add(kind, fields, body?)`, `change(item, fields?, body?, base?)`, `remove(item)`, `move(item, folder?, file?)`, `append record(item, record)`, `change record(item, anchor, record)`, `remove record(item, anchor)` | The new IDs and versions; with `preview`, the plan for a confirm sheet, nothing written | `invalid`, `conflict`, `not-allowed`, `too-large`, `limit` | 1,000 changes and 16 MiB a write; all or nothing |
| `s.fetch(req, { secret?, sign-in? })` | An HTTP request to a declared destination | `{ status, headers, body }` | `not-allowed`, `unreachable`, `needs-sign-in`, `too-large`, `timeout` | 32 MiB reply, 60 s |
| `s.connect(sign-in, item)` | A declared IMAP or SMTP sign-in for an item | The open connection the server holds | `needs-sign-in`, `unreachable` | 1 per sign-in and item |
| `s.notify(people, { item, text })` | People who may read the item | Delivered or skipped (muted) | `not-allowed`, `limit` | 100 recipients a call |
| `s.job(program, items, input?)` | A declared program and items | A job ID; results come back through the plugin's own operations | `not-allowed`, `limit` | 10,000 items a job |
| `s.run(program, { scope, targets })` | An agent harness and its scope | A handle `{ id, send(input), stop(), status }`; `send` only by the run's actor, `stop` by anyone who may read the target (A18) | `not-allowed`, `limit`, `stopped` | per the run's quota |
| `s.call("<plugin>.<operation>", input)` | A listed operation of another plugin | That operation's output | `not-allowed`, plus that operation's errors | depth 8 (fixed) |
| `s.state` | `get(key)`, `set(key, value)` | This person's state for this plugin | `too-large` | 64 KiB |
| `s.cache` | `get`, `set`, `delete`, `file(name)` for large derived files on the HDD tier | Rebuildable data; the server may delete it at any time | `limit` | per-plugin quota |
| `s.share(item, people, role)` | People or groups, and a role | The share | `not-allowed`; reaching people outside the space goes through the server's sheet | 100 people a call |
| `s.me`, `s.space`, `s.event` | — | Who is acting and their role; the space; why the code started (`{ kind, key, item, before?, request? }`) | — | — |

### 8.2 Events

| Event | Fires when | The operation receives | It returns |
| --- | --- | --- | --- |
| `on.call` | Someone runs the operation (a coded line in the operations list, a `search:` operation, another plugin's `s.call`) | The form's fields or the input | An output, or an error |
| `on.time(<kind>.<field>)` | An item's time field arrives | The item | nothing |
| `on.schedule(<kind>.<repeat field>)` | An item's repeat rule comes due | The item and the occurrence | nothing |
| `on.change(<kind or collection>)` | Items of its own kinds, or of kinds named in `reads`, change | The item and `s.event.before` (the fields before) | nothing |
| `on.outside(<name>)` | A request reaches a declared outside address | `{ method, path, headers, body }` | `{ status, headers, body }` |
| `on.connection(<sign-in>)` | Data arrives on a server-held connection | The item and what arrived | nothing |
| `on.after(<plugin>.<operation>)` | Another plugin's operation named in `listens:` has committed | `{ input, result }`, filtered to what the acting person may read | nothing; it cannot change the result, but may call operations, including the owner's undo |

**Delivery:** `on.change` and `on.after` share one delivery path, raised by
the call route after commit. Every event is delivered at least once, with a key; a write made with
the same key is a replay. A plugin's own writes do not fire its own
`on.change`. Changes are coalesced per item, so a burst of edits is one
event. Call depth 8 applies to reactions. One process per plugin per space,
stopped when idle.

## 9. Agent rules files

A person's rules are `Agents/rules.md` in their personal space, applied to
their runs in every space. A space's rules are `.calternal/agent-rules.md`,
written by the Owner or a Manager. A person's rules can narrow the space's
but never widen them (R2).

```text
- allow|ask|deny <operation>[ on <kind>][ in <collection>][ where <condition>][ outside this chat][ for <harness>]
- allow|ask|deny add|change|remove in <folder>[ for <harness>]
- allow|ask|deny fetch <host>[ for <harness>]
```

```markdown
---
rules: agents
format: 1
---
- ask calternal-mail.send
- deny calternal-money.* on transactions where amount > 500 EUR
- allow change in Notes/Work
- ask remove
- deny fetch github.com for calternal-chat-codex
```

- **Operations** are named in the §1.2 form; `<plugin>.*` covers every
  operation of a plugin, and `add`, `change` and `remove` alone mean the
  built-ins of any kind.
- **Precedence:** deny, then ask, then allow; the most restrictive match
  wins.
- **Fixed ceilings:** the starter's own rights, the run's scope, the space's
  Agent access switch, and the server's risky actions, which always go
  through its sheet with a passkey.
- **Default without rules ("auto-approve for me", R1):**
  - in the personal space, everything is free except reaching other
    people;
  - in a space with members, removing and changing other people's items
    also ask.
- **The prompt** offers Allow once, Allow for this run, Always allow and
  Don't allow. Always allow writes one line, with `for <harness>` for the
  harness that asked (R3). It never expires (R6). When each rule was added
  and last used is host state, shown in Settings and not written in the
  file (§16, choice 5).
- **No reviewing model** decides (R4).
- **Harnesses:** a "direct, full access" run on the person's own machine
  uses the harness's own permissions and says so (R5). Our harnesses run
  permissive inside our sandbox, so only these rules ask (R7).

## 10. Theme files (M4)

A theme is one file of tokens, `themes/<name>.md` in a plugin, installed
into `.calternal/themes/` and picked by the person. A space profile may give
a default (§11).

```text
theme: <id>
name: <name>
format: 1
palette: "<colour>…"
fonts:
  interface|chat|code|wordmark: <font name or file>
icons: <folder of SVGs, one per host icon name>
sounds:
  <host sound name>: <file>
motion: <host preset>
wallpaper: <file>
window: opaque|frosted
for:
  <plugin>: [accent "<colour>"][, density compact|regular]
```

```markdown
---
theme: dusk
name: Dusk
format: 1
palette: "#1f1f1f #f7f7f7 #c96f4a #5b7f9e #d9a441 #6f8f5b #a65e7c #8a8a8a"
fonts:
  interface: Google Sans Flex
  code: Cascadia Code
icons: files/icons/
sounds:
  sent: files/sounds/sent.m4a
motion: calm
wallpaper: files/dusk.avif
window: frosted
for:
  calternal-money: accent "#2f7d5b", density compact
---
```

- **What a theme cannot do:** no CSS, positions or sizes, so a theme cannot
  move, cover or hide anything.
- **Guarantees:** the renderer keeps a contrast floor, reduced motion
  always wins, and host screens are not themed.
- **Assets** pass the font validator, the wallpaper pipeline and a sound
  check (type by magic number, a few seconds, a few hundred KiB).
- Inherited: themes are tokens, never CSS (2026-11-18).

## 11. The space profile, `.calternal/space.md` (M5)

A space's own defaults. The Owner writes it, or applies a profile a plugin
ships (`profiles/<name>.md`) from the install sheet, which lists every line.

```text
space profile: <id>
format: 1
strip: <plugin>…
picks:
  <place>: <plugin>
rights:
  <plugin>.<kind>: change manager|owner
agent rules:
  <agent rule line as in §9>
first run:
  <plugin>: operation <operation>
theme: <theme>          # OPEN: whether this sets the look for everyone
```

```markdown
---
space profile: family-money
format: 1
strip: calternal-money, calternal-tasks
picks:
  shell panel: calternal-chat-claude-code
rights:
  calternal-money.transactions: change manager
agent rules:
  - ask remove on calternal-money.transactions
first run:
  calternal-money: operation calternal-money.setup
---
```

- **It only tightens:** a profile only raises roles and narrows rules (R2).
  It never widens them.
- **What only one person sees** stays theirs to change.
- **It installs nothing by itself:** it names plugins, and the install
  sheet installs them.
- **Picks** fill places that others receive. Rights use G5's role line.
- **`first run`** runs once, the first time the app opens in the space.
- The server reads the file when the space opens and caches it by
  revision, like a sidebar.

## 12. What is never open

No key, place, extension or profile reaches these (coordinator, third set,
from §3 of the extension audit):

- the host screens (sign-in, passkeys, recovery, the profile popover,
  Settings and Plugins pages, consent requests, safe mode) and Stop;
- the one call check and its table;
- another plugin's inputs before its operation;
- removing, renaming or retyping another plugin's fields or operations;
- a file's kind, ID, author stamp and outside-editing switch;
- other spaces (reading, collections, formulas);
- the sync route, the journal and the merge;
- secrets' values, the network list and programs;
- code in the client outside a frame, and CSS;
- free notification text;
- the share sheet and any widening of sharing;
- host-reserved shortcuts (a plugin's shortcut loses to the person's and
  to the reserved list);
- another person's private state;
- refusing Stop, undo, deleting one's own items, or leaving a space;
- the canvas of windows, for now.

## 13. OPEN items (the owner's)

| Item | The question | What each answer changes here |
| --- | --- | --- |
| **O1** partner sharing across spaces | May a collection include items of another space shared with you? | Yes: `from:` gains `, shared with me` (read-only), and S4 gets one exception. No: nothing changes; partners use a shared space |
| **O2** native Apple surfaces | Notification buttons, share targets, Shortcuts, Live Activities, device bridges | Yes as proposed: `s.notify` gains `actions:`, the item's declared operations (on a locked device only those the person marked safe); operations gain a progress value; kinds gain `health: workout`. No: `receive` and `timer` are all there is |
| **O3** Workouts' first version | Log only, or with progression? | Only whether Workouts has coded operations |
| **O4** reading rights inside a space | May a kind or folder be readable by fewer people than its space? | Yes: a kind key `read: manager\|owner\|<group>…`, G5's twin, and the same line in `space.md` rights. No: spaces stay the only read boundary |
| **`refuses:`**, the before half of M3 | May a plugin refuse another plugin's operation before it runs? | Yes: `plugin.md` gains `refuses:` with lines in the `checks:` form (`<plugin>.<operation>: <condition>, "<sentence>"`), evaluated by the engine on the server and the device. It never changes inputs, never runs code, and can never target Stop, undo, deleting one's own items or leaving a space. No: plugins only react after |
| **Plugin pure modules on devices** | May a plugin ship a module with no I/O (an interpreter inside the shared core) that runs on devices? | Yes: coded operations could apply offline, and plugins could add formula functions, field types and merges. No (today): coded operations queue offline, and functions come only from the engine |
| **A space-wide look in a profile** | May `space.md`'s `theme:` set the look for every member, or only offer a default? | Everyone: `theme:` is binding, with members unable to change it. Default only: each member picks, and a space keeps an accent and icon so members know where they are |

## 14. PENDING details (from the Money, Mail and Chat test, not yet recorded as decided)

| # | Line | Used by |
| --- | --- | --- |
| P1 | A formula as a view column (`list: payee, next(repeat)`) | Money scheduled |
| P2 | `change: only <fields>` in the operations list (written into §2.19; widely used) | Mail, Chat, Calendar, Documents |
| P3 | `page: web(<field>)`, an HTML body in a web view | Mail |
| P4 | `page: records(from(<kind>.<link>))`, a records view over several items | Mail threads |
| P5 | `link(<kind> records)`, a link to one record | Chat quotes |
| P6 | A `state:` section with per-person keys, and `set state.<key> = …` in one-liners | Chat |
| P7 | `records` and `state.` in formulas and `checks:` | Chat |
| P8 | A host-computed `session()` scope for runs | Chat AI |

## 15. Contradictions resolved

| # | Older text | Newer decision | Here |
| --- | --- | --- | --- |
| 1 | `actions:` and `create:` keys (revision 5 of this file) | One `operations` list; `create` folds into `add` (round 29) | §2.19 |
| 2 | "Every kind gets add, change and remove for free" (rounds 25, primitives §4.4) | Only editable-outside kinds have them unlisted; operations-only kinds have only what they list (round 29) | §2.5, §2.19 |
| 3 | `card` and `cell` keys | Dropped (coordinator, second set) | §2.18 |
| 4 | Sixteen functions | Twenty-six, with F1, F2 and the coordinator's details | §4.2 |
| 5 | "1,000 items per `from()` or link list", silent on conditions | Counted after the condition, through an index (F4) | §4.3 |
| 6 | Format example 2.12: a Markdown file beside each scan | A file item with an XMP sidecar in `.calternal/` (FS1, coordinator); Documents files by date (C5) | §2.4 |
| 7 | `.calternal/` "not in the WebDAV mount" (host-gaps 2.4 (a)) | Visible and readable, refused for outside writes (round 29) | §1.1 |
| 8 | "Reading is free inside a space" for plugin code (primitives §4.4) | Code reads another plugin's kinds only with `reads:` (C4); collections still show anything the person may read | §7.1, §8.2 |
| 9 | "A plugin changes only its own kinds' items" | It may add fields and operations to another plugin's kind (coordinator, first set) | §7.2 |
| 10 | "Customising a sidebar is adding, removing or reordering collections and nothing else" | Sidebars also hold `each:`, `section:` and `folders:` lines, and other plugins' collections (coordinator, second set; G14) | §6 |
| 11 | "No Wasm" (ADR 0048 as changed, 2026-11-19) | The one Rust engine runs in the browser as Wasm; plugin code still never runs on devices (round 33) | §4.3 |
| 12 | `on.outside` as "an API call with a person's token" (host-gaps 4.1), then "a secret address with an optional signature" (round 29) | A full HTTP request under the plugin's address, proved by secret, signature or a person's token (C2) | §7.4, §8.2 |
| 13 | "Totals are count, sum, min, max and average grouped by one field; anything more is the plugin's own cache" (round 22) | `totals:` with `all()` and `per <period>` fields (F1, F2) | §2.7, §5.4 |
| 14 | Twelve calls, then fifteen (G15) | Thirteen: `send` and `stop` are methods of `s.run`'s handle (coordinator, second set) | §8.1 |
| 15 | `available: code`, a field computed by code (Money stress test) | `per month` fields with `previous()` (F1); there is no code-computed field | §2.7 |
| 16 | `search:` with numeric weights (suite stress test) | Fields in order of weight (G9) | §2.15 |
| 17 | `secrets: <name>, per <kind>` (Money/Mail/Chat test) | `sign-ins:`, several per plugin, tied to person and item (G12) | §7.3 |
| 18 | "A form is the kind's create form" (primitives §4.3) | The `add` line is the form | §2.19 |
| 19 | Daily notes in `Notes/` (ADR 0026) | In `Calendar/` (ADR 0067, audit precedence fix 1); the format only supplies `folder:` and `file:` | §2.3 |
| 20 | Inbox "not expressible" (Money/Mail/Chat test row 8) | One field per hop (F3); corrected in that test | §4.3 |
| 21 | Added fields and operations (coordinator, first set) and G14's sidebar lines as separate rules; `show in:` on a collection (revision 6 draft) | One `extends:` block for fields, operations, look additions, marks, sidebar and Home lines; added values stored under the adding plugin's name (M2) | §7.2 |
| 22 | G13 slots as an app-only mechanism | Host places and app slots in one table and one shape; `fills:` for both (M1) | §7.5 |
| 23 | Six events | Seven: `on.after`, declared with `listens:`, sharing `on.change`'s delivery path (M3) | §7.7, §8.2 |
| 24 | "Who owns a space's look" open as a whole | Themes are a token file (M4); a space's defaults are `.calternal/space.md` (M5); only a binding space-wide look stays open | §10, §11, §13 |
| 25 | "Reading is free in the space" (extension audit, citing primitives §2) for reactions | `listens:` gives only the operation's input and result, filtered to the acting person; querying another plugin's kind still needs `reads:` (C4) | §7.7, §8.2 |

## 16. Choices this revision makes that no decision covers (for the coordinator)

1. **Definition files use this document's line grammar, not YAML** (§1.3).
   Item files keep YAML. The older examples were not valid YAML.
2. (Withdrawn: an earlier draft of this revision used `show in:` on a
   collection; M2's `extends:` lines replace it.)
3. **The `repeat:` kind key is dropped:** a kind has at most one field of
   type `repeat`, and that is the repeat field.
4. **Record fields are `key: value` lines** right after the record's
   header.
5. **The "added" and "last used" times of agent rules are host state**, so
   the rules file stays hand-writable.
6. **The outside address path** `/outside/<space>/<plugin>/<name>/…`, and
   `outside:` in `plugin.md` with three proofs.
7. **`ignoring default`** in `from:` as the one way out of `default where:`
   (the suite test wrote `including hidden`).
8. **`format(v, pattern)`** as the one text-formatting function, which also
   covers "on this day".
9. **One error shape with ten codes** (§8).

## 17. Reading List, written out in full

```markdown
<!-- plugin.md -->
---
plugin: calternal-reading-list
name: Reading List
version: 2.0.0
icon: book
format: 1
sends out: nothing
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
  add: url, title, share
  mark read: set read = yes, when not read, swipe
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
// operations.ts: the only code, for the reminder
export const remind = on.time("reading.remind", async (item, s) =>
  s.notify([s.me], { item, text: "Time to read this" }));
```

Change and remove are not listed because the kind is editable outside.
Without `remind`, it is a description-only plugin any member may add. The
two stress tests write every other app in the suite. Where their syntax
differs from this revision, this revision wins (§15 rows 3, 15, 16 and 17;
§16 choices 3 and 7).

## 18. Counts, and two ways to say one thing

| | Count |
| --- | --- |
| Kind keys | 24, with the body (§2.1) |
| Field flags | 6 |
| Operation line forms | 12 (§2.19) |
| Field types | 23 (§3), plus 4 built-in fields |
| Formula functions | 26 (§4.2) |
| Collection keys | 13, with the body (§5.1) |
| Views | 12 (§5.2) |
| Sidebar line forms | 5 (§6) |
| `plugin.md` keys | 20, with the body (§7.1) |
| `extends:` sub-keys | 5 (`fields`, `operations`, `row: +`, `page: +`, `marks`), plus sidebar lines |
| Places | 13 host places, plus each app's slots (§7.5) |
| Theme keys | 11 (§10) |
| Space profile keys | 8 (§11) |
| Agent rule line forms | 3 (§9) |
| Calls | 13 (§8.1) |
| Events | 7 (§8.2) |
| Error codes | 10 |

**What still feels like two ways to say one thing:**

1. **`total:` and `totals:` in a collection.** One aggregates the
   collection's own items; the other names values over whole kinds. They
   could be one `totals:` key, where unnamed lines (`count`, `sum(amount)`)
   are the old `total`.
2. **The kind key `change: manager` and the operation `change: only a, b`.**
   They are the same word for a role and for an operation. Renaming the key
   to `role: manager` would remove the clash, at the cost of reopening G5's
   wording.
3. **`empty` as a value and `empty(x)` as a function.** `x = empty` would
   do for both.
4. **`row:` and the `list:` view.** A list view names its fields too.
   `row` stays because search results and links need a line outside any
   collection.
5. **`timeline: a .. b` and `calendar: a .. b`.** Both draw spans over
   time. `timeline` stays: the owner chose it in round 24.
6. **`sign-ins:` and `secrets:`.** Both are something the server holds for
   the plugin. One is an account per person and item, the other a value
   the Owner enters once. They could be one key with `per <kind>` optional.
7. **`from: files` with `folders:` lines and the `folders` view, beside
   kinds' own `folder:` patterns.** Files sees the tree; kinds see their
   folders. This is by design, but a builder will ask.
8. **`search:` in `plugin.md` and `fills: search: operation …`.** They are
   the same thing; `search:` could go.
9. **A mark and a computed yes/no field shown on the row.** A mark is a
   label from a condition, and so is `row: + <computed field>`. Marks could
   be computed fields with a `mark` flag.
10. **`checks:` and the open `refuses:`.** If the owner says yes, `refuses:`
    is `checks:` attached to another plugin's operation, with the same
    lines.
11. **`on.change` and `on.after`.** One delivery path, two events: a file
    changed, or an operation committed. A builder implements one queue.

## 19. Change list from revision 5

Each change, with the decision behind it:

- **Operations list:**
  - one `operations` list replaces `actions` and `create` (round 29);
  - the built-ins are `add`, `change`, `remove` and `duplicate`;
  - new line forms: `set f = ask`, `open where`, `timer`, and record
    operations (C3).
- **New kind keys** (G5, G9, G10, C1 and the coordinator's second set):
  - `file:` and `folder:` patterns;
  - `item:` with three forms;
  - `change:` (role), `lines:`, `unique:`, `together:`, `checks:`,
    `default where:`, `keep:`, `shown:`, `search:`, `template:`;
  - `ical:` and `vcard:`.
- **Field types:** added `when`, `dates`, `durations`, `colour`, `number
  <unit>`, `data` and `records(…)`; built-in `created` and `edited`;
  field flags `heading`, `was`, `from metadata` and `in sidecar`.
- **Formulas** (F1 to F4, round 33):
  - `per day|week|month|year` fields with `previous()` and `period`;
  - `totals:` with `all()`;
  - conditions inside list functions;
  - new functions `first`, `last`, `under`, `streak`, `date` and
    `format`, plus two-value `min` and `max`;
  - the cap counts after the condition;
  - the engine also runs as Wasm.
- **Collections:**
  - new views `map`, `matrix` and `folders`;
  - new keys `expand:` and `drop:`, and `sort: manual`;
  - new sources `files`, `<kind>.records` and `lines(checkbox)`.
- **Sidebars:** `each:`, `section:`, `folders:` and other plugins'
  collections.
- **`plugin.md`:** `reads`, `extends` (M2), `sign-ins`, `outside`,
  `slots` and `fills` in one table of places (M1), `listens` (M3),
  `settings` and `search`.
- **New files:** theme files (M4) and `.calternal/space.md` (M5), and a
  section on what is never open.
- **Calls and events:**
  - thirteen calls, each with inputs, outputs, errors and bounds;
  - seven events, adding `on.after` (M3);
  - `move` and `preview` in `s.write`;
  - `s.event.before`;
  - `on.outside` as a full HTTP request;
  - one error shape.
- **Agent rules files** (R1 to R7).
- **Removed:**
  - the `card`, `cell`, `actions`, `create` and `repeat:` keys;
  - the comparison with Obsidian Bases (it lives in revision 5 at
    `22a6b698`);
  - the fifteen worked examples (now in the two stress tests).
- **New sections:** OPEN items, PENDING details, and the list of
  contradictions resolved.
