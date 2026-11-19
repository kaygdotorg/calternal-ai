# Description contract: dispositions of the review rounds (2026-11-19)

Dated review record, moved out of `docs/description-contract.md` (the contract holds no review notes). Round 3 is Sol (46 findings); round 4 is Opus (see the table below).


## Round 4 (Opus 5.5 on the combined specification 4edd9099): the description side

| # | Finding (short) | Status |
| --- | --- | --- |
| spot | Schema text still teaching the old model (Invoke, preview, ValueSource, FilterValue.selection, Composer.send, FormNode, 9.4.3) | FIXED in the schema (Invoke, ValueSource, FilterValue, Composer); web lane for FormNode and 9.4.3 |
| spot | `{bind,index,field}` addresses a row by index | FIXED: no index; the context pins a stable row id; legal only in component config and a header title |
| spot | `Invoke.operation.not` lacks purge, share, turns.start, turn.stop | FIXED (schema) and verified against the catalogue's `generic_invoke` |
| spot | `Footer.account.initial` person data in a persisted template | FIXED: removed; the shell draws the initial from the session |
| spot | `{selection: other bind}` legal operand? | FIXED: NO; a row's `open` sets a route, and the route is the one way a bind steers another |
| 1 | No message delivers `activation_context` | FIXED by the plugins lane (WindowResult.activation_contexts, ViewOpenResult, lease renewal) and cited in 2.6.6, 3.2 |
| 2 | Two shapes for the window message; EventSelection.row pattern | FIXED: the schema cites the catalogue's WindowRequest and WindowResult; row ids are opaque refs |
| 3 | Composer cannot submit what it shows | PARTLY: Composer.config is the thread and the host controls (send/target/enabledWhen/{control} removed); the host composes the send; web lane to finish the descriptions |
| 4 | Missing operations (create, archive, snooze, follow-up, created ref) | A (catalogue); the schema adds `Invoke.then` for the created ref |
| 5 | patch-metadata not invokable | A (catalogue); fixtures use it for pin and mark-read with `fullSwipe` |
| 6 | Verifier: manifest per fixture, fullSwipe on plugin ops, plugin_kind, not/if/allOf/dependentRequired | FIXED: manifests per fixture, fullSwipe rejected unless the catalogue allows it, kinds resolved from catalogue or manifest, engine implements the keywords |
| 7-9, 11, 14, 15, 20, 24 | Manifest, catalogue, owner decisions, runtime, build order | A / owner |
| 10 | No wire for undo, status, sheets, request card | A (catalogue routes); the Event union cites view_open, lease_renew, undo, action_status, choice_options |
| 12 | Stale text; fixture bugs | FIXED: plural kinds, one settle rule, 8.1/8.2 updated; notes route item/open note, header title from an item bind (verifier enforces), footer search with its own bind |
| 13 | One place per fact | FIXED for host duties (2.6 here, Part A for the rest), one bind list per app, one place for confirmation (catalogue); web lane for selectionMode and enabled |
| 16, 17 | Every mutation challenged; chat send three round trips | FIXED in 3.2 by the catalogue's `confirm: none | challenge`, `dispatch: true`, composer `final_text` |
| 18 | Prefetch replies held; stale selection on activation | FIXED in 2.3 and section 8 item 4: prefetch replies for the drawn version apply at once, a held selection is flushed before an activation |
| 19 | Cold open, resume | FIXED in section 6: do not paint is not discard; lease renewal repaints; persistent member rows remain an owner question |
| 21 | Bulk Trash round trips | A: up to 50 ids in the resolve's interaction |
| 22 | Remove from the first contract | FIXED: confirm, preview.material, pending, result.show, viewer space and roles (except `showWhen.minRole`), cards, appearance/scheme/accent/titleImage, one bind list; commands (A) |
| 23 | Parity exceptions | choice from a bind and host.collection.move in the catalogue (web lane writes the prompt kind); nested child rows and user sections stay OPEN |

## Round 3 (Sol, 46 findings)

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

## Round 5 (Sol, final narrow check of d2ebcbc8) and revision 6

| # | Finding / decision | Status |
| --- | --- | --- |
| 1 | Sheet-free mutations only where the engine commits effect and Undo together | FIXED in 3.2: catalogue `atomic_undo`, worker-free; a manifest risk grants none; plugin operations take resolve + dispatch (two exchanges), a sheet only where material requires |
| 2 | Context must match the DRAWN selection | FIXED in 3.2: SelectionContextResult after each settled selection; wait if missing; the host never resolves against the server's current selection |
| 3 | Final text is message content only | FIXED in 3.2: parser-derived changes (mention, slash command) invalidate the proof and need a fresh preview; receipt bound to the final text |
| 4 | LeaseRenewResult correlated and authenticated | FIXED in 6: renewal_id, lease, partition, retained_results by window_ref; superseded ignored; rows hidden on timeout or refusal |
| 5 | One window envelope | FIXED: the catalogue's WindowRequest `{type:"window", request}` is the only envelope; the schema cites it (with optional `parent`) |
| Q10 | Member own-space rows persist | RECORDED in 6 with Sol's objection and the conditions (unlocked device, account+space keys, 14-day retention, never guest/public, never contexts or credentials, erase on sign-out/switch/known revocation, display only); `Cache.memberRows`, `Bind.persist` |
| Q11 | Nested child rows and user-made sections in the first contract; drag an exception | FIXED: `BindSource.parent`, `RowTemplate.children`, `{parent}` operand, `Section.repeat`, host layout operations; drag recorded as a parity exception |
