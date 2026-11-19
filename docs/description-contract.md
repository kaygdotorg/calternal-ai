# Description contract (v2)

Status: **draft 1** on `lane/description-contract`, for sign-off by the web renderer lane (leads this document) and the native renderer lane (owns the
review against SwiftUI). The plugins lane keeps runtime, authority and engines and references this document and
[`contract/description-v2.schema.json`](../contract/description-v2.schema.json) instead of defining its own description schema. Nothing here is built;
nothing in it is a promise of an implementation.

The schema is checked with `python3 contract/verify-description.py` against the fixtures in `contract/fixtures/description-v2/` (valid ones must pass,
`invalid-*` ones must fail). The fixtures are the paper exercises of section 9.

Section ownership: **1 to 4, 6, 7, 10** native lane (first draft); **5 and the content node types, 9** web lane; **8** both. Each side edits its own sections and
rebases.

## 0. What a description is

A description is data. A host (or a page that relays for it) publishes it; a renderer draws it with its platform's own controls: SwiftUI lists, buttons and
menus natively, the web client's components on the web. It is never code, never HTML from a plugin, and **it carries no viewer data**: where a person's
titles, counts or state appear, the description holds a *bind* (a request) and the host fills it at the moment of drawing, under the viewer's rights. Because it
is data-free, a description can be cached, shared and compared by digest.

Everything below is one contract for chrome (sidebar, header, footer, panel) and content (the card): the same `bind`, the same `invoke`.

## 1. Vocabulary

| Term | Meaning |
| --- | --- |
| **kind** | A type of item a plugin or the host declares (`note`, `thread`, `request`, `collection`) with its **indexed fields** and an icon. Threads and trashed items are kinds/fields the host owns. |
| **bind** | A data-free request: a list window, one item, or a search, with filter, sort and the fields wanted. |
| **row template** | How an item becomes a row: fields to title, icon, marks and actions. |
| **row scope** | Inside a row template, `{row: ...}` value sources name the row an action was activated on. |
| **invoke** | An action: an operation plus named inputs from value sources. |
| **value source** | Where a value comes from: a literal, the viewer, the route, the selection, a row, a form, a draft, a prompt, or a field of another bind. |
| **component** | A rich surface the host draws (item editor, transcript, composer, request card, sealed frame), configured by the description. |

## 2. Bind

One definition (`$defs/Bind`):

```json
{ "id": "notes",
  "source": { "type": "list", "kind": "note", "collection": { "route": "collection" } },
  "fields": ["title", "updated", "pinned"],
  "filter": { "all": [{ "field": "trashed", "op": "eq", "value": { "literal": false } }] },
  "sort": [{ "field": "updated", "direction": "desc" }],
  "window": { "size": 60, "count": "exact" },
  "row": { "title": { "field": "title" }, "icon": { "kindIcon": true }, "open": { "route": { "name": "item", "value": "id" } } },
  "states": { "empty": { "title": "No notes here" } } }
```

### 2.1 Sources

* `list`: an index window over a kind's items, optionally one **collection** (a collection id from a literal, the route or the selection, so a collection
  UUID works; it is not a static pattern).
* `item`: exactly one item by reference from the route or the selection. A literal row is never accepted.
* `search`: items of a kind matching a text value source (a form or a draft); the footer search sends the **settled** text only (about 150 ms after the last
  key), so a bind never re-queries per keystroke.

Threads, trashed items and requests are kinds or indexed fields the host (or Turns) declares: `trashed`, `trashed_at`, `waiting_on`, `pending_for` are host-indexed
fields, so the Notes Trash and the Threads Inbox are ordinary binds.

### 2.2 Filter and sort

`filter` is up to 16 predicates in `all` and `any` (no nesting) on **declared indexed fields**: `eq ne in lt lte gt gte exists prefix`. Values are value sources:
`{viewer: person}` makes "waiting on me" writable (`waiting_on eq viewer.person`); `{route: collection}` and `{literal: ...}` complete the set. `sort` is at most two
indexed fields; the item id is the implicit last key, so order is total and stable across windows.

### 2.3 Windows

The native list and the web list both ask for **index windows**, not cursors, because a long list must size its scroll bar from a count and the person walks it with
the keyboard at 30 rows a second:

```
shell -> host   { type: "window", bind: "notes", from: 120, to: 199 }
host  -> shell  { bind: "notes", revision: 41, state: "ready", count: 5000, offset: 100, rows: [ { id, fields:{...} }, ... ] }
```

* `window.size` (1 to 200, default 60) is the rows the host returns per request; `count` is `exact`, `estimate` or `none`.
* The renderer prefetches ahead of the visible rows and, when a key or a drag walks past the edge of what it holds, asks for the next window at once (no
  settling); the host may answer with a larger range than asked (`offset` and `rows` say what came back).
* `revision` identifies the result set. A new revision replaces the window wholesale; the renderer skips a replacement equal to what it draws, so a republish that
  only moved the selection costs nothing.
* **Live:** with `live: true` the host pushes a changed window (new revision) while the view is open. The renderer applies the **latest** only and, while a key
  repeats, holds incoming windows until the selection settles (or applies a few a second).

### 2.4 Row template

`title` is one field (one line; there is no second line and no free-form row content). `icon` is a catalogue name, a field's value or the kind's own icon.
`marks` (up to three) are trailing dots, counts, texts or icons, each with an optional `when` predicate and a tone. `leadingActions`, `trailingActions` and `menu` are
`invoke` definitions; only the first leading or trailing action may set `fullSwipe`. `open` navigates (sets a route parameter to the row id) or invokes.
Rows are identified by the item id everywhere: selection, events, scope.

### 2.5 States

`states.empty`, `loading` and `error` are small messages (`title`, `detail`, `icon`, an optional action). Their text is the description's or host-written; never worker
output. A renderer shows `loading` until the first window arrives, `empty` when `count` is 0, `error` when the host answers `state: "error"`.

### 2.6 What the schema needs from the host (authorization is the host's job)

The description only asks. Before any indexed work the host must:

1. authorize the **kind**, the **collection**, every **filter field**, every **sort field**, the **search** and every **output field** against the viewer, the space and,
   for a guest, the item's grant;
2. reject a field not declared indexed by the kind, and a field outside the audience's projection (a guest entitled to titles must not be able to infer hidden
   metadata from the count of an `eq` filter or from row order: filters, sorts and counts on a field the audience cannot read are refused, not filtered);
3. produce **host-owned projections** for guests: rows are built by the host from the item's grant, never by plugin code;
4. cap windows (200 rows), predicates (16), binds per view (16), and bound work by offset.

The renderer never receives a row it was not entitled to; it needs no rule of its own.

## 3. Invoke

One definition (`$defs/Invoke`) used as a content node, a row action, a header action, a footer action or a card action:

```json
{ "id": "rename", "title": "Rename", "icon": "edit",
  "operation": "host.item.rename",
  "inputs": { "item": { "row": "id" },
              "title": { "prompt": { "title": "Rename note", "initial": { "row": "field", "field": "title" } } } },
  "preview": { "material": ["item"] },
  "result": { "show": ["pending", "error", "undo"] } }
```

### 3.1 Operation and inputs

`operation` is a plugin operation id from the manifest, or a **host built-in** (`host.item.trash`, `host.item.rename`, `host.item.restore`, `host.trash.purge`,
`host.undo`, `host.approval.resolve`, `host.route.set`, `host.session.end`, `host.item.share`). Built-ins that need no worker run in the host without waking the space
runtime; a swipe on a row must not start a runtime.

`inputs` maps a name to a value source. There is no literal row and no destination the interface picks at activation: every value is a source the host can
re-resolve. Sources that live on the device: **row** (the row scope), **selection**, **form** (`{form, field}`: values of a form node held on the device),
**draft** (`{draft: composerId}`: the composer's text) and **prompt** (the shell asks for text when the action fires). They reach the host in the activation
request below, resolved and shown.

### 3.2 Activation request and the trusted preview

Activation sends the host a typed request (`$defs/Event`, `type: "invoke"`):

```
{ type: "invoke", id: "rename", bind: "notes", row: "n-7f3", invocation: "inv-91c…",
  renderedRevision: 41, inputs: { item: "n-7f3", title: "Q3 plan" } }
```

* **Resolved against what was rendered.** The host resolves every input source against the state the person saw (`renderedRevision`, the row, the selection at
  that moment). If the row left the window, the selection moved, or the revision no longer holds, the outcome is `stale` and nothing runs; the shell re-renders
  and the person acts again. A button labelled Refresh cannot submit a hidden destination or act on a different item after the selection moved.
* **Trusted preview.** For an operation the host classifies (from the manifest, not from the description) as **mutating or disclosing**, the shell shows the person the
  *material inputs* before sending: the resolved item, destination, recipient or audience, in the shell's own sheet (system confirmation dialog natively). The
  description may add material inputs (`preview.material`) or a `confirm`; it cannot remove one the manifest declares. Confirmation binds to the exact resolved
  inputs and the invocation id: a retry with different inputs asks again.
* **Retry and idempotency.** `retry: "safe"` (idempotent), `"keyed"` (default: the host de-duplicates by `invocation`, so a retry after a dropped connection does not
  repeat the effect) or `"never"` (the interface shows an unknown outcome instead of retrying). `pending: "disable"` (default) greys the action while it is in flight.
* **Full swipe.** A full swipe performs the first action only when it sets `fullSwipe: true`, and never a confirmation.

### 3.3 Outcomes

The host answers (`$defs/InvokeOutcome`): `ok`, `pending` (with a host-owned **status** reference), `error` (a code and a plain message **the host writes**), `stale`,
`refused`, plus an optional **undo** `{ref, label, expiresAt}`. `result.show` says which of toast, pending, error, undo and status the interface may show. Undo is
a button that invokes `host.undo` with the ref; the status reference is polled through a host built-in. **Worker payload is never carried to an interface.**

## 4. Chrome v2

`$defs/Chrome` replaces the chrome v1 file that lives on one branch (`contract/chrome-v1.schema.json` on `lane/ios-shell`): a real schema in the repository.

* **`binds`**: every bind of the view, by id; sidebar sections, selector counts, cards, the footer search and components reference them by id (several binds in a
  view are addressed by id, so `binding.item` is never ambiguous).
* **`sidebar`**: `cards`, `selectors`, `sections`, `title`/`titleImage`, `width`, `refresh`. A **section** has literal `rows` (fixed shortcuts), a `bind`, or both
  (literal first), `actions` and `menu` as invokes in its header, a `collapsed` flag only when it folds, and `showWhen {route, equals}` so a selector can switch
  sections. A section without a `title` draws no header and therefore no header actions.
* **`selectors`**: a segmented control whose options set a route parameter; an option's `count` is a bind with `count: "exact"`, so Inbox 4 is a bind.
* **`header`**: a literal title or a field of a bind, and up to two leading and three trailing invokes. The sidebar toggle is the shell's.
* **`footer`**: `account` (with a menu of invokes), `apps`, `search`.
* **`panel`**: the right panel, built only of host components (the side chat is a transcript above a composer).
* **`appearance`**, **`scheme`**, **`accent`**: as in v1; the page's wallpaper values are the web app's defaults.

How the native renderer draws it (the SwiftUI primitive for each part is in `docs/ios-shell.md`): sections are `Section`s of a `List(selection:)` in the sidebar style
(plain on a phone), rows are `Label` with `.badge`, actions are `swipeActions` and `contextMenu`, selectors are a segmented `Picker`, the footer is a bar with
glass controls, the header is the system toolbar.

## 5. Host components (web lane leads; native review notes inline)

Each is `{type, id, config}` in `$defs/Component`; the host draws it and receives its events directly (no page). Proposed configuration, to be settled by the web lane:

| Component | Config | Events | Native review |
| --- | --- | --- | --- |
| `item-editor` | `item` (value source: a route or selection reference), `mode` (`edit`/`read`) | `title-change` (an invoke of `host.item.rename`), `link-open` (host handles) | A native editor is a text view over the item's document; **live rooms are an attribute of the kind**, not of the component. A metadata-only save that does not rewrite the body is required (see 10). |
| `turn-transcript` | `thread` (value source), window as a bind | `quote` (ask in side chat), `open-item` | A long, streaming list: an index-window list with the latest turn growing; the renderer appends. |
| `composer` | `placeholder`, `enabled`, `send` (an `invoke`) | `draft` (device only), `send` | The draft text is held on the device and becomes operation input through `{draft: composerId}`; it is cleared only after `ok`, kept on `error`. |
| `request-card` | `request` (value source), `mechanism` (`turn-protected-request` or `job-wait-approval`) | `resolve` (an invoke of `host.approval.resolve`) | The card names which approval mechanism it serves; approve and deny show the material arguments (the trusted preview). |
| `sealed-html-frame` | `content` (a digest, host-resolved), `height` policy | `resize`, `open-link` (host handles) | A web view without network, scripts only inside the frame; it cannot reach chrome. |

## 6. What may be cached on the device

`cache.policy` is `account` (default) or `none`; `maxAgeDays` bounds it (1 to 90, default 30).

* Under `account` the shell may keep the **description** and the **last windows** for the first paint at the next launch, in one file of at most 4 MB, keyed by the
  signed-in account id. A description with host-filled binds holds a person's titles once rows are filled, so: **sign-out deletes it, switching account deletes
  it, a different account never reads it, and a policy of `none` keeps nothing.**
* The first paint from cache is marked stale; the host's first window answer replaces it (a different revision) and a row the host no longer returns disappears.
* The cache is the shell's, not the page's.

## 7. Version negotiation

* The shell advertises the versions it draws (`chromeVersions`, today `[1]`, with this contract `[1, 2]`); the host or page advertises the versions it can produce.
  The version used is the highest in common. A description carries `version`.
* A description may carry `requires: [feature, ...]` (for example `bind-search`); a shell that does not know one refuses the description whole, rather than draw it
  wrongly.
* A refusal is `{ native: false, reason, supported: [...] }`; the page then draws its own chrome. **v1 stays renderable until retired**: the retirement date is a
  decision of the owner, recorded in the document.
* Unknown fields are ignored; unknown enum values in a field the renderer needs refuse the description.

## 8. Native review (SwiftUI)

What the renderer needs from the contract, and what it costs:

1. **Windows, not pages**: `List` with `ForEach` over thousands of rows drops frames; the native list holds a window plus spacer rows sized from `count` and a
   measured real row height (the system sets row height). Index windows and a count are therefore required. Measured: a held arrow key through a 5,000-row list
   at 33 keys a second has to ask for windows without settling.
2. **Selection outside the row models**: a row reads "is selected" from the list, not from the description; the description does not carry `selected` flags for
   drawing. (A page may still say which row it selected; the shell treats a later change as a reveal request.)
3. **Equatable rows**: row values are small and comparable so an unchanged row is skipped when a window is replaced.
4. **While a key repeats, nothing crosses the bridge**: outgoing events (`selection`) go out once after the key settles; incoming windows are held and applied
   when the selection settles, decoded and compared off the main thread. A republish that only confirms the selection must not touch the list.
5. **One line per row**, one icon column per list (the system aligns titles by it), single neutral accent: the accent is passed only to the unread mark and a prominent action.
6. **Swipe actions** need `title`, `icon`, `tone` and `fullSwipe` on the invoke; they draw with the system's swipe look, tinted by tone, never by the root tint.
7. **Haptics and menus** are the shell's: selection feedback on `route` and `selection` events, a system menu for `menu`.
8. **Touch targets** are 44 pt; the renderer, not the description, sizes controls.
9. **The first paint** may come from the cache (section 6) before any host is reachable; therefore binds in the cached description have a defined stale state.

## 9. Paper exercises (fixtures)

All three are in `contract/fixtures/description-v2/` and validate against the schema. What is written and what is not:

### 9.1 Community reading list: `reading-list.json`

Written: a bound list of `reading-item` with a read mark and a per-row "Mark read" swipe action (`fullSwipe`, safe); an empty state; the header's Add invoke
whose inputs come from a form on the device (`{form: add, field: url}` and `title`), with `url` as a material input.
**Cannot be written yet:**
* the **two-field add form itself**: a `form` content node with named fields (text, URL) that holds values on the device. The value source exists; the node is the
  web lane's content vocabulary.
* the **per-person reminder**: nothing in a description can notify anyone. The need is a host-owned notification path (a schedule's missed-run rule, a reminder
  kind whose due items the host turns into notifications to that person). That is the plugins lane's (feeds and schedules), not a description feature; the
  description would only render the reminder list as another bind with a `due` mark.

### 9.2 Notes: `notes-sidebar.json`

Written: Collections as a bound section (a row sets the `collection` route); Notes as a list bound to `route.collection`, filtered `trashed eq false`, newest first,
paged by index windows (thousands of notes); a pinned mark; a swipe set (Pin, Trash with an undo outcome); a context menu **Rename with a prompt** whose result
offers **Undo** (a host-owned reference, never worker payload); a Trash view (`trashed eq true`, a count from a bind in the selector, Restore with full swipe, **Empty
Trash** as a destructive confirmed `host.trash.purge`); the footer search; the header's Share with `item` and `audience` as material inputs.
**Cannot be written yet:**
* sections **generated per collection** (a repeater over a bind); Collections is a navigation section instead.
* the **frontmatter style setting** and a metadata-only save under a live room: a content/editor matter (a `host.item.patch-metadata` built-in that is
  revision-checked against the room's head is required from the plugins lane).
* the **search results** presentation: the footer search names a `bind` in the schema but the fixture leaves it out (a search bind per kind needs the host's
  search authorization).

### 9.3 Threads: `threads-sidebar.json`

Written: an Inbox of **waiting on me** (`waiting_on eq viewer.person`) with a count in the selector; All Threads; provider icons from a field; a state mark; archive
with undo; a card shown while the `requests` bind has rows, whose action names the first request through `{bind, field}`; the **side chat** as a transcript above a
composer; a **composer that sends**: `turns.start` with `thread` from the route and `text` from `{draft: "composer"}`, keyed retry, pending and error shown; an approval
**request-card** naming its mechanism (`turn-protected-request`).
**Cannot be written yet:**
* the **provider to icon** mapping: a row's icon read from the `provider` field needs the kind to declare an icon per value (kind metadata, not description).
* **two approval models** (a job's `wait` of kind approval and Turns' protected request) are both reachable only through the `mechanism` field; the host must say
  which one a given request is. Open question for the plugins lane.
* streaming turns need the transcript to receive appended rows; the window protocol covers replacement, so appending needs `live` windows with an `offset` that
  grows; fine, but a token-by-token update of the last row is not specified (the host republishes the row).

## 10. Open questions and what this contract does not decide

1. A **metadata-only save** and the **purge** built-in must exist on the host side (plugins lane).
2. **Notification path** for reminders (plugins lane).
3. **Content node types** (form, text, list, card layouts) and how they embed `bind` and `invoke` (web lane; this document reserves `$defs.contentNode`).
4. **Generic TCP** is outside this contract; the owner's decision is recorded elsewhere and is not a description concern.
5. **Section repeaters** (one section per item of a bind) are deferred; navigation sections cover the cases above.
6. The **retirement date of v1** is the owner's.

## 11. Sign-off

* Native renderer lane: draft 1 written; review of sections 1 to 4, 6 to 8 is mine.
* Web renderer lane: to review and lead; sections 5 and 9 are theirs.
