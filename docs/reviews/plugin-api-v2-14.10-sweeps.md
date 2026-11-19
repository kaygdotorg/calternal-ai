# Revision 14.10 · Intent, confirmation and regex sweeps

The [catalogue](../../contract/host-catalogue-v2.json) owns the execution
intent rules and confirmation material table. The fixtures verify the
contract traces and shapes. Step 2 still has to exercise the writer,
crash orderings and receipt replay on the real engines.

## Rules generalized from Send to execution intent

| Site | Rule now shared by prepared Send and protected Resume |
| --- | --- |
| §3, catalogue `execution_intents.admission`, `execution_start` | Check live head and cutover fences, use one quota and one unresolved intent or active attempt per conversation, and admit at most one attempt per intent. Use the owning job and durable receipt on replay. |
| §3 Resume and manifest composer job | A fresh protected continuation uses the same admission and owning job. Its origin determines whether a new message is appended. The job declaration names the shared intent receipt. |
| §§3, 4.1 and 4.2, description composer, catalogue `host.execution.stop`, `execution_stop` and `view.renew` | Stop uses an exact unresolved execution intent or admitted attempt. Either source can expose Starting. Neither stale context nor replacement intent can be retargeted. |
| §§3 and 4.4, catalogue `host.item.archive` | An unresolved intent from either origin disqualifies idle archive. Protected archive refuses that exact pending intent in the FULL head and Undo transaction. Undo preserves the durable refusal. |
| §§3 and 4.4, catalogue `host.item.snooze` | Snooze and Undo touch the actor's deadline and its index only. They leave all intents, capacity and retries alone. |
| §8 Step 2 cold job/Stop and archive writer | Run intent-first, archive-first and committed archive followed by restart/Undo before job admission for each source. The cross-source race for the final slot or quota must admit one. Stop tests both origins before job claim. |
| §8 Step 2 snooze writer | In both pending-intent arms recheck intent, quota and retry state across snooze and Undo. |
| §8 Step 3 and catalogue cutover | Close old admission and prove zero active attempts and zero unresolved execution intents before switching. Retain receipts and durable refusals through rollback. |

Send still owns its message, prepared final text, parsed recipients and
steering behavior. Resume owns the historical binding and confirmation
with no new message.

## Required confirmation material

The single machine-readable table is
[`confirmation.material_by_kind`](../../contract/host-catalogue-v2.json#/confirmation/material_by_kind).
The verifier checks each row and its conditional entries by removing each
required entry in turn. The following is an index of the swept routes.

| Group | Rows and checked material |
| --- | --- |
| Composer and execution | Send uses conversation, provider, model, reasoning, permission and machine, plus present attachments, mentions and quote. Resume uses conversation, provider, machine, original profile and permission. Stop uses conversation and effect. |
| Item lifecycle | Rename uses item/title, Trash uses checked items/effect, restore uses item/effect plus a history source when applicable, archive uses item/effect plus an applicable execution effect and retired-grant audience, unarchive uses item/effect, snooze uses item/deadline, and metadata patch uses item/changes. |
| Collection move, audience and permanent purge | Move uses checked items, destination and audience. Share uses checked items, audience and permission. Purge uses checked items and effect. |
| Requests, panel and account | Approval challenge uses effect and decision, plus lifetime for approval. Panel ask uses destination/text plus present item/draft/quote. Sign-out uses account and effect. |
| Layout | Move to uses placement/section, Move up and Down use placement/effect, section creation uses layout/title, rename uses section/title, removal uses section/effect. |
| Worker action | Operation and effect, checked target/material and all applicable named manifest material inputs, including untouched prefills. |
| No challenge | Direct empty-item create, route change, status and protected Undo do not mint an activation challenge. |

## Absolute-end checks

All 29 pinned v2 value patterns below use the JSON Schema-compatible
`(?![\s\S])` absolute-end assertion, including the already-safe timestamp.
The verifier's internal first-swipe path suffix uses Python `\Z`.
The JSON Pointer array-index parser already uses `fullmatch`. Other path
regexes match structural prefixes or look for forbidden characters. Those
searches do not validate a whole value. Embedded author patterns retain
ordinary Draft 7 semantics. The older generated v0 and operation artifacts
are outside the v2 verifier's schema registry and revision scope.

| Source | Pattern sites |
| --- | --- |
| Description identifiers (4) | `Id`, `FieldName`, `Invoke.id`, `Invoke.operation` |
| Description route names and operands (8) | `ValueSource.route`, `RowTemplate.open.route.name`, `Invoke.then.route.name`, `Selector.route`, `ShortcutOpen.route.name`, `Section.showWhen.route`, `FilterValue.route`, `ShellAction.shell.route.name` |
| Manifest (7) | `localId`, `operationId`, asset `path` including its internal dot-segment boundary, asset `sha256`, kind `extension`, provider image digest, `hostname` |
| Catalogue (10) | `Timestamp`, `KindId`, `RowImagePrepare.source_state_sha256`, `RowImagePrepare.sha256`, `ViewOpenResult.cache_proof_key.public_key`, `RowImageRef`, `RowImageIcon.image.field`, `RowImageIcon.image.fallback`, `HostWorkerInvokeFrame.digest`, `TranscriptAttachment.media_type` |

Each pinned pattern exercises a valid value and rejection of a trailing
newline. Transcript page and follow also have negative cases against
their actual nested attachment field.
