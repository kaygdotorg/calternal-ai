# Description contract: disposition of Opus review 4 (web lane's findings)

Reviewed text: `/home/kayg/.bb/thread-storage/thr_58j59i9gw3/review4-opus.md` against combined revision 4. Owners: **W** web lane (sections 5 and 9, content nodes, composer, fixtures, manifests),
**N** native lane (sections 1 to 4, 6 to 8, bind and chrome defs, verifier engine), **A** plugins lane (catalogue, manifest, API). Status as of the renderer tip that reports this file.

| # | Finding (short) | Owner | Status |
| --- | --- | --- | --- |
| spot | Leftover descriptions teaching the old model (Invoke, preview, ValueSource, FilterValue.selection, Composer.send, FormNode, section 9.4.3) | N, W | FIXED: Invoke/preview/ValueSource/FilterValue by N, FormNode, Composer and 9.4.3 by W |
| spot | `{bind, index, field}` by index | N | FIXED: stable row id |
| spot | `Invoke.operation.not` omits purge, share, turns.start, host.turn.stop | N | FIXED |
| spot | `Footer.account.initial` in a persisted template | N | FIXED: removed |
| spot | `{selection: other bind}` as a filter operand | N | FIXED: not legal (FilterValue is literal, viewer person, route) |
| 1 | No message delivers the activation context | A | v5 draft: WindowResult contexts, ViewOpenResult, lease renewal; W cites them |
| 2 | Two window shapes | A, N | FIXED: description cites the catalogue's WindowRequest/WindowResult; EventSelection.row accepts opaque refs |
| 3 | Composer cannot submit what it shows | W, A | FIXED: Composer.config = thread + which host controls appear (`controls`); the host composes `turns.start` from `interaction.draft` and `interaction.options`; routes `composer.options` and `selection.set` added; no `Composer.send`, `target`, `enabledWhen`, `{control}`, `{draft}` |
| 4 | Operations shipped workflows need (created ref, create thread, archive, snooze, follow-up) | A | v5 catalogue adds turns.create, archive, unarchive, snooze, follow_up and `ActionOutcome.created`; W maps the fixtures by those names |
| 5 | `host.item.patch-metadata` cannot be invoked | A | v5 draft; W's 9.1 and 9.2 say pin and mark-read depend on it |
| 6 | Verifier: manifests, fullSwipe, plugin kind, not/if/allOf/dependentRequired | N | FIXED in the verifier; W supplies one fixture manifest per exercise (`manifests/<fixture>.json`) |
| 7, 8, 9 | Manifest kind fields, generic item fields, `reads` | A | v5 draft; W's manifests use typed fields and audiences |
| 10 | Wire shapes for undo, status, request card, sheets | A, N | v5 catalogue routes (UndoRequest, StatusRequest, RequestCardResolve, ShareSheetRequest, TrashSheetRequest); showWhen takes minRole (N) |
| 11 | Owner decisions (device cache, remote images, No-network badge) | A, owner | OPEN: not a renderer decision |
| 12 | Stale and contradictory text; fixture bugs | N, W | FIXED: 5.0 settle wording and the nonexistent 8.4 (W), fixture bugs fixed (notes route `item` on both sides, header title from an item bind, footer search with a search bind) |
| 13 | One place per fact | N, W | FIXED for W's parts: `selectionMode` on ListNode only, composer enabled/enabledWhen removed, host duties are written in 9.4 only by reference |
| 14 | Operation titles, manifest target forms, palette commands | A | v5 draft |
| 15 | Audience wording for `viewer_self`, `active_turn` | A | OPEN (plugins lane) |
| 16, 17, 21 | Confirmation experience: challenge only where required, resolve-and-dispatch in one exchange, one exchange on Send, bulk ids in resolve | A, N | v5 draft: ResolveActivation `dispatch:true`, `final_text` on the host composer, Interaction rows up to 50; W's 5.1 says the host owns the send in one exchange |
| 18 | Prefetch replies and held selection flush | N | FIXED in section 8 |
| 19 | Cold open: do not paint after lease expiry vs discard | A, N | OPEN (persistent member rows is an owner question) |
| 20 | Tap deadline | A | OPEN (plugins lane) |
| 22 | Remove from the first contract (Invoke.confirm, preview.material, pending allow, result.show, viewer space/roles, sidebar cards, appearance/scheme/accent/titleImage; one bind list per app) | N | FIXED: one `AppDescription` with ONE bind list, all removed |
| 23 | Parity exceptions | W | Prompt kind `choice` from a bind (at most 50 options; the pick is a host-issued `ChoiceRef` in `interaction.choices`) gives Move to collection (`host.collection.move`); **nested child rows and user-made sections with drag stay OPEN for the owner**; conditional fields wait |
| 24 | Build order, gates, spikes | A | v5 draft |
| paper | Exercises rerun, concept count, what `defineView` hides | W | FIXED: section 9 (about 18 concepts before an empty list; hidden: asset digests, method inventory, tickets, `space_id`, the wire; not hidden: placement, bind and row vocabulary, the four risk fields) |
