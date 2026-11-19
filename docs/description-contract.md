# Description contract (v2)

Status: **revision 5** on `lane/description-contract`, for sign-off by the web renderer lane (leads this document) and the native renderer lane (owns the
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

Everything below is one contract for chrome (sidebar, header, footer, panel) and content (the card): the same `bind`, the same `invoke`, and **one bind list per app**: an
`AppDescription` declares its binds once and its `chrome` and `view` refer to them by id.

## 1. Vocabulary

| Term | Meaning |
| --- | --- |
| **kind** | A type of item a plugin or the host declares (the host's own `threads`, `requests`, `collections`; a plugin's, such as `notes`) with its **indexed fields**; every item kind has the host's generic fields (`id`, `title`, `updated`, `trashed`, `trashed_at`). |
| **app description** | One app's `binds` (declared once), its `chrome` (sidebar, header, footer, panel) and its `view` (the card's content). |
| **bind** | A data-free request: a list window, one item, or a search, with filter, sort and the fields wanted. |
| **row template** | How an item becomes a row: fields to title, icon, marks and actions. |
| **row scope** | Inside a row template, `{row: ...}` value sources name the row an action was activated on. |
| **invoke** | An action: an operation plus named inputs from value sources. |
| **value source** | Where a value comes from: a literal, the viewer, the route, the selection set, a row, a form, a draft, a prompt, or (in a component's config) a field of another bind's pinned row. |
| **component** | A rich surface the host draws (item editor, transcript, composer, request card, sealed frame), configured by the description. |

## 2. Bind

One definition (`$defs/Bind`):

```json
{ "id": "notes",
  "source": { "type": "list", "kind": "notes", "collection": { "route": "collection" } },
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
shell -> host   { type: "window", request: { bind: "notes", from: 120, to: 180, parent?: "<row ref>" } }                 // the catalogue's WindowRequest: the ONE envelope
host  -> shell  { bind: "notes", audience, parent?, window_ref, state: "ready", result_version, count: {mode, value}, offset: 100,
                  rows: [ { id, fields: {...} }, ... ], activation_contexts: [ { row, contexts: { action_id: token } } ] }   // the catalogue's WindowResult
```

The catalogue defines one `WindowRequest` envelope and one `WindowReply` union of `WindowResult` and `WindowPlanError` (status 422, code `unsupported_query_plan`, `reason`: `skip_limit`, `unsupported_plan` or `materialization_limit`).
**End-relative windows.** The request may carry `anchor: "end"` in the same envelope (`anchor` is `start` by default): `[from, to)` is then counted back from the last row, with `to - from` at most the catalogue's `max_request_span` (200), and the distance may exceed the absolute offset cap, so paging can reach any scope. End, Page Down near the end and a scrollbar drag into the last part of the list use it, and no request needs an absolute offset past the skip cap. A result for an end-anchored request echoes `anchor: end` and an `offset` that is the distance of the nearest returned row from the end, with rows in the bind's normal order (no absolute total the audience may not have is exposed). The member cache record is start-anchored only.
**Refusal handling.** For `reason: skip_limit`, page from the nearer known end using its anchor. Use the start when the end is unknown. For any other reason, or when that paging is refused too, show the bind's `error` state.
Row ids are opaque and stable. No position is an authority reference.

* A window is the half-open range `[from, to)` of at most 200 returned rows. The highest supported absolute offset is 500,000. The host may materialize up to 4,096 rows and,
  separately, skip up to 250,000 entries in an index-only jump from the nearer end of the scoped covering index. Exact counts use the maintained count for the authorized scope.
  `window.size` is 1 to 200, default 60. `count` is `exact`, `estimate` or `none`.
* The renderer prefetches ahead of the visible rows and, when a key or a drag walks past the edge of what it holds, asks for the next window at once (no
  settling), coalesced to at most 4 window **requests** a second; the host may answer with a larger range than asked (`offset` and `rows` say what came back).
* `result_version` identifies the result set. A new version replaces the window wholesale; the renderer skips a replacement equal to what it draws, so a republish that
  only moved the selection costs nothing.
* **Live:** with `live: true` the host pushes a changed window (new `result_version`) while the view is open. While a key repeats the renderer **applies at once every reply to a
  prefetch for the result version it already drew** (so arrowing never runs into skeleton rows) and **holds only pushes that change the version**, applying the latest when the
  selection settles (or a few a second). A selection held by the settle window is flushed before any activation is sent, so an action never runs against the previously
  settled row.

### 2.4 Row template

`title` is one field (one line; there is no second line and no free-form row content). `icon` is a catalogue name, a field's value or the kind's own icon.
`marks` (up to three) are trailing dots, counts, texts or icons, each with an optional `when` predicate and a tone. `leadingActions`, `trailingActions` and `menu` are
`invoke` definitions; only the first leading or trailing action may set `fullSwipe`. `open` navigates (sets a route parameter to the row id) or invokes.
Rows are identified by the item id everywhere: selection, events, scope.

### 2.5 States

`states.empty`, `loading` and `error` are small messages (`title`, `detail`, `icon`, an optional action). The description or host provides their text.
Show `loading` until the first window arrives, `empty` when `count` is 0, and `error` on `state: "error"` or a `WindowPlanError`, except while retrying a `skip_limit` refusal by paging (§2.3).

### 2.6 What the schema needs from the host (authorization is the host's job)

The description only asks. Before any indexed work the host must:

1. authorize the **kind** (a package binds only kinds in its manifest `reads`: the SDK fills a package's own kinds into `reads` automatically, so an author writes `reads` only for another app's kinds), the **collection**, every **filter field**, every **sort field**, the
   **search** and every **output field** against the viewer, the space and, for a guest, the item's grant;
2. reject a field not declared indexed by the kind, and a field outside the audience's projection (a guest entitled to titles must not be able to infer hidden
   metadata from the count of an `eq` filter or from row order: filters, sorts and counts on a field the audience cannot read are refused, not filtered);
3. produce **host-owned projections** for guests: rows are built by the host from the item's grant, never by plugin code;
4. authorize the **whole dependency graph** of an app (no bind takes an operand from another; a search is pinned to the audience's corpus; a count is taken over the
   already-authorized projection); a guest gets an `exact` count only over an explicit shared root/child projection with count disclosure enabled, otherwise **`none`** (no estimate), nested windows included;
5. bound the work: windows of at most 200 rows, 16 predicates, **16 binds per app** (chrome and view together: there is one list), absolute offsets to 500,000, and per actor and
   per space caps on open subscriptions, window bytes and invalidations a second, coalesced before querying; pushes are **host-only** and **never wake the space runtime**;
6. **deliver activation contexts**: with each window (`WindowResult.activation_contexts`, one entry per row and action), with the view when it opens (`ViewOpenResult`, for the header,
   form and composer actions) and when the subscription lease renews (`LeaseRenewRequest`/`LeaseRenewResult`, one 15-second lease for the subscription and its contexts). The
   host reconstructs row, route and selection from a context and never accepts the renderer's value for them. The renderer keeps filled rows in memory after a lease lapses but
   does not paint them as current until a renewal returns the same grant revision and result version (section 6).

The renderer never receives a row it was not entitled to; it needs no rule of its own. The host's own duties (authority, engines, projections) are written once, in the
Plugin API (Part A); this section lists only what the description relies on.

### 2.7 Nested rows and user-made sections

* **Nested child rows.** A row template may declare `children: {bind}`: a bind whose source has `parent: true`. **The declared edge is the mandatory constraint:** every window request on such a bind
  carries `request.parent`, a host-issued `WindowParentRef` taken from the parent row's `parent_refs`, scoped to that edge and its ancestry and checked by the host on every request (a parent bind
  that no edge references is refused by the verifier). `{parent: true}` in a filter is only an optional extra predicate. **A per-parent bind is consumed only through its declared edge**: no ordinary section, list, count or header may reference it (the verifier rejects it), and `persist` is refused on it (only ROOT windows may be persisted, section 6). The renderer asks for a parent's children with its own windows, only when the row is expanded; a row with children draws the system's disclosure control (a `List`
  with `children`/`DisclosureGroup`). Depth is at most 3, counted through the chain; the host checks it and the verifier rejects cycles. Child threads under their parent are this.
* **User-made sections.** `Section.repeat: {over, rows}` makes one section per row of the bind `over`, which **must read `layout_sections`** (the verifier rejects any other kind; the per-person layout the host stores: the host virtual kind `layout_sections` with
  `id`, `title`, `position`, viewer-only) with each section's rows from the per-parent bind `rows`. The layout is stored and authorized by the host; **Move to, Move up and Move down are host
  operations** (`host.layout.move_to {entry, section}`, `host.layout.move_up` and `host.layout.move_down {entry}`, and `host.layout.section.create`, `rename`, `remove` for personal sections)
  that a row or section menu names. **Typed, context-derived operands:** a row action names the row's underlying item with `{context: "item"}` (a `ThreadRef` in Threads) and, inside the rows of a `Section.repeat` only, its placement with `{context: "placement"}` (a `LayoutEntryRef`); `host.layout.move_to` takes **either** `thread: {context: "item"}` (adds a placement for an unplaced thread) **or** `entry: {context: "placement"}` (moves a placement), plus a `section` choice (`ChoiceRef`); `move_up` and `move_down` take `entry`; the verifier checks each operand against the catalogue's `input_schema` type. Both operands are reconstructed by the host from the activation context; the renderer sends neither. **In the rows of a `Section.repeat` the bare row id is a placement, not a thread**: a `ThreadRef` input there must be `{context: item}` (and only from a row of kind `threads`), a `LayoutEntryRef` input must be `{context: placement}`, a bind may not be both a section's rows and a child-rows bind when it uses a placement operand, and `host.layout.move_to` always names a `section`. In ordinary Threads rows (not repeated) the bare row id is the thread. **Drag is not in the first contract**: a recorded parity exception, with Move to and Move up/down standing in.

## 3. Invoke

One definition (`$defs/Invoke`) used as a content node, a row action, a header action or a footer action. A shell-only control is a `ShellAction`; chrome-level lists take an
`Action` (either):

```json
{ "id": "rename", "title": "Rename", "icon": "edit",
  "operation": "host.item.rename",
  "inputs": { "item": { "row": "id" },
              "title": { "prompt": { "title": "Rename note", "initial": { "row": "field", "field": "title" } } } },
  "then": { "route": { "name": "item" } } }
```

An invoke has an id, a title, an icon, an operation, named inputs, an optional `role: destructive`, `tone`, `fullSwipe` and `disabled`, and an optional `then` that sets a route
parameter to the item the operation **created** (the outcome's `created` reference). It has **no** confirmation, preview, retry, pending or result fields: what the person is asked,
what is retried and what is shown come from the host catalogue and the manifest.

### 3.1 Operation and inputs

`operation` is a plugin operation id from the manifest, or a **host built-in from the host catalogue** (`contract/host-catalogue-v2.json`, written by the plugins lane:
exact ids, typed inputs, effect class, `confirm: none | challenge`, authority, whether it may be a generic invoke, whether it may be a full swipe, worker-wake behaviour).
Built-ins that need no worker run in the host without waking the space runtime; a swipe on a row must not start a runtime. The catalogue, not this document, says which
operations a description may name; the schema and the verifier reject the ones that belong to a component, an outcome, a sheet or the shell: `host.approval.resolve` (the request
card), `host.undo` and `host.action.status` (an outcome), `host.panel.ask` (the transcript's quote control), `turns.start` and `host.turn.stop` (the composer),
`host.trash.purge` and `host.item.share` (protected sheets), `host.route.set` and `host.session.end` (shell controls). A description reaches the Trash and share sheets and the
shell controls through a `ShellAction` (`shell: {sheet: trash | share}`, `{route}`, `{reload}`, `{signOut}`).

`inputs` maps a name to a value source. There is no literal row and no destination the interface picks at activation. The **host** reconstructs row, route, selection and bind
sources from the activation context; the **device** sends only what it holds: **form** values (`{form, field}` in that form's own submit), the **draft** of a composer, **prompt**
answers and a host-issued `selection_ref` for a ticked set. Bind fields are legal only in a component's config (and a header title) and name a row by the stable id the
context pinned, never by an index.

### 3.2 Activation: context, resolve, confirm (when required), dispatch

The shapes are the host catalogue's (`ResolveActivation`, `ConfirmActivation`, `DispatchActivation`, `ActivationResolution`, `ActionOutcome`, `Interaction`); this document
cites them and defines none of its own (`$defs/Event` references them).

```
shell -> host  { type: "invoke", action_id, activation_id, activation_context, interaction?, dispatch?: true }                   // resolve (and dispatch when no confirmation)
host  -> shell { activation_id, resolution_ref, confirmation_challenge?, material?: [ {name, label, value} ] }                       // or the ActionOutcome when dispatched
shell -> host  { type: "invoke_confirm", activation_id, resolution_ref, confirmation_challenge }  (composer: final_text)             // only when the host challenged
shell -> host  { type: "invoke_dispatch", activation_id, resolution_ref, confirmation_proof? }                                       // dispatch
host  -> shell { activation_id, state: ok | pending | error | stale | refused | unknown, status_ref?, undo?, created?, code?, message? }
```

* **The event carries a context and ONLY device-held values.** `activation_context` is the host-issued snapshot of what was rendered (viewer, space, template digest, action, route,
  stable row, the versions the action depends on): opaque to the renderer. `interaction` holds only what lives on the device: `form` values, `prompt` answers, the `draft`, `choices`
  and a host-issued `selection_ref`. **The event never carries a resolved row, selection, route, bind value, target item or viewer; if a modified client adds one, the schema
  refuses the field and the host ignores it.** There is no single revision number: the context names the versions it depends on, so a row that merely scrolled out of the window or moved
  because of unrelated changes is **still actionable**; only a change to the target, its scope or a dependency makes the activation `stale` (nothing ran).
* **Confirmation is derived from the catalogue, never from the description** (`confirm: none | challenge`). **A sheet-free, one-exchange mutation (`confirm: none`) exists only for
  catalogue entries whose engine commits the complete effect and its Undo together** (the catalogue's `atomic_undo: true`, worker-free). **The catalogue's flag decides, not this document or the description:** an operation is sheet-free only where the catalogue sets `atomic_undo`, and Trash, Restore and rename are not until the catalogue establishes complete effect plus Undo at one commit for them. A manifest's
  `risk` or `reversible` declaration grants **no** eligibility. For these the shell sends the resolve with `dispatch: true` and the host answers with the outcome, **one exchange**.
  **A plugin (worker-backed) operation always takes the ordinary path:** a resolve, then a separate dispatch: **two exchanges**, and **no sheet** unless the host says material requires a
  challenge (a destination, recipient, audience or permission, a bulk set, an irreversible, external or access-widening effect, or an input the manifest declares material): then the host
  answers the resolve with a `confirmation_challenge` and a **material** list **it wrote** (values the person did not edit included: untouched prefills, defaults, autofill), the shell
  shows it in its own protected sheet, `invoke_confirm` returns the challenge, and the host returns a `confirmation_proof` bound to the normalized input and the `activation_id`
  (different input: a new challenge); a challenged action is three exchanges. A description cannot add, remove or relabel anything here. **Typing is never consent**: no value counts as
  confirmed because it was typed; where a challenge is required, the person's confirmation through the protected step is the consent.
* **Chat send is one exchange, and the final text is message content only.** The composer pre-resolves only when the prepared material changes (provider, model, machine and other options,
  attachments, mentions, a quote); the proof binds to that material and not to the text, so one `invoke_confirm` carrying `final_text` (the composer-only arm) confirms and dispatches, and the
  receipt is bound to the final text (a retry cannot send different words). The host treats `final_text` as inert at admission: **any parser-derived change to recipients, routing, permissions or other
  prepared material** (an @-mention, a slash command, a destination typed in the text) **invalidates the proof and requires a fresh resolve and preview**. A description names no send operation.
* **Retry and idempotency are the host's.** A description cannot declare an action retry-safe or waive a check: effect class and idempotency come from the catalogue and the manifest, and
  the host de-duplicates by `activation_id` (a retry after a dropped connection repeats no effect).
* **Full swipe.** A swipe action may set `fullSwipe` only on the first action of its set **and** only if the catalogue marks its operation `full_swipe: true` (a catalogue entry that commits its effect and Undo together and needs no confirmation: only the entries the catalogue marks); the verifier rejects it on any plugin operation. A full swipe never opens a confirmation.
* **The context must match the selection DRAWN.** A selection held by the settle window is sent before an activation that uses it, and the action's `activation_context` must be the one the host
  issued for the stable row actually drawn as selected when the person activates (the renderer sends the catalogue's `SettledSelectionRequest` ({type: "selection", bind, row}, route `selection.settled`, distinct from a bulk `selection_set`), and the host answers with a `SelectionContextResult` bound to the lease, cache partition, bind and row). The renderer uses a context only if it matches the currently drawn row; if it is missing, **it waits for the host-issued context** and sends nothing. The host never resolves an
  activation against whichever selection is current on the server.
* **No generic route to protected operations** (3.1).

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

The host answers a resolve that needs no challenge, and every dispatch, with an `ActionOutcome` (the catalogue's): `ok`, `pending` (with a host-owned `status_ref`, polled through the
host's own status control, not by the description), `error`, `refused`, `stale` and `unknown` (the interface shows an unknown outcome instead of retrying a non-idempotent action); on `ok`
an optional host-owned **undo** `{ref, label, expires_at}` and, when the operation created an item, **`created`** (a host-minted reference an invoke's `then: {route}` opens: a new note,
a new thread). Error text is written by the host. **Worker payload is never carried to an interface.** A row with an action in flight keeps its place with a quiet progress mark; it leaves the
list only when the host's next window no longer contains it.

## 4. Chrome and the app description

`$defs/AppDescription` is the root: `{version: 2, kind: "app", revision, requires?, cache?, binds, chrome?, view?}`. It replaces the chrome v1 file that lives on one branch
(`contract/chrome-v1.schema.json` on `lane/ios-shell`) and the separate chrome and view documents of earlier drafts: a real schema in the repository, with **one bind list per app**.

* **`binds`**: every bind of the app, by id (at most 16, chrome and view together); sidebar sections, selector counts, the footer search, the header title and components reference them by
  id, so a reference is never ambiguous and a bind is declared once.
* **`chrome.sidebar`**: `selectors`, `sections`, `title`, `width`, `refresh`. A **section** has literal `rows` (fixed shortcuts), a `bind`, or both (literal first), `actions` and `menu` in
  its header, a `collapsed` flag only when it folds, a `repeat` (user-made sections, 2.7) instead of `bind`/`rows`, and `showWhen` (a route parameter's value, or `minRole` so Empty Trash is not offered to an Editor). A section without a `title` draws
  no header and therefore no header actions. There are no cards: a shortcut row or a `ShellAction` row covers what a card did; an approval is the `request-card` component.
* **`selectors`**: a segmented control whose options set a route parameter; an option's `count` is a bind with `count: "exact"`, so Inbox 4 is a bind.
* **`chrome.header`**: a literal title or a field of an **item** bind (the open item, never a list's first row), and up to two leading and three trailing actions. The sidebar toggle is the shell's.
* **`chrome.footer`**: `account` (an id, a title and a menu; the shell draws the person's initial from the session, so a persisted template holds no person data), `apps`, `search` (the footer
  search feeds its own search bind and sends the settled text).
* **`chrome.panel`**: the right panel, built only of host components (the side chat is a transcript above a composer).
* No `appearance`, `scheme`, `accent` or `titleImage` in this first contract: the person's wallpaper and scheme are the shell's own preferences, and the wordmark is the app's name.

How the native renderer draws it (the SwiftUI primitive for each part is in `docs/ios-shell.md`): sections are `Section`s of a `List(selection:)` in the sidebar style
(plain on a phone), rows are `Label` with `.badge`, actions are `swipeActions` and `contextMenu`, selectors are a segmented `Picker`, the footer is a bar with glass controls, the header is
the system toolbar.

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
* **A `choice` prompt** (`prompt.kind: choice`, options `{bind}`, at most 50 rows) asks for one row of a bind (Move to collection, a tag). **Executable order:** (1) when the action is activated the shell sends the catalogue's `ChoiceOptionsRequest` (`choice_options`, the action's `activation_context` and the prompt's `slot`) and the host answers with `ChoiceOptions`: at most 50 `{ref, label}` pairs, each `ref` a host-issued `ChoiceRef` bound to one eligible row of the authorised option bind, the slot, the viewer and that context; (2) the shell shows the **labels** (its own picker) and the person picks one; (3) the shell submits the picked `ref` in `interaction.choices` under the slot in the resolve. The choice is never a client value: the host resolves the row from the `ChoiceRef` and never accepts a client-supplied collection or item id. Move to collection is `host.collection.move {item | selection_ref, collection}` with the collection as the choice. It is the only way a description offers a choice from data.
* **Forms** hold their values on the device. A field is checked there (required, length, `url`/`email`/`number`/`date` shape, `choice` among options) and the submit
  is disabled while a required field is empty or invalid and while the action is pending. The values leave the device only inside the submit's resolved inputs
  (`{form: id, field: fieldId}`); the form shows what was typed, and by the rule in 3.2 the host still previews every material field (typing is never consent); a form may present that list in place. After an `ok` outcome the fields clear (`clear: onOk`,
  the default); after `error` or an unknown outcome they keep what was typed. Errors appear in a `notice` above the submit, never as loose text.
* **Lists** are binds. `selectable: true` gives the keyboard model of `docs/native-feel.md` section 20 on every platform: Up/Down/Home/End/Page Up/Page Down move
  the selection through ALL rows (drawn or not), the selection paints at once, and what the selection opens (a route change) happens when it settles, **about 100 ms after the last key** (or key up); **selection and navigation events wait while a key repeats** (one goes out about 100 ms after the last key), incoming windows are held and applied when the selection settles, and window **requests** for prefetch may still cross, coalesced to at most 4 a second (8.4)
  (section 8, item 4). The contract's figure is about 100 ms; the web renderer's rest is adaptive: 1.6 times the longest gap between repeats in the burst, at least 45 ms and at most 120 ms, which is the same rule measured on the web; a load
  never starts while keys are still repeating, and a new key cancels the previous load.
* **Home, End and Page Up/Down** over rows that are not loaded are a renderer duty: it asks the host for the target window at once and moves the selection when the rows arrive, and the host only answers windows. **Home** asks for `[0, n)` from the start; **End** asks for an **end-relative** window (`anchor: end`, `from: 0`, the same envelope: the last rows, counted back from the last row), never an absolute offset computed from a count; **Page Up/Down** ask for the window around the current index, and near the known end (or for a drag there) use `anchor: end` with the distance to the end instead of an absolute offset past the skip cap.
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
* **Drafts and options.** The composer's text stays on the device; attachments are handed to the host when attached and held as host-owned `AttachmentRef`s, mentions are account references, a quoted passage is a host-issued quote reference (the catalogue's `Draft`), and the chosen provider, model, reasoning level, permission and machine are opaque option references the host returned (`Interaction.options`). The description names none of them: it says which controls appear. The host checks everything (lengths, counts, that every attachment is the viewer's own, that every option is one it offered for this thread) and composes the send; it never trusts the client. The one-exchange Send treats the final text **solely as message content**: anything the host parses out of it that changes recipients, routing, permissions or other prepared material (a mention, a slash command) needs a fresh preview before it takes effect (3.2). The draft clears only after `ok`; `error` keeps it.
* **Transcript reads** use the catalogue's `transcript.page` route: a `TranscriptPageRequest` names the thread and an opaque `after` cursor, the `TranscriptPage` returns at most 128 committed events and a `committed_cursor`; every page is reauthorised and bounded (128 KiB); an expired or revoked cursor is refused and the renderer reads again from the end. No description or renderer invents a cursor.
* **Streaming** is the host's: the transcript's rows grow and change without a description revision; the host caps the update rate, commits a final message at the
  end, and drops an in-flight load when the viewer moves to another thread (the old transcript load is cancelled, not finished).
* **First screenful stamp.** When the transcript has drawn its first screenful the host sets `data-thread-rendered=<thread id>` (web), dispatches
  `calternal:thread-rendered` and records the measure `calternal:thread:open`; the native app has its own equivalent. The budget is 150 ms from the viewer
  settling on the thread (4x CPU slowdown as the stand-in for a slow device; today's numbers are in `docs/native-feel.md` section 20 and the open defect).
* **Reduced motion and keyboard** follow `docs/native-feel.md` for everything a component draws.
* **Events never carry worker payload.** A component event reaches a named invoke only as typed inputs (an id, a selected passage the person chose).

## 6. What may be cached on the device

`cache.templates` is `keep` (default) or `none`; `cache.memberRows` is `keep` (default) or `none`; `maxAgeDays` bounds what is kept (1 to 90, default 30; persisted member rows are capped at 14).

* **Data-free templates persist.** A template is a description with its binds as requests: sections, selectors, titles the author wrote, icons, actions. It holds no row, no title of a
  person's item, no count, no account initial and no activation context, so it may be stored (one file of at most 4 MB, keyed by the signed-in account, deleted on sign-out and on account
  switch). `templates: none` keeps nothing.
* **Q10 (owner decision, 2026-11-19): own-space member rows can persist.** Only opted-in root windows without route-dependent or parent scope are eligible, and only for binds whose query inputs are **invariant**: a `list` source with at most a literal collection and no reference, search text, route, form, selection or parent operand anywhere in the source, filter or sort (account-bound `{viewer: person}` stays invariant); the schema and the verifier refuse `persist` otherwise, no query identity is added to the record, and only start-anchored windows are persisted.
  Accept only `MemberRowCacheRecord`, the display-only exception to session-only projections. The authenticated `ViewOpenResult` and `WindowResult` name the audience.
  The schema permits the record only on ready or empty start-anchored own-space member replies without a parent. Before writing, match it to the authenticated active view and window.
  On write and load check the schema, host proof, scope, opt-ins and expiry (§4.2 of the Plugin API). Load only into the matching own-space root display.
  Encrypt with account- and space-bound keys. On an unlocked device paint at launch as unconfirmed, then confirm or correct the rows with the first host answer.
  Keep for at most 14 days or `maxAgeDays` if shorter. Erase on sign-out, account switch or known revocation. Every action needs a fresh host context.
  *Sol's recorded objection:* protected rows should be revalidated by the host before they are painted, because a revocation the device has not yet learned of leaves titles visible for the
  moment before the first answer; the owner accepted that cost knowingly in exchange for an instant first paint. The mitigations are the conditions above (short retention, device lock, key
  binding, erase on a known revocation).
* **Everything else filled is session-only.** A guest's or a public view's rows, counts and anything the host filled for them stay in memory, **partitioned by session, space and audience** (a member
  and a guest of the same account never share a partition), are never written to storage, and are cleared on revocation, on closing a guest link, on sign-out, on account switch and when the
  session ends. Activation contexts, resolution references, confirmation challenges and proofs are never stored anywhere.
* **What the person sees at first paint.** The template at once (the sidebar's structure, selectors, section titles the author wrote, the footer, the header), with the member's persisted rows
  where an eligible root bind opted in (marked as not yet confirmed) and otherwise skeleton rows and the bind's `loading` state; the host's first window answer confirms or corrects them (one round trip). Offline
  or unauthorised, the person sees the template and the state the host or the shell can say truthfully (`offline`, `sign in again`); a guest or public viewer never sees a title from a past session.
  A shell that cannot meet these rules (an older client) gets a host fallback that obeys the same authority rules, or the page's own chrome.
* **"Do not paint" is not "discard".** When a lease lapses or the app is in the background, filled rows stay in memory (same session, space and audience partition) but are not drawn as current. The
  **lease renewal** is correlated and authenticated: the request carries a `renewal_id`, the `lease_ref`, the `cache_partition` and the held `result_versions` keyed by `window_ref`; the reply
  (`LeaseRenewResult`) echoes `renewal_id`, `request_lease_ref` and `cache_partition` and names the exact `retained_results` (`window_ref`, `result_version`), and is accepted only from the current
  session. The renderer repaints **only** the windows named as retained, **ignores a superseded reply** (an older `renewal_id`, another lease or partition) and **keeps rows hidden on timeout or
  refusal**; a changed grant or version refetches. A longer absence therefore costs a renewal, not a skeleton screen. (First window at offset 0 is a performance gate on the perf VM; see the plugin
  API build order.)
* The cache is the shell's, not the page's.
* **Implementation status:** the apps keep only the frame (`ChromeCache.frame(of:)`: no rows, cards, counts, account initial or item title) of a v1 description until the v2 renderer exists; persisting
  member rows with encryption and erase-on-revocation is part of the v2 renderer.

## 7. Version negotiation

* The shell advertises the versions it draws (`chromeVersions`, today `[1]`, with this contract `[1, 2]`); the host or page advertises the versions it can produce.
  The version used is the highest in common. A description carries `version: 2` and `kind: "app"`; the content of the card is its `view`.
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
   prefetch may cross, coalesced to at most 4 a second; replies to prefetch for the result version already drawn are applied at once and only pushes that change the version are held until the selection settles (or applied a few a second),
   decoded and compared off the main thread; a held selection is flushed before an activation is sent. A republish that only confirms the selection must not touch the list.
5. **One line per row**, one icon column per list (the system aligns titles by it), single neutral accent: the accent is passed only to the unread mark and a prominent action.
6. **Swipe actions** need `title`, `icon`, `tone` and `fullSwipe` on the invoke; they draw with the system's swipe look, tinted by tone, never by the root tint.
7. **Haptics and menus** are the shell's: selection feedback on `route` and `selection` events, a system menu for `menu`.
8. **Touch targets** are 44 pt; the renderer, not the description, sizes controls.
9. **The first paint** may come from the cache (section 6) before any host is reachable: the template, and a member's persisted rows marked as not yet confirmed (their actions disabled until a fresh context arrives).
10. **Nested rows and user sections** map to a `List` with `children` (disclosure) and one `Section` per layout row; windows per parent are requested only for expanded rows.

### 8.1 Review of sections 5 and 9 against SwiftUI (native lane)

Everything in 5.0 and 5.1 is drawable with system controls. Findings, in order of consequence:

1. **Settling time (resolved).** One rule everywhere: a selection opens its target about 100 ms after the last key (or key up); selection and navigation events wait while a key repeats,
   window requests and replies for the drawn result version do not (section 8, item 4).
2. **Home, End and Page keys over rows that are not loaded.** "Through ALL rows (drawn or not)" needs the window protocol: the renderer asks for the target window at once (Home from the start; End as an end-relative request from 0, `anchor: end`; Page Up/Down around the current index, `anchor: end` near the known end) and moves the selection when the rows arrive. State this as a renderer duty; the host only has to answer windows.
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

1. **Round trips (resolved by the catalogue).** A reversible action with `confirm: none` is one exchange (`dispatch: true`); only a challenged action takes resolve, confirm, dispatch; a chat
   send is one exchange (`final_text`). The renderer shows the action as pending from the first send.
2. **Pending and outcome on a row.** A row with an action in flight shows a quiet progress mark and keeps its place; the row leaves the list only when the host's next window
   no longer contains it (no optimistic removal), so a failed Trash cannot leave a ghost gap. Undo is the outcome's button, shown for its `expiresAt`.
3. **The confirmation sheet** is a system confirmation dialog (an alert or sheet with the host-written material lines, `Cancel` and the action named with the operation's own verb);
   it is drawn by the shell, so a plugin cannot restyle or cover it.
4. **First paint** (section 6) is drawn from the template alone: the shell stores the frame (structure, author-written titles, apps, selectors) and paints skeleton rows with the
   bind's `loading` state; **no row, card, count, account initial or item title is stored** (implemented in the apps: `ChromeCache.frame(of:)`).
5. **Selection sets and composer controls** map to `List(selection: Set<ID>)` in edit mode and system menus/pickers; the checked attachment references are produced by the
   system pickers and uploaded through the host's route, so the draft carries references only.

## 9. Paper exercises (fixtures) (web lane)

Fixtures in `contract/fixtures/description-v2/` with one **fixture manifest** per exercise (`manifests/`, a subset of the plugin manifest v2 format: kinds, indexed fields, operations,
reads), so a fixture is checked against what the plugin would declare, not against nothing: `reading-list-app`, `notes-app`, `threads-app` (and `notes-bulk-and-snooze-app`). Run `python3 contract/verify-description.py`.
**Reads.** Every bind needs its kind in the manifest's `reads`; the SDK fills a package's own kinds into `reads` automatically, so an author writes `reads` only for another app's kinds (Threads reads `threads` and `layout_sections`, the host's). The verifier still requires the entry in the manifest it checks.
**Persisted rows (owner decision Q10).** A member's own-space rows may persist on the device (encrypted, painted at once, then confirmed or corrected by the host); a bind opts in with `persist` (**root binds only**: a nested or per-parent bind, such as child threads or the rows of a user section, never persists, and the schema refuses `persist` on a parent bind), a description may opt out with `cache.memberRows: none`, and nothing persists for a guest or a public view (a guest or public reply cannot carry the member record). The conditions are section 6's. The fixture `threads-nested-catalogue-app.json` shows both (`cache.memberRows`, `persist` on the `all` bind).
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
send; an approval as a `request-card` over the protected request (`requests`, `thread_ref`); snooze as a `datetime` prompt; **nested child rows** (owner decision Q11: child threads under their parent: the `all` bind's rows declare `children: {bind}`, a bind with `source.parent: true` windowed per parent row, depth at most 3); **user-made sections** (Q11: `Section.repeat {over, rows}`: one section per row of the host's per-person layout, a host virtual kind `layout_sections`, each section's threads a per-parent bind), with **Move to**, **Move up**, **Move down** and section create, rename and remove as host layout operations (`host.layout.*`, entries named by the host-issued `LayoutEntryRef`, which the row id is inside a repeated section). Operands are context-derived (`{context: "item"}` is the underlying thread, `{context: "placement"}` the placement entry inside a repeated section): Move to adds an unplaced thread by `thread` plus the section choice, a placed row moves by `entry`, Archive always takes the thread. The nested window request carries the host-issued parent ref the parent row came with (`WindowRow.parent_refs` to `request.parent`; fixture `event-catalogue-window-parent.json`). Fixture: `threads-nested-catalogue-app.json`. The renderer-side replies are exercised as fixtures: `reply-catalogue-window-ready`, `-loading` and `-plan-error` (the typed refusal `{status: 422, code: unsupported_query_plan}`: the typed refusal carries a `reason`: `skip_limit` is the one after which a renderer pages from the nearer known end (2.3); `unsupported_plan` and `materialization_limit` show the bind's error state; never loading for ever). A request may be **end-relative** (`anchor: end`, `[from, to)` counted from the last row, the same envelope, width at most 200): End and near-end jumps use it, so no request needs an absolute offset past the skip cap, and an end result carries no member cache record (durable keys stay absolute), with negatives for a wrong code, a ready reply without a result version and an unknown field. The member cache record is exercised by `reply-catalogue-window-member-record` (an own-space-member, root, ready reply carrying the record) and the negatives `invalid-reply-catalogue-window-guest-record`, `-public-record` and `-nested-record` (a guest or public reply, or a nested one, carrying the same valid record); they validate once the catalogue has `WindowAudience` (revision 8).
**Not written / open:**
* **New thread** (no operation creates a thread: `turns.create`), **archive and unarchive** (distinct from Trash: `turns.archive`, `turns.unarchive`), **snooze** (`turns.snooze` and a
  `snoozed_until` field so the Inbox can hide snoozed threads), **follow-up** (`turns.follow_up`): plugins lane catalogue additions; the fixture maps them by name until they land.
* **Drag to reorder** is a recorded **parity exception** (owner decision Q11): on a phone and on the web the person moves a thread with **Move to** (a `choice` prompt over the section rows) and **Move up / Move down**; drag waits.
* **Bind-backed selectors** (chips) and **conditional fields** can wait.

### 9.4 What this schema needs from the host (authorisation is the plugins lane's)

1. Authorise every bind (kind, collection, filters, sorts, search, output fields) and every ref (`item`, `thread`, `request`, `frame`) against the viewer, the space and a guest's
   grant **before** any indexed work or read, and reject a field the audience cannot read instead of filtering it (counts and order leak otherwise).
2. Classify each operation as reading, mutating or disclosing from the manifest, and name the material inputs the trusted preview shows; the description may only add.
3. Reconstruct `{row}`, `{selection}` and route sources from the activation context (never from the event) and read `{form}` and `{prompt}` only as device-held values in `interaction`; answer `stale` when the context no longer holds.
4. Provide the host built-ins named here (`host.item.rename`, `host.item.trash`, `host.undo`, `host.approval.resolve`, `host.panel.ask`, `host.turn.stop`; `host.route.set` is a shell control, not an operation) so a
   swipe or a card never wakes a space runtime, and a **checked draft payload** for the composer.
5. Own the transcript's streaming, appends, rate cap and final commit; and the frame's sandbox.

## 10. Open questions and what this contract does not decide

1. `host.item.patch-metadata` (maps to `item_save`'s metadata arm) and `host.trash.purge` (confirmed, Manager or Owner, as a job) are the plugins lane's, part A.
2. Reminders notify through `notify_creator` on the existing scheduler (plugins lane, part A).
3. **Content node types** and the five host components: written in section 5 (web lane), exercised in section 9; the gaps are listed there and are the next vocabulary requests (set-valued selection `{selection, set: true}` of at most 50 ids with `selectionMode: multiple`; selector `options: {bind}` of at most 12 rows; composer typed controls; the share sheet as a host-owned sheet behind `host.item.share`, not a sixth component; `prompt.kind` text/date/datetime; nested rows (windows per parent); drag to move; section repeaters).
4. **Generic TCP** is outside this contract; the owner's decision is recorded elsewhere and is not a description concern.
5. **Section repeaters** (one section per row of a bind) and **nested child rows** are in the first contract (owner decision Q11, `Section.repeat`, `RowTemplate.children`); **drag** is a recorded parity exception, with Move to, Move up and Move down standing in.
6. The **retirement date of v1** is the owner's.

## 11. Sign-off

* Native renderer lane: signs off at the tip reported with this revision. Sections 1 to 4 and 6 to 8 are mine; sections 5 and 9 were reviewed against SwiftUI (8.1, 8.2). All fixtures pass
  without a catalogue and against the plugins lane's `contract/host-catalogue-v2.json` (`CALTERNAL_HOST_CATALOGUE=<path> python3 contract/verify-description.py`).
* Web renderer lane: sections 5 and 9 and the content nodes; its sign-off is its own message.
* Dispositions of the review rounds are in `docs/reviews/description-contract-reviews-2026-11-19.md` (review notes are not part of the contract).
