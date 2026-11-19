# Revision 14.8 · Step 3 cutover wire audit

[Step 3](../plugin-api-v2.md#step-3--notes-first-then-threads) states the
release gate. The [pinned catalogue](../../contract/host-catalogue-v2.json)
carries renderer shapes. Engine calls without a pinned client shape and
operator checks have separate entries below. The sealed generic-engines
proposal §§A.3, A.5 and C.1 requires a proved historical session to Resume
under its original profile and retires native Resume for an unproved
binding.

| Step 3 requirement | Wire carrier or qualification evidence |
| --- | --- |
| Notes cold collection, search and indexed Threads list | Collection and search Bind source, `WindowRequest` and `WindowResult`. Search settle and the window carry `settle_generation`. |
| Edit a 2 MiB note, patch metadata during Yjs editing, reconnect the offline draft | `editor.open` yields the checked room and revision. The existing room protocol handles sync and reconnect. §3 defines staged save, `item_save` and committed revision. The catalogue does not define the room delta or offline draft record formats. |
| Retain body and history, restore from Trash or history | `item_range` pins the revision, `item_history` reads summaries and `host.item.restore` accepts the checked current head and optional `source_revision`. The engine history listing does not have a renderer component schema in the pin. |
| Rename and Undo, Trash and Empty Trash | `host.item.rename`, `host.item.trash`, `ActionOutcome.undo` and `undo`. `trash_sheet` yields the checked purge action for Manager confirmation. |
| Linked-user View/Comment, revocation and the Share sheet | `host.item.share` and `share_sheet` carry the action and checked audience. Live grants, lease and delivery fences govern the result. The pin names permissions and the grant mutations. It has no portable comment submission or comment record shape. |
| Bulk Trash/share, collection creation and Choice-from-Bind Move | `Interaction.rows` or `selection_ref`, checked context and `ActivationResolution`. Collection creation uses `host.item.create` and `ActionOutcome.created`. Move uses `choice.options` and a checked `ChoiceRef` for `host.collection.move`. |
| Zero worker wakes for built-ins and recovery of a committed save | Engine wake counters, committed revision, outbox and request receipts. Compare revisions and receipts on the release pair. No renderer field asserts that no wake happened. |
| Enforce Notes/Step 2/custody prerequisites, halt old admission and wait for zero active attempts | Operator and engine checks. `ExecutionStatus` is per attempt and cannot prove space-wide quiescence. The pin has no operator cutover request or quiescence report. |
| Migrate exact native binding, retire unproved profile, show notice, keep transcript readable | `composer_actions.resume` and its same-reply context, `ResolveActivation`, `ResumeResolution`, `ResumeConfirmation`, `ResumeOutcome` and `status`. The unavailable outcome shows the required notice. Binding proof, migration and removal of native session data use protected storage. No viewer wire field carries the binding, the native ID, or the migration result. |
| Preserve workspace, grants, pending intents and receipts through rebuild and rollback | Protected workspace custody and receipts are engine evidence. Already-admitted intents remain bound through `ExecutionJobContext.intent` and exact Stop scopes. The client retries using `activation_id` and the existing status ref. Workspace and pending-intent inventories, grant-diff reports and the release fence are operator evidence. |
| Sanitize file, metadata, history, journal and backups. Restore complete public history with stable IDs and equal render | Restore the canonical file and test it through `transcript.page`, `transcript.follow` and `transcript.body`. `record_id`, `role`, `text`, `body`, `attachments`, `committed_cursor` and `older_cursor` carry content and order. Sanitation, quarantine, restore and byte-for-byte historical comparison are operator gates. The committed-record projection has no historical heading or author/time/agent attribution fields for exact visual parity. |
| Reconnect after sign-in with unchanged and grown tails, then revoke | `TranscriptPageRequest.after` on page or follow, `TranscriptPage.notices` and `TranscriptUpdate.notices`, the committed cursor and replacement record. Revoke corpus and require opaque not-found with no cursor, notice or record. Revoke an attachment independently and require its descriptor to disappear and its former ref to fail. |
| Submit a composer draft with an attachment on the person's eligible machine | `Interaction.draft.attachments`, `Interaction.options.machine`, checked activation and `ConfirmActivation.final_text`. The pin has no portable attachment upload or mention lookup schema. The closed transcript projection types attachment read and download only. |
| Queue a follow-up | Prepared Send covers a new confirmed submission and checked steer via `execution_input`. It has no portable queue/edit/cancel request or queued-follow-up record. Existing account-owned pending operations remain outside the pinned renderer wire. |
| Act once on a protected approval | `request-card.read`, `RequestCardRecord`, `request_resolve` and `RequestCardResolve`. Exact gateway dispatch and receipt replay are engine checks. |
| Archive and unarchive, snooze, stop | `host.item.archive`, `host.item.unarchive`, `host.item.snooze` with checked `until`, `composer_actions.stop` and exact `ExecutionStopTarget`. Archive and snooze stay behind their writer gates. The pin carries a timestamp for custom snooze and has no offered-preset catalogue or protected default/time-zone picker model. |
| Exactly one submission per receipt, correct machine/login attribution and no committed transcript loss | `activation_id`, `ActionOutcome.status_ref`, committed records and cursors. `ComposerOptionPage` displays offered machines and `TranscriptPage.notices` displays bounded sign-in state. Neither proves per-record machine/login attribution. Compare protected execution provenance in the qualification run. |
| Provider, model, reasoning, permission and machine choices | `composer.options`, `ComposerOptionPage` and opaque `Interaction.options` fields. Eligibility is checked again at confirmation and admission. |
| Nested children, user sections and Move parity | `WindowRequestPayload.parent`, `WindowResult`, Section.repeat, `layout_sections` and the protected `host.layout.*` catalogue actions. Drag is the recorded parity exception. |
| New writes after rollback and downgrade refusal | Reopen through the same page/follow/body and attachment paths after restoring the release pair with projections removed. Compare IDs, authorizable attachments, receipts and order. The compatibility check and writer switch are deployment gates with no viewer payload. |

Portable user workflows with missing pinned shapes still need a checked
existing route, a numbered contract addition or an explicit Owner parity
exception before replacing the shipped workflow. The existing preview
engine and operator cutover gate stay in place until qualification.
