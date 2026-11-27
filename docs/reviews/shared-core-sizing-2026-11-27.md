# Sizing the shared Rust core for the browser · 2026-11-27

Lane `lane/core-sizing` (issue #109). **Paper study only:** nothing was built,
run, tested or measured. The owner decided on 2026-11-27 that the query and
formula engine is one Rust engine, compiled to Wasm in the browser and linked
natively on Apple platforms (morning report, round 33), and asked for the rest
of the question, how much **more** browser logic moves into that core, to be
sized on paper first. This is that sizing.

- Old app: `kayg/calternal`, `origin/dev` at `ea57b84b9`, read with git only.
- New app: `dev` at `cf8ec64c`, which landed during this lane. Its coordinator
  decisions of 2026-11-27 are counted here, above all **C1** (the shared core
  reads task and time lines inside notes), **C3** (listed record operations,
  applied offline) and repeat expansion in the engine
  ([suite stress test](../proposals/format-stress-test-suite-2026-11-27.md); the
  owner can overturn them). A second commit, `0db49001` (extension surface
  audit, `docs/proposals/extension-surface-2026-11-27.md`), is counted in
  candidate 10 and where noted.
- Every Wasm size below is an **estimate** unless it says "measured". "KB" is
  brotli unless stated. The measured engine reports bytes / 1000 and the old
  budgets bytes / 1024; the 2.4% difference is far below the error here.
- Every disagreement marked "by reading" was found by comparing the two
  sources. None was reproduced.

## The short answer

Move what **reads, checks, changes or merges an item**, including the two line
forms inside notes. Keep the editor, and whatever copies a whole document on a
keystroke, in TypeScript. Do not ship SQLite or Yrs to the browser.

| # | Candidate | Old app: Rust / TypeScript lines | Added Wasm (estimate, KB brotli) | Copies disagreed? | Recommendation |
| --- | --- | --- | --- | --- | --- |
| 1 | Queries, formulas, checks | 1,595 / 780 (task lists; no formula language) | 120 to 180 for the whole engine (82 measured for the spike) | Yes (Matrix urgency, today; two in the spike) | **Move** (decided) |
| 2a | Item, kind and collection files (frontmatter, byte-keeping writer) | part of 6,436 / part of 1,944 | +40 to 90 | Yes (tags in properties, today) | **Move**, second only to the engine |
| 2b | Task and time lines in notes, record lines | part of the above | +15 to 50; +35 to 60 more if it needs a Markdown parser | Yes (log times today; line extraction, fixed 10-31) | **Move** (C1, C3) |
| 2c | The rest of Markdown bodies (links, tags) | rest of the above | +60 to 160 including a parser | Yes (tag grammar today) | **Keep out**: editor parser in the browser, server index for the rest |
| 3 | Field-by-field merge with `together:` | 1,080 / 314 (no generic merge) | +5 to 15 | No generic merge exists to compare | **Move** |
| 4 | One-line and record operations | 3,597 / 831 (task operations) | +5 to 20 | Not found | **Move** |
| 5a | Repeats (expansion, `next()`, streaks), date functions | none: expansion was never built | +15 to 45 without a time zone database | Nothing to compare | **Move**, in civil time |
| 5b | Date phrases typed by a person | 44 / 1,222 | +25 to 75; an old build was 273 (contents unverified) | Yes (two TypeScript readers; log times, today) | **Keep in TypeScript**; measure before Apple |
| 6a | Sync decisions (operation IDs, replay, staleness) | 4,242 / 1,873 (mostly server and I/O) | +10 to 30 | One protocol edge (fixed 11-02) | **Move** the decisions only |
| 6b | Sync transport, IndexedDB, service worker | | none | | **Keep in TypeScript** |
| 7a | Search text rules (fold, words, query syntax, order) | 3,563 / 1,181 (all search) | +8 to 20 | Yes (three folding rules, today) | **Move** the rules |
| 7b | Full-text search on the browser device | none on device | SQLite with FTS5: about +336 (third-party figure) | | **Measure first**; do not ship now |
| 8 | Live co-editing | 7,019 / 1,766 | about +190 to 260 net over Yjs | Protocol edge, as 6a | **Keep Yjs in the browser** |
| 9 | Link resolution, block IDs, tag tree, sort order | small, scattered | +3 to 10 | Yes (sort order, in the spike) | **Move** with 1 and 2a |
| 10 | Pure plugin modules: a JavaScript interpreter in the core (open, owner) | none | audit: "about a megabyte"; my guess 200 to 400 compressed; unmeasured | Nothing to compare | **Measure first**; not in the totals |

**Download total (estimate).** The recommended core is about **220 to 460 KB**
brotli in one module: 120 to 180 for the engine and **100 to 280 more** for what
this report recommends moving (35 to 60 on top if line reading needs a Markdown
parser). The old app's recorded JavaScript was **590 KB** in all and **233 KB**
before first paint, so the module adds 37 to 78% to the whole download, and would
roughly double to triple first paint if it were loaded there (it should not be).
Moving everything (rows 2c, 5b, 7b and 8 too) comes to about **0.8 to 1.3 MB**.
Neither figure includes the interpreter of row 10, which would be the largest
single addition after SQLite, or larger.

## How the estimates were made

| Anchor | Figure | Standing |
| --- | --- | --- |
| Spike engine (1,263 non-comment Rust lines, `serde`, `serde_json`) | 81.6 KB size build, 102.9 KB speed build, brotli; 198 and 376 KB raw | Measured on the dev VM (`lane/engine-wasm-vs-ts`, `results/sizes.jsonl`) |
| TypeScript engine, same behaviour | 5.6 KB brotli | Measured, same place |
| An older Wasm date parser | 273 KB brotli | Recorded 2026-10-19 (`ops/orchestrator/memory/feedback-wasm-boundaries.md`); its results file and dependencies were not available |
| SQLite in Wasm | 835 KB raw, 336 KB brotli | [A third-party README](https://docs.rs/sqlite-wasm/0.1.0); FTS5 not stated |
| Yjs against Ywasm | 20.1 against 213.8 KB **gzip** (13.6.11, 0.9.3) | Read in [crdt-benchmarks](https://github.com/dmonad/crdt-benchmarks). A search summary gave 25 against 285 for newer versions; not re-read |
| `pulldown-cmark` in Wasm | 80 to 180 KB raw, about 35 KB gzip | [Tutorial figures](https://botmonster.com/web-dev/compile-rust-to-wasm-tutorial/) that disagree among themselves; weak |
| QuickJS in Wasm | one build variant installs at about 1.1 MB, a single-file build unpacks to 795 kB, both including JavaScript | [Package pages](https://unpkg.com/quickjs-emscripten@0.31.0/README.md) seen in a search summary; not the compressed Wasm size |
| Rust `regex` against `regex-lite` | +410 to 565 KB against +94 KB, **native** | [regex-lite README](https://docs.rs/crate/regex-lite/latest); not Wasm. Full Unicode classes are the expensive part |

**Marginal code rule (an assumption, not validated):** each further non-comment
Rust line adds 25 to 50 bytes brotli once `std` and `serde` are already in the
module. The spike averages 65 bytes per line including that fixed floor, so the
marginal figure should be lower. Old-app line counts bound the work from above:
the new design replaces app-specific code with one generic path.

**Line counts** are from `origin/dev`, without inline Rust test modules and test
files, with comments and blank lines. A file can serve two candidates, so rows
do not add up. There is **no Swift source in the old app** (0 files).

## The shape that decides every answer

The measurement settled the rule. With items **inside** Wasm, a query crosses
the boundary with about 100 bytes of text (about 0.001 ms) and returns a page
of 50 rows and a view of the ids (0.01 to 0.4 ms); one changed field costs 1.6
µs. Sending items as JSON on each call was 8 to 16 times slower than
TypeScript. Loading is the one large crossing: 2.2 ms for 1,000 items, 22 ms for
10,000, 234 ms for 100,000 (Node), about a third of it `JSON.stringify`. An
earlier measurement of the date parser found about 29% of Wasm time at the
string boundary and a gain under 1.3 µs per key.

So the core should be an **item store**, not a library of functions:

- **In once:** file bytes (from `fetch` or IndexedDB `ArrayBuffer`s, no UTF-16
  conversion) or server records. The core parses them into its columns. This
  also removes the `JSON.stringify` step the spike paid.
- **Per gesture:** a small request (query text, "set `read` = yes on item X",
  "merge these two versions").
- **Out:** a page of rows, ids, totals, or **replacement bytes for one file**,
  which TypeScript writes to IndexedDB and sends.
- **Never:** a call on a keystroke that copies a document, or per-frame calls.
  A call with one short string is cheap (the date parser took 0.6 to 2.2 µs in
  Wasm); copying the document is what grows.

A function that needs the whole note text from CodeMirror on every key pays the
copy every time and stays in TypeScript. One that needs a changed line or a short
field can cross, and then the question is only whether it is worth a second copy.

## Candidates

### 1. Queries, formulas and checks

**Today.** There is no formula language in the old app, and Money exists only as
a plan (`docs/plans/money-architecture.md`). Queries are app-specific:

| Side | Files (lines) |
| --- | --- |
| Rust | `crates/tasks/src/lists.rs` (676), `lists/grouping.rs` (140), `counts.rs` (205), `index.rs` (508), `crates/core/src/visible.rs` (66) |
| TypeScript | `apps/web/src/lib/notes-index.ts` (282), `list-window.ts` (133), `tag-tree.ts` (64), `lists.ts` (59), `tasks/data.svelte.ts` (127), `tasks/status.ts` (78), `tasks/matrix.ts` (37) |
| Swift | none |

**Disagreements.**

- **Matrix urgency (today, by reading).** The server marks a task urgent from its
  Deadline, or When if it has none (`lists.rs:575`, ADR 0066 "Matrix"). The web
  Matrix takes the **earlier** of the two (`matrix.ts:18`) and places cards with
  that (`Matrix.svelte:14`). A task with When today and a Deadline next month is
  urgent on screen and not urgent in `tasks.list`.
- `URGENT_DAYS = 2` is declared on both sides (`lists.rs:50`, `matrix.ts:13`).
- The spike's two engines disagreed twice in one afternoon: number parsing (one
  bit, changed an average) and text order (UTF-16 against UTF-8).

**New design.** One condition language for computed fields, `where`, `when`,
`total` and `checks:` ([format §1.6](../proposals/kind-and-collection-format-2026-11-24.md),
[host gaps 2.3](../proposals/host-gaps-2026-11-24.md)); `per month`,
`previous()`, `all()`, one link hop, the 1,000-item cap through an index
([formulas research](../proposals/formulas-research-2026-11-24.md), F1 to F4);
evaluated on devices and marked not yet confirmed (S3). The 11-27 decisions add
per day, week and year, records across items, conditions inside list functions,
`under(tags, …)`, `matrix` views, manual order, and re-running time-dependent
formulas; the audit's `marks:` formulas (and `refuses:` conditions, if the owner
allows them) use the same evaluator.

**Size (estimate).** 82 KB measured for a subset. Missing: `from()`, `next()`,
`now()`, money with currencies, durations, per-month fields, totals, `all()`,
cycle check, step budget, reverse-link index, checks, and the 11-27 additions.
At 1,500 to 2,000 more lines: **120 to 180 KB** size build, about a quarter more for the speed build
(the choice moves tens of KB and should be measured). To know: build the full
language, or measure after each feature.

**Boundary.** As above; once per query or change. Results must stay a page.

**Apple and offline.** The same crate, native, 1.1 to 1.45 times faster than its
Wasm form at 10,000 items and up (dev VM). Through UniFFI, expose an item-store
object whose calls return pages; UniFFI copies records, so never a whole list.
Offline works as S3 says, over the items the device holds. Memory at 100,000
items: 39 MB of Wasm memory, which never shrinks (1.1 MB floor).

**Recommendation: move** (decided). Checks are the same evaluator, so they cost
almost nothing more.

### 2. Markdown, frontmatter and kind files

**Today** (29 Rust files, 6,436 lines; 22 TypeScript files, 1,944 lines).

| Job | Rust | TypeScript |
| --- | --- | --- |
| Frontmatter read and write | `crates/notes/src/strict_frontmatter*` (1,289), `props.rs` (706), `frontmatter.rs` (184), `frontmatter_guard.rs` (88) | `packages/editor/src/syntax/frontmatter.ts` (64), `apps/web/src/lib/fm-tags.ts` (70), `fm-tags-edit.ts` (22), `packages/editor/src/blocks/hidden-refs.ts` (85) |
| Tags, links, blocks | `notes/src/tags/{scan,front,mod}.rs` (379), `links.rs` (449), `blocks.rs` (228), `visibility.rs` (421), `block_visibility.rs` (161) | `editor/src/syntax/links.ts` (142), `transforms/links.ts` (104), `lib/backlinks.ts` (95), `links/resolve.ts` (48), `transforms/paths.ts` (22), `text/src/{block-id,visible-text}.ts` (61) |
| Line extraction | `crates/markdown-view` (about 1,565, on a vendored `pulldown-cmark`) | `editor/src/markdown/extraction.ts` (204) |
| Daily log lines | `crates/daily/src/{log,day,times}.rs` (599) | `editor/src/daily/log.ts` (315) |
| Tables | none | `editor/src/tables/{model,cells,parser}.ts` (498) |
| Contact cards (a body format) | `crates/contacts/src/note_io*` (1,960), `card.rs` (145) | `apps/web/src/contacts/{card,edits}.ts` (333) |

Where the `---` block ends is decided separately in at least a dozen places, with
different rules (the editor needs the closing line on line 3 or later;
`fm-tags.ts` and `hidden-refs.ts` accept an empty block; `daily/log.ts`,
`frontmatter.rs` and `props.rs` also accept `...`).

**Disagreements, today (by reading).**

1. **Tags in properties; the file can be damaged.** `fm-tags.ts` says "the server
   reads the same", but reads list items only when indented. The server accepts
   `- a` at column 0 (`tags/front.rs`; its own test in
   `visibility/style_tests.rs:20` uses that form). On such a note the editor's
   tag row shows none, and adding one rewrites only the `tags:` line, leaving indented and
   unindented items mixed, which is not valid YAML. The save guard
   (`frontmatter_guard.rs`) lets any change inside a closed block through.
2. **`#tag` grammar.** `tags/scan.rs` (sidebar, search, renames) needs a letter
   first. The editor highlight (`cm-extras.ts:28`) colours `#2024plan`; the line
   extractor on both sides (`lines.rs:31`, `extraction.ts:148`) also allows a
   digit first but, unlike `scan.rs`, not a tag after `(`; the Composer
   (`phrases-tasks.ts:149`) uses a fourth boundary rule.
3. **Log times.** `- 13:00 pm Walk` is an entry at 13:00 in Rust and not an entry
   in TypeScript. Commit `71be3c7bd` (10-19) changed only `log.rs`; the shared
   cases (`contracts/log-entries.json`) cover only the unspaced form.

**Fixed earlier.** The editor and Rust line extractors differed on tags after
formatting, lone carriage returns and escaped spans (`d6fa3c667`, `8206cdeb5`,
`3c301179d`, 10-31; `docs/plans/0067-slice2/README.md`). The editor's grey and
"+ Log" once read times differently (#556, 10-17).

**New design.** Kinds, collections, sidebars and items are Markdown with
frontmatter in a line-based form, with types, computed fields, `operations`,
`checks:`, `together:`, `search:` and a format version
([format](../proposals/kind-and-collection-format-2026-11-24.md), rounds 28 to
31). Reading is lenient, nothing unknown is rewritten, a file that does not
parse is shown and not changed ([host gaps 2.1](../proposals/host-gaps-2026-11-24.md)).
Since 11-27, a kind may declare `lines:`: the core reads checkbox and leading-time
lines on the server and on devices and keeps a task's box and status in step (C1);
record lines have listed operations that apply offline (C3).

**Size (estimate).**

- **2a, item and kind files: +40 to 90 KB** with one strict parser and writer for
  the format (old equivalent about 1,900 non-comment lines). Keeping a general
  YAML decoder beside it, as the old writer does as a safety net, adds an
  unmeasured amount (guess: 30 to 60).
- **2b, line forms and records: +15 to 50 KB** with a narrow line scanner (the
  old log reader, task finder and line extractor are about 1,600 lines with
  comments). The scanner must still skip code, HTML and comments. The old plan
  for the same job chose `pulldown-cmark` with 70 shared line vectors
  (`docs/plans/0067-vectors`); if a scanner cannot pass them, add 35 to 60.
- **2c, the rest of bodies: +60 to 160 KB** (`pulldown-cmark` plus link and tag
  readers; less if 2b already carries the parser). The old extractor also uses
  `regex` with Unicode classes, which could add 100 to 250 on top; a port should
  not take it into Wasm.

To know: run the line vectors against a scanner, and build with and without each
library.

**Boundary.** 2a: once per file at load, once per write. 2b: once per note at
load; for the note being edited, when a box is ticked or at the save pause (750
ms, ADR 0021), which copies that one note. For a note open in a live room the
core must return **text edits** in UTF-16 positions for TypeScript to apply
through Yjs, not replacement bytes. 2c would be per file when indexing, but per
keystroke for anything drawn in the editor.

**Apple and offline.** One parser is what lets a phone change an editable file
offline without damaging it (ADR 0048: never rewrite what cannot be parsed
safely), and Apple has no third copy yet.

**Recommendation.**

- **2a: move.** It is the data-safety case, the engine needs it to load bytes,
  and the copies disagree today. Browser TypeScript gets no frontmatter writer.
- **2b: move**, as decided (C1). One line reader replaces `daily/log.rs`,
  `daily/log.ts` and the two line extractors. The editor's greying may keep its
  Lezer drawing or call the core for each changed line; measure, and do not keep
  a second grammar.
- **2c: keep out.** The browser already carries a Markdown parser for the editor
  (Lezer). Let the server send each item's links and body tags as metadata; for
  the one open, edited note, TypeScript reports them from its tree. This also
  answers "Linked here" offline (`where: links to this`) without a second parser.
  One written tag rule with shared vectors covers the server and the editor.
- No further line or body format enters the core without a decision (C1). The
  contact card's body format becomes declared fields that 2a reads, or stays in
  its plugin.
- If 2b ends up carrying `pulldown-cmark`, revisit 2c: its added cost then falls.

### 3. Field-by-field merge with `together:`

**Today.** No merge driven by declared fields exists. Pieces:
`crates/notes/src/diff.rs` (187), `room/merge.rs` (260), `crates/core/src/stale.rs`
(175, per-field staleness), `crates/contacts/src/merge.rs` (458); in TypeScript
`editor/src/offline/replay.ts` (50, `diff-match-patch`), `lib/prefs-sync.ts`
(177), `contacts/merge.ts` (87, a different job: joining two cards).

**Disagreements.** No same-rule pair to compare. The contact merge had a series
of fixes on 10-20 (`ceab3c0c0`, `111c72605`, `681b577b6`), a sign of what
format-specific merges cost.

**New design.** One merge for every kind, driven by the kind file; the same field
changed on both sides keeps both (S2); `together:` fields merge as one value
(round 29); a merged result that fails a check keeps both versions (host gaps
2.3). Bodies keep the inherited three-way text merge (ADR 0045).

**Size (estimate).** **+5 to 15 KB** on top of 1 and 2a. A three-way body merge
in Wasm, only if the browser ever needs one outside Yjs, +10 to 20 more.

**Boundary.** Only on a conflict or a catch-up: three versions of one file in,
one or two out. Rare; cost does not matter.

**Apple and offline.** Same function; it is the reason offline edits can resolve
on the device. The server stays the authority.

**Recommendation: move.** Written once, before any copy exists. It shares the
parser and evaluator, so a TypeScript version would be a second one of all three.

### 4. One-line operations

**Today.** Coded per app: `crates/tasks/src/{status,status_action,update,order,reorder,create,delete,duplicate,moves,undo}.rs`
and `crates/core/src/action.rs` (about 3,600 lines). The browser repeats the
intent optimistically (about 800 lines in eleven files, among them
`tasks/status.ts`, `lib/note-visibility.ts` and `lib/actions.ts`). The generated
`actions.gen.ts` keeps input **types** in step, not behaviour.

**Disagreements.** None found beyond the Matrix rule (candidate 1).

**New design.** `set field = value, when condition` lines in one `operations`
list, with free add, change and remove (rounds 25, 29); applied at once offline,
coded operations wait as "not sent yet" (S1). Since 11-27 also `append`, `change`
and `remove record` (C3), `set field = ask` and a built-in `duplicate`.

**Size (estimate).** **+5 to 20 KB**: the evaluator and the writer already exist.

**Boundary.** One call per gesture: operation name, item id, inputs in; new file
bytes and changed fields out. Far inside a frame.

**Apple and offline.** This is what makes offline simple changes identical on
every device and the server.

**Recommendation: move.** The old per-app operation code and its optimistic twin
have no successor.

### 5. Repeats and date math

**Today.** Expansion was **never built**. The web writes and reads a subset of
`RRULE` (`tasks/when/rule.ts` 94, `composer/task-input.ts` 34), the server
**refuses** a status change on a repeating task (`crates/tasks/src/status.rs`,
"recurring outcomes require…"), and the vendored `rrule` crate of ADR 0066 is
not in `vendor/` or `Cargo.lock`. Typed phrases are read only in TypeScript:
`composer/{recognise,phrases-tasks,dates,date-words}.ts`, `tasks/when/*.ts`
(about 1,200 lines in all). Rust reads log times (`daily/src/times.rs`, 44).

**Disagreements (by reading).** Two TypeScript date readers differ: the When
field reads "in 2 months" as 60 days (`when/parse.ts`); the Composer reads only
days and weeks (`phrases-tasks.ts:23`). Log times: see candidate 2.

**New design.** One `repeat` field type the server understands, `next(repeat)`,
`today()`, `days()`, `week()`; task times float, event times keep their zone,
Today follows the device (round 22, ADR 0066, [host proposal §12](../proposals/primitives-only-host-2026-11-21.md)).
The 11-27 decisions put repeat expansion with overrides and skipped dates,
`next(repeat, after: …)` and `streak(log)` in the engine, and add `phrases` on an
`add` line, read by "the host's language engine".

**Size (estimate).**

- **5a: +15 to 45 KB** for the needed `RRULE` subset expanded in **civil (wall
  clock) time**, with no time zone database. Event times need offsets: take them
  from the platform (`Intl`) or the server, once per zone and range, never per
  occurrence.
- A bundled time zone database costs more (size not found; unmeasured, guess 50
  to 200 KB). Note: this repository's `crates/core` already depends on `jiff`
  with `tzdb-bundle-always`. **The browser engine must not depend on that crate
  as it stands.** The `rrule` crate brings `chrono` and `chrono-tz`; unmeasured.
- **5b, phrases: +25 to 75 KB** without a regex library.

**Boundary.** 5a inside the query: free. 5b would cross on every keystroke with
a new string and a result.

**Apple and offline.** 5a native, offline "next occurrence" works. 5b: ADR 0053
chose TypeScript with shared vectors for the browser; a native composer would need
a Swift or Rust second copy, or the TypeScript one run in JavaScriptCore.

**Recommendation.** **5a: move.** **5b: keep in TypeScript** (merge the two
readers into one) and **measure first** before choosing the Apple route. Nothing
in round 33 changes ADR 0053's reason: it is a per-keystroke path with a gain
under 1.3 µs.

### 6. The outbox and sync

**Today.** Rust is mostly the server: `crates/store/src` (journal, tokens),
`crates/server/src/{live,upload}.rs`, `crates/core/src/{journal,drain,room_queue,rooms,stale}.rs`
(about 4,200 lines). Browser: `editor/src/offline/{local-copy,meta,closed-copy,replay}.ts`,
`lib/{action-outbox,live,store.svelte,prefs-sync,prefs-store,sharing-sync,wipe,notes-cache,pending-tags}.ts`,
`sw/sw.js` (about 1,900). `action-outbox.ts` is used only by its test: the
generic outbox was never wired. In this repository `crates/client` follows the
journal and stores bookmarks and says it has no merge engine.

**Disagreements.** The Rust room refused a reader's empty sync step until
`a073c9344` (11-02) made it ignore one. No other client against server rule
disagreement was found in commit messages.

**New design.** One sync route and journal; an outbox of operation IDs derived
from installation, file, base hash and new hash, kept until confirmed (ADRs 0044,
0047); coded operations keyed per gesture (A22); every write a list of changes
committed together.

**Size (estimate).** **+10 to 30 KB** for the pure decisions: ID derivation (with
a hash), coalescing unsent changes, what may replay, epoch restamping, staleness,
and calling the merge.

**Boundary.** Per save or reconnect, with small records. Journaling each
keystroke of note text (ADR 0021) stays in TypeScript and IndexedDB.

**Apple and offline.** This is the hardest logic to keep identical, and the Apple
apps, CLI and MCP need it anyway (ADR 0072, "Swift networking around a small Rust
parser" is rejected there).

**Recommendation.** **Move the decisions** as a state machine that emits effects;
**keep in TypeScript** the socket, `fetch`, IndexedDB, service worker and tab
coordination. Offline start also needs the Wasm file in the service worker's
precache.

### 7. Device search

**Today.** Search runs on the server only (ADR 0031): `crates/search/src`
(about 2,970 lines), `crates/notes/src/search.rs` (444), `crates/core/src/search.rs`
(149). The browser calls it and adds its own instant matching and ranking:
`lib/{search,list-search,link-targets,tag-source}.ts`,
`editor/src/transforms/{link-rank,tag-rank}.ts`, `ui/src/search/words.ts` (451,
plain-English filters), about 1,200 lines. No device full-text index exists.

**Disagreements (by reading).** Text folding has three rules:
`contacts/names.ts` strips all accents; `link-rank.ts:75` strips none, so
"zurich" does not match "Zürich" in the picker's own ranking;
`crates/search/src/text.rs` strips a hand-written list of Latin accents. There are
also two query readers with different syntax (`words.ts`, `query.rs`).

**New design.** A `search:` line per kind (G9); Apple devices keep a full-text
index of notes and documents (ADR 0046); offline lists, filters and formulas run
in the engine (S3).

**Size (estimate).** **7a: +8 to 20 KB** (the old pure parts are 376 non-comment
lines). **7b:** SQLite with FTS5 about **336 KB** plus a worker and
origin-private storage; a small word index in the core instead, +15 to 40 KB, with
index memory not estimated.

**Boundary.** With items inside, per-keystroke title and field matching is a query
(about 100 bytes in, ids out), so it need not stay in TypeScript. The spike's
sort over 1,000 items took about 0.1 ms; text matching was not measured at scale.

**Apple and offline.** Native SQLite and FTS5 through `rusqlite`, reusing the old
crate's schema and rules. Browser offline: titles and fields through the engine;
bodies not searched until measured.

**Recommendation.** **7a: move** (folding, word splitting, query syntax, order).
**7b: measure first**, and do not ship SQLite to the browser for search alone.

### 8. Live co-editing

**Today.** `yrs` on the server: `crates/notes/src/room/*`, `ydoc.rs`,
`crates/server/src/rooms.rs` (about 7,000 lines). Yjs in the browser:
`editor/src/sync/*`, `lib/note-live.ts` (about 1,800) with `y-codemirror.next`.
The two interoperate through the binary protocol.

**Disagreements.** The empty sync step above. Separately, the Rust side needed a
vendored fix for quadratic appends (`1372b812d`, `vendor/yrs`).

**New design.** The old live rooms carry over (S5); the new
[live editing service](../live-rooms.md) keeps Yjs checkpoints and the old
transport shape.

**Size (estimate).** Ywasm replaces about 20 to 25 KB of Yjs with about 214 to 285
KB (gzip, published figures): **about +190 to 260 KB net**. Linking `yrs` into the
engine module might be somewhat smaller; unmeasured.

**Boundary.** Every keystroke and every remote update, and the editor binding is
written against the Yjs JavaScript API. This is the worst case for the boundary.

**Apple and offline.** Native `yrs` (through UniFFI or the y-crdt Swift
bindings), applying batched edits; `crates/client` already forbids copying a
document per keystroke. Offline replicas as today.

**Recommendation: keep Yjs in the browser.** One upstream project maintains both
sides and the wire format is the contract.

### 9. Other things written twice

| Item | Where | Recommendation |
| --- | --- | --- |
| Link resolution and relative paths | `notes/src/links.rs`; `links/resolve.ts`, `transforms/paths.ts`, `attachments/path.ts` | **Move**: formulas follow links, so the engine needs it |
| Block ID creation and marker hiding | `notes/src/blocks.rs`; `text/src/{block-id,visible-text}.ts` | Keep the 60 lines of TypeScript; the server is the authority (ADR 0056); shared vectors |
| Tag tree (a tag counts for its parents) | `notes/src/meta.rs` and `tasks/src/index.rs` (`expand`, twice in Rust); `lib/notes-index.ts`, `tag-tree.ts` | **Move** with grouping |
| List order | "the server lists notes in this same order" (`notes-index.ts:6`) | **Move**: one order in the engine, with the code-point rule |
| Field types, required, defaults | coded per app today | **Move** with 2a: the kind file is the one declaration |
| Contact card read, edit, merge | see candidate 2 | Do not port; make contacts a kind whose facts are fields. vCard stays on the server |
| Table editing | TypeScript only | Keep; written once |
| Daily log reader | twin, disagreeing | Replaced by the core's line reader (2b, C1) |

### 10. Pure plugin modules inside the core (open question)

**Today.** Nothing. The extension audit asks the owner (its question O1) whether
a plugin may ship plain JavaScript functions that compute, check, merge or
preview, run by "the shared Rust core" in its own interpreter with no network,
files or clock, on devices and on the server. It recommends yes, starting with
formula functions and field checks.

**Size (estimate).** The audit says about a megabyte, unmeasured. The published
package figures above are uncompressed and include JavaScript, so I would expect
the compressed interpreter to be well under that, perhaps **200 to 400 KB**: a
guess. A C interpreter such as QuickJS must also be shown to build into the same
module as the Rust core; not checked.

**Boundary.** This decides the layout. A plugin formula function runs **once per
item**. In a second Wasm module or in browser JavaScript, every call would cross
the boundary, which is the cost the measurement showed. The interpreter would
have to sit in the same module as the items, or take columns in batches.
Interpreted speed at 10,000 and 100,000 items is unknown.

**Apple and offline.** The same interpreter inside the native core, or a bare
`JSContext` (then two engines that must agree). Offline coded operations would
work. It could also run the one TypeScript date-phrase reader natively.

**Recommendation: measure first**, as the audit itself asks, and settle O1 before
step 1 fixes the module's layout. If it is adopted, spaces without such modules
should ideally not pay for it, which pulls against "same module"; that trade-off
is part of the measurement.

## Proposed order

| Step | What | Gate before the next |
| --- | --- | --- |
| 0 | A pure engine crate: no `tokio`, `rusqlite`, `regex`, bundled time zones; checked for `wasm32`. A per-feature size budget for the one module. A physical iPhone run. The owner's answer on plugin modules (10) | Size, cold start and module layout known |
| 1 | Item store loading bytes: 2a parser, queries, formulas, checks, links, order (1, 2a, 9) | Load time and memory against targets |
| 2 | One-line and record operations, the byte-keeping writer, line forms (4, 2b) | Round-trip vectors: untouched bytes stay; the line vectors pass |
| 3 | Field merge with `together:` (3) | Conflict vectors, server and device identical |
| 4 | Repeat expansion, `next()`, streaks and date functions in civil time (5a) | DST, 31st and 29 February fixtures |
| 5 | Sync decisions, shared with `crates/client` (6a) | Replay and epoch vectors |
| 6 | Search text rules (7a); then decide 7b from measurements | |
| Not moved | Editor, the rest of Markdown bodies, date phrases, live co-editing, transport and storage, SQLite | Revisit only with a measurement |

Steps 2 and 3 write files, so they come after the parser and before any device
writes offline. Rust builds one module that every list pays for, so a feature
that lands there is paid by every user; hold it to the budget from step 0.

## Download estimate

| | KB brotli (estimate) |
| --- | ---: |
| Engine, full language (decided) | 120 to 180 |
| Item and kind files | 40 to 90 |
| Line forms and records (narrow scanner) | 15 to 50 |
| Merge, operations, links and order | 13 to 45 |
| Repeats | 15 to 45 |
| Sync decisions | 10 to 30 |
| Search text rules | 8 to 20 |
| **Recommended module** | **about 220 to 460** |
| If line reading needs `pulldown-cmark` | +35 to 60 |
| Not recommended: rest of bodies / phrases / SQLite / Yrs net | 60 to 160 / 25 to 75 / ~336 / 190 to 260 |
| **Everything** | **about 830 to 1,300** |

TypeScript that the module replaces would come off the total; its compressed size
is not recorded, and from the spike (5.6 KB for an engine) it is likely small.

**What it compares with.** From the old app's `perf/budgets.json` (limits set at
the sizes measured on its dev VM, last updated 11-15):

| Old web bundle | KB brotli |
| --- | ---: |
| First paint JavaScript (the `index` chunk) | 232.6 |
| First paint CSS | 19.1 |
| All JavaScript, every lazy chunk and Vim (sum of 71 chunk limits) | 589.9 |
| Largest lazy chunks: idle screens, Vim, JavaScript grammar | 60.5, 37.9, 24.4 |

This repository's web app records one figure, **105 KB** first paint
(`docs/ui-performance.md`, 11-17, earlier than the current screens); its current
total is **unknown**. The module is 2.1 to 4.4 times that first paint.
Load it after first paint. Whether a first list may wait for it is a measurement,
not a guess; a second TypeScript list path to cover the wait would undo the point.

## Measurements for a later lane

On `calternal-dev-01-compute02` (sizes are exact there; timings are development
timings, not a benchmark pass; real performance claims need a perf VM).

1. **Size, one dependency at a time**, `-Oz` and `-O3`, raw, gzip, brotli, with
   per-crate attribution: the spike engine; each missing formula feature; own
   frontmatter parser against `yaml-rust2`; `pulldown-cmark`; `regex` against
   none; `jiff` with and without the bundled database; `rrule` against an own
   subset; the line scanner against `pulldown-cmark` (and whether the scanner
   passes the 70 line vectors); `similar`; the hash; `yrs`. The published `sqlite3.wasm` with FTS5,
   compressed.
2. **Load:** 1,000, 10,000 and 100,000 real item files from `ArrayBuffer`s into
   the store (parse inside Wasm against JSON in), memory, and time to the first
   list from a cold cache and from the service worker.
3. **Boundary per gesture:** one-line operation, merge, `next()`, title match per
   keystroke at 10,000 and 100,000 items, on the main thread and in a worker.
4. **Real WebKit:** cold start and memory limit on a physical iPhone. The 10-19
   record says 8.7 s cold start in WebKit and 1.4 s in the Simulator; the 11-24
   spike read a few milliseconds. This gap is unexplained and decides whether the
   module may sit in front of a first list.
5. **Apple** (on `std`, then `mba`, per the machine rules; not the dev VM):
   UniFFI call cost, copying 50 rows, library size, and memory inside a widget
   and a keyboard extension, which the stress test expects to use the core.
6. **Disagreements:** run vectors for the cases marked "by reading" against both
   old copies, then keep them as the new core's vectors.
7. **Offline search:** a core word index against SQLite FTS5 on 10,000 notes:
   size, build time, memory, query time.
8. **Records:** one and ten years of workout sets (about 7,000 records a year,
   the stress test's figure) as record rows: memory and query time.
9. **Interpreter (before O1):** compressed size of QuickJS (and one pure-Rust
   interpreter) inside the engine module; a plugin function over 10,000 and
   100,000 items against the same formula built in.

## Questions this leaves

**Which code reads typed phrases in the native apps?** `phrases` on an `add`
line uses "the host's language engine" (11-27), which today exists only in
TypeScript (ADR 0053). *Why now:* the first native add form needs it, and a Swift
port would be a second copy. *Example:* typing "call Amma friday 5pm" in the iPhone
add form with no signal. *Options:* (a) run the one TypeScript copy in the
system's JavaScriptCore: no second copy, a JavaScript runtime in the app and
possibly too heavy for a keyboard extension; (b) port it to the Rust core and use
it everywhere: one copy, +25 to 75 KB in the browser and a boundary on each
keystroke, reversing ADR 0053; (c) port to Rust for native only, shared vectors:
small browser, two copies. *Recommendation:* measure (a) and (b) first; ADR 0053
stands until then. If the owner says yes to plugin modules on devices (10, the
audit's O1), the core's interpreter becomes a further way to run the one copy.

**What does a device load for an operations-only kind?** It has no file there
(S1). *Why now:* the item store's loader in step 1 needs its input forms.
*Example:* an offline "mark read" on a mail. *Options:* (a) the server sends the
record in the same Markdown field form: one parser; (b) a separate JSON or binary
row form: faster to load, a second loader. *Recommendation:* (a) until
measurement 2 says otherwise.

## What could not be verified

- No size, speed or behaviour was measured or run in this lane.
- The marginal bytes-per-line rule is an assumption; all totals rest on it and on
  third-party figures of unknown build settings.
- The 273 KB and 8.7 s figures of 10-19 could not be traced to their results.
- The SQLite figure is not confirmed to be SQLite's own build or to include FTS5.
  No size was found for the time zone database or `rrule` in Wasm.
- Disagreements "by reading" were not reproduced; whether people hit them is not
  known. Searches of old issues and commit messages are keyword searches and can
  have missed reports; no filed issue was found for the six current ones, which
  was not checked exhaustively. I did not trace every consumer of the line
  extractor's tag pattern.
- Line groupings are my assignment of files to candidates.
- Nothing about a physical iPhone or an Apple build.

## Simplification opportunities

- One loader reading file bytes removes `JSON.stringify`, the second copy and
  in-app item objects.
- One tag rule, one text fold, one text order, one frontmatter boundary, written
  once in the format's contract with shared vectors.
- Server-sent links and body tags remove a body parser from the engine.
- Contacts as declared fields removes a body format with two parsers and two
  merges.
- Merge the two TypeScript date readers now; do not port the second.
- C1's one line reader retires four (`log.rs`, `log.ts`, two extractors).
- `crates/core` mixes wire types, time zone data and domain logic. A separate pure
  engine crate keeps Wasm small and `jiff`'s bundle out.
- Stale text to fix when the core work starts: "no Wasm" in
  `docs/architecture-proposal.md` (§ client core and host language),
  `docs/server-host-assessment.md` §2.7 and the host proposal's §12 row for ADR
  0048, all superseded by round 33.
- Old defects worth carrying as vectors, not code: Matrix urgency, unindented tag
  lists, tag first character, spaced `pm`, folding, "in N months".
- The Matrix rule becomes a `matrix` view over one formula, declared once.
