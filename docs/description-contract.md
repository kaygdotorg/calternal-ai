# Description contract (v2)

Status: **revision 14 draft (based on 13.18)**, paired with the [Plugin API v2 specification](plugin-api-v2.md). The [description schema](../contract/description-v2.schema.json) defines the shared renderer shapes.

The schema is checked with `python3 contract/verify-description.py` against the fixtures in `contract/fixtures/description-v2/` (valid ones must pass,
`invalid-*` ones must report the reason in their directory’s `expected-errors.json`). The fixtures are the paper exercises of section 9. The verifier uses `replies/origins.json` to resolve each standalone window reply to its description fixture and Bind, then checks every projected value against the kind’s declared field type.

To verify emitted artifacts, run `python3 contract/verify-description.py --package DIR` or supply `--manifest FILE` with explicit `--app FILE` paths. Replies require an origins file only when present.

A description, manifest, event or reply conforms only if it passes contract/verify-description.py at the pinned revision. The JSON Schemas define necessary structural rules and are insufficient on their own. A schema validator used alone must assert formats, including calendar validity for date-time. Differences in verifier-only rule classes are by design. [Rule classes and enforcement](plugin-api-v2.md#7-author-sdk-and-conformance)

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
| **kind** | A type of item a plugin or the host declares (the host's `requests` and `collections`; a package's `notes` or conversation kind) with its **indexed fields**; every item kind has the host's generic fields (`id`, `title`, `updated`, `trashed`, `trashed_at`). |
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
  "row": { "title": { "field": "title" }, "open": { "route": { "name": "item", "value": "id" } } },
  "states": { "empty": { "title": "No notes here" } } }
```

### 2.1 Sources

* `list`: an index window over a kind's items, optionally one **collection** (a collection id from a literal, the route or the selection, so a collection
  UUID works; it is not a static pattern).
* `item`: exactly one item by reference from the route or the selection. A literal row is never accepted.
* `search`: match text from a claimed-text kind whose manifest declares `searchable: true`, or use the conversation search plan in the catalogue. A foreign-kind search takes eligibility from its owning package and requires the requesting package’s `reads`. Search covers all authorized searchable fields by default. An author may narrow it with the catalogue’s optional `source.field_filter`. The footer sends settled text through `EventSearch`. Its optional `match` chooses literal AND terms or a single literal token prefix independently of the field filter. For claimed-text search, `source.collection` selects the authorized manual-collection projection.

The owner declares its conversation kind. Item lifecycle and protected Inbox supply projected fields: `trashed`, `trashed_at`, `waiting_on`, `pending_for` are host-indexed
fields, so the Notes Trash and the Threads Inbox are ordinary binds.

### 2.2 Filter and sort

`filter` is up to 16 predicates in `all` and `any` (no nesting) on **declared indexed fields** of the kind in the host catalogue (`contract/host-catalogue-v2.json`). A `TextSet` supports `contains` with one Text operand and `exists`. It cannot be sorted.
`eq ne in lt lte gt gte exists prefix`; `in` takes a homogeneous literal list of at most 32. Values are `FilterValue`s: a literal, `{viewer: person}` (so "waiting on me" is
`waiting_on eq viewer.person`; `waiting_on` and `pending_for` accept only `self` as an operand), `{route: ...}` or `{selection: ...}`. **Never a field of another bind, a row, a
form or a prompt**: a hidden field of one bind must not steer another bind's query (the cross-bind operand leak). A search takes a `TextSource` (a literal, a form
field or a composer draft). `{bind, field}` stays legal only in an invoke's inputs and a component's config, where the host reconstructs it. `sort` is at most two
indexed fields. The host-issued row reference is the implicit last key, so order is total and stable across windows.

### 2.3 Windows

The native list and the web list both ask for **index windows**, not cursors, because a long list must size its scroll bar from a count and the person walks it with
the keyboard at 30 rows a second:

```
shell -> host   { type: "window", request: { bind: "notes", from: 120, to: 180, parent?: "<row ref>" } }                 // the catalogue's WindowRequest: the ONE envelope
host  -> shell  { bind: "notes", audience, parent?, window_ref, state: "ready", result_version, count: {mode, value}, offset: 100,
                  rows: [ { id, fields: {...} }, ... ], activation_contexts: [ { row, contexts: { action_id: token } } ] }   // the catalogue's WindowResult
```

The catalogue defines one `WindowRequest` envelope and one `WindowReply` union of `WindowResult` and `WindowPlanError`. The typed refusal carries status 422, code `unsupported_query_plan`, and a reason.

**End-relative windows.** End and near-end jumps use `anchor:"end"` only on non-search Binds with exact count authority. The range counts backward from the last authorized row, with distance zero at that row. Results keep the Bind’s normal order, echo the end anchor, and carry `count.mode: exact`. The offset is the end distance of the returned row nearest the end.

**Forward paging.** The host checks search windows against `EventSearchWindow`, the same window envelope restricted to the start anchor.
For search Binds or with effective count mode `estimate` or `none`, the host refuses an end anchor with `end_unavailable` and sends no end distance. Search End pages forward. An explicit `anchor:"start"` can cross the 500,000 absolute jump ceiling only at the checked next row of a ready window for the same active view, Bind, parent and result version. Deep start replies omit count and the member cache record. In mode `none` show no scroll extent that implies a total.

**Recovery rule.** After skip_limit the renderer pages from the nearer permitted end (start for search Binds or when the view lacks exact-count authority), keeping the original jump target as its goal. After end_unavailable it pages forward from the last loaded row. For a nonempty result set, forward paging ends on a short `ready` window, including a zero-row `ready` window at a page boundary. Keep the rows already shown. Only `state: "empty"` shows the Bind’s empty message. It means the whole result set is empty and is valid only when the requested `from` is zero. On `refine_query`, stop any pending End or forward-paging navigation, keep the rows already shown, and show the refine message. Do not retry the same window. Every other reason shows the Bind error state.
Row ids are opaque and stable. No position is an authority reference.

* A window is the half-open range `[from, to)` of at most 200 returned rows. The absolute jump ceiling is 500,000. The host may materialize up to 4,096 rows and,
  separately, skip up to 250,000 entries in an index-only jump from the nearer end of the scoped covering index. Non-search lists use the maintained count for the authorized scope.
  Claimed-text kinds can count qualified matches with exact-count authority within the separate count-work budget. Threads search uses count mode `none`.
  See the catalogue’s `query.window` for window sizes and `query.search` for the shared search depth. Beyond that depth, the host serves search windows while both work budgets hold.
* The renderer prefetches ahead of the visible rows and, when a key or a drag walks past the edge of what it holds, asks for the next window at once (no
  settling), coalesced to at most 4 window **requests** a second; the host may answer with a larger range than asked (`offset` and `rows` say what came back).
* `result_version` identifies the result set. A new version replaces the window wholesale; the renderer skips a replacement equal to what it draws, so a republish that
  only moved the selection costs nothing.
* **Live:** with `live: true` the host pushes a changed window (new `result_version`) while the view is open. While a key repeats the renderer **applies at once every reply to a
  prefetch for the result version it already drew** (so arrowing never runs into skeleton rows) and **holds only pushes that change the version**, applying the latest when the
  selection settles (or a few a second). A selection held by the settle window is flushed before any activation is sent, so an action never runs against the previously
  settled row.

### 2.4 Row template

`title` draws one ellipsised line from a declared field. The optional
`secondary` line draws below it in the secondary colour and is also
ellipsised. It is an ordered list of up to three declared projected fields,
each formatted as `relative_date`, `short_date` or `text`. Join present
parts with ` · ` and omit absent parts and their separators. Date formats
take timestamps and text takes plain text. The [catalogue’s `Timestamp`](../contract/host-catalogue-v2.json) uses RFC 3339 with seconds 00 to 59 only, and the host records any leap second as second 59 with the fraction clamped.
The renderer computes relative
dates at draw time from the local clock and refreshes them without host
pushes. Rows without `secondary` stay single-line. Row text comes only from
fields.

Any plugin may declare a leading icon on a Bind-produced item row. The optional `icon` may name a symbol, derive from a row field or kind, or use the catalogue’s `RowImageIcon` to select a small host-authorized image from a projected `RowImageRef` field. Follow the catalogue’s `query.row_images` read and cache rules. The renderer draws exactly the icons declared by the description. Thread rows use the host-projected `provider_icon` only when the provider mark binding matches the installed contribution. Missing publisher verification, missing mark ownership or a conflicting binding uses the declared generic fallback symbol. A note preview fits a portrait area of about 30 × 36 pt. Note rows declare the existing `RowImageIcon` portrait aspect and its `image.fallback` snippet field. Keep the last authorized retained image while fresh capture is pending. The host labels it with `stale_images` on the row and rechecks item, source, field and body authority at every fetch. While the thumbnail loads or when none is available, draw a simple document miniature from the freshly authorised snippet. If no authorised snippet is available, draw the document miniature without text. Shortcut rows retain their declared icons.
`marks` (up to three) are trailing dots, counts, texts or icons, each with an optional `when` predicate and a tone. `leadingActions`, `trailingActions` and `menu` are
`invoke` definitions; only the first leading or trailing action may set `fullSwipe`. `open` navigates (sets a route parameter to the row id) or invokes.
Use the host-issued row reference for row identity in selection, events and scope. Item rows use the kind’s row reference. Rows of a repeated section use a `LayoutEntryRef`.

### 2.5 States

`states.empty`, `loading`, `error` and `refine_query` are small messages (`title`, `detail`, `icon`, an optional action). The description or host provides their text.
Show `loading` until the first window arrives and `error` on `state: "error"` or a `WindowPlanError` without an available paging recovery (§2.3).
For a nonempty result set, forward paging ends on a short `ready` window, including a zero-row `ready` window at a page boundary. Keep the rows already shown. Only `state: "empty"` shows the Bind’s empty message. It means the whole result set is empty and is valid only when the requested `from` is zero.
At `from: 0`, the host sends `empty` for a wholly empty result. A `ready` reply at offset zero contains rows. An `empty` reply carries an exact count of zero when a count is supplied. Count-free replies omit it. When a `ready` reply carries an exact count, the host checks the returned range against that count under the catalogue’s `WindowResult` rule.

**`refine_query`.** A qualified search that exhausts its posting or row budget returns the catalogue’s `WindowResult` in this state. See `query.search` and `query.live_limits` for limits.
On `refine_query`, stop any pending End or forward-paging navigation, keep the rows already shown, and show the refine message. Do not retry the same window.
Show provisional rows on an initial refinement reply with the Bind’s message or “Refine your search.” Treat provisional rows as display only and show no total-implying scroll extent or End.
A broad prefix starts no expansion and may reuse only an already-issued window for the same authorized query and generation. A changed query starts a new search.
While the authorized row-and-field projection is behind or stale after revocation, show `loading` with no rows, count or activation contexts.

### 2.6 What the schema needs from the host (authorization is the host's job)

The description only asks. Before any indexed work the host must:

1. authorize the **kind** (a package binds only kinds in its manifest `reads`: the SDK fills a package's own kinds into `reads` automatically, so an author writes `reads` only for another app's kinds), the **collection**, every **filter field**, every **sort field**, the
   **search** and every **output field** against the viewer, the space and, for a guest, the item's grant;
2. reject a field not declared indexed by the kind, and a field outside the audience's projection (a guest entitled to titles must not be able to infer hidden
   metadata from the count of an `eq` filter or from row order: filters, sorts and counts on a field the audience cannot read are refused, not filtered);
3. produce **host-owned projections** for guests: rows are built by the host from the item's grant, never by plugin code;
4. authorize the **whole dependency graph** of an app (no bind takes an operand from another; a search is pinned to the audience's corpus; a count is taken over the
   already-authorized projection). A guest gets an `exact` count only over an explicit shared root or child projection with count disclosure enabled. Otherwise its mode is `none`, including nested windows. For search Binds or any audience whose mode is `estimate` or `none`, refuse the end anchor with `end_unavailable` and send no end distance.
5. bound the work: windows of at most 200 rows, 16 predicates, **16 binds per app** (chrome and view together: there is one list), and absolute jumps up to 500,000, with the contiguous paging exception for views without exact count authority in §2.3. Apply per actor and
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
  that no edge references is refused by the verifier). `{parent: true}` in a filter is only an optional extra predicate. **A bind is a section's rows OR a children target, never both** (the verifier rejects every dual-edge bind; section rows always present `LayoutEntryRef`, child rows the kind's own row ref). **A per-parent bind is consumed only through its declared edge**: no ordinary section, list, count or header may reference it (the verifier rejects it), and `persist` is refused on it (only ROOT windows may be persisted, section 6). The renderer asks for a parent's children with its own windows, only when the row is expanded; a row with children draws the system's disclosure control (a `List`
  with `children`/`DisclosureGroup`). Depth is at most 3, counted through the chain; the host checks it and the verifier rejects cycles. Child threads under their parent are this.
* **User-made sections.** `Section.repeat: {over, rows}` makes one section per row of the bind `over`, which **must read `layout_sections`** (the verifier rejects any other kind; the per-person layout the host stores: the host virtual kind `layout_sections` with
  `id`, `title`, `position`, viewer-only) with each section's rows from the per-parent bind `rows`. The layout is stored and authorized by the host; **Move to, Move up and Move down are host
  operations** (`host.layout.move_to`, `host.layout.move_up` and `host.layout.move_down`, and `host.layout.section.create`, `rename`, `remove` for personal sections)
  that a row or section menu names. For a `LayoutEntryRef` input use `{context: "placement"}` in a repeated section row, the bare row id (`{row: "id"}`) in section rows,
  or a matching single-selection operand (`{selection: bind}`) over the section’s rows. The verifier checks the operand against the catalogue input type.
  In a repeated section a conversation ItemRef input uses `{context: "item"}`. An ordinary conversation row ID is its ItemRef.
  `host.layout.move_to` takes either `thread` to add an unplaced thread or `entry` to move a placement, plus a `section` choice (`ChoiceRef`). Move up and Move down
  take `entry`. The host reconstructs the references from the activation context. Drag is a recorded parity exception, with Move to and Move up/down standing in.

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
card), `host.undo` and `host.action.status` (an outcome), `host.panel.ask` (the transcript's quote control), prepared Send and `host.execution.stop` (the composer),
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
  For an otherwise authentic host.item.create activation whose collection was deleted or lost edit access after resolve, the opaque collection_unavailable outcome takes precedence over the generic stale dependency outcome.
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
  no header and therefore no header actions. A shortcut's literal route value, including one set by its shell action, must be an `Id` that the renderer can send in `EventRoute.row`. There are no cards: a shortcut row or a `ShellAction` row covers what a card did; an approval is the `request-card` component.
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
* **Lists** are binds. On every platform, Up and Down move the selection through the rows when `selectable: true`. The route opens when selection settles, about 100 ms after the last key or on key up. Selection and navigation events wait during repeats. Apply prefetch replies for the result version already drawn at once. Hold only version-changing pushes until selection settles. Prefetch requests still cross up to the normal request cap (§8). The web renderer waits 1.6 times the longest gap between repeats in the burst, between 45 and 120 ms. A route load starts only after repeats settle, and a new arrow cancels the previous load.
* **Home, End and Page Up/Down** scroll the list without changing selection on any platform. The renderer requests unloaded windows for the viewport. Home asks for the initial window. On a non-search Bind with exact count authority, End requests the end anchor and Page Up/Down may use it near the tail. Search End and End without exact count authority page forward from the last loaded row. Follow §2.3 during recovery. Count-free views show no extent implying a total.
* **Search refinement** uses the `refine_query` Bind state in §2.5.
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
| `conversation-transcript` | `conversation` (checked ItemRef from route or selection), `follow` end/none, `window`, `quote` (boolean: the host's Quote control) | `transcript.page` (the host route: `TranscriptPageRequest` -> `TranscriptPage`, catalogue), `quote` (a host-issued quote reference into a draft), `scrolled-away`, `rendered` | A host-owned windowed list: the screenful at the end is read first and drawn, the rest follows in steps; rows are appended and updated by the host; heavy parts are the host's. |
| `composer` | `conversation` (checked ItemRef), `controls` (which host controls appear: `attachments`, `mentions`, `provider`, `model`, `reasoning`, `permission`, `machine`, `stop`), `placeholder` | `draft` (device only), `send` and `stop` (the host's), `attach`, `mention`, `composer.options` (`ComposerOptionRequest` -> `ComposerOptionPage`, catalogue) | The draft is held on the device. The host owns the send: it dispatches the kind’s configured Send with a checked item, draft and options in one prepared exchange; a native composer is a text field with the system keyboard and system pickers. |
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
* **Drafts and options.** The composer's text stays on the device. It reuses the existing host routes for attachment upload and mention lookup. Attachments are held as host-owned `AttachmentRef`s, mentions are account references, a quoted passage is a host-issued quote reference (the catalogue's `Draft`), and the chosen provider, model, reasoning level, permission and machine are opaque option references the host returned (`Interaction.options`). The description names none of them: it says which controls appear. The host checks everything (lengths, counts, that every attachment is the viewer's own, that every option is one it offered for this thread) and composes the send; it never trusts the client. The one-exchange Send treats the final text **solely as message content**: anything the host parses out of it that changes recipients, routing, permissions or other prepared material (a mention, a slash command) needs a fresh preview before it takes effect (3.2). The draft clears only after `ok`; `error` keeps it.
* **Transcript reads** use `transcript.page` with a checked conversation ItemRef. With neither cursor, read the newest page. Use `older_cursor` as `before` when scrolling up, and the committed cursor as `after` when catching up. Supply one cursor at most. Pages are returned oldest to newest, default 60, maximum 128 records and 128 KiB. On reconnect refresh the boundary record and the bounded notice prefix in that reply. Follow through the existing view subscription, replacing records by stable UUID and applying the returned committed cursor. Cancel on selection or authority change. The renderer never invents a cursor.
* **Prepared composer Send** uses the host-issued `composer.send` activation context for the active composer. Resolve that action with checked draft/option material, then confirm with `final_text`. The reply is the committed `ActionOutcome`, with a status ref while Starting. Retry the exact activation and text. The host supplies `composer.stop` only with current execution-control authority.
* **Composer choices** send the current checked option refs in `composer.options`. Resolve provider first, then model, then reasoning. Machine and permission choices are checked against the admitted provider/placement and viewer. A change upstream invalidates downstream refs. `selected` and `default` are nullable option refs, and the renderer sends selected refs under the same keys in `interaction.options`.
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
  Accept only `MemberRowCacheRecord`, the display-only exception to session-only projections. Its host-approved offline display fields exclude every body-derived part. For Notes the cached secondary line keeps the authorised date as `subtitle_timestamp`. Format it from the local clock at draw time using the pinned template. Reserve the same line height until a fresh authorised reply adds the snippet. The authenticated `ViewOpenResult` and `WindowResult` name the audience.
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
   measured real row height (the system sets row height). Index windows are required. Use a count for the scroll extent only when the Bind authorizes it. Measured: a held arrow key through a 5,000-row list
   at 33 keys a second has to ask for windows without settling.
2. **Selection outside the row models**: a row reads "is selected" from the list, not from the description; the description does not carry `selected` flags for
   drawing. (A page may still say which row it selected; the shell treats a later change as a reveal request.)
3. **Equatable rows**: row values are small and comparable so an unchanged row is skipped when a window is replaced.
4. **Up and Down move selection. Home, End and Page Up/Down only scroll** on every platform. While an arrow repeats, selection and row events go out once after the key settles (about 100 ms after the last key). Window **requests** for
   prefetch may cross, coalesced to at most 4 a second; replies to prefetch for the result version already drawn are applied at once and only pushes that change the version are held until the selection settles,
   decoded and compared off the main thread; a held selection is flushed before an activation is sent. A republish that only confirms the selection must not touch the list.
5. **One title line**, then the declared secondary line in the secondary colour. Ellipsise both lines. Rows without a secondary line stay single-line. Use one aligned icon column where the description supplies leading icons. While a navigation key repeats, defer image fetches and cancel reads in flight. After it settles, fetch only still-visible rows’ current refs. For a note without a current thumbnail, draw the miniature from the authorised snippet, or the document miniature without text before the snippet is available. Use a single neutral accent: the accent is passed only to the unread mark and a prominent action.
6. **Swipe actions** need `title`, `icon`, `tone` and `fullSwipe` on the invoke; they draw with the system's swipe look, tinted by tone, never by the root tint.
7. **Haptics and menus** are the shell's: selection feedback on `route` and `selection` events, a system menu for `menu`.
8. **Touch targets** are 44 pt; the renderer, not the description, sizes controls.
9. **The first paint** may come from the cache (section 6) before any host is reachable: the template, and a member's persisted rows marked as not yet confirmed (their actions disabled until a fresh context arrives).
10. **Nested rows and user sections** map to a `List` with `children` (disclosure) and one `Section` per layout row; windows per parent are requested only for expanded rows.

## 9. Paper exercises

Run `python3 contract/verify-description.py` with the host catalogue. The fixtures in `contract/fixtures/description-v2/` pair each app with its manifest in `manifests/`. The SDK supplies each package’s own reads and the fixtures include the emitted entries. Section 6 governs the optional member rows used by `threads-nested-catalogue-app.json`. The Notes and Threads walks follow §2.4. Threads shows the installed provider contribution’s icon or its fallback symbol. Note rows use the current thumbnail or miniature. Notes collection rows use the kind icon.

### 9.1 Community Reading List and author trial

The list binds `reading-item` and shows a title, read mark and empty state. Mark read uses a first full swipe through the host metadata toggle, with one exchange, Undo and no runtime wake. The two-field Add form calls `reading.add` and opens the host-minted `created` ref through `then`. The footer searches all authorized searchable fields of the same opted-in kind. New blank item uses the manifest’s `host_create` defaults through `host.item.create`, commits Undo with the item, and opens it through `then` without waking the plugin.

The reminder form calls `reading.remind`, saves the reminder with `item_save` and enrolls the declared `reading.reminder-due` operation using `schedule_create` with a one-date calendar and `notify_creator`. The example keeps the reminder in the person’s own space. A reminder about a shared item lives there and points to the shared item under §5 of the API. The person filter only controls which reminder rows the list shows. The SDK defaults for a first list and the required author trial are in the API’s §§7–8.

### 9.2 Notes

The windowed sidebar binds a manual collection with an exact count and shows a Collections section and a Trash Bind. Note rows show the updated date and then the host-indexed snippet, bounded by the [catalogue’s `query.body_excerpt`](../contract/host-catalogue-v2.json). Restore uses a full swipe on an eligible single item. Empty Trash uses the host sheet with a Manager visibility rule, and Share uses its host-owned audience sheet. Rename through Notes’ link-rewriting path resolves and dispatches separately, with Undo fetched through the typed outcome/status route. Bulk Trash resolves the checked `selection_ref`, then confirms and dispatches. Move to collection fetches the scoped choice and applies the catalogue’s challenge rule for the destination audience.

The `item-editor` edits the routed `yjs-v1` item. New note invokes `host.item.create` with the Notes manifest defaults and the current route’s collection when present. It commits the new item, membership and Undo together without waking the plugin or showing a challenge, then opens `created` via `then`. Without a collection route the same invoke creates the note without placement. The footer searches the authorized manual collection through `source.collection`. Window, bulk, snooze and search cases are covered by `notes-app.json`, `notes-bulk-and-snooze-app.json` and the search event fixtures.

### 9.3 Threads

Inbox filters `waiting_on eq viewer.person` with an exact count on that viewer’s projection. All Threads supports per-parent child windows and personal sections repeated over `layout_sections`. Section rows distinguish the placement address from the underlying thread (§2.7). The host supplies Move to, Move up/down and section create, rename and remove. Drag remains the recorded Q11 parity exception.

New thread uses manifest `host_create`, `host.item.create` and `then`. The composer uses host provider, model, reasoning, permission and machine controls, checked attachments and mentions, and prepared one-exchange Send. The host streams `transcript.page` and live updates and owns the approval card and follow-up. The footer searches authorized titles and messages in one Bind. The catalogue’s optional field filter can restrict that Bind to titles or messages. The API’s Step 2 writer gates control idle archive full swipe with Undo and per-actor snooze. An active archive uses a challenge.

The `threads-app.json` and `threads-nested-catalogue-app.json` fixtures exercise the composer, Bind and layout shapes. The paired `search-page-boundary` fixtures show a full first page followed by a zero-row `ready` window, which preserves the rows on screen. Their later-page `empty` variant is refused. The window reply fixtures also cover exact-count End, a guest refusal, refinement and the member cache record.

### 9.4 What this schema needs from the host (authorisation is the plugins lane's)

1. Authorise every bind (kind, collection, filters, sorts, search, output fields) and every ref (`item`, `thread`, `request`, `frame`) against the viewer, the space and a guest's
   grant **before** any indexed work or read, and reject a field the audience cannot read instead of filtering it (counts and order leak otherwise).
2. Classify each operation as reading, mutating or disclosing from the manifest, and name the material inputs the trusted preview shows; the description may only add.
3. Reconstruct `{row}`, `{selection}` and route sources from the activation context (never from the event) and read `{form}` and `{prompt}` only as device-held values in `interaction`; answer `stale` when the context no longer holds.
4. Provide the host built-ins named here (`host.item.rename`, `host.item.trash`, `host.undo`, `host.approval.resolve`, `host.panel.ask`, `host.execution.stop`; `host.route.set` is a shell control, not an operation) so a
   swipe or a card never wakes a space runtime, and a **checked draft payload** for the composer.
5. Own the transcript's streaming, appends, rate cap and final commit; and the frame's sandbox.

## 10. Open questions and what this contract does not decide

1. [Metadata patch](../contract/host-catalogue-v2.json#/operations/host.item.patch-metadata) uses the metadata arm of `item_save`. [Trash purge](../contract/host-catalogue-v2.json#/operations/host.trash.purge) is a confirmed Manager or Owner job.
2. Reminders use the scheduler’s [`notify_creator`](../contract/host-catalogue-v2.json#/$defs/ScheduleCreateInput/properties/notify_creator).
3. **Generic TCP** is outside this contract; the owner's decision is recorded elsewhere and is not a description concern.
4. **Section repeaters** (one section per row of a bind) and **nested child rows** are in the first contract (owner decision Q11, `Section.repeat`, `RowTemplate.children`); **drag** is a recorded parity exception, with Move to, Move up and Move down standing in.
5. The **retirement date of v1** is the owner's.

## 11. Review record

The native and web renderer lanes record their sign-offs on the pinned combined revision. Earlier [native-lane notes](reviews/description-contract-native-history-2026-11-19.md) and [review dispositions](reviews/description-contract-reviews-2026-11-19.md) are historical.
