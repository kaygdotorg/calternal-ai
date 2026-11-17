# The Notes block editor: port plan (2026-11-17)

The owner decided that every inline field (the thread composer, rename fields, the palette field) becomes the Notes block editor, and that the editor is **taken from the
existing app, rewritten with reference to it and simplified wherever that helps speed, modularity and reuse**. This is the plan, written before any code. Stage 1 is built
after it; the build stops before stage 2 for review.

Read-only reference: `/home/kayg/Developer/calternal` at `origin/dev` (package `packages/editor`, host `apps/web/src/components/Editor.svelte`).
Counts below are lines of TypeScript measured on that checkout.

## 1. What the existing editor is built on

- **CodeMirror 6** (`@codemirror/state`, `view`, `commands`, `language`, `search`, `lang-markdown`) draws and edits the text. The shared text is the document.
- **Yjs** (`y-codemirror.next`) binds the editor to a collaborative `Y.Text` per note (`TEXT_KEY`); the server holds the Yrs side. `NoteSession` (`note.ts`, 560 lines) is the
  client's handle on a live note; `sync/` (1,717 lines) is the live room protocol (`LiveSocket`, `SyncClient`, receipts, coverage, replay base) and `offline/` (700) is the local
  copy in IndexedDB with recovery. ADR 0020 lists the guards (lineage ids, receipts decide "Saved", IME-safe remote changes) and ADR 0021 the save rule (first keystroke, local first).
- **Lezer markdown** with the app's own parser (`tables/parser.ts`) and **hidden syntax**: the note is plain Markdown on disk and in the document; markers are hidden by decorations
  (`syntax/hidden.ts`, 470 lines) so the person reads a drawn document, not source. A `parseAhead` plugin parses only the drawn lines before their frame.
- **Blocks** (`blocks/`, 1,490): a block is a paragraph, list item, heading, code or table; block chrome (handles, drag, swipe, selection, "hidden blocks") sits in the margins.
  Stable block ids follow ADR 0056 rules B1-B5 (only linked blocks get an id; delete wins; the server is the single authority).
- Around them: `tables/` (2,664: a grid editor), `syntax/` (2,380: hiding, links, embeds, frontmatter, headings), `transforms/` (1,050: link, tag and task transforms, the inline picker for
  `/`, `[[`, `@`, `#`), `lists/` (603: tree lines, Enter), `code/` (490 plus lezer grammars), `vim/` (674), `daily/` (400), `insert/` (259), `markdown/` (243), `input/ios-keys` (86).
- Total source: **118 files, 13,841 lines** in `src`, plus 59 test files (7,874 lines, 391 cases); the host component is 618 lines. (A separate read of the module map, written by a research agent and checked against the line counts above, is in this lane's notes; the figures here are measured.)
- Block identity, as found: a block is *derived from the lezer tree* (`blocks/blocks.ts`), it has no stored id; a stable id is a `^id` marker in the text (ADR 0056), and server jobs address a block by a Yjs relative position plus a line hash (`sync/anchor.ts`). The B1-B5 rules are **not enforced inside the editor package** (the server and the markers module own them), so the port builds the client side of them once, as one module.

## 2. What to keep, simplify or drop for Calternal-AI

Constraints that change the design: the **server owns state and clients subscribe**; there is **no offline merge beyond replaying unsent input** (the server already queues
that: `pending.*`, `drafts.*`); plugins add behaviour as data and as sandboxed code, not by importing editor internals; and the **`InlineEditor` interface already in `packages/ui`
is the swap point** for every field.

| Area | Decision | Reason |
| --- | --- | --- |
| CodeMirror 6 view, state, commands, markdown language | **Keep** for the block editor (stage 2) | It is the fast, accessible, IME-correct text engine the existing editor is tuned on; rewriting a text engine is the wrong place to spend. Tree-shake: bare Markdown language, no HTML/CSS/JS bundles (the existing editor already does this, about 50 KB saved). |
| Native `<textarea>`/`<input>` | **Keep for stage 1** (inline fields) | A field of a few lines needs no CodeMirror: native editing is fastest, has perfect IME and accessibility, and costs no bundle. It sits behind the same interface as CM. |
| Yjs / Yrs, `y-codemirror.next`, `note.ts` binding | **Drop** | State lives on the server. Replace with an **operation-based document adapter**: the editor emits text changes tagged with an origin and a base revision; the server orders them and echoes accepted changes; the client applies remote changes as CM transactions. The server contract needs a patch operation (see section 5). |
| `sync/` live room protocol, receipts, coverage, nonces | **Drop**; keep the *ideas* and the reconnect logic: Saved is decided by a server receipt only; remote changes wait while an IME composes (`ImeGate`, 40 lines worth keeping) | The server's account feed, `pending.*` and `drafts.*` already cover durability and replay. |
| `offline/` local copy and recovery | **Drop** (replaced by `drafts.*` and `pending.*`, already wired for notes) | Out of scope by decision. |
| `syntax/hidden.ts`, `parse-ahead`, headings, links, frontmatter, embeds | **Keep, simplify, port** (stage 2) | This is what makes the editor feel like a document. Port the decoration builders and the "parse only drawn lines" idea; drop code paths for features not shipped (embeds gallery first). |
| `blocks/` (chrome, selection, hidden blocks, swipe, drag) | **Keep selectively** (stage 2) | Handles, selection and drag are the block editor. Swipe and `follow` are phone-specific and ride on the gestures already built. Block ids: derive blocks from the parse tree as the existing editor does, keep ids as `^id` markers in the text, and put the ADR 0056 rules in one module with shared vectors; the server is the authority (B5). Block addresses for server jobs replace the Yjs relative position with a revision plus line hash. |
| `lists/` (tree lines, Enter) | **Keep** (stage 2) | Small, high value, well tested. |
| `transforms/` (links, tags, tasks, picker) | **Keep the picker core; reuse the palette's engine** | The `@`, `/` and `#` pickers already exist in the shell on the palette engine (`palette/mentions.ts`); the editor consumes one picker API instead of a second one. Link and tag ranking (`link-rank`, `tag-rank`) are ported as data for the palette. |
| `tables/` (2,664 lines) | **Defer**, then simplify | Tables are a grid editor on their own; they come after the block editor works (lazy chunk, as in the existing app). |
| `code/` and the lezer grammars | **Port lazily**, one chunk per language, as the existing app does | Code blocks highlight only when a note has one. |
| `vim/` | **Defer** | A setting, not core. |
| `daily/`, `insert/`, `markdown/contract` | **Defer / port by need** | Daily notes and the insert grid are Notes features, not editor core. `markdown/contract` (the disk format rules, ADR 0068) is **kept as the single parse/write module** for frontmatter and lines. |
| `input/ios-keys` | **Keep** | Real-device input quirks; carry the tests. |
| Tests | **Port the tests with the code**, module by module; the 59 files are the specification of the guards in ADR 0020. | A port without its tests is a rewrite. |

## 3. Performance budgets to carry over

From `perf/budgets.json` and `apps/web/perf` of the existing app (Chromium, **4x CPU slowdown** by default, `PERF_SLOWDOWN`):

| Budget | Value | Gate in the existing app |
| --- | --- | --- |
| Composer keystroke (recognise and key) at 10,000 notes | **p95 8 ms**, 0 blank frames | gated |
| List search keystroke at 10,000 notes | p95 8 ms, 0 blank frames | gated |
| Keystroke to paint in a 50,000-word note, and in a 1,000-item nested list | **p50 150 ms** (a ceiling at 4x, measured at about 16 keys per second) | not gated yet |
| Scroll, 50,000-word note and the all-notes list | p95 frame 50 ms | not gated yet |
| Open a note (server) | p95 15 ms; a long note 250 ms | gated |
| Boot, cold and warm | cold note 3.5 s, warm 2.5 s; total blocking time 1 s cold, 0.5 s warm | gated |
| Idle CPU, typing CPU on the server | 0.1 % and 2 % | gated |
| Bundle | first paint chunk about 233 KB brotli; every lazy chunk has its own line (code grammars 8 KB runtime) | gated, per chunk |

Calternal-AI adds, for the field editor: **a keystroke is one frame at 4x CPU** (the palette already measures this: 16.4 ms median, one frame, `e2e/perf-suite.mjs`), and the editor costs
**no bytes in the first paint** unless a note is open (stage 1 adds a few KB; CodeMirror is a lazy chunk loaded by the Notes middle card).

## 4. Design: what stage 1 builds and why

One small package, `packages/editor` (TypeScript, no framework, no CodeMirror), holding what every field and the block editor share, so the swap behind `InlineEditor` is real:

- **`Transaction` and `Origin`**: every change is `{from, to, insert, origin}`; origins are `user`, `remote`, `replay`, `command` (the existing `origins.ts` idea, 22 lines). Automatic transforms (links, lists) run only on `user` origin (ADR 0020, "Editor").
- **`History`**: undo and redo of the person's own changes only (existing ADR 0022), coalesced by time, bounded; the native field keeps `Ctrl-Z` working and an explicit history so programmatic edits (a picked mention, a restored draft) undo as one step.
- **`ImeGate`**: remote changes wait while composition is active, then apply (ADR 0020).
- **`FieldEditor`**: the native-backed editor behind `InlineEditor`: value, selection, marks (decorations as ranges, already built), pickers (the `@`/`/` engine), Enter/Escape rules, auto-grow, and **a keystroke probe** (`?test=1`: key press to next frame, as `lib/probe.ts` in the existing app) so budgets are measured, not claimed.
- The interface that stage 2's CodeMirror block editor also implements (`EditorHost`: `value`, `selection`, `apply(tx)`, `onChange`, `marks`, `focus`, `destroy`), so a rename field and a note use one contract.

## 5. Stages

**Status 2026-11-17: stages 1 and 2 are built and measured; the work stops here for review before stage 3 (which needs a server decision).** What exists: `packages/editor` (`origins`, `transaction`, `history`, `ime`, `probe`, `field`; 16 tests), `InlineEditor` running on `FieldEditor`
(callers unchanged; a picked mention is one undo step), the probe wired to `?test=1`, and numbers in `docs/ui-performance.md` (key to paint median 7.3 ms at 4x CPU, 2.0 KB brotli).


**Stage 1: the minimal fast inline editor** (built now). Package `packages/editor` with the transaction model, history, IME gate and the probe, tests ported or written for each guard that applies to a field
(origin rules, undo of own changes only, IME hold, UTF-16 positions, never splitting a character), and `InlineEditor` moved onto `FieldEditor` with no change for callers. Exit: all existing tests pass; the
composer, rename and palette fields work as before; the keystroke probe is in place and the numbers are recorded in `docs/ui-performance.md`; the editor adds under 3 KB brotli to the first paint.

**Stage 2: the block editor for a note, in the Notes middle card with the contents rail.** CodeMirror 6 as a lazy chunk; the document is Markdown text; hidden syntax and `parseAhead` ported; blocks with ids per ADR 0056
(one module); lists, headings, links and the picker (on the shared engine); the contents rail fed by `headingsOf`; code blocks lazily highlighted. Tests ported with each module. Exit: the existing editor's
unit tests for the ported modules pass; typing in a 50,000-word note meets p50 150 ms at 4x; open of a long note and scroll meet the table above.

**Stage 3: live editing against the server.** Needs a server operation the server does not have yet: **a text patch against a base revision** (and a live feed of accepted patches in the space stream), because `notes.update` replaces the whole
note. The client emits patches with an origin and a base revision; the server orders them, rejects stale ones with the current revision, and echoes accepted ones; "Saved" is a server receipt (the existing `receipts.ts` rule, simplified to one authority).
Drafts and pending submissions (already wired) carry offline input. Exit: two browsers edit one note against a real server without loss; a dropped connection replays unsent input once.

**Stage 2 as built.** `packages/editor/src/block`: `blocks` (derived from the parse tree; a nested list item is a block of its own; the host classifies kinds), `hidden` (syntax drawn away: headings, emphasis, code, quotes, links as chips, bullets, task boxes, rules, fenced code; blank lines as small gaps),
`parse` (parse ahead of the frame, the rest at idle), `lists` (Enter continues, Tab indents), `headings` (for the rail), `triggers` (`@`, `[[`, `#`, `/`: the host draws the picker), `chrome` (one handle that moves with the pointer: press for the menu, drag to move a block), `editor` (`createBlockEditor`). 32 tests in `packages/editor/test`, 16 checks in `e2e/editor.mjs`.
Notes (`apps/notes/NoteEditor.svelte`) supplies the document, says which blocks are tasks, draws the block menu with the design system's Menu, and saves: the text is autosaved after a pause (and kept as a private draft after a shorter one), with no Save button. Not built in stage 2 (added since, see below): tables, frontmatter, block ids, code-block grammars and the host's pickers. Still not built: vim, embeds.

## 6. Open points for the owner

1. **Stage 3 needs a server patch operation.** Ask the backend lane for `notes.patch` (base revision, changes) and patch frames in the live feed, or accept whole-note replace for one author at a time.
2. **Block ids**: confirm the server is the authority (ADR 0056 B5) and give the client a place to propose an id; stage 2 builds the client module only.
3. **Tables and vim**: after the block editor, or never for the first release?
4. **Is a native field enough for a short line?** This plan says yes for stage 1 (fastest, accessible, no bytes) and CodeMirror for notes; if the owner wants one engine everywhere (CodeMirror in the composer too), the field becomes a lazy CodeMirror chunk and the first-paint saving is lost.

## Simplification opportunities

The existing editor carries three layers that this design removes (Yjs binding, the room protocol with nonces and receipts, the offline local copy), about 3,000 of the 3,300 lines in `sync/` and `offline/` (the transport and reconnect, the IME gate and honest save status stay) and their tests; an operation-based adapter replaces them with one small module.

**Stage 2, the rest (2026-11-17).** The pickers: Notes draws `@` (people, agents, threads, notes), `[[` (notes), `#` (tags) and `/` (the insert menu: headings, list, task, quote, code block, table, divider) with the same `palette/MentionPicker.svelte` the thread composer now uses; choosing is one undoable change, and a tag does not reopen its own picker. Tags are fixture words until the server lists them.

The lazy pieces (`packages/editor/src/block/features.ts` detects what a note needs with cheap patterns and `prepare(text)` loads it before the editor is created or a note replaced, so the first draw is right; a note that needs none fetches none):

| Piece | Chunk (brotli-sized gzip) | What it does |
| --- | --- | --- |
| frontmatter | 0.4 KB | the leading `---` block is one hidden range, never drawn (existing ADR 0025); `frontmatterEnd` is pure |
| block IDs (`ids.ts` + `ids-view.ts`) | 1.1 KB | `ids.ts` is the one module that parses and writes `^id` markers (ADR 0056: 16 Crockford characters, legacy shorter IDs valid, collision-checked inside the note). `ids-view.ts` hides markers (atomic), keeps typing before a marker, gives a new or edited task its ID (B1), and a pasted copy of an ID already in the note keeps none (a task gets a fresh one); copy and cut put marker-free text on the clipboard. Enter keeps the marker on the first part (B3). `ensureBlockId(pos)` is what the first link to a block calls |
| tables | 1.3 KB | a table the caret is not in draws as a `<table>` (alignment from the delimiter row); the caret entering it, or a click, brings the raw rows back |
| code grammars | 4.9 KB + one chunk per language (JavaScript 5.7 KB) | `@codemirror/language-data` loads a fence's grammar when a note names it; tokens are coloured from the design tokens |

Not done, and why: cut-and-paste does not keep a block's ID (clipboard text is marker-free, which makes every paste a fresh copy); retiring IDs, aliases on join and uniqueness across spaces belong to the server (B2, B3, B5); a task already in a loaded note gets its ID the first time it is edited, not on load; table cells are not editable in the grid (edit the raw rows) and column widths are not read from the frontmatter yet.
