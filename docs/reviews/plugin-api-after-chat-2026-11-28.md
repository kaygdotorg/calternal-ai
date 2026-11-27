# What Chat proved about the plugin API · 2026-11-28

The new host can install Chat from a local plugin folder, keep conversations
as readable Markdown and let a signed-in person create, rename and send
messages in a browser. The browser learns which lists, fields and actions
exist from the host. A sandboxed plugin operation handles Send. The host
enforces access, stamps the author and records the durable result. Two
browser sessions for the same owner can see the transcript update and
recover missed messages after reconnecting. This is the useful base for
other apps. The full journey from a browser send to a real provider reply
is still open.

This review reads merged `dev` at `32dc5517`, the later browser work at
`origin/lane/browser-chat` `4d0c500d` and the run-service work at
`origin/lane/runs-harness` `293af65a`. Test results below are those reported
by the lanes and coordinator. [Coordinator's record](../morning-2026-11-21.md#slices-4-and-5-on-the-real-host-merged-2026-11-27) ·
[Browser follow-up](https://git.kayg.org/kayg/calternal-ai/src/commit/4d0c500d3c5e77e901922a5278619bb7a7c8b0e5/docs/reviews/browser-chat-findings-2026-11-27.md) ·
[Run-service findings](https://git.kayg.org/kayg/calternal-ai/src/commit/293af65acdcf47de8920f8b2ec1a23e313081809/docs/reviews/runs-harness-findings-2026-11-27.md)

## What worked and what had to change

| What Chat showed | What the API needed |
| --- | --- |
| A plugin can describe a real collection, sidebar, transcript and ordinary item actions. The browser follow-up uses the same shell components as the mockup and sends requests to the standalone host. | A compiled `s.declarations` catalog replaced browser-side parsing. The host returns per-person state and evaluated row conditions, which let Pin and Unpin change with the viewer's state. |
| Rename and send work as code while the host remains responsible for rights and writes. The worker calls back through the checked route. Killing it before the write commits leaves no half-written message. | Coded operations needed typed `inputs:` and a single `item` key for the target. The format allows a slot to reference the same schema. Change listeners are discovered in an isolated install probe. Workers recycle when the acting person changes, and oversized worker replies receive a bounded error. |
| A committed message has a stable position, even if another arrives in the same minute. Identical-key replay reuses the append, and a complete host-written append can be recovered after an interruption. | Unread state needed a record anchor and a ledger-order count. Append needed a receipt with the host-issued record link, and a 34-byte header seal to authenticate a complete tail after a crash. The pure format engine keeps that seal opaque. |
| The browser branch selected a disposable installed harness filler through the actual call route. A sender can choose a third-party filler that Chat did not name at install. | The explicit selection admits exactly one declared operation during the call. Chat passes the appended message link to that operation under the same outer request key. A failed start can leave the message committed. A retry with that same key recovers the append receipt. |
| Browser sign-in and live delivery worked over the real host. The later browser lane reported five passing Chromium suites, including the picker, generated rename form, Pin/Unpin, reconnect and sign-out, with desktop and phone-sized screenshots. | Strict passkey session cookies and origin checks replaced the preview session path. The WebSocket uses the same authorized reads as the call route. A private state update is queued for its own viewer. The growing reply and Stop controls were exercised with a browser fixture. |
| The pure format engine compiled Chat's declarations and could evaluate queries and writes without filesystem access. | Money-relevant arithmetic needed base ten with 38 coefficient digits, 18 fractional places, truncating division and ties rounded away from zero. The server owns install validation, record authority, storage and query context. |

[Format engine findings](format-engine-findings-2026-11-27.md) ·
[Host core findings](host-core-findings-2026-11-27.md) ·
[Worker findings](plugin-process-findings-2026-11-27.md) ·
[Host call route](host-call-route-2026-11-27.md) ·
[Live protocol](live-window-protocol-2026-11-27.md)

## What remains unproven

- **One complete agent turn.** The merged host refuses the bundled
  Claude Code manifest's programs and outbound destinations. The runs
  branch separately proved an allowed fake-provider request through the
  host proxy, denied direct networking and input from another actor, and
  stopped a process family within two seconds. The browser branch used
  a disposable filler and a simulated stream and Stop. Connecting these
  pieces must still prove scope checks on each call, replay of an actual
  run start, reconnect to the run log, real Stop from the browser, one
  completed reply record and a real provider turn.
- **A retry from the browser.** The worker and host tests exercise stable
  request keys. The browser creates a new key each time Send is submitted.
  After a start refusal or lost response it puts the text back and drops
  the selected quote. A second click can therefore be a new append.
- **People other than the owner.** The current boundary is one owner per
  personal space. Two browsers sign in as that same owner. Live access
  loss needs an explicit close-and-clear signal before anyone else is
  admitted. Shared writers also need an authoritative author source for
  sealed-tail recovery.
- **Everyday resilience and install trust.** Change delivery can disappear
  on a host restart between commit and notification. Whole-worker
  resource ceilings, private operator setup on a standalone deployment,
  application install approval, asset classification, and image program
  loading still need evidence. Developer folder install proves local
  packaging. The operator still needs a path that makes each privilege
  visible before an app install or update.
- **Breadth.** Chat exercises one browser and a narrow part of the format.
  It leaves generated CLI and MCP actions, native rendering, the slim
  shared Wasm client, offline writes, attachments and mentions, shared
  collections, DAV, fuller repeats, period fields, extensions, themes,
  general views, and other apps unproven. A link to a record in a real
  transcript has a storage type. The browser's quote gesture has no
  working end-to-end link contract yet.

## What the client or host still has to infer

The browser branch removed icon guesses based on words in titles, but
several remaining defaults still choose the meaning of plugin data. This
inventory follows `apps/web/src/host` and `packages/canvas-shell` at
`4d0c500d`, plus the merged host's declaration projection.

| Surface | What the code does today | What the declaration or host still owes the client |
| --- | --- | --- |
| Sidebar entry icons and view | `HostSidebar` renders collection entries without an icon, in collapsible lists capped at six visible rows. `HostCollection` also uses a list regardless of the view. The pinned fixture declares `compact`. | **Decided:** optional catalogue `icon:` on a collection and on its sidebar entry, with the entry taking precedence. Declare `strip` on Pinned. Typed view data must reach the renderer so it can draw the specified layout. |
| Row roles and accessibility | `rowTitle` picks the first usable text value in `kind.row`, then tries `title` or `name`, then “Untitled.” `rowBadge` picks the first positive computed or numeric row value and reads its raw field name aloud. Both row renderers use the title's initials as the avatar and `edited` as the secondary text or time. | **Decided:** `badge` selects a numeric field, and the field name labels it for accessibility. Explicit title, subtitle, leading image or person, timestamp source, and any richer badge visibility or label policy still need a presentation contract where an app requires them. |
| Composer slot, text, draft and quote | `resolveComposerSlot` chooses the first single slot with a recommended or first operation filler whose final name matches an operation on the current kind. It picks the first required text input, then any text input, then `text`. It picks the first record-link-looking input. `HostRecords` still checks and reads a field named `quote`, stores text in browser storage under the item ID, and sends a bare anchor. The quote preview looks for an exact matching anchor in the loaded transcript. | **Decided:** the composer's typed `gives` names the text input, optional draft state key and optional quoted record-link input, checked against the operation schema and target kind. The host must supply the canonical link for the selected readable record. Link resolution for display, stale or unloaded quoted records, pending draft edits and draft cleanup still need behavior. |
| Harness picker and labels | `resolvePickerSlot` takes the first other single person-picked slot with operation fillers. The UI uses the humanized slot ID and the filler plugin's name. The host discards the fill's existing `as` label. The browser uses one sparkle icon and remembers the pick in that composer instance. | **Decided:** declare a slot `label` and carry the filler `as` label through `s.declarations`. A slot's relationship to this composer, choice persistence, and how a slot takes part in a particular gesture still need a contract if there is more than one eligible picker. |
| Which app, sidebar and window open | `HostApp` treats plugins with sidebars as apps, chooses the first such plugin and first sidebar, then prefers its first list collection before falling back to another collection. It groups an item under the plugin prefix in its kind ID. Sidebar lookup strips qualifiers and uses the current plugin's collections and kinds. Strip defaults to a generic app icon; `AppIcon` draws nothing for an unknown token. Custom sidebar names and strip order are local component state. | Declare or project which reachable sidebar and default window to open, and preserve qualified references across plugins. The format already gives first-run authority to the space profile. The resulting browser projection and persistence of personal customisations are missing. |
| Forms and item actions | `operationFormFields` humanizes identifiers and prefills a string when a row field has the same name as the input. Add falls back to `title`. `HostFormSheet` sends strings and has special controls only for long text and URL. Boolean Change becomes a generated toggle, with names such as “Unarchived” derived from the raw key. Menus treat a missing `row.when` entry as allowed. The client directly writes one literal set target. Computed, prompted and multi-target actions have no general invocation here. | Typed inputs need a wire value and native control for each supported type, a defined source for defaults and prefill, and action labels, ordering and placement where identifiers cannot serve. Operation applicability needs an explicit missing-entry rule. A general invocation has to cover the declared operation forms, including computed assignments and record-specific conditions. |
| Transcript and record actions | `HostApp` draws a transcript for item windows without selecting a declared `page` entry. `HostRecords` renders the body as text, picks five-minute author-grouping, uses browser-local dates and falls back to author ID. Record edit/remove menus check ownership and operation presence and use a body-only editor. They do not use the declared record fields, edit time window or a per-record eligibility result. Quote preview uses the first body line. | Use typed page entries and projected, authorized per-record affordances. Decide which transcript choices are renderer defaults and which the plugin can override. Supply readable linked previews, field controls and resolved author names through the host. |
| User intent, retry and live status | The browser submits every collection as a live subscription and caches opened item rows and records. Its live client has no UI handler for per-subscription errors and does not receive a rights-loss event. It shows a local simulated run with a timer and fixture Stop. Retry recreates the send key. | A client needs a complete accepted/committed/failed story, a stable gesture key and a way to identify and resume or stop a real run. Access loss needs a host signal that clears affected cached rows, transcript and UI. Session sign-out already has a separate socket and client reset path. |
| Typed projection through the host | `declarations/browser.rs` divides view column strings on commas, recovers page roles by matching text, and parses fill targets from strings. That can discard labels or misread a comma inside a formula. The browser receives only the queryable one-kind collection subset, without a capability description for omitted surfaces. | Carry typed presentation nodes from the format engine to the catalog. Distinguish a valid but unavailable feature from one the plugin did not declare. Keep caller-specific rights on the host. |
| Shell features and fixtures | `Canvas` has a Share button with an empty handler and side-panel controls supplied by the shell caller. The real browser does not supply the Details/Assistant panel. The space picker is fixed to “Personal,” and the sidebar search affordance has no host integration here. The shell's icon catalogue, focus rules, motion, screen breakpoints and device-local sizes are its own defaults. | Share, search, panel and space choices need real capabilities or declared placements before they are offered. Shell interaction defaults can stay with the platform and need no per-plugin guess. |
| Install and generated clients | The format accepts a program ID without a binding. The host rejects unclassified assets altogether. Field type and constant data come through the catalog, but the browser uses JavaScript's default JSON number parser. | A content-based install classifier and bound program runner are needed. Define how clients preserve exact values when rendering or editing them, especially past JavaScript's exact range. A path list cannot prove data-only content. |

[Browser code at the audited revision](https://git.kayg.org/kayg/calternal-ai/src/commit/4d0c500d3c5e77e901922a5278619bb7a7c8b0e5/apps/web/src/host) ·
[Shared shell](https://git.kayg.org/kayg/calternal-ai/src/commit/4d0c500d3c5e77e901922a5278619bb7a7c8b0e5/packages/canvas-shell) ·
[Engine findings](format-engine-findings-2026-11-27.md)

The coordinator's presentation decisions and exact fixture line edits
are listed in revision 10. The proposed fixture changes add the
`badge` flag to Unread, switch Pinned to `strip`, give Archived an
explicit icon, bind the composer to `text`, `state.draft` and `quote`,
and label the harness slot. The Claude Code fill already has the correct
`as "Claude Code"` label. Its projection needs to keep it. The harness's
program and start-input changes follow the run integration. [Fixture line list](../proposals/kind-and-collection-format-2026-11-24.md#chat-fixture-follow-up) ·
[Code and specification differences](../proposals/kind-and-collection-format-2026-11-24.md#22-code-and-specification-disagree)

## Questions for the owner, in blocking order

1. **Who can enable a plugin that runs a program and sends data outside?**
   The real harness install needs an approval path before the browser can
   offer it. Claude Code, for example, starts a bundled program and sends
   readable chat text to its declared provider. An Owner approval for
   the bound bundle and destinations, followed by the person's explicit
   harness choice, costs one install prompt and a clear disclosure at
   selection. Allowing any member to install program-and-network
   plugins removes the Owner step but extends trust to every member's
   install choices. I recommend retaining Owner approval and the
   declared-host proxy, with no program execution before the isolation
   and resource limits are verified.
2. **What should a person see when a message is saved but its agent
   cannot start?** This needs an answer before testing retry with a real
   harness. If a provider is unavailable, the current worker can save
   “Review this” and then return an error. Showing the saved message
   with a retry tied to that gesture needs a partial-result status and
   a stable request key, and preserves the transcript. Making it appear
   all-or-nothing requires durable coordination of the append and
   external start across crashes, with more latency and failure cases.
   I recommend showing the saved message and retrying only the start
   under the original request key.
3. **Should someone who loses access have the affected window
   closed and cleared immediately?** This is the release gate for
   shared live windows. If a person is removed from a chat while it is
   open, the host can revoke that window and require the client to drop
   the transcript, previews and pending state. That costs a protocol
   event and cache eviction paths. Keeping the last snapshot visible
   and blocking future updates is cheaper, but leaves previously
   delivered content on screen after access was withdrawn. I recommend
   immediate revocation and keeping live windows owner-only until it
   works on every supported client.
4. **Should a draft follow a person across their signed-in devices?**
   The composer now has a declared draft state key, so the first
   implementation has to pick its persistence behavior. Starting a
   reply on a phone and continuing on a laptop requires per-person host
   state, access-loss and logout cleanup, and a conflict rule for two
   open editors. Device-only drafts avoid server writes and reduce
   exposure but do not follow the person. I recommend device-only
   drafts by default and synced per-person drafts only where the plugin
   explicitly binds a draft state key.
5. **How much of a new app should the same declarations draw across
   browser and native clients?** This determines the next proof after
   Chat. A Money list should distinguish Balance from a badge, name
   its actions and edit an exact decimal without turning it into text.
   A compact set of typed presentation roles costs a few format additions
   and native renderers, then provides a reusable second-app test. A
   general layout and action language offers more flexibility but asks
   every client to implement a larger surface before it ships. I recommend
   testing a second app with the compact roles and growing the catalogue
   when an actual screen needs more.
6. **Which background reactions must survive a host restart?** A
   committed change can currently be followed by a crash before its
   listener runs. Rebuilding a cache after restart can be cheap and can
   tolerate that loss, while a committed action that must update another
   item needs durable delivery and idempotent effects. A queue for every
   reaction adds stored work and recovery to writes. Constraining
   reactions to rebuildable work keeps the host smaller and defers
   durable cross-item behavior. I recommend the constraint for the
   Chat-first release and a durable delivery contract before plugins
   can rely on reactions for user-visible changes.
