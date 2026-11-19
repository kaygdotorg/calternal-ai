/* Generated from the sealed plugin API schemas. */

/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Ref".
 */
export type Ref = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ActionId".
 */
export type ActionId = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ActivationId".
 */
export type ActivationId = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ContextRef".
 */
export type ContextRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ProofRef".
 */
export type ProofRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Text".
 */
export type Text = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Boolean".
 */
export type Boolean = boolean
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Number".
 */
export type Number = number
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Timestamp".
 */
export type Timestamp = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "KindId".
 */
export type KindId = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "CollectionRef".
 */
export type CollectionRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ThreadRef".
 */
export type ThreadRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TurnRef".
 */
export type TurnRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RequestRef".
 */
export type RequestRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "AccountRef".
 */
export type AccountRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "AttachmentRef".
 */
export type AttachmentRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "StatusRef".
 */
export type StatusRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "UndoRef".
 */
export type UndoRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TrashSelectionRef".
 */
export type TrashSelectionRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "DestinationFrameRef".
 */
export type DestinationFrameRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RevisionRef".
 */
export type RevisionRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "QuoteRef".
 */
export type QuoteRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ItemRef".
 */
export type ItemRef = string
export type SelectionRef = string
export type ComposerOptionRef = string
export type ChoiceRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SelectionRef".
 */
export type Ref1 = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ComposerOptionRef".
 */
export type Ref2 = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ChoiceRef".
 */
export type Ref3 = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ConfirmMode".
 */
export type ConfirmMode = ("none" | "challenge")
export type WindowParentRef = string
/**
 * start counts from the first row; end counts backward from the last authorized row, with zero at the last row. The end anchor is available only for a non-search Bind whose effective authorized count mode is exact.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowAnchor".
 */
export type WindowAnchor = ("start" | "end")
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowParentRef".
 */
export type Ref4 = string
/**
 * Host-asserted audience of the authenticated active view. member denotes other member views. Compare with the active view before accepting any window.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowAudience".
 */
export type WindowAudience = ("own_space_member" | "member" | "guest" | "public")
/**
 * Installed provider identifier. Known built-in IDs are icon-map seeds, not a closed allowlist.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ProviderId".
 */
export type ProviderId = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TranscriptCursor".
 */
export type Ref5 = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ComposerControl".
 */
export type ComposerControl = ("provider" | "model" | "reasoning" | "permission" | "machine")
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "FieldAudience".
 */
export type FieldAudience = ("member_item" | "shared_projection" | "viewer_self" | "designated_request_actor" | "eligible_turn_actor" | "personal_layout")
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "FieldType".
 */
export type FieldType = ("Text" | "Number" | "Boolean" | "Timestamp" | "ItemRef" | "CollectionRef" | "ThreadRef" | "TurnRef" | "RequestRef" | "AccountRef" | "ProviderId" | "LayoutSectionRef" | "LayoutEntryRef" | "RowImageRef")
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ViewerRole".
 */
export type ViewerRole = ("owner" | "manager" | "editor" | "reader")
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Scalar".
 */
export type Scalar = (string | number | boolean | null)
export type CachePartitionRef = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "CachePartitionRef".
 */
export type Ref6 = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ScheduleCalendar".
 */
export type ScheduleCalendar = ({
type: "once"
at: Timestamp
} | {
type: "recurring"
expression: string
})
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "LayoutEntryRef".
 */
export type Ref7 = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "LayoutSectionRef".
 */
export type Ref8 = string
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowReply".
 */
export type WindowReply = (WindowResult | WindowPlanError)
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ClaimedTextSearchFilter".
 */
export type ClaimedTextSearchFilter = ("all" | "title" | "body")
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TurnsSearchFilter".
 */
export type TurnsSearchFilter = ("all" | "title" | "messages")
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SearchFieldFilter".
 */
export type SearchFieldFilter = (ClaimedTextSearchFilter | TurnsSearchFilter)
/**
 * Terms combines compiled literal tokens with AND within query.search.max_literal_and_terms. Prefix explicitly requests one literal token prefix within query.search.max_token_prefix_expansions. Punctuation never chooses the match shape. Raw Boolean, phrase and fuzzy syntax and mixtures of terms and prefixes are unqualified.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SearchMatchShape".
 */
export type SearchMatchShape = ("terms" | "prefix")
/**
 * Host-issued opaque display reference carried in a projected field. It has no sort or filter operators and cannot supply an operation operand. Read through query.row_images.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RowImageRef".
 */
export type RowImageRef = string
/**
 * Stable identifier: lower-case, 1 to 64 characters. Ids are chosen by the author of the description and never change for the same thing across revisions.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Id".
 */
export type Id = string
/**
 * A JSON literal: string, number, boolean or null. Bounded: strings at most 4096 UTF-8 bytes.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Literal".
 */
export type Literal = (string | number | boolean | null)
/**
 * Where a value comes from. A description never carries viewer data; it names a SOURCE. Row, route, selection and bind sources are reconstructed by the HOST from the activation context; form, draft and prompt sources are the only values the device sends (`interaction`). Exactly one key.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ValueSource".
 */
export type ValueSource = ({
literal: Literal
} | {
viewer: "person"
} | {
route: string
} | {
selection: Id
/**
 * The set of rows the person ticked in a list whose `selectionMode` is `multiple` (at most 50), as a HOST-ISSUED selection reference (`interaction.selection_ref` in the catalogue's Interaction): the shell sends the ticked row ids to the host, the host validates them against the bind's snapshot and the viewer and returns the reference; an action names the reference, never the ids.
 */
set?: true
} | {
row: ("id" | "field")
field?: Id
} | {
bind: Id
field: FieldName
} | {
form: Id
field: Id
} | {
draft: Id
} | {
prompt: {
title: string
placeholder?: string
initial?: ValueSource
/**
 * text: a text field; date or datetime: a picker; choice: one row of a bind (Move to collection, a tag). A choice is NOT a client value: the shell asks the host to issue a `ChoiceRef` for the picked option row and sends it in `interaction.choices` under the prompt's slot; the host resolves the row from the authorised option bind and never accepts a client-supplied id as the choice.
 */
kind?: ("text" | "date" | "datetime" | "choice")
/**
 * For kind `choice`: the options, as the rows of a bind.
 */
options?: {
bind: Id
}
}
} | RowActionTarget)
/**
 * A field of a kind, DECLARED INDEXED by the kind's manifest. Anything not declared is rejected by the host before any query runs.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "FieldName".
 */
export type FieldName = string
/**
 * Where a filter, collection or item-reference operand may come from: a literal, the viewer, or the route. NOT the selection of another bind and not a field of a bind, row, form or prompt: a selected row changes the route (a row's `open`), and the route is the one way one bind's result steers another. `in` takes a homogeneous list of at most 32 literals. `{parent: true}` is the parent row of a per-parent bind (nested child rows, a user-made section's rows).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "FilterValue".
 */
export type FilterValue = ({
literal: (Literal | Literal[])
} | {
viewer: "person"
} | {
route: string
} | {
parent: true
})
/**
 * Up to two indexed fields; the id is the implicit final tie-break, so order is stable across windows.
 * 
 * @maxItems 2
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Sort".
 */
export type Sort = {
field: FieldName
direction?: ("asc" | "desc")
}[]
/**
 * The text of a search: a literal, a form field or a composer draft held on the device (what the person typed). Never a field of a bind.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TextSource".
 */
export type TextSource = ({
literal: Literal
} | {
form: Id
field: Id
} | {
draft: Id
})
/**
 * A catalogue icon name, or an object. One icon set; native draws SF Symbols, web its own strokes.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Icon".
 */
export type Icon = (Id | {
name?: Id
field?: FieldName
/**
 * A static map from the value of `field` to an icon name, owned by the renderer (provider to its icon); an unmapped value draws the kind's icon.
 */
map?: {
[k: string]: Id
}
/**
 * Use the icon the kind declares (a provider's, a note's).
 */
kindIcon?: boolean
})
/**
 * @maxItems 3
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Marks".
 */
export type Marks = {
kind: ("dot" | "count" | "text" | "icon")
field?: FieldName
value?: Literal
when?: Predicate
tone?: ("good" | "warn" | "bad")
icon?: Icon
}[]
/**
 * An action in chrome: an Invoke (an operation the host dispatches) or a ShellAction (a control the shell performs).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Action".
 */
export type Action = (Invoke | ShellAction)
/**
 * A host component: a rich surface the host draws natively; the description only configures it, and its events go to the host (or to the named invoke), not to a page.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Component".
 */
export type Component = (ItemEditor | TurnTranscript | Composer | RequestCard | SealedHtmlFrame)
/**
 * Shell to host. In a page-hosted shell the page relays; in a native app without a page the shell sends the same typed request to the host directly. Each variant has its own required fields. The three activation events are the host catalogue's (ResolveActivation, ConfirmActivation, DispatchActivation: action_id, activation_id, activation_context, interaction; the shell sends no resolved row, route or bind value, and the host reconstructs every server-owned source). Also the catalogue's view_open, lease_renew, choice_options, undo and action_status requests (each cited, none redefined).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Event".
 */
export type Event = (EventWindow | EventSelection | EventSelectionSet | EventRoute | EventSearch | EventSection | EventSidebar | EventLayout | EventApp | EventRefresh | EventComponent | ResolveActivation | ConfirmActivation | DispatchActivation | ViewOpenRequest | LeaseRenewRequest | ChoiceOptionsRequest | UndoRequest | StatusRequest)
/**
 * Host to shell, in the order of the protocol: the reply to a resolve is an ActivationResolution (a resolution_ref and, where the host requires one, a confirmation_challenge with the host's own material preview); the reply to a dispatch is an ActionOutcome (ok, pending with a status_ref, error, stale, refused, unknown, and an undo reference). Both are the catalogue's shapes.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "InvokeOutcome".
 */
export type InvokeOutcome = (ActivationResolution | ActionOutcome)
/**
 * What a view's card may contain: one closed vocabulary, every node drawn by the renderer's own controls on its platform, so no plugin is written twice.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ContentNode".
 */
export type ContentNode = (TextNode | NoticeNode | ButtonNode | FormNode | ListNode | DetailNode | StackNode | ComponentNode)
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "description_v2_WindowReply".
 */
export type DescriptionV2_WindowReply = (WindowResult | WindowPlanError)

export interface ContractModel {

}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Quote".
 */
export interface Quote {
thread: ThreadRef
quote_ref: QuoteRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Draft".
 */
export interface Draft {
text?: string
/**
 * @maxItems 16
 */
attachments?: AttachmentRef[]
/**
 * @maxItems 32
 */
mentions?: AccountRef[]
quote?: Quote
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Interaction".
 */
export interface Interaction {
form?: {
[k: string]: unknown
}
prompt?: {
[k: string]: unknown
}
draft?: Draft
selection_ref?: SelectionRef
options?: ComposerOptions
/**
 * Stable row IDs from the context-bound Bind for a bulk activation.
 * 
 * @maxItems 50
 */
rows?: Ref[]
choices?: {
[k: string]: ChoiceRef
}
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ComposerOptions".
 */
export interface ComposerOptions {
provider?: ComposerOptionRef
model?: ComposerOptionRef
reasoning?: ComposerOptionRef
permission?: ComposerOptionRef
machine?: ComposerOptionRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ResolveActivation".
 */
export interface ResolveActivation {
type: "invoke"
action_id: ActionId
activation_id: ActivationId
activation_context: ContextRef
interaction?: Interaction
/**
 * Resolve and dispatch in one exchange only for an eligible catalogue operation with effective confirm:none. Otherwise return a resolution without running the operation.
 */
dispatch?: true
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ConfirmActivation".
 */
export interface ConfirmActivation {
type: "invoke_confirm"
activation_id: ActivationId
resolution_ref: ContextRef
confirmation_challenge: ProofRef
/**
 * Message content for the prepared host composer only. Any parser-derived change to recipients, routing, permissions or other prepared material requires a fresh resolve and preview.
 */
final_text?: string
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "DispatchActivation".
 */
export interface DispatchActivation {
type: "invoke_dispatch"
activation_id: ActivationId
resolution_ref: ContextRef
confirmation_proof?: ProofRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ConfirmationMaterial".
 */
export interface ConfirmationMaterial {
name: Text
label: Text
value: unknown
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ActivationResolution".
 */
export interface ActivationResolution {
activation_id: ActivationId
resolution_ref: ContextRef
confirmation_challenge?: ProofRef
/**
 * @maxItems 32
 */
material?: ConfirmationMaterial[]
confirm?: ConfirmMode
selection_ref?: SelectionRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "UndoResult".
 */
export interface UndoResult {
ref: UndoRef
label: Text
expires_at: Timestamp
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ActionOutcome".
 */
export interface ActionOutcome {
activation_id: ActivationId
state: ("ok" | "pending" | "error" | "stale" | "refused" | "unknown")
status_ref?: StatusRef
undo?: UndoResult
code?: string
message?: string
created?: ItemRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowRequest".
 */
export interface WindowRequest {
type: "window"
request: WindowRequestPayload
}
/**
 * Request half-open [from,to) from the chosen anchor. The semantic loader requires from < to and a span within query.window.max_request_span for either anchor. Check authorized scope before using its count or index and return rows in normal Bind order. An end anchor requires a non-search Bind and an effective authorized count mode of exact. Refuse before computing or returning any end distance. Absolute jumps remain capped by query.window.max_offset, including explicit start anchors. Above the cap, an explicit start anchor is accepted only for a contiguous forward continuation without exact count authority at a checked, current ready window boundary.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowRequestPayload".
 */
export interface WindowRequestPayload {
to: number
from: number
bind: ActionId
parent?: WindowParentRef
anchor?: WindowAnchor
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowRow".
 */
export interface WindowRow {
id: Ref
fields: {
[k: string]: unknown
}
/**
 * Host-issued parent row reference for each authorized child Bind reached through a declared edge. Omit when the row has no eligible edge.
 */
parent_refs?: {
[k: string]: WindowParentRef
}
}
/**
 * An authenticated reply for the active view and its audience. At from zero, the host sends empty for a wholly empty result. A ready reply at offset zero contains rows. Check the session, lease partition and audience before consuming rows or a cache record. Any cache record must match this view and window. Only a non-search Bind in a view with exact count authority receives an end anchor and end-relative offset. Every end-anchored reply carries count.mode exact. Check count policy for the active Bind and scope on each delivery. An absolute offset above max_offset is only returned to a contiguous forward continuation without exact count authority, with anchor:start and no count or persistent cache record.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowResult".
 */
export interface WindowResult {
result_version: (Ref | null)
/**
 * Actual offset of the delivered range from the reply anchor. Start is measured at its first returned row; end is the distance of the nearest returned row to the end. The rows are always in normal Bind order. If no rows are returned, use the requested from. The host checks this equality against the originating request before issuing the reply.
 */
offset: number
/**
 * Opaque stable row IDs; no position is an authority reference.
 * 
 * @maxItems 200
 */
rows: WindowRow[]
/**
 * Present only for an authorized ready or empty result. On empty, a supplied count is exact zero. On ready with an exact count, the host checks the returned range against that count before delivery, from either anchor. If rows are returned, offset plus their number is at most the count. A zero-row ready reply starts at or beyond the count.
 */
count?: {
mode: ("exact" | "estimate")
value: number
}
bind: ActionId
/**
 * refine_query asks the viewer to narrow a qualified search that exceeded a posting, row work or prefix expansion limit in query.search. Its rows are provisional and display only. On `refine_query`, stop any pending End or forward-paging navigation, keep the rows already shown, and show the refine message. Do not retry the same window. For a nonempty result set, forward paging ends on a short `ready` window, including a zero-row `ready` window at a page boundary. Keep the rows already shown. Only `state: "empty"` shows the Bind’s empty message. It means the whole result set is empty and is valid only when the requested `from` is zero.
 */
state: ("loading" | "ready" | "empty" | "error" | "throttled" | "refine_query")
/**
 * Present only for an authorized ready or empty result.
 * 
 * @maxItems 200
 */
activation_contexts?: RowActivationContexts[]
window_ref: (Ref | null)
parent?: WindowParentRef
member_cache_record?: MemberRowCacheRecord
audience: WindowAudience
anchor?: WindowAnchor
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RowActivationContexts".
 */
export interface RowActivationContexts {
row: Ref
contexts: ActionContexts
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ActionContexts".
 */
export interface ActionContexts {
[k: string]: ContextRef
}
/**
 * The only persistable viewer projection. Issue only on an opted-in, ready or empty start-anchored root window for a live own-space member whose Bind has invariant query inputs. A list may name a literal collection. No reference, search text, route, form, selection or parent operand in its source, filter or sort. Authenticate the account, space, template, Bind, window and offset. The host builds the display record from the authoritative title and host-allowlisted non-body display fields approved for offline use. Exclude body excerpts and any field marked body_derived from every display part, regardless of its template position. Omit the record if any required title is ineligible. Body-derived parts appear only from a fresh authorized window. Never include row IDs, contexts, action material or credentials.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "MemberRowCacheRecord".
 */
export interface MemberRowCacheRecord {
audience: "own_space_member"
account: AccountRef
space: Ref
template_digest: Ref
bind: ActionId
window_ref: Ref
offset: number
issued_at: Timestamp
expires_at: Timestamp
/**
 * @maxItems 200
 */
rows: {
title: Text
subtitle?: Text
}[]
proof: ProofRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TranscriptPageRequest".
 */
export interface TranscriptPageRequest {
thread: ThreadRef
after?: Ref5
limit?: number
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "CommittedTranscriptEvent".
 */
export interface CommittedTranscriptEvent {
event_id: Ref
kind: Text
/**
 * Host-owned typed event from the admitted Turns schema.
 */
data: {
[k: string]: unknown
}
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TranscriptPage".
 */
export interface TranscriptPage {
/**
 * @maxItems 128
 */
events: CommittedTranscriptEvent[]
committed_cursor: Ref5
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SealedFrameOpen".
 */
export interface SealedFrameOpen {
frame: DestinationFrameRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SealedFrameResize".
 */
export interface SealedFrameResize {
frame: DestinationFrameRef
height: number
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SealedFrameOpenLink".
 */
export interface SealedFrameOpenLink {
frame: DestinationFrameRef
link_ref: Ref
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SelectionSetRequest".
 */
export interface SelectionSetRequest {
type: "selection_set"
bind: ActionId
/**
 * Stable rows in the checked Bind window set. An empty array clears the selection.
 * 
 * @maxItems 50
 */
rows: Ref[]
/**
 * Optional renderer telemetry. Never used for authorization, ordering or expiry.
 */
sentAt?: number
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SelectionSetResult".
 */
export interface SelectionSetResult {
type: "selection_result"
bind: ActionId
selection_ref: SelectionRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ComposerOptionRequest".
 */
export interface ComposerOptionRequest {
thread: ThreadRef
control: ComposerControl
after?: Ref
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ComposerOptionPage".
 */
export interface ComposerOptionPage {
control: ComposerControl
/**
 * @maxItems 50
 */
options: {
ref: ComposerOptionRef
label: Text
}[]
next?: Ref
selected: (ComposerOptionRef | null)
default: (ComposerOptionRef | null)
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SubscriptionLease".
 */
export interface SubscriptionLease {
lease_ref: Ref
lease_seconds: 15
grant_revision: Ref
cache_partition: CachePartitionRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ViewOpenResult".
 */
export interface ViewOpenResult {
lease: SubscriptionLease
activation_contexts: ActionContexts
audience: WindowAudience
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "LeaseRenewRequest".
 */
export interface LeaseRenewRequest {
type: "lease_renew"
lease_ref: Ref
grant_revision: Ref
/**
 * Exact result versions keyed by current host-issued window_ref, including parent-specific windows.
 */
result_versions: {
[k: string]: Ref
}
renewal_id: Ref
cache_partition: CachePartitionRef
}
/**
 * Accept only an authenticated host reply matching the outstanding renewal_id, current request_lease_ref and cache_partition. Retain only exact window_ref and result_version pairs authorized and previously issued to that lease. Ignore superseded replies and keep live rows hidden on timeout or refusal.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "LeaseRenewResult".
 */
export interface LeaseRenewResult {
lease: SubscriptionLease
/**
 * @maxItems 16
 */
retained_results: RetainedResult[]
activation_contexts: ActionContexts
/**
 * @maxItems 16
 */
windows: WindowResult[]
renewal_id: Ref
request_lease_ref: Ref
cache_partition: CachePartitionRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RetainedResult".
 */
export interface RetainedResult {
window_ref: Ref
result_version: Ref
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "UndoRequest".
 */
export interface UndoRequest {
type: "undo"
undo_ref: UndoRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "StatusRequest".
 */
export interface StatusRequest {
type: "action_status"
status_ref: StatusRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RequestCardResolve".
 */
export interface RequestCardResolve {
type: "request_resolve"
card_context: ContextRef
request_ref: RequestRef
decision: ("approve" | "deny")
lifetime?: ("once" | "this_thread")
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ShareSheetRequest".
 */
export interface ShareSheetRequest {
type: "share_sheet"
activation_context: ContextRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TrashSheetRequest".
 */
export interface TrashSheetRequest {
type: "trash_sheet"
activation_context: ContextRef
kind: KindId
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ScheduleCreateInput".
 */
export interface ScheduleCreateInput {
operation: Ref
input: {
[k: string]: unknown
}
calendar: ScheduleCalendar
time_zone: Ref
notify_creator?: boolean
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ChoiceOptions".
 */
export interface ChoiceOptions {
bind: ActionId
/**
 * @maxItems 50
 */
options: {
ref: ChoiceRef
label: Text
}[]
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ViewOpenRequest".
 */
export interface ViewOpenRequest {
type: "view_open"
view_ref: Ref
}
/**
 * Fetch scoped options before displaying the prompt. The host reconstructs the option Bind from the activation context and slot. Show the returned labels, then submit only the picked ref under that slot in interaction.choices.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ChoiceOptionsRequest".
 */
export interface ChoiceOptionsRequest {
type: "choice_options"
activation_context: ContextRef
slot: ActionId
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ShareSheetResult".
 */
export interface ShareSheetResult {
sheet_ref: Ref
/**
 * @maxItems 50
 */
audiences: {
ref: ChoiceRef
label: Text
}[]
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TrashSheetResult".
 */
export interface TrashSheetResult {
sheet_ref: Ref
kind: KindId
selection_ref: TrashSelectionRef
count: number
}
/**
 * Host issues selection contexts for exactly the stable row reported as drawn. Renderer waits if none matches that row and ignores unrelated or superseded contexts.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SelectionContextResult".
 */
export interface SelectionContextResult {
type: "selection_context"
lease_ref: Ref
cache_partition: CachePartitionRef
bind: ActionId
row: Ref
activation_contexts: ActionContexts
}
/**
 * Settled single selection in the current authorized view. The host validates the drawn row under the Bind and returns its SelectionContextResult.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SettledSelectionRequest".
 */
export interface SettledSelectionRequest {
type: "selection"
bind: ActionId
row: Ref
/**
 * Optional telemetry. Never used for authorization or ordering.
 */
sentAt?: number
}
/**
 * Author operand resolved by the host from the checked row action context. item is the underlying kind reference, ThreadRef for threads. placement is a LayoutEntryRef and exists only for a row from Section.repeat.rows. Neither value is supplied by the renderer.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RowActionTarget".
 */
export interface RowActionTarget {
context: ("item" | "placement")
}
/**
 * Typed response to the corresponding window request when no qualified bounded plan is available.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "WindowPlanError".
 */
export interface WindowPlanError {
status: 422
code: "unsupported_query_plan"
/**
 * skip_limit means an otherwise qualified index-only jump would skip too many entries. end_unavailable means the requested end anchor targets a search Bind or the effective count mode is estimate or none. unsupported_plan means no qualified plan for the scope, filters, sort, search or count. materialization_limit means a qualified non-search plan would materialize too many rows. Search row-budget exhaustion returns refine_query. After skip_limit the renderer pages from the nearer permitted end (start for search Binds or when the view lacks exact-count authority), keeping the original jump target as its goal. After end_unavailable it pages forward from the last loaded row. For a nonempty result set, forward paging ends on a short `ready` window, including a zero-row `ready` window at a page boundary. Keep the rows already shown. Only `state: "empty"` shows the Bind’s empty message. It means the whole result set is empty and is valid only when the requested `from` is zero. On `refine_query`, stop any pending End or forward-paging navigation, keep the rows already shown, and show the refine message. Do not retry the same window. Every other reason shows the Bind error state.
 */
reason: ("skip_limit" | "unsupported_plan" | "materialization_limit" | "end_unavailable")
}
/**
 * Install and gallery badge and filter predicate for an already validated manifest. Worker egress denial separately follows network:none.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "NoNetworkBadgeManifest".
 */
export interface NoNetworkBadgeManifest {
network: "none"
content_fetch?: never
}
/**
 * The description names a projected field of type RowImageRef. The renderer reads it as specified by query.row_images. Use the optional fallback according to query.row_images.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RowImageIcon".
 */
export interface RowImageIcon {
image: {
field: string
/**
 * Projected body_excerpt Text field. Draw a simple document miniature from the freshly authorized snippet when the image is missing or loading. When no snippet is available, draw the document miniature without text.
 */
fallback?: string
}
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SearchWindowRequest".
 */
export interface SearchWindowRequest {
type: "window"
request: WindowRequestPayload
}
/**
 * One comparison on an indexed field. `exists` takes no value.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Predicate".
 */
export interface Predicate {
field: FieldName
op: ("eq" | "ne" | "in" | "lt" | "lte" | "gt" | "gte" | "exists" | "prefix")
value?: FilterValue
}
/**
 * Predicates combined: all of `all` and at least one of `any` (when present). At most 16 predicates, no nesting. Values come from value sources, so `waiting on me` is {field: waiting_on, op: eq, value: {viewer: person}}.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Filter".
 */
export interface Filter {
/**
 * @maxItems 8
 */
all?: Predicate[]
/**
 * @maxItems 8
 */
any?: Predicate[]
}
/**
 * What a bind reads. The host authorizes kind, collection, every filter field, every sort field, the search and the output fields against the viewer, the space and (for a guest) the item's grant BEFORE any indexed work, and projects rows for the audience. The description only asks.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "BindSource".
 */
export interface BindSource {
/**
 * list: an index window over a kind's items (optionally one collection); item: exactly one item by reference; search: match text in a qualified kind under the catalogue’s query.search policy, over all searchable fields by default.
 */
type: ("list" | "item" | "search")
kind: Id
collection?: FilterValue
ref?: FilterValue
text?: TextSource
/**
 * This bind is windowed PER PARENT ROW: it lists the children of one row of another bind (child threads under their parent) or the rows of one user-made section. **The declared edge (a `RowTemplate.children` or a `Section.repeat`) is the mandatory constraint**: every window request on such a bind carries `request.parent`, a host-issued row ref (`WindowParentRef`, taken from the parent row's `parent_refs`) scoped to that edge and its ancestry, which the host checks on every request. `{parent: true}` in a filter is only an optional extra predicate. A parent bind that no edge references is refused.
 */
parent?: true
field_filter?: SearchFieldFilter
}
/**
 * How an item becomes one row: fields map to title, optional secondary line, icon and marks; actions name the row through the row scope. Rows have no free-form content, so the host can project them for any audience.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RowTemplate".
 */
export interface RowTemplate {
/**
 * The host-issued row reference identifies the row in selection, events and scope. Item rows use their kind’s row reference. Rows of a repeated section use a LayoutEntryRef.
 */
id?: "id"
/**
 * One ellipsised title line from a declared field.
 */
title: {
field: FieldName
fallback?: string
}
/**
 * Optional ellipsised secondary line under the title, in secondary colour. Parts read projected fields in order; date formats require Timestamp and text requires Text. Join present parts with a space, middle dot and space. Omit an absent part and its separator. Format relative dates at draw time against the local clock, without host pushes. Rows without the declaration have one line.
 * 
 * @minItems 1
 * @maxItems 3
 */
secondary?: [{
field: FieldName
format: ("relative_date" | "short_date" | "text")
}, ...({
field: FieldName
format: ("relative_date" | "short_date" | "text")
})[]]
icon?: (Icon | RowImageIcon)
marks?: Marks
selectable?: boolean
/**
 * Swipe-right actions. Only the first may set fullSwipe.
 * 
 * @maxItems 3
 */
leadingActions?: Action[]
/**
 * Swipe-left actions.
 * 
 * @maxItems 3
 */
trailingActions?: Action[]
/**
 * Context menu.
 * 
 * @maxItems 12
 */
menu?: Action[]
/**
 * What activating the row does: set the route parameter (navigate) or invoke an action.
 */
open?: ({
route: {
name: string
value: "id"
}
} | {
invoke: Action
})
/**
 * Nested child rows: a bind with `source.parent: true`, requested per parent row with its own windows. Depth is at most 3 (host-checked, counted through the chain of children); a row with children draws a disclosure control, the system's own. A bind that is a children target is never also a section's rows (`Section.repeat.rows`): the verifier rejects every dual-edge bind.
 */
children?: {
bind: Id
expanded?: boolean
}
}
/**
 * ONE definition used as a content node and as a row, header or footer action. Activation sends the host ONLY an activation context and the values held on the device (`interaction`): the host reconstructs row, selection and route from the context it issued for what was rendered, validates the operation for the acting person, and answers with an outcome (3.2). The description names inputs as value sources; it never carries a resolved row, a destination chosen at activation time, a confirmation or a retry policy.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Invoke".
 */
export interface Invoke {
/**
 * The ACTION ID: chosen by the description's author, stable across revisions, unique within the description. It names the action in events and pending state.
 */
id: string
title?: string
icon?: Icon
/**
 * A plugin operation id as the package's manifest declares it, or a host built-in the host catalogue marks `generic_invoke` (contract/host-catalogue-v2.json: host.item.trash, host.item.rename, host.item.restore, host.item.patch-metadata, ...). Operations that belong to a designated host component, outcome, sheet or shell control are NOT nameable here: the catalogue says which, and the verifier rejects them (approval, undo, status, panel.ask, route, session, trash purge, share, turns.start, turn.stop). Effect class, idempotency, confirmation (`none` or `challenge`) and retry come from the catalogue and the manifest, never from the description.
 */
operation: string
/**
 * Named inputs and where each comes from (a ValueSource). The host, not the renderer, resolves row, route, selection and bind sources from the activation context; only form, draft and prompt sources are values the device holds.
 */
inputs?: {
[k: string]: ValueSource
}
role?: "destructive"
/**
 * Tints a swipe action or button; absent is neutral.
 */
tone?: ("good" | "warn" | "bad")
/**
 * On the FIRST action of a swipe set only, and only for a reversible host built-in that returns an Undo and needs no preview (the catalogue says which); a full swipe never opens a confirmation and never runs an irreversible, disclosing or widening operation.
 */
fullSwipe?: boolean
/**
 * Greyed out when the predicate holds on the row or bind scope, or always.
 */
disabled?: (boolean | Predicate)
/**
 * After an `ok` outcome that carries `created`, set this route parameter to the created item's reference (open the new note or thread). The reference is host-minted; a description never supplies one.
 */
then?: {
route: {
name: string
}
}
}
/**
 * A control the SHELL performs without a generic operation: navigate (host.route.set), reload, sign out (host.session.end), or open a host-owned protected sheet (Trash set / purge, share audience). No generic Invoke can reach them.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ShellAction".
 */
export interface ShellAction {
id: Id
title: string
icon?: Icon
/**
 * A control the SHELL performs without an operation, so no generic Invoke can reach it: navigate, reload the view, sign out.
 */
shell: ({
route: {
name: string
value: ValueSource
}
} | {
reload: true
} | {
signOut: true
} | {
/**
 * Open a host-owned protected sheet: the Trash set (confirm and purge, Manager or Owner), or the audience sheet of a share. The sheet, not the description, names the operation and shows the material facts.
 */
sheet: ("trash" | "share")
item?: ValueSource
})
role?: "destructive"
}
/**
 * A message state for a bind: empty, loading or failed. The title and detail are literals of the description or host-written text; never worker output.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "State".
 */
export interface State {
title: string
detail?: string
icon?: Icon
action?: Action
}
/**
 * ONE definition used by chrome and content: a data-free request for rows, counts or one item. Several binds may be in one view; every reference names its bind id. A description with binds contains no viewer data, so it may be cached and shared.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Bind".
 */
export interface Bind {
id: Id
source: BindSource
/**
 * Indexed fields the row template or counts read: the output projection. Fields the template does not use are not requested.
 * 
 * @maxItems 16
 */
fields?: FieldName[]
filter?: Filter
sort?: Sort
/**
 * Index windows (not cursors): a request names {from, to} indices into the sorted, filtered list; the host answers with the window state, offset and rows, plus a count when authorized and supported by the plan. The renderer asks for windows as the person scrolls or arrows, with prefetch ahead of the visible rows.
 */
window?: {
/**
 * Rows the host returns per window request.
 */
size?: number
/**
 * Whether to request a total with the rows. Search defaults to none under the catalogue’s query.search.count policy. Non-search defaults to exact, subject to count authority and the relevant count-work budget.
 */
count?: ("exact" | "estimate" | "none")
}
row?: RowTemplate
states?: {
empty?: State
loading?: State
error?: State
refine_query?: State
}
/**
 * Whether the host pushes changed windows (revision bump) while the view is open.
 */
live?: boolean
/**
 * Opt-in: this ROOT bind's own-space rows may be persisted for a member (see Cache.memberRows), and ONLY if every query input is INVARIANT: a `list` source with no reference, search text or parent and with at most a LITERAL collection restriction, and filter operands that are literals or `{viewer: person}` (an account-bound viewer operand stays invariant); no route, form, selection or parent operand anywhere in the source, filter or sort. The record is start-anchored (an end-relative window is never persisted). The schema and the verifier refuse `persist` otherwise, and no query identity is added to the record. A guest's or public view's rows are never persisted.
 */
persist?: boolean
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SelectorOption".
 */
export interface SelectorOption {
id: Id
title: string
/**
 * A count taken from a bind (with count: exact); the bind need not render rows.
 */
count?: {
bind: Id
}
}
/**
 * A segmented control. Counts come from binds; the options change the route, and binds filter on the route value.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Selector".
 */
export interface Selector {
id: Id
/**
 * @minItems 2
 * @maxItems 6
 */
options: [SelectorOption, SelectorOption, ...(SelectorOption)[]]
selected?: Id
/**
 * Route parameter the selection sets; each option's value is its id.
 */
route?: string
}
/**
 * A group of rows in the sidebar. Rows are literal shortcuts, or a bind's rows, or both (literal first).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Section".
 */
export interface Section {
id: Id
/**
 * Absent: the section has no header, and therefore no header actions.
 */
title?: string
/**
 * Present only if the section folds (a chevron is drawn); the value is its initial state.
 */
collapsed?: boolean
/**
 * @maxItems 24
 */
rows?: {
id: Id
title: string
icon?: Icon
marks?: Marks
/**
 * As a row template's `open`.
 */
open?: {
[k: string]: unknown
}
}[]
bind?: Id
/**
 * Shown only while a route parameter has a value, or while the viewer's role in the space is at least `minRole` (so Empty Trash is not offered to an Editor).
 */
showWhen?: ({
route: string
equals: string
} | {
minRole: ("editor" | "manager" | "owner")
})
/**
 * @maxItems 3
 */
actions?: Action[]
/**
 * @maxItems 12
 */
menu?: Action[]
/**
 * User-made sections: one section per row of the bind `over` (the per-person layout the host stores: its title is the row's title, its order the person's order), each section's rows from the per-parent bind `rows` (`source.parent: true`; the declared edge is the mandatory constraint and a `{parent: true}` filter is only an optional extra predicate). The host stores and authorizes the layout; moving a row or a section is a host operation (Move to, Move up, Move down in the catalogue), never a description. A per-parent bind is consumed ONLY through its edge: no ordinary section, list or count may reference it, and a bind is a section's rows OR a children target, never both (section rows present LayoutEntryRef, child rows the kind's own row ref).
 */
repeat?: {
over: Id
rows: Id
}
}
/**
 * The item's editor, drawn by the host over the item's document (a note's Markdown file directly: there is no second representation between editor and file). Live rooms are an attribute of the item's KIND, not of this component; a metadata-only save does not rewrite the body.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ItemEditor".
 */
export interface ItemEditor {
type: "item-editor"
id: Id
config: {
item: ValueSource
mode?: ("edit" | "read")
/**
 * The item's title is the first line of the editor and renames through host.item.rename.
 */
showTitle?: boolean
onTitleChange?: Invoke
}
}
/**
 * The conversation of one thread: a host-owned windowed list with its own streaming (the host appends and updates rows; the description never receives message text), scroll anchoring and loading in steps. Heavy parts (code, pictures, steps, cards) are the host's. The host stamps `calternal:thread-rendered` when the first screenful is drawn, and drops a load when the viewer moves to another thread.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TurnTranscript".
 */
export interface TurnTranscript {
type: "turn-transcript"
id: Id
config: {
thread: ValueSource
/**
 * Open at the end and stay there while the person reads the end; never pull a person who scrolled away.
 */
follow?: ("end" | "none")
/**
 * Messages per read; the host reads the screenful first and the rest in steps.
 */
window?: number
/**
 * Offer Quote on a selected passage. The host handles it (a typed quote reference into the side chat's composer: `Draft.quote`); a description cannot name an operation for it.
 */
quote?: boolean
}
}
/**
 * The message box. The draft text lives on the device (it survives a failed send and a route change within the session and is never published); it becomes operation input only through the {draft: id} value source at activation time, and is shown to the person as the material input of a send.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Composer".
 */
export interface Composer {
type: "composer"
id: Id
/**
 * Composer configuration: the thread and which host controls appear. Send, Stop, the option lists, the draft and its checking are the host's.
 */
config: {
thread: ValueSource
/**
 * Which host-owned controls appear with the composer. Their options come from the host and are authorised for the viewer (`composer.options`); a description never lists options. Absent: the text box and Send only.
 * 
 * @maxItems 8
 */
controls?: ("attachments" | "mentions" | "provider" | "model" | "reasoning" | "permission" | "machine" | "stop")[]
placeholder?: string
}
}
/**
 * A request or approval card. There is ONE protected request record and resolver for a job, a Turn and a host action, so the card names no mechanism. It shows the material arguments the host resolved (what is asked, for what, with which scope) and offers the host's own resolve actions (host.approval.resolve): the description cannot relabel, reorder or hide them, and a plugin layout can never cover the card.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "RequestCard".
 */
export interface RequestCard {
type: "request-card"
id: Id
config: {
request: ValueSource
compact?: boolean
}
}
/**
 * Sealed HTML from a plugin: shown in a frame the host sandboxes (no network, scripts only inside the frame, no top navigation, no access to chrome, a message channel the host fixes). The frame reports its size inside the caps; its links open through the host.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "SealedHtmlFrame".
 */
export interface SealedHtmlFrame {
type: "sealed-html-frame"
id: Id
config: {
frame: ValueSource
minHeight?: number
maxHeight?: number
}
}
/**
 * The header over the content card: a literal title or one from the selected item, and actions. The sidebar toggle is the shell's.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Header".
 */
export interface Header {
title?: (string | {
bind: Id
field: FieldName
})
/**
 * @maxItems 2
 */
leading?: Action[]
/**
 * @maxItems 3
 */
trailing?: Action[]
}
/**
 * Account at the left, the apps in the middle, search at the right.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Footer".
 */
export interface Footer {
/**
 * The account control; the shell draws the person's initial and name from the session, so the persisted template holds no person data.
 */
account?: {
id: Id
title?: string
/**
 * @maxItems 12
 */
menu?: Action[]
}
/**
 * @maxItems 6
 */
apps?: AppEntry[]
/**
 * The search control; `bind` is a search bind that receives the settled text.
 */
search?: {
placeholder?: string
bind?: Id
}
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "AppEntry".
 */
export interface AppEntry {
id: Id
title: string
icon?: Icon
badge?: {
bind: Id
}
}
/**
 * The sidebar column. A single scroll of cards, selectors and sections over the footer.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Sidebar".
 */
export interface Sidebar {
open?: boolean
width?: number
title?: string
/**
 * @maxItems 2
 */
selectors?: Selector[]
/**
 * @maxItems 12
 */
sections?: Section[]
refresh?: boolean
}
/**
 * The right panel. Content is host components only.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Panel".
 */
export interface Panel {
id: Id
title?: string
open?: boolean
/**
 * Stacked top to bottom; the side chat is a turn-transcript above a composer.
 * 
 * @maxItems 4
 */
components?: Component[]
}
/**
 * What the device may keep: a data-free template, and (Q10) for a MEMBER only, the host-authenticated `MemberRowCacheRecord` of an opted-in ROOT window of their own space (no nested or per-parent windows, nothing route-dependent) painted at once and then confirmed or corrected by the host. Everything else the host filled (a guest's or a public view's rows, counts, contexts, resolution references, challenges, proofs, credentials) is session-only: memory, partitioned by session, space and audience, cleared on revocation, link closure, sign-out and account switch, never written to storage (docs/description-contract.md 6).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "Cache".
 */
export interface Cache {
/**
 * keep: the shell may persist this DATA-FREE description for the next launch (bounded by maxAgeDays); none: it keeps nothing.
 */
templates?: ("keep" | "none")
/**
 * Retention in days of what the device keeps (templates; and member rows when kept): 1 to 90, default 30; persisted member rows are further capped at 14 days.
 */
maxAgeDays?: number
/**
 * keep: the device may persist the host-authenticated `MemberRowCacheRecord` of a member's own-space rows for binds with `persist: true` (encrypted, keys bound to the account and the space), painted at once and then confirmed or corrected by the host. Records exist only for an own-space member; guest and public views have no record to persist, whatever this says. none: persist no rows.
 */
memberRows?: ("keep" | "none")
}
export interface EventWindow {
type: "window"
request: WindowRequestPayload
}
/**
 * Settled single selection in the current authorized view. The host validates the drawn row under the Bind and returns its SelectionContextResult.
 */
export interface EventSelection {
type: "selection"
bind: ActionId
row: Ref
/**
 * Optional telemetry. Never used for authorization or ordering.
 */
sentAt?: number
}
export interface EventSelectionSet {
type: "selection_set"
bind: ActionId
/**
 * Stable rows in the checked Bind window set. An empty array clears the selection.
 * 
 * @maxItems 50
 */
rows: Ref[]
/**
 * Optional renderer telemetry. Never used for authorization, ordering or expiry.
 */
sentAt?: number
}
/**
 * A route parameter changed (a row opened, a selector option chosen).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventRoute".
 */
export interface EventRoute {
type: "route"
id: Id
row?: Id
sentAt?: number
}
/**
 * Send the settled text for the named search Bind, or for the active footer search Bind when bind is omitted. The authenticated Bind supplies the optional field filter. The event supplies text and may request a match shape independently.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventSearch".
 */
export interface EventSearch {
type: "search"
bind?: Id
match?: SearchMatchShape
text: string
}
/**
 * A section was folded or unfolded.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventSection".
 */
export interface EventSection {
type: "section"
id: Id
collapsed: boolean
sentAt?: number
}
/**
 * The sidebar was opened or closed.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventSidebar".
 */
export interface EventSidebar {
type: "sidebar"
open: boolean
sentAt?: number
}
/**
 * A layout preference changed (the sidebar's width).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventLayout".
 */
export interface EventLayout {
type: "layout"
id: Id
width?: number
sentAt?: number
}
/**
 * Another app was chosen in the footer.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventApp".
 */
export interface EventApp {
type: "app"
id: Id
sentAt?: number
}
/**
 * Pull to refresh: ask the host again for a bind or for all.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventRefresh".
 */
export interface EventRefresh {
type: "refresh"
bind?: Id
sentAt?: number
}
/**
 * A host component's own call to the host (a transcript page read, a sealed frame's open, resize or link). The `request` is validated against the catalogue's schema for the route.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventComponent".
 */
export interface EventComponent {
type: "component"
component: Id
/**
 * The host route a COMPONENT's call uses. This enum is a SUBSET of the catalogue's `component_routes` (the ones a component or prompt reaches through `EventComponent`); routes the shell reaches directly have their own event: `selection.settled` is the shell's direct `EventSelection` ({type: selection}), `window` is `EventWindow`, `selection_set` is `EventSelectionSet`. Request and reply shapes are the catalogue's (TranscriptPageRequest, ComposerOptionRequest, ChoiceOptionsRequest, ViewOpenRequest, ShareSheetRequest, ...), cited, never defined here.
 */
route: ("transcript.page" | "sealed_frame.open" | "sealed_frame.resize" | "sealed_frame.open_link" | "selection.set" | "composer.options" | "undo" | "status" | "request_resolve" | "share_sheet" | "trash_sheet" | "view.open" | "view.renew" | "choice.options")
request: {
[k: string]: unknown
}
sentAt?: number
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventWindow".
 */
export interface EventWindow1 {
type: "window"
request: WindowRequestPayload
}
/**
 * Settled single selection in the current authorized view. The host validates the drawn row under the Bind and returns its SelectionContextResult.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventSelection".
 */
export interface EventSelection1 {
type: "selection"
bind: ActionId
row: Ref
/**
 * Optional telemetry. Never used for authorization or ordering.
 */
sentAt?: number
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventSelectionSet".
 */
export interface EventSelectionSet1 {
type: "selection_set"
bind: ActionId
/**
 * Stable rows in the checked Bind window set. An empty array clears the selection.
 * 
 * @maxItems 50
 */
rows: Ref[]
/**
 * Optional renderer telemetry. Never used for authorization, ordering or expiry.
 */
sentAt?: number
}
/**
 * One field of a form: a typed control whose value is held on the device and checked there (required, length, url/email shape) before anything is sent.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "FormField".
 */
export interface FormField {
id: Id
label: string
input: ("text" | "multiline" | "url" | "email" | "number" | "date" | "choice" | "toggle")
required?: boolean
maxLength?: number
placeholder?: string
initial?: Literal
/**
 * For `choice`.
 * 
 * @maxItems 32
 */
options?: {
value: string
label: string
}[]
}
/**
 * Fixed copy written by the description's author. Never viewer data: a person's titles come through a bind.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "TextNode".
 */
export interface TextNode {
type: "text"
text: string
style?: ("title" | "heading" | "body" | "caption")
tone?: ("good" | "warn" | "bad")
}
/**
 * A message panel with an optional action (an error or a notice above a form, never loose text under a button).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "NoticeNode".
 */
export interface NoticeNode {
type: "notice"
tone?: ("good" | "warn" | "bad")
title: string
detail?: string
action?: Invoke
}
/**
 * An action as a button. The label is the invoke's title; the trusted-preview rule applies (the host resolves it against what was rendered).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ButtonNode".
 */
export interface ButtonNode {
type: "button"
invoke: Invoke
style?: ("primary" | "quiet")
}
/**
 * Named fields held on the device plus one submit action: the values reach the host only inside the invoke's resolved inputs, where the person sees the material ones.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "FormNode".
 */
export interface FormNode {
type: "form"
id: Id
title?: string
/**
 * @minItems 1
 * @maxItems 12
 */
fields: [FormField, ...(FormField)[]]
submit: Invoke
/**
 * Clear the fields after an `ok` outcome; an error or unknown outcome keeps what was typed.
 */
clear?: ("onOk" | "never")
}
/**
 * A bound list. Windows, counts, live updates, empty/loading/error states and row actions come from the bind (sections 2 and 3).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ListNode".
 */
export interface ListNode {
type: "list"
bind: Id
/**
 * Up and Down move selection through the rows. Home, End, Page Up and Page Down scroll without changing selection. The selected row's id is the selection source for this Bind. Its route loads when selection settles.
 */
selectable?: boolean
title?: string
/**
 * multiple: rows can be ticked (at most 50); actions on the set use {selection, set: true}, and the host-written preview shows the count and first titles.
 */
selectionMode?: ("single" | "multiple")
}
/**
 * Labelled fields of one bound item (a reminder's due date, a reading item's source).
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "DetailNode".
 */
export interface DetailNode {
type: "detail"
bind: Id
/**
 * @maxItems 16
 */
fields: {
label: string
field: FieldName
}[]
}
/**
 * Layout: children in a column or a row. Depth is at most 6 and a view has at most 200 nodes (checked by the host and by contract/verify-description.py). There is no free layout: no sizes, colours, fonts or margins.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "StackNode".
 */
export interface StackNode {
type: "stack"
axis?: ("vertical" | "horizontal")
title?: string
/**
 * @minItems 1
 * @maxItems 24
 */
children: [ContentNode, ...(ContentNode)[]]
}
/**
 * A host component placed in the card.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ComponentNode".
 */
export interface ComponentNode {
type: "component"
component: Component
}
/**
 * The shell around the content card: sidebar, header, footer, right panel. Data arrives only through the app's binds.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ChromeBody".
 */
export interface ChromeBody {
sidebar?: Sidebar
header?: Header
footer?: Footer
panel?: Panel
}
/**
 * The content of the card for one place.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "ViewBody".
 */
export interface ViewBody {
title?: string
root: ContentNode
}
/**
 * One app's description: its binds, the chrome and the view, data-free. Rows reach the renderer only as the host's answers to window requests.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "AppDescription".
 */
export interface AppDescription {
version: 2
kind: "app"
revision: number
/**
 * Feature names the description needs (for example `bind-search`); a shell that does not know one refuses the description instead of drawing it wrongly.
 * 
 * @maxItems 16
 */
requires?: string[]
cache?: Cache
/**
 * The app's binds, declared ONCE: chrome and view refer to them by id (at most 16 per app, chrome and view together).
 * 
 * @maxItems 16
 */
binds?: Bind[]
chrome?: ChromeBody
view?: ViewBody
}
/**
 * The only persistable viewer projection. Issue only on an opted-in, ready or empty start-anchored root window for a live own-space member whose Bind has invariant query inputs. A list may name a literal collection. No reference, search text, route, form, selection or parent operand in its source, filter or sort. Authenticate the account, space, template, Bind, window and offset. The host builds the display record from the authoritative title and host-allowlisted non-body display fields approved for offline use. Exclude body excerpts and any field marked body_derived from every display part, regardless of its template position. Omit the record if any required title is ineligible. Body-derived parts appear only from a fresh authorized window. Never include row IDs, contexts, action material or credentials.
 * 
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "description_v2_MemberRowCacheRecord".
 */
export interface MemberRowCacheRecord1 {
audience: "own_space_member"
account: AccountRef
space: Ref
template_digest: Ref
bind: ActionId
window_ref: Ref
offset: number
issued_at: Timestamp
expires_at: Timestamp
/**
 * @maxItems 200
 */
rows: {
title: Text
subtitle?: Text
}[]
proof: ProofRef
}
/**
 * This interface was referenced by `ContractModel`'s JSON-Schema
 * via the `definition` "EventSearchWindow".
 */
export interface EventSearchWindow {
type: "window"
request: WindowRequestPayload
}
