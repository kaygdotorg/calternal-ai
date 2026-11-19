# The block editor: port map (2026-11-20)

Owner decision (2026-11-20): the existing app's editor works and is the reference. Our editor is a **block editor**, taken from `packages/editor` of the existing app (`/home/kayg/Developer/calternal`, ref `origin/dev`) as it is,
adapting only what spaces, UUIDs and our server operations require. The Yjs binding (`y-codemirror.next` over the note's shared text, ADR 0018) is the editing path. Where our first rewrite (`packages/editor/src/block`, 20 files,
about 1,200 lines) and the old editor differ, the old editor wins unless `docs/open-questions.md` says otherwise. This lane (Notes UI) owns `packages/editor`; the bar lane and the style-sheet lane send what they need from it.
Read with git only: `git -C /home/kayg/Developer/calternal show origin/dev:packages/editor/src/<path>`.

Sizes are lines of TypeScript. "Old" is the existing app, "ours" is this repository. The existing editor is 118 source files (13.8 k lines) and 60 test files; ours is 40 source files.

## Key

- **as is**: copied, only imports and names changed.
- **adapted**: copied, then changed where a space, a UUID or one of our operations requires it (reason given).
- **present**: already in `packages/editor` (ported earlier, with the live-rooms work), kept; differences noted.
- **replaced**: our rewrite is deleted and the old file takes its place.
- **not ported**: with the reason.

## Port map

| Old path (`packages/editor/…`) | Lines | What it is | Decision |
| --- | ---: | --- | --- |
| `src/editor.ts` | 291 | `createEditor`: CodeMirror bound to the note's session (`yCollab`), preview, attach, mode, undo, IME gate | **adapted**, replaces `src/block/editor.ts` (167). Our `createBlockEditor` takes text and an `extensions` slot; the old one binds `NoteSession` (`attach(session)`), which we already have in `src/note.ts`. The host (`NoteEditor.svelte`, `live.svelte.ts`) moves to the old attach/preview contract. |
| `src/index.ts`, `src/hidden-ext.ts`, `src/insert-api.ts` | 62, 8, 9 | package exports and lazy entry points | **as is** (entry points kept so the lazy chunks stay lazy: `./lists/shape`, `./tables/tables`, `./vim`, `./code`, `./insert`, `./hidden-blocks`, `./embeds`, `./swipe-start`). |
| `src/note.ts` | 560 | `NoteSession`: the note's Y.Doc, sync, local copy | **present** (570; adapted to space-addressed rooms and live-rooms joins). No change. |
| `src/sync/*` (10) | 1,717 | live room protocol, receipts, coverage, gate, status, anchor, pending times | **present** (adapted: `calternal-live-rooms.v1`, chunking, access revoked, lineage). No change. |
| `src/offline/*` (4) | 700 | local copy, closed copy, meta, replay | **present** (adapted, coalesced IndexedDB writes). No change. |
| `src/origins.ts`, `src/shared-copy.ts` | 22, 82 | edit origins, shared-copy format | **present**. |
| `src/undo.ts` | 40 | ADR 0022: undo only your own changes (`yUndoManagerKeymap`, `noteUndo`, `editPermission`, `historyInput`) | **as is**. Today ours sits in `src/live/extension.ts` (my `undo` keys) and `src/history.ts`; the old module wins, the live extension keeps only awareness and the intent record. |
| `src/syntax/mode.ts` (86) | 86 | default (hidden syntax) and source mode (ADR 0019) | **as is**. New (ours has no source mode). |
| `src/syntax/hidden.ts`, `breaks.ts`, `widgets.ts`, `headings.ts`, `parse-ahead.ts`, `tree-to.ts` | 470, 198, 119, 50, 121, 24 | hidden markers, soft breaks, widgets, parse only drawn lines | **replaced**: our `block/hidden.ts` (131), `block/headings.ts`, `block/parse.ts` are deleted. |
| `src/syntax/frontmatter.ts`, `block-ids.ts`, `visible-text.ts` | 64, 41, 3 | hidden frontmatter, `^id` markers (ADR 0056), visible text | **replaced**: our `block/frontmatter.ts`, `block/ids.ts`, `block/ids-view.ts`. `@calternal/text`'s `block-id.ts` and `visible-text.ts` (two small files, the editor's only dependency on it) are added under `src/syntax/`, not as a package. |
| `src/syntax/links.ts` | 142 | links and `[[ ]]` drawn as links | **adapted**: link targets are `/notes/<uuid>` (ADR 0062 layout, spaces by UUID), not file paths. |
| `src/syntax/embeds*.ts`, `embed-*.ts` (7) | 1,000 | notes, galleries and lines embedded in a note | **adapted** after attachments exist: embeds need files; until then loaded, drawn as plain links, and the file embeds disabled with the reason (same rule as the old Attach menu's "Comes with attachments"). |
| `src/blocks/blocks.ts`, `box.ts`, `chrome.ts`, `spring.ts` | 77, 122, 164, 44 | blocks derived from the parse tree, the gutter box, handles and the right-side pills, handle spring | **replaced**: `block/blocks.ts` (81) and `block/chrome.ts` (80) are deleted. The handle and pill CSS (`apps/web/src/styles/cm.css`) is the bar lane's; the chrome code is mine. |
| `src/blocks/selected.ts`, `picked.ts` | 88, 306 | block selection (drawn as blocks, never as words), picked blocks, multi-block operations on touch | **as is**. New. |
| `src/blocks/touch.ts`, `swipe-hold.ts`, `swipe-start.ts`, `follow.ts` | 191, 74, 25, 109 | phone gestures: swipe to select, hold, follow the block under the finger | **as is**. New. |
| `src/blocks/hidden-blocks.ts`, `hidden-refs.ts` | 205, 85 | hidden blocks (ADR 0056) | **as is**. New (lazy chunk `./hidden-blocks`). |
| `src/lists/enter.ts`, `branch.ts`, `shape.ts`, `tree-lines.ts` | 53, 29, 201, 320 | list Enter, tab, tree lines | **replaced**: `block/lists.ts` (50). |
| `src/transforms/*` (9) | 1,300 | the inline picker for `/`, `[[`, `@`, `#`; link, tag and task transforms | **adapted**: the old picker core drives `/` and `[[`; the candidate lists come from our `palette/mentions` pools and `notes.lookup`/`tags.lookup` (as `NoteEditor.svelte` does today). Task toggle as is. `@` mentions of people, agents and threads are ours (kept). |
| `src/insert/*` (4) | 259 | the insert registry (`/` and `+` offer only what Markdown can hold) | **as is**. New. |
| `src/code/*` (12) | 490 + grammars | fenced code: language chip, lazy grammars | **replaced**: `block/code.ts` (82). |
| `src/tables/*` (20) | 2,664 | the table grid editor | **replaced**: `block/tables.ts` (102). Largest piece; done last. |
| `src/daily/*` (3) | 400 | `+ Log`, day notes | **not ported now**: daily notes are not in our product yet; the module is added when they are. |
| `src/vim/*` (10) | 674 | vim mode (a setting) | **as is, after the rest** (lazy `./vim`; off by default; the setting itself is Settings'). |
| `src/input/ios-keys.ts` | 86 | iOS keyboard quirks | **as is**. New. |
| `src/markdown/contract.ts`, `extraction.ts` | 39, 204 | disk-format rules (ADR 0068), extraction | **checked first**: the same rules live in the Rust notes plugin; ported only if the editor needs them, with a test that both agree. |
| `src/dates.ts` | 11 | date helpers | **as is** (only if `daily` or `links` need it). |
| `src/field.ts`, `src/text/*`, `src/ime.ts`, `src/history.ts`, `src/probe.ts`, `src/transaction.ts` | — | ours only: the shared text-field layer (one text implementation for fields and notes, `open-questions.md` 2026-11-18), IME gate, operation history | **kept**: not in the old package (the old one had no field editor). They are the one text layer the composer and rename fields use; the block editor extends it as before. `history.ts`/`transaction.ts` stay for fields; the note path uses the old `undo.ts`. |
| `src/live/*` (extension, intent) | 229 | cursor and presence awareness, rescue intent | **kept**: ours; the old editor has awareness through its own `yCollab`; the rescue record (`LocalIntent`) is new here. |
| `test/*` (60 files) and `integration/` (5), `perf/` (2) | — | tests | **ported with each module** (a port without its tests is a rewrite); the old Playwright specs for chrome, selection and touch are ported to our harness (`apps/web/e2e`). |

## Hooks the other lanes get

- **Bar lane** (`thr_mmabkcccrr`): the editor exposes what the old `editing.svelte.ts` read: `selectBlocks`, `toggleBlock`, `clearBlocks`, `showBlocks`/`parked`/`selectedBlocks` (blocks drawn as selected), `frontmatterEnd`, `blockAt`, `plusLog` (when daily exists), `yUndoManagerKeymap`. Until `blocks/selected` lands, `block/formatting.ts` (marks, block kinds, duplicate, move, delete, the old key map) is a working stand-in written against our current editor.
- **Style-sheet lane** (`thr_sg485jnyu3`): frontmatter `calternal-*` keys are read and written through `frontmatterEnd` and `input.properties` edits (the one way into the hidden properties); this stays as the old app has it.

## Order

1. **Foundation** (first): `undo.ts`, `syntax/mode.ts` (source mode), `editor.ts` + `index.ts` with `attach(session)`/`preview`; move `NoteEditor.svelte` and `live.svelte.ts` onto them. Gate: all editor tests green, `e2e:live-typing` within the keystroke budget.
2. **Syntax**: `hidden`, `breaks`, `widgets`, `headings`, `parse-ahead`, `frontmatter`, `block-ids`, `links` with their tests. Delete `block/hidden.ts`, `block/parse.ts`, `block/headings.ts`, `block/frontmatter.ts`, `block/ids*.ts`.
3. **Blocks and lists**: `blocks/blocks`, `box`, `chrome`, `spring`, `selected`, `picked`, `touch`, `swipe-*`, `follow`, `hidden-blocks`; `lists/*`. Delete `block/blocks.ts`, `block/chrome.ts`, `block/lists.ts`. Port the chrome, selection and touch Playwright specs.
4. **Pickers and insert**: `transforms/*`, `insert/*`, `input/ios-keys`.
5. **Code and tables**: `code/*` (chip, grammars), `tables/*`.
6. **Embeds, vim**, then `markdown/*` if needed and `daily/*` when daily notes exist.

Each step is one or more small commits on this lane's branch, with the old tests ported and green, and a before/after of the keystroke cost (`e2e:live-typing`, `e2e:quiet`): nothing may run per key beyond what the old editor ran ("nothing animates after a key").

## Differences from the old app that are forced

- Spaces and UUIDs: rooms are `<space>/notes/<item>`, links are `/notes/<uuid>` (ADR 0062), no file paths or `Home`.
- Our server: saves and receipts go through `notes.update`/live rooms (already in `sync/`); no `block-jobs` operations exist yet, so "Make it a note" and "Move to note" stay disabled with the reason until they do.
- Attachments do not exist yet: Photos, Camera, File, embeds of files stay disabled with the reason (as the old app's own seam did).
