# Drawing spike: do declarations draw on the web and in SwiftUI? · 2026-11-21

Lane `lane/spike-drawing`, issue #107. Throwaway code under `experiments/drawing-spike/`; nothing here
merges into the product. Interface work, Claude lane.

**Question.** The primitives-only host design (`docs/proposals/primitives-only-host-2026-11-21.md`,
`primitives/drawing.md` and `primitives/authoring.md` on `lane/v2-host-base`) says plugins never draw:
a kind declares its looks by naming fields, a collection declares views, the host fills the values in as
the viewer, and each client draws with its own controls. A chat is a file of records drawn by one
generic records component that contains no chat words. The simplicity review called this the riskiest
untested claim. The spike tests it on the web (Svelte) and on iPhone (SwiftUI) from the same
declarations.

**Verdict, short.** The claim holds on both platforms, with additions. Reading List and Chat's
transcript drew from the authoring section's declarations with no app-specific drawing code. It took 11
changes to the declarations (D1 to D11), 9 to the wire and the generic components (W1 to W9), and an
order-preserving JSON reader on Apple. None of them is large, and none puts plugin code in a client.
Two areas are not proven: touch gestures on iOS (swipe and long-press menus were built but not
captured), and anything the fixtures do not contain (live updates, consent, the working line, Stop).
Details follow.

## 1. What was built

| Part | Where | What it is |
| --- | --- | --- |
| Declarations | `decl/*.ts` → `data/declarations.json` | authoring.md 2.1 (Reading List) and 2.3 (Chat core and `calternal-chat-ai`) copied as written, plus the official composer part. A data-only `sdk.ts` serialises them; handlers become `"code"`. Changed lines end in `// Dn`. |
| Fixtures | `data/make-fixtures.mjs` → `data/fixtures.json` | What a host would hand the viewer Ana: windows per collection with look values filled in, one record file (27 records over two days, in two pages), quotes resolved as Ana, Ana's position and unread count, and the fillers of the slots. No server. |
| Web | `web/` (Svelte 5, Vite, `packages/ui` by path) | `lib/model.ts` (lookups), `lib/look.ts` (looks, predicates, derived actions and forms), views `RowLook`, `CardLook`, `Strip`, `Sidebar`, `CollectionView` (list and board), `ItemPage`, `Records`, `Composer`, `FormSheet`, `Inline`. Built and captured on `calternal-dev-02-std`. |
| iPhone | `apple/` (SwiftUI, iOS 27, one target, file-system-synchronised Xcode project) | The same rules in `Look.swift`; `Lists.swift` (sidebar, rows, strip, list, board, card), `Records.swift` (records, composer, inline text), `FormSheet.swift`, `Shell.swift`. Built and run in the spike's own Simulator (`drawing-spike`) on `kayg@std`. |

No file in either renderer names a plugin, a kind, a field or an operation. Everything is looked up from
the declaration. `grep -i 'chat\|message\|harness'` over both renderers finds only the host icon table
(`chat` → a glyph) and doc comments.

## 2. What drew cleanly from the declarations alone

Both platforms draw all of these from the same JSON:

- **Reading List sidebar**: two collections, each shown small, with their counts. **Add** comes from
  the `create` operation for the collection's kind (a "+" in the section header and the collection's
  toolbar). It opens a form derived from `inputs: ["url", "title"]`, typed from the kind's fields
  (a URL keyboard on iOS).
- **Row look**: `title`, `detail: ["site(url)", "created"]` as a quiet second line, and `trailing: [{ dot:
  { when: "!read" } }]` as the unread dot (it also bolds the title). The `card` look draws on the
  board. Defaults fill whatever the kind leaves out (no leading becomes the app icon; cell falls back to
  row).
- **Mark read** from the `set` operation: a trailing swipe action on iOS (`.swipeActions`), an icon
  button on the web row, and an entry in the context menu on both. `when: "!read"` hides it on items
  already read. The context menu also holds the host's Rename, Move, Share and Trash.
- **List and board views of "All"**: a list grouped by `read`, and a board with one column per value.
  The person switches between them (a segmented control on iOS, a `Selector` on the web; interface
  state, not declaration). On iOS a card dragged to the other column becomes `host.files.set`
  (`.draggable` / `.dropDestination`).
- **Chat sidebar**: the Pinned strip (`view: "strip"`), with the avatar as the tile, the title as its
  label and the unread count as a badge. Chats and Archived follow as compact lists. Each row is
  `title` (a 1:1 chat falls back to the other person's name), the other people's avatars,
  `last.at`, and `{ count: "me.unread", quiet: "me.muted" }`; the muted 12 is drawn grey.
- **Chat transcript** from `page: { records: … }`:
  - own records on the trailing side
  - one author's records within five minutes grouped under one name, with tighter corners
  - day lines ("Yesterday", "Today")
  - an unread line after Ana's stored position, opened there
  - a quote that references another record, resolved as Ana; tapping it scrolls to that record
  - a quote of a redacted record draws nothing
  - the redacted record itself draws nothing
  - the edited mark
  - the agent reply, drawn as "Claude for you" with the run's mark
  - a mention of the viewer, tinted
  - `code` and **bold** in the inline subset
  - paging upward with the first visible record kept in place
  - jump to the latest, with the unread count
- **Record actions** from `page.records.actions`, filtered by `when: "own"`:
  - **Reply** fills the composer's quote and sends nothing.
  - **Edit** opens a form derived from `calternal-chat/edit`, prefilled with the record's body.
  - **Delete for everyone** shows the declared sentence in a system confirmation (`confirmationDialog`
    on iOS).
  - **Ask an agent** opens a form for `calternal-chat-ai/call`: a prompt, plus a picker of the harness
    slot's fillers.
- **Composer**: the part the viewer picked for `calternal-chat/composer`. The text field holds the
  draft, which is stored per item; a quote picked by Reply shows above it; Send is one call to
  `calternal-chat/send` with `{ chat, text, quote }`. A missing part draws a notice naming the
  recommended part.

Every gesture ends in the stand-in call table as `operation + inputs`. The web shows the last calls at
the foot of the sidebar; iOS shows them in a "Calls (spike)" section.

## 3. Changes to the declarations (the main result)

Each change is the smallest one that let the declaration be drawn. Shapes are as written in
`decl/*.ts`.

| # | Where | Change, exact shape | Why it could not be drawn without it |
| --- | --- | --- | --- |
| D1 | Reading List `done` | `set("reading", { read: true }, { title: "Mark read", icon: "check", when: "!read", place: "swipe" })` | `set` had no label, icon or placement, so a derived action has nothing to show and nowhere to go. Without `when` it is offered on items already read. drawing.md 5.1 had a placement argument; authoring.md dropped it. |
| D2 | Reading List `add` | `create("reading", { inputs: [...] }, { title: "Add link" })` | The "New" button and the form need a name. Deriving one from the kind ("New reading") reads badly, and kinds have no display name. |
| D3 | field builder | `url().label("Link")` | A derived form and the inspector show the field name. "Url" is wrong, and authors will want words other than their identifiers. Also the place for translations later. |
| D4 | field builder | `bool(false).values({ false: "To read", true: "Read" })` | A board's columns and a grouped list's headings need names for the values. "false" and "true" cannot be shown. (A `choice([...])` field type would carry the same thing for non-boolean fields.) |
| D5 | Reading List "All" | `collection("All", { …, group: "read" })` | A board needs a group-by field. authoring.md's `collection()` has none; drawing.md puts it in the collection file's options. |
| D6 | Chat records | record field `quote: link()`, a record look `record: { body: "body", quote: "quote" }`, and `send`/`reply` writing `fields: { quote }` instead of `[↩](<#^anchor>)` at the start of the body | The generic component would otherwise have to know that a link at the start of a body is a quote, which is Chat's convention, not a Markdown rule. The look's `quote` place (drawing.md 1.3) names a field, so the quote has to be one. `send`'s `quote` input also becomes `link()` (it holds an anchor, not text). |
| D7 | Chat `edit` action | `defaults: { text: "record.body" }` beside `inputs` | `inputs` fixes values. An edit form must *start* from the record's text, which is a different thing: a default the person changes. |
| D8 | Chat `ask` action | `pick: { harness: "calternal-chat-ai/harness" }` | `harness: text()` as a form field would ask the person to type a plugin id. The input has to be a choice among the parts filling a slot (authoring.md item 2). The host fills the list as the viewer (`slots[...]` in the fixtures). |
| D9 | composer part | `composer: { send, draft, mention, inputs: { chat: "this", text: "draft", quote: "quote" } }` | The slot `gives: ["this", "draft", "quote"]` and `send` takes `{ chat, text, quote }`, but nothing said which goes where. Matching by name or type is a guess. authoring.md writes no official composer; this is drawing.md 5.3's sketch in authoring's shape. |
| D10 | collection identity | none in the declaration; the spike uses `<plugin>/<title>` | A sidebar file links collections by id, and a package collection has only a title. The host should derive or require an id. This is noted here rather than changed. |
| D11 | Chat row | `title: ["title", "people"]` (the first non-empty one) | A 1:1 chat has no title. The row would say "Untitled" unless the look can fall back to another field. This is the only fallback form added; it is a list, not an expression. |

The declaration lines the spike did *not* need to touch: Reading List's row, card and two collections;
Chat's `row` (apart from D11), `page.records.actions` for reply and delete, the `composer` slot, all
three sidebar collections, `position: "read"`, `edit`/`redact` policies and every operation input.

## 4. Additions to the wire and the generic components

These are not in any declaration; the host and clients must agree on them.

| # | Addition | Exact shape (as in `data/fixtures.json`) | Reason |
| --- | --- | --- | --- |
| W1 | Typed values in windows | an instant is `{ "at": "2026-11-20T10:15:00Z" }`; a person list is `{ "people": [{ "id", "name", "me": true? }] }`; anything else is a JSON scalar | The renderer formats times in the viewer's zone and drops the viewer from avatar lists. It cannot tell `created` (a host field) or `last.at` (derived) from text otherwise. The alternative is a host table of host-field types in every client. |
| W2 | Values keyed by the look's field names, including derived ones | `"site(url)": "brandur.org"`, `"last.excerpt"`, `"me.unread"`, `"me.muted"` | The window carries exactly what the looks and their `when` predicates name, plus the collection's `group` field. That is how "the plugin never wakes" holds. The window must include fields that are only predicates (`read` for the dot), not just the visible places. |
| W3 | Records window | `{ position, unread, page: { records, refs, before }, pages: { <cursor>: … } }` | Paging from the bottom needs a cursor. The unread line needs the stored position and the count (the host computes both: `after_position`). |
| W4 | Resolved references | `refs: { a23: { author, excerpt } }`, or `{ gone: true }` for a redacted record | A quote may point at a record in a page not loaded. The host resolves it as the viewer; the client never fetches another person's record to draw a quote. The excerpt should be plain text (the fixture's has backticks, and they show). |
| W5 | Run identity in the stamp | `stamp: { person, run: { name: "Claude", icon: "claude" }, at }` | "Claude for you" needs the run's display name and mark (authoring.md item 11). The mark's colour is hard-coded per renderer today. It belongs with the icon vocabulary (W7). |
| W6 | Slot fillers as values | `slots: { "calternal-chat/composer": "calternal-chat-composer", "calternal-chat-ai/harness": [{ plugin, name, icon }] }` | The viewer's pick for a slot, and the choices for a `pick:` input (D8). |
| W7 | A closed icon vocabulary | `book`, `chat`, `check`, `edit`, `spark`, `claude`, `codex`, … mapped to `packages/ui` icons and to SF Symbols (`web/src/lib/icons.ts`, `symbol()` in `Look.swift`) | Declarations name icons. `packages/ui` has no `book` or `chat`, and SF Symbols names differ. The host needs one list of icon names with a mapping per platform, checked at install. |
| W8 | Mentions in bodies | `[@Ana](person:p-ana)`: a Markdown link with the `person:` scheme | The generic inline renderer draws a person link as a mention (tinted when it is the viewer) without knowing Chat. `send` parsing `@<id>` out of plain text (authoring.md item 10) does not survive the composer showing names. |
| W9 | Key order | JSON objects in declarations (`fields`, `operations`, `actions`) are read in order | The order of actions is the menu order; the order of fields is the form order. Browsers keep object key order. `JSONDecoder` and `JSONSerialization` do not, so the Apple client has its own 82-line parser (`JSON.swift`). The declaration wire format should say that order matters, or use arrays. |

Generic component behaviours that came from neither section and that every platform has to implement
the same way: grouping (same author, same day, within five minutes); "own" is the stamp's person when
there is no run; the unread line sits after the position and opens the view there; a redacted record is
omitted and a quote of it draws nothing; a person list in a look omits the viewer; a missing look
falls back (card to row, row to `title`). Each was 2 to 6 lines per platform. They belong in drawing.md
as rules, so that the two renderers cannot drift.

## 5. Native controls on iPhone

**System controls used:**

- `NavigationSplitView`, collapsing to a stack on iPhone, with `preferredCompactColumn`
- `List` with `.listStyle(.sidebar)`, `Section` and selection
- `.swipeActions` and `.contextMenu`
- toolbars, with a segmented `Picker` for list or board
- `Menu` with a `Picker` as the app switcher
- `Form` in a sheet with `presentationDetents`, `TextField(axis: .vertical)` and `Picker`
- `confirmationDialog`
- `ContentUnavailableView`
- `.draggable` and `.dropDestination`
- `ScrollView` with `.defaultScrollAnchor(.bottom)`, `.scrollPosition(id:)` and `onScrollGeometryChange`
- `.scrollTargetBehavior(.viewAligned)`
- the glass button styles and `.glassEffect` for the composer

**Custom, each with its written reason:**

| Element | Reason |
| --- | --- |
| Message bubble (`UnevenRoundedRectangle` with grouped corners) | SwiftUI has no chat or timeline control. |
| Day chip and unread line | No system equivalent. Each is a `Text` on a capsule, or two `Rectangle`s and a label. |
| Quote block (a bar and two lines) | No system equivalent. It is a plain `Button`, so it is accessible. |
| Board columns | No system board exists, as drawing.md 1.4 predicted. It is a `LazyHStack` of scrolling columns, one column per page. |
| Strip | No system horizontal list in a sidebar. It is a `ScrollView(.horizontal)` inside a `List` row. |
| Avatar | A stand-in with initials (`Av`). The host would send a picture file. |

Nothing in the transcript needed a sealed frame or web content. The open question 6 of drawing.md (a
generic records view in the catalogue) is answered yes by this spike: it is about 220 lines of SwiftUI
and draws with system scrolling, menus and sheets.

## 6. Size

Non-blank, non-comment lines.

| | Lines |
| --- | ---: |
| Reading List declaration (whole manifest) | 11, of which the interface (looks, action placement, sidebar) is about 6 |
| Chat core declaration (whole manifest, with handlers) | 51, of which the interface (row, record look, page and actions, slot, sidebar) is about 13 |
| Official composer part | 7 (3 of them interface) |
| `calternal-chat-ai` (copied only for its `call` declaration) | 30 |
| Web generic renderer (TypeScript and Svelte, of which 125 are CSS) | 640 |
| SwiftUI generic renderer (of which 82 are the JSON reader) | 705 |
| Fixture generator (stands in for the host's windows) | 87 |

So about 22 interface lines of declaration for two apps against about 650 to 700 lines of generic
renderer per platform. The renderer is a fixed cost: a third app of the same shapes adds declaration
lines only. For comparison, revision 14's fixtures for the same two apps are 285 (`reading-list-app.json`)
and 143 to 493 (`threads-*.json`) JSON lines, and the hand-drawn mockup's `Conversation.svelte` alone is
103 dense lines for one fixture chat.

## 7. What I did not get to, said plainly

- **Touch gestures on iOS are built, not captured.** Swipe actions, long-press context menus and board
  drag on iOS use the system modifiers, but `simctl` cannot long-press or swipe, so the captures show
  their results (the form, quote and confirmation states) reached through launch arguments instead. Web
  menus are captured.
- **Web swipe is not implemented.** An icon button stands in on the web row. Moving a card on the web
  board is a right-click to the other column, not a drag.
- **Selection ranges.** `records.selection` binds to the one record the menu was opened on. Neither
  platform has a multi-record selection mode yet.
- **Not in the fixtures, so not drawn:** live updates while open (a record arriving and "1 new"),
  the consent request, the working line and Stop, typing, attachments, mention autocomplete in the
  composer, iPad and Mac layouts, gallery, table, calendar and facet views, Notes' editor page.
- **No device check.** Motion is token durations on the web and system defaults on iOS. Neither was
  checked for fluidity on a real device, and no VoiceOver or keyboard audit was run.
- **The web shell** is a plain two-pane layout, not the mockup's floating shell, wallpaper and
  switcher card. It reuses `packages/ui` surfaces (`Selector`, `Menu`, `Sheet`, `Button`,
  `IconButton`, `Icon`) and tokens, and the transcript follows the mockup's `Conversation.svelte`.
- **`packages/ui` imports the note editor** through its index and `Menu` (`lib/textfield.ts` →
  `packages/editor`), so even a list-only client pulls in CodeMirror. The spike installs those
  dependencies to build.

## 8. Verdict

**"A kind declares looks, a records component draws any record file" holds on both platforms**, given
D1 to D11 and W1 to W9. None of the changes puts code or computed text into a declaration. The largest
are D6 (a quote is a record field, not a body convention), D9 (a slot part binds the operation's inputs
explicitly) and W1 (values carry their type). Every other change is a label, a placement or a value
name. The generic records component has no chat words, and it would draw a comment thread or an agent's
log from the same fields, because it uses only stamp, body, quote reference, edited and redacted.

What the design must take from this before step 9:

1. Add D1 to D9 and D11 to the authoring vocabulary. Write the component rules from section 4 into
   drawing.md as normative.
2. Specify the window wire format (W1 to W4) and the run identity in stamps (W5), and give the
   declaration format a defined order (W9).
3. Make the icon vocabulary closed and per-platform (W7). A declaration naming an unknown icon should
   fail at install, not draw a generic glyph.

## Captures

All on #107 (fixture data only): web at desktop and phone width and the iPhone Simulator, light and
dark, with the phone-width web next to iOS for nine screens.

## Simplification opportunities

- `create` and `set` with D1/D2's display options make Reading List's whole interface declarative. The
  per-operation `place` could default from the operation type (a `set` that has a `when` goes to the
  swipe), which would remove most of D1.
- D7's `defaults` and the composer's `inputs` binding (D9) are the same idea, "where an input's value
  comes from". One `from:` map on actions and parts would cover both: `{ text: "record.body" }`,
  `{ chat: "this", text: "draft" }`.
- The row look's `trailing` count and the strip's badge are the same mark. The strip can keep reading
  the row's marks, as the spike does, with no strip-specific place.
- `packages/ui`'s index should not pull in the editor. Split `textfield.ts` (or `Menu`'s use of it) so
  that list-only clients do not ship CodeMirror.
- The Apple JSON reader disappears if the declaration format uses arrays for ordered things (W9).
