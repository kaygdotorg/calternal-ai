# Description contract (v2)

Status: **draft 1** on `lane/description-contract`, for sign-off by the web renderer lane (leads this document) and the native renderer lane (owns the
review against SwiftUI). The plugins lane keeps runtime, authority and engines and references this document and
[`contract/description-v2.schema.json`](../contract/description-v2.schema.json) instead of defining its own description schema. Nothing here is built;
nothing in it is a promise of an implementation.

The schema is checked with `python3 contract/verify-description.py` against the fixtures in `contract/fixtures/description-v2/` (valid ones must pass,
`invalid-*` ones must fail). The fixtures are the paper exercises of section 9.

Section ownership: **1 to 4, 6, 7, 10** native lane (first draft); **5 (content nodes and host components) and 9** web lane; **8** both. Each side edits its own sections and
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

`filter` is up to 16 predicates in `all` and `any` (no nesting) on **declared indexed fields** of the kind in the host catalogue (`contract/host-catalogue-v2.json`):
`eq ne in lt lte gt gte exists prefix`; `in` takes a homogeneous literal list of at most 32. Values are `FilterValue`s: a literal, `{viewer: person}` (so "waiting on me" is
`waiting_on eq viewer.person`; `waiting_on` and `pending_for` accept only `self` as an operand), `{route: ...}` or `{selection: ...}`. **Never a field of another bind, a row, a
form or a prompt**: a hidden field of one bind must not steer another bind's query (the cross-bind operand leak). A search takes a `TextSource` (a literal, a form
field or a composer draft). `{bind, field}` stays legal only in an invoke's inputs and a component's config, where the host reconstructs it. `sort` is at most two
indexed fields; the item id is the implicit last key, so order is total and stable across windows.

### 2.3 Windows

The native list and the web list both ask for **index windows**, not cursors, because a long list must size its scroll bar from a count and the person walks it with
the keyboard at 30 rows a second:

```
shell -> host   { type: "window", bind: "notes", from: 120, to: 199 }
host  -> shell  { bind: "notes", revision: 41, state: "ready", count: 5000, offset: 100, rows: [ { id, fields:{...} }, ... ] }
```

* A window is the half-open range `[from, to)` of at most 200 rows; the highest supported ranked offset is 100,000 and the host never scans linearly to reach an offset
  (a request past the limit is refused, and the renderer shows the end). `window.size` (1 to 200, default 60) is the rows returned per request; `count` is `exact`,
  `estimate` or `none`.
* The renderer prefetches ahead of the visible rows and, when a key or a drag walks past the edge of what it holds, asks for the next window at once (no
  settling), coalesced to at most 4 window **requests** a second; the host may answer with a larger range than asked (`offset` and `rows` say what came back).
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

5. authorize the **whole dependency graph** of a view, not each bind alone (no bind takes an operand from another; a search is pinned to the audience's corpus; a
   count is taken over the already-authorized projection); a guest gets an `exact` count only over an explicit shared root/child projection with count disclosure enabled,
   otherwise `estimate` or `none`;
6. bound the aggregate: at most **16 binds per chrome and view together** per open surface, and per actor and per space caps on open subscriptions, window bytes and
   invalidations a second, coalesced before querying; pushes are **host-only** and **never wake the space runtime**;
7. issue, with each rendered window, the **activation context** that later actions name (the plugins lane's activation protocol): the host reconstructs row, route and
   selection from it and never accepts the renderer's value for them.

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

`operation` is a plugin operation id from the manifest, or a **host built-in from the host catalogue** (`contract/host-catalogue-v2.json`, written by the plugins lane:
exact ids, typed inputs, effect class, authority, whether it may be a generic invoke, whether it may be a full swipe, worker-wake behaviour). Built-ins that need no worker
run in the host without waking the space runtime; a swipe on a row must not start a runtime. The catalogue, not this document, says which operations a description may name:
`host.approval.resolve` (the request card), `host.undo` and `host.action.status` (an outcome), `host.panel.ask` (the transcript's `onQuote`), `turns.start` (the composer's
`send`), `host.trash.purge` and `host.item.share` (protected sheets) are not generic invokes. A description reaches the Trash and share sheets and shell-only controls
through a `ShellAction` (`shell: {sheet: trash | share}`, `{route}`, `{reload}`, `{signOut}`); `host.route.set` and `host.session.end` are shell controls, not operations.
Chrome-level action lists take an `Action` (an `Invoke` or a `ShellAction`).

`inputs` maps a name to a value source. There is no literal row and no destination the interface picks at activation: every value is a source the host can
re-resolve. Sources that live on the device: **row** (the row scope), **selection**, **form** (`{form, field}`: values of a form node held on the device),
**draft** (`{draft: composerId}`: the composer's text) and **prompt** (the shell asks for text when the action fires). They reach the host in the activation
request below, resolved and shown.

### 3.2 Activation: context, resolve, trusted confirm, dispatch

The shapes are the host catalogue's (`contract/host-catalogue-v2.json`: `ResolveActivation`, `ConfirmActivation`, `DispatchActivation`, `ActivationResolution`, `ActionOutcome`,
`Interaction`); this document cites them and defines none of its own (`$defs/Event` and `$defs/InvokeOutcome` reference them). An activation is three round trips in a fixed order:

```
shell -> host  { type: "invoke", action_id: "rename", activation_id: "a-91c", activation_context: "<opaque>", interaction: { prompt: { title: "Q3 plan" } } }
host  -> shell { activation_id, resolution_ref, confirmation_challenge?, material?: [ {name, label, value} ] }       // resolve
shell -> host  { type: "invoke_confirm", activation_id, resolution_ref, confirmation_challenge }                      // only if the host asked, after the person confirmed
shell -> host  { type: "invoke_dispatch", activation_id, resolution_ref, confirmation_proof? }                       // dispatch
host  -> shell { activation_id, state: ok | pending | error | stale | refused | unknown, status_ref?, undo?, code?, message? }
```

* **The event carries a context and ONLY device-held values.** `activation_context` is the host-issued snapshot of what was rendered (viewer, space, template digest, action,
  route, row, the versions of the binds the action depends on): opaque to the renderer. `interaction` holds only what lives on the device: `form` values, `prompt`
  answers, the `draft` (text, host-owned attachment references, mentions, a quote reference) and a host-issued `selection_ref`. **The event never carries a resolved row, selection,
  route, bind value, target item or viewer; if a modified client adds one, the host ignores it** (the schema refuses the field outright). The host reconstructs every server-owned
  source from the context and so a changed client cannot name another item than the one drawn. There is no single revision number: the context names the versions it
  depends on, so an unrelated window change neither retargets nor invalidates an action, and a row that left what was rendered is `stale` (nothing ran).
* **Trusted confirm.** For an operation the host classifies as mutating or disclosing (from the catalogue and the manifest, never from the description) the host answers
  the resolve with a `confirmation_challenge` and a **material** list it wrote: the item, destination, recipient, audience, permission change or count, resolved from what it
  reconstructed, **including values the person did not edit** (an untouched prefill, a default, an autofill). The shell shows the material in its own sheet; if the person
  confirms, `invoke_confirm` returns the challenge and the host returns a `confirmation_proof` bound to the normalized input and the `activation_id` (different input: a new
  challenge). A description may add material inputs (`preview.material`) but cannot remove or relabel one the host requires, and a destination, recipient, audience or
  permission is **always** previewed.
* **Typing is never consent.** The host previews **every material field** of a mutating or disclosing operation, from the catalogue and the manifest, not from the description:
  the destination, recipient, audience, permission change, provider, model, machine and other options, attachments, mentions, and **defaults, prefills and autofill**,
  whether or not the person touched them. What a person typed (a composer's text, a form field, a prompt answer) is shown by its own control, and the host's material list may
  therefore leave out the text itself, but **no value counts as consented to because it was typed, and no input is exempt from the host's classification**. A *trusted* control
  (the composer, the form's own sheet) may present the host's material list in place, next to the field, **as long as the host wrote that list and the confirmation goes through the
  protected confirm step**; the shell never sends a flag claiming what was typed or consented, and the host never believes one. A chat send (`turns.start`) is a mutating,
  external operation: the composer shows the host-written material (thread, provider, model, machine, attachments, mentions, quote) and the send is the confirmation the host
  challenges; a Share with an audience always opens the host's sheet.
* **A row moved by churn is still actionable.** The context names the target, its scope and the versions the action depends on. A row that merely scrolled out of the window
  or moved position because of unrelated changes is **still actionable** (nothing the action depends on changed). Only a change to the **target, its scope or a dependency** makes
  the activation `stale` (nothing ran).
* **Retry and idempotency are the host's.** A description cannot declare an action retry-safe or waive a check: effect class and idempotency come from the catalogue and the
  manifest, and the host de-duplicates by `activation_id` (a retry after a dropped connection repeats no effect).
* **Full swipe.** A swipe action may set `fullSwipe` only if it is the first action of its set **and** the catalogue marks its operation `full_swipe: true` (a reversible host
  built-in that returns an Undo and needs no confirmation); a full swipe never opens a confirmation. Today the catalogue marks none, so no description sets it (Restore from Trash
  included, until the plugins lane marks it).
* **No generic route to protected operations.** A description cannot name `host.approval.resolve`, `host.undo`, `host.action.status`, `host.panel.ask`, `host.route.set`,
  `host.session.end`, `host.trash.purge` or `host.item.share` as an invoke, nor `turns.start` or `host.turn.stop` anywhere but the composer (send, stop): they are reached
  through the request card, an outcome's undo reference, the host's quote control, shell events, the Trash and Share sheets and the composer.

### 3.3 Identifiers: action, activation, request, ticket

Four identities, defined once with the plugins lane's activation protocol:

* **`action_id`** (`invoke.id`): chosen by the description's author, stable across revisions (`rename`, `trash`), unique within a description; names the action in events and
  pending state.
* **`activation_id`**: fresh per activation, generated by the renderer; the key the host de-duplicates by.
* **`request_id`**: the host's durable request id behind an activation; **host-side only**, never in a description, never needed by a renderer.
* **`ticket`**: the opaque space-channel ticket; **wire only, never durable, never visible to a renderer, never a receipt or an idempotency key.**
  Descriptions, events and outcomes never carry it.

An activation names the host-issued **activation context** (an opaque snapshot of what was rendered, section 2.6) and carries only values held on the device.

### 3.4 Outcomes

The host answers a resolve with an `ActivationResolution` and a dispatch with an `ActionOutcome` (both the catalogue's): `ok`, `pending` (with a host-owned `status_ref`, polled
through the host's own status control, not by the description), `error`, `refused`, `stale` and `unknown` (the interface shows an unknown outcome instead of retrying a
non-idempotent action), and an optional host-owned **undo** `{ref, label, expires_at}` on `ok`. `result.show` says which of toast, pending, error, undo and status the interface may
show. Error text is written by the host. **Worker payload is never carried to an interface.**

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

## 5. Content nodes and host components (web lane)

### 5.0 Content nodes (`$defs/View`, `$defs/ContentNode`)

A **view** is the content of the card for one place, as a sibling of chrome: `{version: 2, kind: "view", revision, title?, binds, root}`. It is data-free like chrome:
binds are requests the host fills; a view never carries a row, a title of a person's item, a message, or HTML. It is drawn with each platform's own controls (Svelte on
the web, SwiftUI natively); no plugin is written twice, and anything the vocabulary cannot say stays a **host component** (5.1), never plugin-drawn.

| Node | Says | Embeds |
| --- | --- | --- |
| `stack` | children in a column or a row, optional title | nodes (depth at most 6, 200 nodes per view, 24 children per stack) |
| `text` | fixed copy by the author: `title`, `heading`, `body`, `caption`, optional tone | nothing |
| `notice` | a message panel (tone, title, detail) with an optional action | `invoke` |
| `button` | an action, label from the invoke's `title`, `primary` or `quiet` | `invoke` |
| `form` | named fields held on the device, one submit | `invoke` (its inputs read `{form, field}`) |
| `list` | a bound list; `selectable` adds the keyboard model | `bind` by id (the bind's row template draws the rows) |
| `detail` | labelled fields of one bound item | `bind` by id (an `item` source) |
| `component` | a host component (5.1) | `Component` |

* **No free layout.** There are no sizes, colours, fonts, margins or custom controls; spacing, type and motion come from the renderer's tokens, so the web and the
  Mac and iPhone apps stay one product. Anything beyond the vocabulary is a request to extend it, not a hole to draw through.
* **A `choice` prompt** (`prompt.kind: choice`, options `{bind}`, at most 50 rows) asks for one row of a bind (Move to collection, a tag): the pick is not a client value: the shell asks the host for a `ChoiceRef` for the picked option row and sends it in `interaction.choices` under the prompt's slot (the catalogue's name), and the host resolves the row from the authorised option bind and never accepts a client-supplied collection or item id as the choice. Move to collection is `host.collection.move {item | selection_ref, collection}` with the collection as the choice. The option bind is authorised like any other. It is the only way a description offers a choice from data.
* **Forms** hold their values on the device. A field is checked there (required, length, `url`/`email`/`number`/`date` shape, `choice` among options) and the submit
  is disabled while a required field is empty or invalid and while the action is pending. The values leave the device only inside the submit's resolved inputs
  (`{form: id, field: fieldId}`); the form shows what was typed, and by the rule in 3.2 the host still previews every material field (typing is never consent); a form may present that list in place. After an `ok` outcome the fields clear (`clear: onOk`,
  the default); after `error` or an unknown outcome they keep what was typed. Errors appear in a `notice` above the submit, never as loose text.
* **Lists** are binds. `selectable: true` gives the keyboard model of `docs/native-feel.md` section 20 on every platform: Up/Down/Home/End/Page Up/Page Down move
  the selection through ALL rows (drawn or not), the selection paints at once, and what the selection opens (a route change) happens when it settles, **about 100 ms after the last key** (or key up); **selection and navigation events wait while a key repeats** (one goes out about 100 ms after the last key), incoming windows are held and applied when the selection settles, and window **requests** for prefetch may still cross, coalesced to at most 4 a second (8.4)
  (section 8, item 4). The contract's figure is about 100 ms; the web renderer's rest is adaptive: 1.6 times the longest gap between repeats in the burst, at least 45 ms and at most 120 ms, which is the same rule measured on the web; a load
  never starts while keys are still repeating, and a new key cancels the previous load.
* **Home, End and Page Up/Down** over rows that are not loaded are a renderer duty: it asks the host for the window around the target index (`0`, `count - 1`, the current
  index plus a page) at once, moves the selection when the rows arrive, and the host only answers windows.
* **A `list` takes the remaining height of its stack and scrolls itself.** One `list` per scroll region: a `stack` that holds a list does not scroll, and nested scrolling
  regions are refused (a long list inside a scrolling column loses laziness, selection and windows on every platform). The selected row's id is the `selection` source for that bind.
* **Selection is not state in the description.** A renderer reads "selected" from the list; a description never carries `selected`.
* **Budgets** are the host's and the verifier's: at most 16 binds per view, 24 children per stack, depth 6, 200 nodes, 12 form fields, 4 KB per field value.

### 5.1 Host components (`$defs/Component`, five types)

Each is `{type, id, config}` with a typed `config` (a closed object: an unknown key is refused); the host draws it and receives its events directly (no page, no
plugin code). A description can run a named **invoke** on a component's event only where a config says so (today only `onTitleChange`); every other event, **including a composer's send and stop and a transcript's quote, is the host's alone**.

| Component | Config | Events (typed; handled by the host unless an invoke is named) | Native review |
| --- | --- | --- | --- |
| `item-editor` | `item` (route or selection, never a literal id), `mode` edit/read, `showTitle`, `onTitleChange` | `title-change` (the host saves through `host.item.rename`), `link-open`, `presence` (live rooms: an attribute of the item's KIND) | A text view over the item's document (a note's Markdown file directly: no second representation between the editor and the file, an owner rule). A metadata-only save must not rewrite the body. |
| `turn-transcript` | `thread` (typed thread ref from route or selection), `follow` end/none, `window`, `quote` (boolean: the host's Quote control) | `transcript.page` (the host route: `TranscriptPageRequest` -> `TranscriptPage`, catalogue), `quote` (a host-issued quote reference into a draft), `scrolled-away`, `rendered` | A host-owned windowed list: the screenful at the end is read first and drawn, the rest follows in steps; rows are appended and updated by the host; heavy parts are the host's. |
| `composer` | `thread` (typed thread ref), `controls` (which host controls appear: `attachments`, `mentions`, `provider`, `model`, `reasoning`, `permission`, `machine`, `stop`), `placeholder` | `draft` (device only), `send` and `stop` (the host's), `attach`, `mention`, `composer.options` (`ComposerOptionRequest` -> `ComposerOptionPage`, catalogue) | The draft is held on the device. The host owns the send: it composes `turns.start` from the thread, `interaction.draft` and `interaction.options` and answers in one exchange; a native composer is a text field with the system keyboard and system pickers. |
| `request-card` | `request` (typed request ref), `compact` | `resolve` (host.approval.resolve, from the host's own buttons) | One protected record and resolver for a job, a Turn and a host action; the card shows the material arguments (the trusted preview) and cannot be reordered, relabelled or covered by a plugin layout. |
| `sealed-html-frame` | `frame` (a frame id), `minHeight`, `maxHeight` | `sealed_frame.open`, `sealed_frame.resize` (inside the caps), `sealed_frame.open_link` (a host link id; the host re-authorises), all catalogue routes | A web view with no network and no top navigation; its message channel and sandbox are fixed by the host. |

**Shared shapes cited, not copied.** Ticked rows of a multiple-selection list travel as the catalogue's `SelectionSetRequest` and are answered with its `SelectionSetResult` (the
`selection_ref` actions use); the options of a composer control (provider, model, reasoning, permission, machine) are asked for with `ComposerOptionRequest` and returned as a
`ComposerOptionPage`: the renderer never lists an option the host did not return.

**Rules that hold for all five**

* A component is **configured, not wrapped**: a description cannot place it inside anything that clips, scales or virtualises it, and cannot draw over a
  `request-card`. Its size is the card's (the transcript and editor fill it; the composer is at the bottom; the frame reports its height inside the caps).
* **Refs are typed.** `thread`, `item` and `request` take a value source that resolves to a reference of that type or the host refuses the description; the host
  authorises the viewer for the ref (a guest's grant included) before anything is read. The description holds no viewer data.
* **Drafts and options.** The composer's text stays on the device; attachments are handed to the host when attached and held as host-owned `AttachmentRef`s, mentions are account references, a quoted passage is a host-issued quote reference (the catalogue's `Draft`), and the chosen provider, model, reasoning level, permission and machine are opaque option references the host returned (`Interaction.options`). The description names none of them: it says which controls appear. The host checks everything (lengths, counts, that every attachment is the viewer's own, that every option is one it offered for this thread) and composes the send; it never trusts the client. The draft clears only after `ok`; `error` keeps it.
* **Transcript reads** use the catalogue's `transcript.page` route: a `TranscriptPageRequest` names the thread and an opaque `after` cursor, the `TranscriptPage` returns at most 128 committed events and a `committed_cursor`; every page is reauthorised and bounded (128 KiB); an expired or revoked cursor is refused and the renderer reads again from the end. No description or renderer invents a cursor.
* **Streaming** is the host's: the transcript's rows grow and change without a description revision; the host caps the update rate, commits a final message at the
  end, and drops an in-flight load when the viewer moves to another thread (the old transcript load is cancelled, not finished).
* **First screenful stamp.** When the transcript has drawn its first screenful the host sets `data-thread-rendered=<thread id>` (web), dispatches
  `calternal:thread-rendered` and records the measure `calternal:thread:open`; the native app has its own equivalent. The budget is 150 ms from the viewer
  settling on the thread (4x CPU slowdown as the stand-in for a slow device; today's numbers are in `docs/native-feel.md` section 20 and the open defect).
* **Reduced motion and keyboard** follow `docs/native-feel.md` for everything a component draws.
* **Events never carry worker payload.** A component event reaches a named invoke only as typed inputs (an id, a selected passage the person chose).

## 6. What may be cached on the device

`cache.templates` is `keep` (default) or `none`; `maxAgeDays` bounds it (1 to 90, default 30).

* **Only data-free templates persist.** A template is a description with its binds as requests: sections, selectors, titles the author wrote, icons, actions. It holds no row, no
  title of a person's item, no count and no activation context, so it may be stored (one file of at most 4 MB, keyed by the signed-in account, deleted on sign-out and on account
  switch). `none` keeps nothing.
* **Filled rows are session-only.** Rows, counts and anything the host filled stay in memory, **partitioned by session, space and audience** (a member and a guest of the same
  account never share a partition), are never written to storage, and are cleared on revocation, on closing a guest link, on sign-out, on account switch and when the session
  ends. A shared or offline device therefore holds no titles after the session.
* **Nothing protected is painted before the host has authorised the viewer now.** There is no stale-first paint of rows. Activation contexts, resolution references,
  confirmation challenges and proofs are never stored anywhere.
* **What the person sees at first paint instead.** The template at once (the sidebar's structure, selectors, section titles the author wrote, the footer, the header), with
  skeleton rows where a list will be and the bind's `loading` state; the host's first window answer replaces the skeletons (one round trip, no waiting on a full list). Offline
  or unauthorised, the person sees the template and the state the host or the shell can say truthfully (`offline`, `sign in again`), never a title from a past session. A shell
  that cannot meet these rules (an older client) gets a host fallback that obeys the same authority rules, or the page's own chrome.
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
4. **While a key repeats, no selection or row event crosses the bridge**: those go out once after the key settles (about 100 ms after the last key). Window **requests** for
   prefetch may cross, coalesced to at most 4 a second; incoming windows are held and applied when the selection settles (or at a low rate), decoded and compared off the
   main thread. A republish that only confirms the selection must not touch the list.
5. **One line per row**, one icon column per list (the system aligns titles by it), single neutral accent: the accent is passed only to the unread mark and a prominent action.
6. **Swipe actions** need `title`, `icon`, `tone` and `fullSwipe` on the invoke; they draw with the system's swipe look, tinted by tone, never by the root tint.
7. **Haptics and menus** are the shell's: selection feedback on `route` and `selection` events, a system menu for `menu`.
8. **Touch targets** are 44 pt; the renderer, not the description, sizes controls.
9. **The first paint** may come from the cache (section 6) before any host is reachable; therefore binds in the cached description have a defined stale state.

### 8.1 Review of sections 5 and 9 against SwiftUI (native lane)

Everything in 5.0 and 5.1 is drawable with system controls. Findings, in order of consequence:

1. **Settling time.** Section 5.0 says a selection opens its target "about 45 ms after the last key, longer while a key repeats". The native measurement says: nothing crosses
   the bridge while a key repeats; the target loads when the selection settles, about 100 ms after the last key (or key up). Use that number, and say that outgoing
   events and incoming windows are both held during a repeat (8.4).
2. **Home, End and Page keys over rows that are not loaded.** "Through ALL rows (drawn or not)" needs the window protocol: the renderer asks for the window around the target
   index (`0`, `count - 1`, current plus a page) at once and moves the selection when the rows arrive. State this as a renderer duty; the host only has to answer windows.
3. **A `list` takes the remaining height of its stack and scrolls itself.** Nested scrolling regions are refused: one `list` per scroll region, and a `stack` that
   holds a list does not scroll. (A SwiftUI `List` inside a `ScrollView` loses its laziness and selection.)
4. **Forms** map one to one: `text` TextField, `multiline` TextEditor, `url`/`email`/`number` TextField with the keyboard type, `date` DatePicker, `choice` a menu Picker,
   `toggle` Toggle; labels use text roles (Dynamic Type), the submit is a prominent button disabled while invalid or pending, the notice sits above it. No change needed.
5. **`Composer.config.enabled` as a Predicate has no row scope.** A predicate names a field of a row; a composer has none. Use a ValueSource or an explicit
   `{bind, field}` source (`{bind: "thread", field: "state"}`) compared by an `enabledWhen` predicate; otherwise two renderers will read it differently.
6. **Set-valued selection** (9.2) is cheap natively (`List(selection: Set<ID>)` with edit mode). Add `selectionMode: "multiple"` to the list node and a source
   `{selection, set: true}` resolved as an ordered array of at most 50 ids; the trusted preview shows the count and the first titles.
7. **Selector options from a bind** is cheap (a segmented control up to four options, a menu beyond); add `options: {bind}` with at most 12 rows.
8. **Share sheet:** make it a host-owned sheet shown by the built-in `host.item.share` (SwiftUI `.sheet`), not a sixth component the description places; one decision for
   every renderer, and the audience is the sheet's own content (a material input shown by the host, not by the description).
9. **Date prompt:** extend `prompt` with `kind: "text" | "date" | "datetime"` (a DatePicker in the same sheet). A snooze presets menu works today.
10. **Nested rows** (child threads) are drawable (`List(children:)`) but conflict with index windows (windows per parent). Defer, as listed.
11. **Inline rename** as a shell prompt on every platform is right natively (an alert with a text field); accept the web's in-row editor as the web's own.
12. **Transcript stamp.** `data-thread-rendered` is web only; the native equivalent is a typed `rendered` event plus a signpost and the existing measurement line; the 150 ms
    budget is measured on a device, not with a CPU slowdown.
13. **Sealed frame** natively is a WKWebView with a non-persistent data store, a content-blocking rule that refuses every network load, content served from a custom
    scheme handler, fixed navigation (none) and one fixed message handler. Its height caps are enforced by the host; a frame inside a scrolling stack must not capture
    vertical scroll when it fits.
14. **Drag to move** exists in SwiftUI (`draggable`/`dropDestination`) and is not in the vocabulary; defer.

### 8.2 Native review of draft 3 (activation, cache, first paint)

1. **Round trips.** An activation is resolve, (confirm,) dispatch. The renderer shows the action as pending from the first send and sends `invoke_dispatch` right after a `resolve` answer that carries no
   `confirmation_challenge`; on a phone over a mobile link two round trips for a swipe are acceptable, three are not. The plugins
   lane may later consider answering a resolve that needs no confirmation with the outcome itself (resolve-and-dispatch in one round trip); that is a future question: the three separate steps stay as specified.
2. **Pending and outcome on a row.** A row with an action in flight shows a quiet progress mark and keeps its place; the row leaves the list only when the host's next window
   no longer contains it (no optimistic removal), so a failed Trash cannot leave a ghost gap. Undo is the outcome's button, shown for its `expiresAt`.
3. **The confirmation sheet** is a system confirmation dialog (an alert or sheet with the host-written material lines, `Cancel` and the action named with the operation's own verb);
   it is drawn by the shell, so a plugin cannot restyle or cover it.
4. **First paint** (section 6) is drawn from the template alone: the shell stores the frame (structure, author-written titles, apps, selectors) and paints skeleton rows with the
   bind's `loading` state; **no row, card, count, account initial or item title is stored** (implemented in the apps: `ChromeCache.frame(of:)`).
5. **Selection sets and composer controls** map to `List(selection: Set<ID>)` in edit mode and system menus/pickers; the checked attachment references are produced by the
   system pickers and uploaded through the host's route, so the draft carries references only.

## 9. Paper exercises (fixtures) (web lane)

Fixtures in `contract/fixtures/description-v2/` with one **fixture manifest** per exercise (`manifests/`, in the plugin manifest v2 format: kinds, indexed fields, operations,
reads), so a fixture is checked against what the plugin would declare, not against nothing: `reading-list`, `notes`, `threads`. Run `python3 contract/verify-description.py`.
This section is a rerun of the three exercises after Opus review 4: what is written, what is not, how many concepts an author meets before the first list, and what
`defineView` and `defineChrome` will hide.

### 9.1 Community reading list

**Concepts before an empty list appears (about 18):** manifest identity, network tier, app, a chrome description and a content description (one `App`), asset digests, kind,
storage placement, kind schema, indexed fields, the required `operations` array, description version and revision, bind and source, field projection, window and count, row
template, sidebar section, view root and list node. Adding the first row adds an operation (risk, idempotency, capability, methods, schemas), a worker and `ctx`.
**`defineView` / `defineChrome` can hide:** asset digests and paths, the method inventory, tickets, `space_id` and the wire. They **cannot** hide the chrome/content pairing
(one `App` after this round), placement, the bind and row vocabulary or the four risk fields: those are the contract.
**Written:** a bound list (title, read mark, a per-row Mark read, an empty state); a two-field add form; a per-person reminder bind with `viewer_self` audience.
**Not written / open:**
* **Add opens the item it created** needs `ActionOutcome.created` and `then: {route}` (plugins lane).
* **Mark read without waking the runtime** needs the host's metadata patch with scalar `field` and `value` and a `full_swipe` flag; today it is a plugin operation, so it
  wakes the runtime and cannot be a full swipe.
* **A reminder on one date** has no scheduler shape (`notify_creator` is in no schema and schedules recur); the reminder bind is written, the notification is not.
* Conditional fields, custom validation messages, edit in place and load more are not in the first vocabulary.

### 9.2 Notes: windowed sidebar, collections, Trash, rename with Undo, bulk Trash and Share, Move to collection

**Written:** collections as a bound section; notes as a list bound to the route's collection in index windows with an exact count; Trash as a bind (`trashed eq true`); Restore;
Empty Trash and Share as host sheets reached by a `ShellAction` (the host writes the material); rename with a prompt and a host-owned Undo; the footer search over a search
bind; the editor as an `item-editor` reading the route's item (the fixture's route is `item` on both sides); the header title from an item bind of the open note; bounded
multi-selection and bulk Trash through `selection_ref`; **Move to collection** as a `choice` prompt over the collections bind (at most 50 options) that sets the note's collection.
**Not written / open:**
* **New note opens the note it creates** (`created`, `then`); **pin without a runtime wake** (the metadata patch); **Undo and status** have no wire shape yet in the event union
  (the plugins and native lanes define them); **Empty Trash** names no kind and an Editor should not see it (`showWhen` on roles).
* **Tag chips**, **sections per collection** and **drag**: the `choice` prompt covers Move to; chips and repeaters are not written.
* **Notes is a host kind in the catalogue today**, so a community notes app cannot declare the same fields (plugins lane: generic item fields in the catalogue).

### 9.3 Threads: Inbox of "waiting on me", composer, approval card

**Written:** the Inbox as a bind (`waiting_on eq viewer.person`) with its count in the selector; All Threads; provider icons from the open `ProviderId` values; state marks; the
transcript with the host's Quote and Stop; the composer as a thread and its controls (provider, model, reasoning, permission, machine, attachments, mentions), the host owning the
send; an approval as a `request-card` over the protected request (`requests`, `thread_ref`); snooze as a `datetime` prompt.
**Not written / open:**
* **New thread** (no operation creates a thread: `turns.create`), **archive and unarchive** (distinct from Trash: `turns.archive`, `turns.unarchive`), **snooze** (`turns.snooze` and a
  `snoozed_until` field so the Inbox can hide snoozed threads), **follow-up** (`turns.follow_up`): plugins lane catalogue additions; the fixture maps them by name until they land.
* **Child threads under their parent** and **user-made sections with drag ordering**: an accepted decision shows child threads under their parent and the sidebar's own layout is a
  person's; both stay **OPEN for the owner** (they need nested rows and a per-person layout stored by the host, or a cut). They are not parity exceptions to wave through.
* **Bind-backed selectors** (chips) and **conditional fields** can wait.

### 9.4 What this schema needs from the host (authorisation is the plugins lane's)

1. Authorise every bind (kind, collection, filters, sorts, search, output fields) and every ref (`item`, `thread`, `request`, `frame`) against the viewer, the space and a guest's
   grant **before** any indexed work or read, and reject a field the audience cannot read instead of filtering it (counts and order leak otherwise).
2. Classify each operation as reading, mutating or disclosing from the manifest, and name the material inputs the trusted preview shows; the description may only add.
3. Reconstruct `{row}`, `{selection}` and route sources from the activation context (never from the event) and read `{form}` and `{prompt}` only as device-held values in `interaction`; answer `stale` when the context no longer holds.
4. Provide the host built-ins named here (`host.item.rename`, `host.item.trash`, `host.undo`, `host.approval.resolve`, `host.panel.ask`, `host.turn.stop`, `host.route.set`) so a
   swipe or a card never wakes a space runtime, and a **checked draft payload** for the composer.
5. Own the transcript's streaming, appends, rate cap and final commit; and the frame's sandbox.

## 10. Open questions and what this contract does not decide

1. `host.item.patch-metadata` (maps to `item_save`'s metadata arm) and `host.trash.purge` (confirmed, Manager or Owner, as a job) are the plugins lane's, part A.
2. Reminders notify through `notify_creator` on the existing scheduler (plugins lane, part A).
3. **Content node types** and the five host components: written in section 5 (web lane), exercised in section 9; the gaps are listed there and are the next vocabulary requests (set-valued selection `{selection, set: true}` of at most 50 ids with `selectionMode: multiple`; selector `options: {bind}` of at most 12 rows; composer typed controls; the share sheet as a host-owned sheet behind `host.item.share`, not a sixth component; `prompt.kind` text/date/datetime; nested rows (windows per parent); drag to move; section repeaters).
4. **Generic TCP** is outside this contract; the owner's decision is recorded elsewhere and is not a description concern.
5. **Section repeaters** (one section per item of a bind) are deferred; navigation sections cover the cases above.
6. The **retirement date of v1** is the owner's.

## 11. Sign-off

* Native renderer lane: **signed off again at the tip below.** Sections 1 to 4 and 6 to 8 (and the new 8.2) are mine or reviewed; draft 3 of sections 3.2, 5, 6, 9 and 12 was reviewed
  against SwiftUI and the apps (the frame-only cache is implemented; activation events are the catalogue's). All fixtures pass without a catalogue and against the plugins
  lane's `contract/host-catalogue-v2.json` (`CALTERNAL_HOST_CATALOGUE=<path> python3 contract/verify-description.py`).
* Web renderer lane: wrote sections 5, 9, 12 and the draft-3 changes to 3.2, 3.4 and 6; its sign-off is its own message to the coordinator.

## 12. Disposition of the third-round review (Sol, 46 findings)

Part B's own findings are fixed here (`FIXED`); findings about the runtime are the plugins lane's (`A`), the native review and sections 1, 2, 4, 7, 8 are the native lane's (`N`, done in
71a76140 and d0a7e100); `WAIT` is a recorded parity exception.

| # | Finding (short) | Status |
| --- | --- | --- |
| 1 | Event accepts `{type: invoke}` alone | FIXED: `$defs/Event` is a oneOf of variants with required fields; activation events are the catalogue's; `invalid-event-*` fixtures |
| 2 | Guest rows after grant loss | FIXED: no persisted rows; filled rows session-only and cleared on revocation (6) |
| 3-7, 9, 11, 12, 14, 17-19, 21, 22, 24-26 | earlier findings reported fixed | no change |
| 8, 20 | v1 guide examples | A |
| 10 | Confused deputy: client-resolved values, no host proof | FIXED: context, resolve, trusted confirm, dispatch; the host writes the material (3.2) |
| 13, 42-44 | Gates and spikes | A |
| 15 | Duplicate action ids, no required fields | FIXED: unique `action_id` per description (verifier, N); required fields per event variant |
| 16 | Attachment and mention encoding | FIXED: catalogue `Draft` in `interaction.draft`: host-owned attachment refs, account mentions, quote ref (3.2, 5.1) |
| 23, 25, 27, 28-32 | Kind and field names, built-in catalogue, effect classes, identities, query model | A (one catalogue); B cites it (kinds `notes threads collections requests`, `thread_ref`, `waiting_on`, `pending_for`, `trashed`, `trashed_at`) |
| 33 | Transcript cursor, component events, quote encoding, frame sandbox | FIXED: `transcript.page` and `sealed_frame.*` routes cited from the catalogue; `{selection: text}` removed, the host's Quote control with a quote reference |
| 34 | Verifier shapes only, duplicate keys | FIXED: strict parse, catalogue resolution (N), events and cited shapes checked, fixtures validated against the catalogue |
| 35 | Resolution of inputs ambiguous | FIXED: the event carries a context and only device-held values; the host reconstructs row, selection, route (3.2) |
| 36 | Typed-input exemption | FIXED: only a value directly edited in a visible trusted control of the same form; untouched prefills, hidden-value choices, other or offscreen forms are previewed; destinations always (3.2) |
| 37 | Cross-bind and search leaks | N (FilterValue, TextSource: no bind field feeds another bind); A (authorise the whole graph of allowed edges) |
| 38 | Aggregate bounds, live fan-out, wakes | N (2.6) and A |
| 39 | One revision for several binds | FIXED: no client revision; the context names the dependency versions (3.2) |
| 40 | Approval via generic invoke; Restore full swipe | FIXED: reserved operations never generic; full swipe only where the catalogue says `full_swipe` (today none, Restore included) (3.2) |
| 41 | Device cache of rows | FIXED: data-free templates persist, rows session-only and partitioned, nothing painted before authorisation, first paint is the template with skeleton rows (6) |
| 45 | Notes: Share sheet, bounded multi-selection; Threads: composer controls with option sources, checked attachments | FIXED: `selectionMode: multiple` with a host-issued `selection_ref`; Share and Trash as host sheets (`ShellAction`); composer `controls`; catalogue `Draft` |
| 46 | Snooze date/time picker; bind-backed selectors, nested rows, drag, repeaters, conditional fields | FIXED: `prompt.kind: date \| datetime`; the rest recorded as WAIT parity exceptions in 9.2, 9.3 |
