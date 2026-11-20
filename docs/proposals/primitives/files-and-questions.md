# Files, records, questions and collections · design section · 2026-11-21

Status: a design lane's section for the night of 2026-11-20/21 (issue #107). It
covers primitives 1 (Files) and 2 (Questions) of
`docs/proposals/primitives-only-thesis-2026-11-21.md`. Nothing here is decided;
section 7 holds the choices that are the owner's.

Sources: the thesis (`origin/dev`), the contract owner's draft
`docs/proposals/primitives-only-host-2026-11-21.md` (`origin/lane/v2-host-base`,
called "the host draft" below), `docs/morning-2026-11-20.md`, the old app's ADRs
0043 (files are the truth), 0056 (block IDs), 0062 (storage tiers), 0068
(frontmatter), the window engine (`origin/lane/window-engine`) and the item-files
lane's final report (thread `thr_wdjuw3bi6e`).

## 0. The short version

- An item is a Markdown file. The host writes it in one of four ways: replace the
  file, set frontmatter fields, append a record, edit or redact one record.
- A record is a block that starts with a header line the host writes:
  `**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9`. The anchor is the old app's
  block ID (ADR 0056). The file stays plain Markdown that any editor can open.
- The **origin stamp** (person, plugin, run, time, hash) does not live in the
  file. It lives in the host's history ledger, which a plugin, a file share or
  another editor cannot write. The name in the header is a readable copy.
- An append costs **2 indexed reads and 2 flushes, independent of the file's
  length**. Today it costs about 11 indexed reads, a full body read, a full file
  read and hash, and three commits.
- The index (appdata) holds files, declared fields, records, links and text. It
  is rebuilt from the files **plus** the history ledger. Every list, section,
  board, search and backlink list is one call: a live window over a query.
- Links stay ordinary relative Markdown links. A rename is visible at once (one
  commit; old paths resolve through an alias); referring files are rewritten in
  the background from the link index. The host is Markdown-aware, not
  app-aware.
- A collection is a Markdown file: a query in frontmatter, hand-added items as
  a list of links in the body. A sidebar is a collection whose items are
  collections. Hand-added items follow renames for free, because they are links.
- Per-person state (read positions, drafts, pin order, a person's sidebar edits)
  is a small file per person and plugin in a hidden folder, buffered in memory
  and written at most every few seconds.
- What an app author learns in this area: **file, record, kind, field, link,
  query, window, collection, person state** (nine words; collection and sidebar
  are files, not extra machinery).

## 1. The primitives

### 1.1 Storage classes

The thesis names two classes. Three exist, and the third must be named so that
nobody stores the only copy of something in a rebuildable index.

| Class | What | Where (ADR 0062) | Rebuildable | Bound |
| --- | --- | --- | --- | --- |
| Userdata | Items, collections, sidebars, per-person state, attachments | Space tree; claimed text on the SSD root, other files on the HDD root | No: it is the truth | Space quota (bytes, files) |
| Host ledger | History (revisions and record deltas), origin stamps, request receipts, trash origins | SSD state, one SQLite database per space, backed up with userdata | No | History retention (operator default), receipts 7 days |
| Appdata | Index, search, link table, caches, window state | SSD state, one SQLite database per space (WAL) | Yes, from userdata + ledger | Derived; size follows userdata |

Auth, grants and the work table also belong to the host ledger class; they are
other sections' concern.

### 1.2 Files

A file has a stable ID (UUIDv7) that survives rename and move, a path, a kind, a
revision and a size. Paths follow the portable-name rules in `crates/fsio`
(`names.rs`). The ID lives in the frontmatter (`id:`) for Markdown and in the
ledger for other files.

| Operation | Inputs | Result | Fails with | Request path cost |
| --- | --- | --- | --- | --- |
| `file.read` | ID or path, optional byte range | bytes, revision | `not_found`, `forbidden` (same answer for both to a reader who may not know it exists) | 1 indexed read (head) + the file read |
| `file.create` | folder, name, kind, fields, body, request ID | ID, revision | `conflict_name`, `invalid_fields`, `quota` | 2 indexed reads (receipt, name), 1 ledger commit, 1 file write (temp, fsync, rename, directory fsync) |
| `file.replace` | ID, expected revision, fields, body, request ID | revision | `stale_revision`, `invalid_fields`, `quota` | 2 indexed reads (receipt, head), 1 ledger commit, 1 file write; the host parses the new bytes once |
| `file.set_fields` | ID, optional expected revision, field values | revision | `invalid_fields`, `stale_revision` | as replace + 1 file read; unknown keys, comments and untouched values survive (ADR 0068) |
| `file.move` | ID, new folder and/or name | revision | `conflict_name`, `forbidden` | 1 indexed read, 1 ledger commit, 1 rename + 2 directory fsyncs; links: see 1.7 |
| `file.trash` | ID | trash entry | `forbidden` | 1 commit, 1 rename into the space's Trash |
| `file.restore` | trash entry | ID, path | `conflict_name` (restores beside, never overwrites) | 1 commit, 1 rename |
| `file.purge` | trash entry | — | `forbidden` (managers and owners, or retention) | background |
| `history.list`, `history.read` | ID, revision | revisions with stamps, bytes | `forbidden`, `redacted` | indexed ledger reads only |
| `change.revert` | change ID | new change ID | `stale_revision` on any file the change touched | as many replaces as files in the change, one commit |

A **change ID** groups every revision one operation made (moving three notes is
one change). Undo is `change.revert`: it writes the previous state back as new
revisions. There is no per-operation undo code in the host (agrees with host
draft question 5).

Write order is the old app's: ledger before file (ADR 0062, "history-before-file").
After a crash, recovery finds heads whose file is behind the ledger and writes the
file from the ledger. That is today's `mat_state = 'pending'` path in
`crates/content/src/db.rs`, kept.

### 1.3 Records

A **record file** is a Markdown file of a kind that declares `records: true`. It
has frontmatter and then records. Each record begins with a header line that the
host writes and ends where the next header line begins:

```markdown
---
id: 019a7c3e-55d1-7c2a-9f3e-0c4b2a1d9e77
kind: chats
format: calternal-chat/v2
---

**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9

Can we move the trip to Friday?

**Sam** · 2026-11-20 17:04 ^a8dp2c9xq1m4z7rt

[↩ Lee, 17:02](<#^k2m8q1w4zre7k3f9>)
Friday works.

**Lee** · 2026-11-20 17:05 ^b3rt6w2mzq8k1p0d

*deleted*
```

- The header shape is fixed by the host: display name, UTC time, anchor. The
  plugin never writes a header line. If a record body contains a line that
  looks like a header, the host escapes it (a leading `\`) on write.
- The anchor is a 16-character Crockford ID, the same module as Notes block IDs
  (ADR 0056). Editors hide it (owner decision for chat files).
- The header is a **readable copy**. The authority is the stamp in the ledger.
- Anchors are unique across the space (ADR 0056), so the index resolves any
  `#^anchor` to its current file even when the file part of a link is stale
  (a record in last month's segment, a renamed chat).
- **Crash between ledger and file:** the ledger commit carries the expected file
  length before the append. Recovery appends the record if the file still has
  that length and skips it if the record is already there, so an append lands
  exactly once.

| Operation | Inputs | Result | Fails with | Request path cost |
| --- | --- | --- | --- | --- |
| `record.append` | file ID, body, optional record fields, request ID | anchor, sequence number | `forbidden`, `too_large` (record cap), `segment_full`, `quota` | **2 indexed reads** (receipt, head), **1 ledger commit** (delta + stamp + receipt + journal), **1 append write with fsync**; no body read, no hash of the file |
| `record.edit` | file ID, anchor, expected record revision, body | record revision | `stale_revision`, `forbidden` (kind policy) | 2 indexed reads, 1 commit, 1 file rewrite (cost grows with the file, so record files have a segment cap) |
| `record.redact` | file ID, anchor | — | `forbidden` (kind policy) | as edit, plus scrubbing the record's bytes from the ledger (1 indexed update) |
| `record.read` | file ID, anchor range or sequence range | records with stamps | `forbidden` per record (agent consent, section 3) | indexed reads + one ranged file read |

A kind declares who may edit and redact a record: `author` (only the person or
run the stamp names), `author-or-manager`, or `writers` (anyone who may write
the file). Time windows such as Chat's 48 hours stay in the plugin.

**Stamp** = `{file, anchor, record revision, person, plugin, run or none, time,
canonical hash}`. The canonical hash is computed over the record body with every
internal link destination replaced by its target ID. A host link rewrite after a
rename therefore never breaks a stamp.

**Group commit:** appends to one space queue on that space's single writer
(the existing per-space writer). Concurrent appends share one ledger commit and
one fsync per file. There is no installation-wide lock.

**Segments:** a record file has a cap (operator default, proposed 4 MiB). An
append past the cap fails with `segment_full`; the plugin starts the next file
(Chat: `Chats/<chat>/2026-12.md`). This keeps `record.edit` and `record.redact`
bounded and keeps every file pleasant to open.

### 1.4 Append cost, today and in this design

| Step | Today (`item_engine` on `lane/v2-host-items`) | This design |
| --- | --- | --- |
| Indexed reads | about 11 | 2 (receipt, head); the grant is checked from the session's cached grant set |
| Body read | the whole current body | none |
| File read and hash | the whole file on disk | one `fstat` compared with the head's size, mtime and inode |
| Commits | 3 synchronous | 1, shared with concurrent appends |
| File flush | the whole new body | the appended bytes, one fsync |
| Grows with file length | yes | no |
| Index update | in the save path | after the save, from the journal (owner agreed, 2026-11-20 question 3) |

The `fstat` check is how outside edits are noticed cheaply: if size, mtime or
inode differ from the head row, the host takes the slow path (1.5) before
appending. The item-files lane's numbers are from reading the code, not a
measurement; the new numbers are a design target to be measured on the perf VMs.

### 1.5 Edits in another editor or over a file share

The host watches both roots and rescans after missed events or restart (ADR 0062
T5, ADR 0043). A changed file is parsed once:

| Case | What the host does |
| --- | --- |
| Record bytes unchanged | Stamp stays verified. |
| Record body changed | New record revision; stamp gains "edited by X". X is the authenticated person for WebDAV, File Provider and `calternal mount` (they go through the host), or `outside` for direct disk access. |
| New header line without a stamp | New record authored by X or `outside`. |
| Header line removed | The record is deleted; the ledger keeps it. |
| Same anchor twice | Keep both, re-key the second (ADR 0056 B4). |
| Frontmatter fails the kind's schema | The file is kept as written; the index marks it invalid and shows it with a warning. An outside edit is never refused or "repaired". |
| Header name edited by hand | Ignored for authority; the next host write of that record restores it. |

### 1.6 Kinds

A kind is declared by one plugin. It is a schema over frontmatter plus how the
kind looks as a row, a card and a cell (drawing is another section).

```yaml
kind: reading
fields:
  url:   { type: url, required: true }
  title: { type: text, index: sort }
  read:  { type: bool, default: false, index: filter }
records: false
```

Field types: text, number, bool, date, datetime, url, link, list of these,
person. `index: filter | sort | text` says which fields the index keeps; a query
may only use indexed fields (1.8). A `link` field is indexed and rewritten on
rename like body links. A `person` field holds a person's UUID; the host shows it
only to readers who share a group with that person (ADR 0060). Common fields
every file has: `id`, `kind`, `title`, `created`, `modified`, `folder`,
`author` (from the stamp of the first revision).

Bounds: at most 32 fields and 8 indexed fields per kind (proposed operator
defaults); a kind's declaration changes only with a plugin update, which starts a
bounded background reindex of that kind.

### 1.7 Links, backlinks and rename

Links stay standard relative Markdown links with angle brackets
(`[Plan](<../Trip/Plan.md>)`, ADR 0024/0056), plus `#^anchor` for a block or
record. The host parses Markdown with one parser (for example `pulldown-cmark`,
whose offset iterator gives the byte range of every link:
<https://docs.rs/pulldown-cmark/latest/pulldown_cmark/>). The index keeps
`links(source, source_anchor, target_id, target_anchor, byte_range)`.

A rename or move:

1. **Foreground (visible at once):** rename the file or folder, write the new
   path and a departed-path row `old path → ID` (one row per folder for a folder
   move; this is ADR 0040's `departed` history row, kept in the ledger), one
   ledger commit. Cost: 1 indexed read, 1 commit, 1 rename, 2
   directory fsyncs, whatever the number of links. Any link with the old path
   resolves through the alias from this moment.
2. **Background:** one durable host job reads the referring files from the link
   index and rewrites only the recorded byte ranges, 8 files per commit (today's
   `crates/content/src/rename_job.rs` and `linked_write.rs`, generalized from
   Notes to every Markdown file). A moved file's own relative links are
   rewritten too. A referring file changed since indexing is re-parsed first;
   one changed outside during the job becomes a retained conflict (ADR 0062).
3. Link text that matched the old title follows the new title (ADR 0024).
   The departed row stays in history, so a link pasted from outside or made
   offline with the old path still resolves later (ADR 0040: one index lookup
   per path, never a walk of the tree).

Obsidian does the same thing (rewrite referring files on rename:
<https://obsidian.md/help/links>); the difference is that our step 1 makes the
rename correct before the rewrite finishes.

So the host can stay **app-agnostic** but not **format-agnostic**: it knows
Markdown links, frontmatter link fields and anchors, and nothing about chats or
notes. Non-Markdown files have no links.

Backlinks are a query (`links_to: <id>`), filtered by the reader's rights.

### 1.8 Questions (the index and live windows)

| Operation | Inputs | Result | Fails with | Request path cost |
| --- | --- | --- | --- | --- |
| `query.window` | query, start (cursor or anchor), size | rows (fields the kind's row needs), version, cursors, count if allowed | `unsupported_query` (a field that is not indexed, too many conditions), `too_large` | O(size) indexed reads in appdata, 0 file reads, 0 flushes |
| `query.watch` | a window | pushes of changed rows and counts | as above | one subscription per window; updates after the journal is published |
| `query.count` | query | number or `hidden` | | 1 indexed count |
| `search` | text, query to narrow | ranked rows with snippets | | FTS5 lookup, filtered before ranking |

Query shape (YAML in a collection file, JSON over the wire):

```yaml
query:
  kind: [reading]
  where: { read: false }
  sort: [-created]
  group: none
```

Conditions: `=`, `!=`, `in`, `<`, `>`, `exists`, `in_folder`, `links_to`,
`text`, `author`, `mine`. Proposed bounds: 8 conditions, 2 sort keys, window
of at most 200 rows and 128 KiB encoded (the window engine's current limits).

**Per-asker row values.** Unread counts need one thing the thesis does not have:
a value computed for the asking person. A record kind may declare
`position: <state key>`; the row then carries `after_position` (records after
the asker's stored anchor, excluding their own) and, if the kind declares a
`mentions` record field, `mentions_after_position`. Cost per row: 1 indexed seek
plus 1 indexed range count. This is the only join in the engine.

**Keeping live windows cheap.** The window engine today keeps a stored row set
per collection, and caps a note at 32 of them (`effective_scope_limit_exceeded`)
so a save stays inside two covering-key writes per scope. With saved queries
over any kind, that cap would bite at once. Proposal: a query window is read
straight from the kind's covering index (no stored row set); only hand-added
members are stored (they are links, already in the `links` table). After a save,
the publisher re-evaluates only *watched* windows whose query names the changed
kind, bounded by a per-space cap on open watches (proposed 256). Cost per save:
0 on the request path; in the background, one indexed probe per affected watched
window. This must be measured against the window engine's 40 ms save gate before
it replaces the stored row sets.

**Index tables** (appdata, per space, WAL): `files` (id, path, kind, revision,
size, mtime, inode, common fields, trashed), one generated table per kind with
its indexed fields and a covering index per sort, `records` (file, sequence,
anchor, stamp fields, state, declared record fields), `links`, `aliases`, FTS5
text. All rebuildable from userdata and the ledger. Publication is asynchronous
from the journal, as agreed for the window engine (about 2 ms behind a save at
p95).

### 1.9 Collections and sidebars

A collection is a Markdown file of kind `collection`:

```markdown
---
id: 019a7c40-…
kind: collection
title: Unread
query: { kind: [reading], where: { read: false }, sort: [-created] }
views: { big: list, small: compact }
---

- [The Rust book](<../Reading/The Rust book.md>)
- [Trip plan](<../Notes/Trip/Plan.md>)
```

- Members = hand-added links (in their written order) first, then the query's
  rows, without duplicates. Either part may be empty.
- Hand-added items are ordinary links, so renames keep them correct for free.
- A **sidebar** is a collection whose hand-added items are links to collection
  files, each with a small view. It is visible with the other collections.
- A collection grants nothing: each member is checked against the reader. A
  member the reader cannot read is left out (not shown as "hidden").

An app's **default arrangement** is a sidebar file the plugin ships. A person's
edits (order, hidden, renamed, added) are a per-person file (1.10) holding a
copy of that sidebar, made on first edit (see question 6).

### 1.10 Per-person state

| Operation | Inputs | Result | Cost |
| --- | --- | --- | --- |
| `state.get` | plugin-scoped key | value | memory, else 1 file read once per session |
| `state.set` | key, value | — | memory only; written to the file at most every 5 s (operator default) and when the person's last session closes |

The file is `.people/<person-uuid>/<plugin-id>.md` in the space: frontmatter
`key: value` lines, readable only by that person (and by nobody else, managers
included). Writes are a whole-file replace, coalesced: a busy chat moves the read
position many times a second; the file is written once per interval. A crash loses
at most one interval. Bounds: 64 KiB per file, 256 keys (proposed).

Keys used below: Chat's `read.<chat-id>`, `draft.<chat-id>`, `mute.<chat-id>`;
the shell's `pins` and `sidebar.<app>`; a device's `offline.<device-id>`.

### 1.11 Delete for everyone, history, devices

- **Delete for everyone** is `record.redact` (or `file.trash` then `file.purge`
  for a whole file). The record's body becomes the tombstone `*deleted*`; the
  ledger scrubs that record's bytes from every stored revision; the index drops
  it from text search; quotes of it draw nothing (they are links, 1.12).
- **History** stores record files as deltas (one row per appended or edited
  record), other files as full revisions or diffs. Retention is an operator
  default (see the host draft's question 1).
- **Devices** hold files (ADR 0043). Sync is per file by revision. A record file
  syncs by appended range: the device asks for bytes after its last known
  sequence, so syncing a long chat costs only the new records. A device offline
  proposes anchors for its appends (ADR 0056 B5); the host confirms them and
  stamps them on arrival, in arrival order. Replace-file conflicts keep both
  versions (ADR 0045). A per-person `offline.<device>` key lists collections or
  folders to keep downloaded.

### 1.12 Quotes

A quote is a link to a record: `[↩ Lee, 17:02](<Trip.md#^k2m8q1w4zre7k3f9>)`.
The client draws the quoted record with the reader's rights. The link text holds
author and time only, never copied words, so a redacted record leaves nothing
behind in other chats (owner decision: deleting removes its quotes elsewhere).
In another editor the quote reads as a link with a name and time.

### 1.13 Search

One FTS5 index per space in appdata, fed from the journal like the window index.
A kind declares weights for its text fields (`title: 3, body: 1`); record files
index each record separately, so a hit points at an anchor. Filtering by the
reader's rights happens before ranking and before snippets, so a rank or a
snippet never reflects a file the reader cannot open. An agent run's search
reaches only files every person in its chat may read (owner decision,
2026-11-20); for that the host intersects the readers' rights once per run, not
per query. Hidden blocks (ADR 0056) and redacted records are not indexed.

### 1.14 Storage tiers (ADR 0062)

| What | Root |
| --- | --- |
| Text of a claimed kind: notes, record files, collections, sidebars, per-person state | SSD userdata tree |
| Everything else: attachments, photos, unclaimed files | HDD userdata tree (same relative paths) |
| Host ledger (history, stamps, receipts, departed paths) | SSD state |
| Index, search, window state | SSD state, rebuildable |

- **A kind declaration is the claim.** ADR 0062 says text is placed by content
  signature and the format plugin's validation, never by folder or extension.
  Here a plugin's kind (frontmatter `kind:` plus the kind's schema check) is that
  validation; files that fail stay on the HDD tree and are reported, as today.
  Plugins never choose a root.
- Create, rename, move, trash and restore of a folder apply to both trees as one
  journaled job (ADR 0062 "mirrored real trees"); the rename job of 1.7 is that
  job.
- Record files sit on SSD, so the append's fsync is an SSD fsync. A chat with
  many attachments keeps its text on SSD and its pictures on HDD in the same
  folder path.
- Loss of either disk stops writes (ADR 0062 T-A1); nothing in this design
  changes that.

## 2. What plugins can do and cannot do

**Plugins can:**

- Declare kinds: which fields a file has and how it looks. *Example: Reading List
  declares `reading` with `url`, `title`, `read`.*
- Create, replace and set fields on files in folders their grant names. *Example:
  "Mark read" sets `read: true` without writing any code.*
- Append records to a file, and edit or redact records under the kind's rule.
  *Example: Chat appends each message; the author edits it within 48 hours.*
- Ask questions and get live windows that update themselves. *Example: an
  "Unread" list that shrinks as items are marked read.*
- Ship default collections and a default sidebar. *Example: Notes ships "All
  notes", "Tags" and "Snoozed".*
- Keep small private state for each person. *Example: Chat stores where each
  person stopped reading.*
- Link to any file or record; links follow renames. *Example: a note links to a
  chat message, and still works after the chat is renamed.*
- Read history and revert a change. *Example: Notes offers "Undo move".*

**Plugins cannot:**

- Write or read outside their grant, or outside the person's rights. *Example:
  Reading List cannot open `Chats/`.*
- Choose or fake who wrote something. *Example: a plugin cannot append a message
  that says it is from Sam when Lee sent it.*
- Write a record header or anchor themselves. *Example: a message body that
  contains a fake header line is escaped.*
- Read another person's private state. *Example: Chat cannot show Lee where Sam
  stopped reading, unless Sam's read receipts are on and Sam's client writes a
  shared receipt record.*
- Query fields their kinds did not declare as indexed, or run an unbounded query.
  *Example: "sort all files by word count" is refused unless a kind indexes it.*
- Learn that a file exists by counts, backlinks or search ranks when the person
  may not read it. *Example: a backlinks list never shows "1 hidden link".*
- Keep deleted words alive. *Example: a redacted message's bytes are gone from
  history, search and quotes.*
- Pick the storage disk. *Example: a photo goes to the HDD root whatever the
  plugin prefers.*

## 3. Security

| Attack | What stops it | Where it is checked |
| --- | --- | --- |
| Write outside the grant (path traversal, symlink, another kind's folder) | Path resolution beneath the space root; grant check on folder and kind | `crates/fsio/src/beneath.rs`; the one call check (Calls section) before the writer |
| Forge an author | The stamp comes from the call's authenticated person, plugin and run; the plugin passes no author | Host writer, inside the ledger commit |
| Fake a record by writing a header line into a body | The host escapes header-shaped lines on write | Host writer |
| Fake a record by editing the file over a share | The share is authenticated: the record is stamped with that person, never with the name typed in the header | Watcher and rescan path |
| Rewrite another person's record | Kind policy `author` / `author-or-manager`; even under `writers`, the stamp records the editor | Host writer |
| Fill the disk or flood appends | Space quota; record cap; segment cap; per-plugin call and write rates | Host writer, before the commit |
| Expensive query to stall the space | Only indexed fields; bounded conditions and windows; per-space query time budget | Query planner (window engine plans) |
| Learn hidden files through counts, ranks, backlinks or timing | Rows are filtered by the reader before counting and ranking; count mode `hidden` where a grant forbids counts | Index, in the query |
| Use a collection to reach a file | A collection grants nothing; each member is checked | Query, per member |
| Read another person's state file | Only the owning person may read `.people/<uuid>/`; plugins read it only for the acting person | Host reader |
| Feed others' messages to an agent | A run may read a record only if every person in its stamp chain has granted that run (Permissions section); records by `outside` are withheld | Host reader, per record |
| Use the rename job as a confused deputy | The host computes and writes only link destination bytes in indexed ranges; no plugin input reaches the job | Rename job |
| Keep deleted words in history or search | Redact scrubs ledger rows and FTS; quotes are links | Host writer and index |
| Corrupt the index to change answers | The index is rebuildable and is never used for authority; grants and stamps come from the ledger | Design rule |

**Where I am not sure it holds:**

1. **Direct disk edits** (an administrator, a backup restore, a sync tool on the
   server) are stamped `outside`. If the space owner edits their own files with a
   local tool on the server, their own messages become `outside` and are withheld
   from agents. Acceptable, but surprising (question 5).
2. **Redaction versus copies outside the host:** backups keep the bytes until
   they rotate; a device that is offline keeps them until it syncs; a modified
   client can keep them forever. Telegram has the same limit. The design cannot
   promise more than "gone from every copy the host controls".
3. **Rename reveals a new name.** When Sam renames a note, Lee's note that links
   to it is rewritten, so Lee sees the new path in their own file even if Lee
   can no longer read the target. Only names leak, not contents.
4. **Link rewrite into files the renamer cannot write** (question 3) is a host
   write that no person asked for in that file.
5. **Timing:** a query over many unreadable rows takes longer than one over few.
   Filtering inside the index helps; it does not make timing constant.
6. **Kind ownership:** two plugins declaring the same kind name must be refused
   at install; otherwise one could reinterpret the other's files. This belongs to
   the Packages section and must be checked there.

## 4. Was and is

| Area | Dev today | Revision 14 | This design |
| --- | --- | --- | --- |
| Truth | SQLite `versions` holds every revision's full bytes; files are materialized after (`mat_state`) | the same, plus spans and staged saves | Files are the truth; the ledger holds deltas and stamps |
| Append | Notes have none; v2 `item_append` reads and rewrites the whole body | `file_append` catalogue rule with attempt fences, receipts, attested framing | `record.append`: 2 reads, 2 flushes, O(record) |
| Authorship | `versions.author` per revision | attested prefix/suffix, `item_spans`, authorship triggers, `consent_only` | Stamp per record in the ledger |
| Per-person state | none generic | viewer-private record, `viewer_item_state` | `.people/<uuid>/<plugin>.md`, buffered |
| Rename and links | Notes-only rename job with backlinks | generic relocate + backlink index | the same job generalized; alias rows make it instant |
| Lists | `notes.list`, collections table | Bind sources, `indexed_fields`, extractors, search profiles, qualified plans | Kinds' indexed fields + one query shape + live windows |
| Collections | `collections` table in the content database | layout sections, `Section.repeat` | Collection files; sidebar = collection of collections |
| Search | Notes search and a separate message search | search profiles | One FTS5 index with field weights per kind |
| Quotes | — | checked spans copied as bytes | Links to record anchors |
| Trash | `trash.rs`, `trash_job.rs` | the same | the same, any kind |

**Deleted:** `item_spans.rs`, `item_engine/attested.rs`, `item_engine/viewer.rs`,
most of `item_engine/stages.rs` (keep upload staging for large replaces), full
body copies per revision for record files, Bind sources, extractors, search
profiles, the separate message search schema (`search/messages/`), the
`collections` table, `notes_rename_*` as Notes-only tables.

**Survives (paths on `origin/dev` unless noted):**

- `crates/fsio/` (beneath, names, atomic writes, bulk, usage): the file layer.
- `crates/content/src/journal.rs`, `recovery.rs`, `db.rs` (head and outbox,
  slimmed), `linked_write.rs`, `rename_job.rs`, `trash.rs`, `trash_job.rs`:
  generalized from Notes to every kind.
- `crates/content/src/search/` (minus `messages/`): one FTS index.
- `crates/content/src/window/` and `window_index.sql` on `origin/lane/window-engine`:
  the live window engine, its sidecar and asynchronous publication. Its
  "qualified plan" idea becomes "a query over declared indexed fields".
- The ADR 0056 block-ID module (wherever the Notes port keeps it): anchors.
- The per-space single writer from slice 2 (`lane/v2-host-items`).

## 5. Worked examples

The SDK shape below is a sketch (`defineKind`, `defineOperation`, `host.*`); the
SDK section owns the final names.

### 5.1 Reading List (no code)

```yaml
# manifest.yaml
id: reading-list
kinds:
  reading:
    fields:
      url:   { type: url, required: true }
      title: { type: text, index: sort }
      read:  { type: bool, default: false, index: filter }
    row: { title: title, subtitle: url }
grant: { folders: [Reading/], kinds: [reading] }
operations:
  add:       { create: reading, folder: Reading/, from: [url, title] }
  mark-read: { set_fields: { read: true } }
sidebar: [collections/unread.md, collections/all.md]
```

`collections/unread.md` is the collection file from 1.9. "Add" is one
`file.create`; "Mark read" is one `file.set_fields`; the Unread window updates
itself about 2 ms later.

### 5.2 Notes

```ts
defineKind("notes", {
  fields: { tags: { type: "list", of: "text", index: "filter" },
            snooze_until: { type: "datetime", index: "sort" } },
  text: { body: 1.0, title: 3.0 },          // search weights
});
// The editor component is bound to the file: saves are file.replace with the
// expected revision. Links and backlinks need no plugin code:
const backlinks = host.query.window({ links_to: note.id, sort: ["-modified"] }, { size: 50 });
await host.file.move(note.id, { name: "Trip plan (final).md" }); // links follow
await host.change.revert(changeId);                               // Undo
```

Collections "All notes", "Tags" (`group: tags`) and "Snoozed" ship as files.

### 5.3 Chat

```ts
defineKind("chats", {
  records: true,
  record: { fields: { mentions: { type: "list", of: "person", index: "filter" } },
            edit: "author", redact: "author-or-manager" },
  position: "read",                 // per-person key holding the last read anchor
  segment: "monthly",               // Chats/<chat>/2026-11.md, 2026-12.md, ...
});

defineOperation("chat.send", async (ctx, { chat, text, quote, requestId }) => {
  const body = quote ? `[↩ ${quote.label}](<#^${quote.anchor}>)\n${text}` : text;
  return ctx.host.record.append(chat, body, { mentions: parseMentions(text) }, requestId);
});

defineOperation("chat.edit", async (ctx, { chat, anchor, rev, text }) => {
  const r = await ctx.host.record.read(chat, { anchor });
  if (Date.now() - r.stamp.at > 48 * 3600e3) throw ctx.error("edit_window_closed");
  return ctx.host.record.edit(chat, anchor, rev, text);   // host checks author
});

defineOperation("chat.delete_for_everyone", (ctx, { chat, anchor }) =>
  ctx.host.record.redact(chat, anchor));

// Read position: buffered, written to .people/<me>/calternal-chat.md
ctx.host.state.set(`read.${chat}`, lastVisibleAnchor);
```

- **People and agents:** an agent's reply is `record.append` by the run; the
  stamp says `run` and the person who called it in. The header reads
  `**Claude (for Lee)** · …`.
- **Consent:** the run's `record.read` returns only records whose stamp chain
  is granted to that run; the rest come back as `withheld` (Permissions section).
- **Unread:** the chat list's row shows `after_position` and
  `mentions_after_position` from 1.8, computed for the asker.
- **An agent run that survives a restart:** the run appends its working log to
  its own record file in the harness's folder (host draft question 4), each chunk
  with a request ID derived from run ID and chunk number. While the server is
  down the machine keeps appending locally; on return it syncs the tail by range
  and the host applies each chunk once (receipts). Chat's reply append uses the
  request ID `<run>/reply`, so a retried reply is applied once.

## 6. Where the thesis is wrong or incomplete

| Thesis says | Problem | Changed to |
| --- | --- | --- |
| "Stamps where a plugin cannot forge it" | A file share or another editor can also forge anything in the file | Stamps live in the host ledger; the file has a readable copy |
| Userdata files + rebuildable appdata | History, stamps, receipts (and auth, grants, work) are neither | A third class, the host ledger (1.1) |
| "The index can always be rebuilt from the files" | Not the stamps | From files plus the ledger |
| Two writes: replace and append | Edit and delete for everyone need in-place record changes; the no-code list app needs a field patch | Four writes: replace, set fields, append, record edit/redact |
| The host "knows nothing" about formats | Link rewriting needs Markdown parsing | The host is Markdown-aware and app-agnostic |
| "Lists, sections, boards, search, backlinks are all the same engine" | Unread counts need a per-asker value; arbitrary fields cannot be fast | One query shape over declared indexed fields, plus one join: "after my position" |
| Per-person state "is a small file" | Written every few seconds by every reader; needs batching and a loss window | `state.set` buffered, one write per interval |
| Delete for everyone not covered | History and search would keep the words | `record.redact` scrubs ledger rows and the text index |
| Record files may grow without end | Edit cost and readability grow with the file | Segment cap; the plugin rolls over |
| Collections listed as part of primitive 2 | They need no host machinery beyond a file kind and the query engine | A collection is a built-in kind; primitives 1 and 2 stay separate (writing and asking have different costs and checks), but nothing else is needed in this area |
| The host draft's question 3 recommends keeping every version of a record in the file | Deleted words would stay readable in the file; the file reads badly | In-place edit, old versions in the ledger (question 2) |

## 7. Questions for the owner

### Q1. Where does "who wrote this" live?

- **What:** every message, note revision and agent reply needs a trustworthy
  record of who wrote it (person, plugin, run). The file shows a name, but anyone
  who can edit the file can change that name.
- **Why now:** agents' consent, edit-own-only, and delete rules all rest on it,
  and it fixes the file format of every append-style app.
- **Example:** Lee opens a chat file in another editor over the file share and
  changes Sam's name to their own on a message.
- **Options:**
  - (a) **A host ledger beside the files** (SSD, backed up, like today's history):
    the file stays clean; the ledger notes that Lee edited the record. Cost: a
    third storage class that is neither a file nor rebuildable, and a restore
    must bring files and ledger back together.
  - (b) **A signed marker in the file** (a hidden signature after each anchor):
    files alone are the whole truth. Cost: noise in every file, every outside
    edit breaks the signature, and the key must never leave the installation.
  - (c) **A host-owned stamp file per file** under a hidden folder: it is a file
    (fits "userdata is files"). Cost: twice the files, two writes per append,
    and it must be protected from shares and plugins anyway, so it behaves like
    (a) with more I/O.
- **Recommendation:** (a). History is already this kind of store (ADR 0062:
  "durable state, not rebuildable"); stamps are one more column. It keeps the
  append at two flushes.

### Q2. Message edits: rewrite in place, or append the new version?

- **What:** how an edited or deleted message looks on disk.
- **Why now:** it is the shape of every record file; the host draft recommends
  the other option.
- **Example:** Lee edits a message twice, then deletes another one for everyone.
- **Options:**
  - (a) **In place:** the file shows the latest text; old versions live in
    history (shown to the author only); a delete leaves `*deleted*`. Readable,
    and deleted words leave the file. Cost: an edit rewrites the file, so files
    are segmented (Chat: one file per month).
  - (b) **Appended versions:** each edit is a new record pointing at the old
    one. Appends only. Cost: every version, including deleted words, stays
    readable in the file, and reading the file in another editor shows all
    versions.
- **Recommendation:** (a), because the owner decided that a deleted message
  "vanishes" and that the product is an interface over readable files.

### Q3. When a rename rewrites links in files the renamer cannot edit, is that allowed?

- **What:** renaming updates every link to the file. Some linking files belong to
  other people, who may not have given the renamer write access.
- **Why now:** it decides whether "links follow every rename" can always hold.
- **Example:** Sam renames a shared note; Lee's private note links to it.
- **Options:**
  - (a) The host rewrites only the link destination in Lee's file, recorded in
    history as "link updated after a rename"; Lee's authorship is unchanged.
    Links always work; Lee's file changes without Lee acting, and Lee sees the
    new name.
  - (b) Only files the renamer may write are rewritten; the rest keep resolving
    through a permanent alias. No surprise writes; aliases pile up and Lee's file
    shows the old name in other editors.
- **Recommendation:** (a). It is what Obsidian users expect, and the host writes
  only bytes the link index recorded, never content.

### Q4. Does delete for everyone also remove the words from history?

- **What:** history normally keeps every version so that Undo and restore work.
- **Why now:** without scrubbing, "restore the previous version" brings a deleted
  message back, and managers could read it in history.
- **Example:** Sam deletes a message with a password in it.
- **Options:** (a) scrub the message's bytes from history and search, keep only
  "deleted by Sam at 17:05"; it cannot be undone after the undo window. (b) keep
  it in history, visible to the author only. Recoverable; the words stay on the
  server.
- **Recommendation:** (a), with Undo within a few seconds handled by the client
  before the redact is sent. State plainly that backups keep the bytes until they
  rotate.

### Q5. Who is the author of bytes changed outside Calternal, and may agents read them?

- **What:** files can be edited by tools that are not Calternal: over the file
  share (we know the person), or directly on the server's disk (we do not).
- **Why now:** it decides what an agent sees and whether outside editing stays a
  first-class way to work.
- **Example:** the owner fixes a typo in a chat with Vim on the server; that
  message is now "edited outside".
- **Options:**
  - (a) Share edits are stamped with the person; disk edits are `outside` and
    are withheld from agents until a person re-saves them in Calternal. Safe;
    outside disk edits feel second-class.
  - (b) Disk edits are stamped with the space owner. Simple; anyone with server
    disk access can then speak as the owner to agents.
- **Recommendation:** (a). Disk access already means administrator trust, but
  attributing it to the owner would let a backup tool or a sync script consent
  on the owner's behalf.

### Q6. A person's changes to an app's sidebar: a copy or a list of changes?

- **What:** an app ships a default sidebar; a person reorders, hides and renames
  its sections.
- **Why now:** it decides what happens when the app's default changes in an
  update.
- **Example:** Chat adds a "Bots" section in a later version; Lee had reordered
  Chat's sidebar.
- **Options:** (a) **a copy**, made on first edit: simple, readable, but Lee
  never sees "Bots" until they add it. (b) **a list of changes** over the
  default (order, hidden, renamed): new sections appear; merging rules are
  needed and the file is less obvious to read.
- **Recommendation:** (b), with new sections appended at the end and a short
  notice; the change list is a small readable file.

### Q7. Obsidian-style `[[wiki links]]` in imported files: read them, or only standard links?

- **What:** Calternal writes only standard Markdown links and never wiki links
  on disk (ADR 0024, settled). It does not settle what happens to wiki links
  already in files that come from elsewhere. Many Obsidian vaults use `[[Note]]`.
- **Why now:** rename-follows-links must cover whatever links are in the files.
- **Example:** someone imports an Obsidian vault of 3,000 notes.
- **Options:** (a) index and rewrite both forms, write only standard links:
  imported vaults keep working; the parser knows two forms. (b) convert wiki
  links to standard links on import: one form; the vault's files change on
  import. (c) ignore wiki links: simplest; imported links do not follow renames.
- **Recommendation:** (a): it keeps outside files untouched and costs one more
  pattern in one parser.

## Simplification opportunities

- One write path: `file.*` and `record.*` share the receipt, head, ledger commit
  and journal; there is no staged-save variant except upload staging.
- One query shape for lists, sections, boards, search, backlinks and collection
  members; the window engine's per-plan qualification becomes "indexed fields
  only".
- Collections' hand-added items as links reuse the rename machinery; no separate
  membership table or move operation.
- Merge the Notes search and the message search into one FTS5 index.
- The anchor module is the Notes block-ID module; no second ID scheme.
- Stamps reuse history rows; no separate spans table.
