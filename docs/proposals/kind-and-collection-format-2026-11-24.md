# The kind and collection format · 2026-11-24

Status: **proposal** for the owner (rounds 21–25 of
`docs/morning-2026-11-21.md`). It specifies how kinds, items, collections,
sidebars and plugins are written on disk, then shows fifteen worked examples
from the app suite. The design it serves is
[primitives-only-host-2026-11-21.md](primitives-only-host-2026-11-21.md),
revision 5.

The owner's requirements: Markdown with frontmatter, never JSON; a person can
write a sum by hand; an agent can write anything larger on request; a
no-code plugin and a kind made in the app are the same files.

## 1. The format in ten minutes

### 1.1 The files

A plugin is a folder of Markdown files, plus one TypeScript file only if some
operation needs code:

```text
calternal-reading-list/
  plugin.md            who it is, what it may send out, what it calls
  kinds/reading.md     one file per kind
  collections/unread.md
  collections/all.md
  sidebars/reading.md  the app's default sidebar
  operations.ts        only for operations that need code (optional)
```

Installing copies the Markdown files into the space's `.calternal/`
(`.calternal/kinds/`, `.calternal/collections/`, `.calternal/sidebars/`,
`.calternal/plugins/<id>.md`). "New kind" in the app writes the same files.
A plugin without `operations.ts` is a **description-only plugin**: any member
may add one (round 22).

Items live in the kind's folder in the space (`Reading/`), one Markdown file
each. The kind file's body is also written to that folder as its `AGENTS.md`.

### 1.2 A kind

```markdown
---
kind: reading
name: Reading
folder: Reading
editable: outside
fields:
  title: text, required
  url: url, required
  read: yes/no, default no
  added: date, default today
  site: = domain(url)
row: title, site
card: title, site
create: url, title
actions:
  mark read: set read = yes, when not read, swipe
---
Links saved to read later. One file per link. `read` is yes or no.
```

| Key | Meaning |
| --- | --- |
| `kind` | The kind's ID, unique in the space |
| `name` | What people see |
| `folder` | Where its items live in the space |
| `editable` | `outside` (in the WebDAV mount, a folder can be handed to an agent) or `operations` (neither); fixed for the kind's life |
| `fields` | One line per field: `name: type[, required][, default v]` or `name: = formula` for a computed field |
| `records` | For a record kind (a chat, a log): the fields of each record, in the same one-line form |
| `row`, `card`, `cell`, `page` | Which fields each look shows, in order |
| `create` | The fields of the kind's create form, in order; the form is not a view |
| `actions` | Named one-line actions: `set <field> = <value>[, when <condition>][, swipe\|menu\|primary]` |
| `repeat` | The field holding the iCalendar repeat rule, if the kind repeats |
| Body | Plain text for people and agents; written as the folder's `AGENTS.md` |

Every kind gets **add, change and remove** for free on the app, the CLI, the
API and MCP: `calternal reading add --url … --title …`.

**Field types** (one word each; `?` after a type in a link means optional):

| Type | Holds | Example |
| --- | --- | --- |
| `text` | A line of text | `title: text` |
| `long text` | Paragraphs (the item's body is the main one) | `notes: long text` |
| `number` | A number | `reps: number` |
| `money` | An exact decimal with a currency, never a float | `amount: money EUR` |
| `yes/no` | True or false | `done: yes/no` |
| `date` | A day | `due: date` |
| `time` | A date and time; one-off times keep their offset, repeating task times float, repeating events keep their zone (ADR 0066) | `starts: time` |
| `duration` | A length of time | `rest: duration` |
| `choice(a\|b\|c)` | One of a fixed set; the order is the board's column order | `status: choice(To do\|Doing\|Done)` |
| `tags` | A list of words | `tags: tags` |
| `url` | A web address | `url: url` |
| `link(kind)` / `links(kind)` | One or several items of a kind (stored as Markdown links) | `project: link(projects)` |
| `person` / `people` | One or several people of the space | `owner: person` |
| `repeat` | An iCalendar repeat rule (`FREQ=WEEKLY;BYDAY=MO`) | `repeat: repeat` |
| `place` | A location (latitude, longitude, name) | `where: place` |
| `file` | An attached file in the item's folder | `scan: file` |

### 1.3 An item

```markdown
---
title: How Discord stores trillions of messages
url: https://discord.com/blog/how-discord-stores-trillions-of-messages
read: no
added: 2026-11-24
---
Optional body: the person's own notes.
```

The server adds `id:` and `kind:` as read-only copies; host state holds the
truth. Computed fields are never written into the item.

### 1.4 A collection

```markdown
---
collection: unread
title: Unread
from: reading
where: not read
sort: added desc
views:
  list: title, site
  table: title, site, added
---
- [A paper I want to keep near the top](<../../Reading/paper.md>)
```

| Key | Meaning |
| --- | --- |
| `collection`, `title` | ID and name |
| `from` | One or several kinds (`from: tasks, events`); absent means hand-added items only |
| `where` | A condition (§1.6) |
| `sort` | `field [asc\|desc], field [asc\|desc]` |
| `group` | One field to group by |
| `total` | Totals per group or for the whole list: `count`, `sum(f)`, `min(f)`, `max(f)`, `average(f)` |
| `fields` | Fields of this collection's own: computed columns (`= formula`), or member fields stored beside each member in the body (a board's `status`) |
| `views` | One line per view: `list`, `table`, `board`, `gallery`, `calendar`, `timeline`, `chart`, and the small `compact` and `strip`; the first is the default |
| Body | Hand-added items as a list of links, each optionally followed by member fields: `- [Mail](<…>) status: Done` |

View lines: `table: a, b, c` (columns); `board: status` (group field);
`gallery: cover, title`; `calendar: starts`; `timeline: starts .. ends`;
`chart: sum(amount) by month(date)` (bar), `chart line: …`; `compact: title`;
`strip: title`. Map comes with Photos (`map: where`).

### 1.5 A sidebar and plugin.md

```markdown
---
sidebar: reading
title: Reading List
---
- [Unread](<../collections/unread.md>)
- [All](<../collections/all.md>)
```

A sidebar is a collection of collections: its body lists them in order, and
customising it is adding, removing or reordering lines. Each member's copy
lives in their personal space.

```markdown
---
plugin: calternal-reading-list
name: Reading List
version: 2.0.0
icon: book
needs: []
sends out: nothing
calls: []
---
Save links to read later.
```

`sends out` is `nothing`, or a list of: named hosts (`api.github.com`),
`a server you name` (the member names it and confirms it on the server's
sheet), `the public web` (install words: "Reads public web pages. Never signs
in as you."), plus `secrets:` and `programs:` lines. `needs` lists
dependencies with versions; `calls` lists other plugins' operations it calls.

### 1.6 Formulas and conditions

One expression language for computed fields, `where`, `when`, `total` and
chart values, written spreadsheet style.

- Values: numbers (`12.5`), money (`12.50 EUR`), text in quotes, `yes`,
  `no`, dates (`2026-11-24`), durations (`7d`, `2h`, `30m`), `empty`.
- Operators: `+ - * /`, `= != < <= > >=`, `and`, `or`, `not`; `date +
  duration`; `date - date` gives a duration.
- A field is named by its name; a linked item's field by a dot
  (`project.name`); the items linking here by `from(kind.field)`.

The functions, kept to sixteen, each because an example below needs it:

| Function | Why it is there |
| --- | --- |
| `if(c, a, b)` | Labels such as `if(late, "Late", "On time")` |
| `sum(list)`, `count(list)`, `min(list)`, `max(list)`, `average(list)` | Totals through link fields (`sum(tasks.hours)`), the same five as query totals |
| `from(kind.field[, condition])` | The items of another kind that link here (a budget's transactions) |
| `today()`, `now()` | Due and overdue; re-evaluated at the person's midnight and when a query runs |
| `days(a, b)` | Ages and gaps in whole days |
| `month(d)`, `week(d)`, `year(d)` | Grouping and charts by period |
| `next(repeat)` | The next occurrence of a repeat rule |
| `round(x[, n])` | Money and averages |
| `domain(url)` | Reading List's site column |
| `contains(text, part)` | Simple text conditions |

**What a formula may not do:** write anything, reach the network, call a
plugin, or read outside the space and the asking person's rights. Its cost is
bounded: one link hop (`project.name`, `from(...)`, not `project.client.name`),
at most 1,000 items per `from()` or link list (an operator default), and a
fixed step budget per item. Computed fields may use other computed fields but
never in a cycle; the server refuses a kind or collection file with a cycle
and says which fields form it.

**What the server stores:** computed values only in the rebuildable index
(appdata), never in the files. Deleting the index loses nothing; the server
recomputes on the next read.

A person writes `spent: = sum(from(transactions.category).amount)` by hand.
Anything longer an agent writes on request, in the same language.

## 2. Worked examples

### 2.1 Money: accounts and transactions (operations only)

```markdown
---
kind: transactions
name: Transaction
folder: Money/Transactions
editable: operations
fields:
  payee: text, required
  amount: money EUR, required
  date: date, default today
  account: link(accounts), required
  category: link(categories)
  repeat: repeat
  cleared: yes/no, default no
row: payee, amount, date
create: amount, payee, category, account
actions:
  clear: set cleared = yes, when not cleared, swipe
---
One file per transaction. Amounts are exact decimals. Change transactions only
through Money's operations.
```

A transfer between two accounts is one write of two changes that commit
together (round 22), through `operations.ts`:

```ts
export const transfer = op(async ({ from, to, amount, date }, s) =>
  s.write([
    s.add("transactions", { payee: "Transfer", amount: -amount, account: from, date }),
    s.add("transactions", { payee: "Transfer", amount, account: to, date }),
  ]));
```

### 2.2 Money: a budget with a computed total

```markdown
---
kind: categories
name: Category
folder: Money/Categories
editable: operations
fields:
  name: text, required
  budget: money EUR
  spent: = sum(from(transactions.category, month(date) = month(today())).amount)
  left: = budget - spent
row: name, spent, left
---
```

### 2.3 Money: this month, by category, as a chart

```markdown
---
collection: this-month
title: This month
from: transactions
where: month(date) = month(today()) and year(date) = year(today())
group: category
total: sum(amount)
views:
  table: date, payee, amount, category
  chart: sum(amount) by category
---
```

### 2.4 Workouts: sets with a computed volume

```markdown
---
kind: sets
name: Set
folder: Workouts/Sets
editable: outside
fields:
  exercise: link(exercises), required
  weight: number
  reps: number
  date: date, default today
  volume: = weight * reps
row: exercise, weight, reps
create: exercise, weight, reps
---
```

```markdown
---
collection: weekly-volume
title: Weekly volume
from: sets
group: exercise
total: sum(volume)
views:
  chart line: sum(volume) by week(date)
---
```

### 2.5 Workouts: today's session from a repeating plan

```markdown
---
kind: sessions
name: Session
folder: Workouts/Plan
editable: outside
repeat: repeat
fields:
  name: text, required
  repeat: repeat
  next: = next(repeat)
row: name, next
---
```

`where: next = today()` in a collection gives "Today's workout"; the server
re-runs it at the person's midnight (round 22).

### 2.6 Mail: the inbox (operations only)

```markdown
---
kind: messages
name: Message
folder: Mail
editable: operations
fields:
  subject: text
  from: text
  to: text
  received: time
  unread: yes/no, default yes
  archived: yes/no, default no
  thread: link(threads)
row: from, subject, received
actions:
  archive: set archived = yes, swipe
  mark read: set unread = no, when unread, menu
---
One file per message, raw message beside it. The server holds the IMAP
connection and raises an event for new mail.
```

```markdown
---
collection: inbox
title: Inbox
from: messages
where: not archived
sort: received desc
views:
  list: from, subject, received
  compact: subject
---
```

### 2.7 Calendar: events this week

```markdown
---
kind: events
name: Event
folder: Calendar
editable: outside
repeat: repeat
fields:
  title: text, required
  starts: time, required
  ends: time
  repeat: repeat
  where: place
  people: people
row: title, starts
create: title, starts, ends
---
```

```markdown
---
collection: this-week
title: This week
from: events
where: week(starts) = week(today())
sort: starts
views:
  calendar: starts
  timeline: starts .. ends
---
```

The server expands repeating events inside the queried range (round 22).

### 2.8 Tasks: Today and Overdue

```markdown
---
kind: tasks
name: Task
folder: Tasks
editable: outside
repeat: repeat
fields:
  title: text, required
  when: date
  deadline: date
  done: yes/no, default no
  hours: number
  project: link(projects)
  repeat: repeat
  late: = deadline < today() and not done
row: title, when, deadline
create: title, when
actions:
  done: set done = yes, when not done, swipe
---
```

```markdown
---
collection: today
title: Today
from: tasks
where: not done and (when <= today() or deadline <= today())
sort: deadline asc
views:
  list: title, deadline
---
```

### 2.9 Tasks: a project's remaining hours

```markdown
---
kind: projects
name: Project
folder: Tasks/Projects
editable: outside
fields:
  name: text, required
  remaining: = sum(from(tasks.project, not done).hours)
  status: = if(remaining = 0, "Done", "Open")
row: name, remaining, status
---
```

### 2.10 Notes: grouped by tag, and linked here

```markdown
---
collection: by-tag
title: Tags
from: notes
group: tags
total: count
views:
  list: title, updated
---
```

A note's page shows "Linked here" as a collection with `where: links to
this`; `this` is the open item.

### 2.11 Files: recent and large

```markdown
---
collection: recent-files
title: Recent
from: files
where: days(updated, today()) <= 7
sort: updated desc
total: count, sum(size)
views:
  table: name, size, updated
---
```

`updated`, `created`, `size`, `name` and `path` are host fields every item has
and no kind declares.

### 2.12 Documents: by correspondent

```markdown
---
kind: documents
name: Document
folder: Documents
editable: outside
fields:
  title: text, required
  correspondent: link(correspondents)
  date: date
  scan: file
  tags: tags
row: title, correspondent, date
---
The original scan is untouched; this file is its sidecar. OCR text is the body,
written by a background job.
```

```markdown
---
collection: by-correspondent
title: By correspondent
from: documents
group: correspondent
total: count
views:
  table: title, date
---
```

### 2.13 Photos: gallery and timeline

```markdown
---
collection: photos-2026
title: 2026
from: photos
where: year(taken) = 2026
sort: taken desc
views:
  gallery: image
  timeline: taken
---
```

(`map: where` arrives with Photos.)

### 2.14 A mixed board for a trip

```markdown
---
collection: lisbon
title: Lisbon trip
fields:
  status: choice(To do|Doing|Done)
views:
  board: status
  list: title, status
---
- [Flight confirmation](<../../Mail/2026-11-02-tap.md>) status: Done
- [Packing list](<../../Notes/packing.md>) status: Doing
- [Book the hotel](<../../Tasks/hotel.md>) status: To do
```

The status lives in this file beside each member, so the mail is Done here and
can be To do on another board (D7).

### 2.15 The skills box

A skill follows the existing `SKILL.md` convention: a folder per skill
holding `SKILL.md`, whose frontmatter has `name` and `description` and whose
body is the instructions. The kind says each item is a folder:

```markdown
---
kind: skills
name: Skill
folder: Skills
item: folder with SKILL.md
editable: outside
fields:
  name: text, required
  description: text, required
  used by: tags
row: name, description
create: name, description
---
Each folder is one skill in the SKILL.md convention. Agents read skills from
here when a person hands them this folder.
```

```markdown
---
collection: skills
title: Skills
from: skills
sort: name
views:
  list: name, description
---
```

## 3. Reading List, rewritten

```markdown
<!-- plugin.md -->
---
plugin: calternal-reading-list
name: Reading List
version: 2.0.0
icon: book
sends out: nothing
---
Save links to read later, with an optional reminder.
```

```markdown
<!-- kinds/reading.md -->
---
kind: reading
name: Reading
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
card: title, site
create: url, title
actions:
  mark read: set read = yes, when not read, swipe
---
Links saved to read later. One file per link.
```

```markdown
<!-- collections/unread.md -->
---
collection: unread
title: Unread
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
---
- [Unread](<../collections/unread.md>)
- [All](<../collections/all.md>)
```

The reminder is the only part that needs code: an operation listening to the
time in `remind` (an event, round 21):

```ts
// operations.ts
export const remind = on.time("reading.remind", async (item, s) =>
  s.notify(s.me, { item, text: "Time to read this" }));
```

| | Today | In this format |
| --- | ---: | ---: |
| Plugin | 226 lines of TypeScript (`src/plugin.ts`) | 51 lines of Markdown in five files, plus 2 lines of TypeScript for the reminder |
| Without the reminder | — | 50 lines of Markdown (no `remind` field), no code, no process: a description-only plugin any member may add |

Counted as non-blank lines, without the file-name comments.

Today's file declares a second kind for reminders, three operations in JSON
Schema, a reminder job, indexed-field audiences and the whole interface
description. Here the reminder is a field and one event, add, change and
remove come free, and the interface comes from the looks and collections.

## 4. Compared with Obsidian Bases

Read with Firecrawl on 2026-11-24 from
[help.obsidian.md/bases/syntax](https://help.obsidian.md/bases/syntax) and
[help.obsidian.md/bases/views](https://help.obsidian.md/bases/views). A
`.base` file is YAML holding `filters`, `formulas`, `properties`,
`summaries` and `views`; by default it covers every file in the vault;
formulas use method chains (`price.toFixed(2)`, `file.hasTag("book")`);
views are table, cards, list, kanban and map.

| | Obsidian Bases | Here | Why we differ |
| --- | --- | --- | --- |
| File | A `.base` YAML file | A Markdown file with frontmatter | Hand-added items and a description live in the body; everything stays Markdown (owner) |
| Types of items | None; any note with matching properties | Kinds declared in their own files | Kinds carry looks, the create form, the outside-editing switch and `AGENTS.md`; Bases has nowhere to put them |
| Scope | Every file in the vault unless filtered | `from:` names kinds | A query over one kind is fast and its fields are known |
| Filters | `and`/`or`/`not` objects of strings | One condition line | Easier to write by hand; the same language as formulas |
| Formulas | Methods on values (`.toFixed(2)`), `file.`, `note.`, `formula.` prefixes | Spreadsheet functions, plain names | The owner asked for spreadsheet style; prefixes are noise when fields are declared |
| Computed fields | Only in the base | In kind files and in collection files (round 25) | A kind's computed field is part of every view of that kind |
| Summaries | Many named summaries per column | Five totals grouped by one field (round 22) | Anything more is the plugin's own cache |
| Rollups across links | Not built in | `sum(from(kind.field).f)`, one hop | Budgets and projects need them; one hop keeps the cost bounded |
| Views | Each a YAML object with options | One line per view | Readable at a glance; the first line is the default |
| Hand-picked items | Not supported (filters only) | A list of links in the body | Collections are rule plus hand (decided) |
| Per-board status | Not supported | Member fields in the collection's body | Mixed boards (D7) |
| Map | Map layout with the Maps plugin | Comes with Photos | Owner's order |

## 5. Open points in this format

| Point | Why it is open |
| --- | --- |
| The word people see for a collection | Not chosen (round 19) |
| Whether `item: folder with SKILL.md` generalises to other folder-shaped items | Only the skills box needs it so far |
| The `money` type's currency handling across accounts in different currencies | No exchange-rate rule is decided; totals refuse to add different currencies |
| The step budget per formula | A number to measure, not to choose |
