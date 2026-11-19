# Description contract native-lane review notes · 2026-11-19

Historical review of earlier drafts, formerly §§8.1 and 8.2. The current contract supersedes these notes, including its decisions on persisted member rows and nested rows.

## Review of sections 5 and 9 against SwiftUI (native lane)

Everything in 5.0 and 5.1 is drawable with system controls. Findings, in order of consequence:

1. **Settling time (resolved).** One rule everywhere: a selection opens its target about 100 ms after the last key (or key up); selection and navigation events wait while a key repeats,
   window requests and replies for the drawn result version do not (section 8, item 4).
2. **Home, End and Page keys over rows that are not loaded.** "Through ALL rows (drawn or not)" needs the window protocol: the renderer asks for the target window at once (Home from the start, Page Up/Down around the current index, and End as described in §2.3) and moves the selection when the rows arrive. State this as a renderer duty; the host only has to answer windows.
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

## Native review of draft 3 (activation, cache, first paint)

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
