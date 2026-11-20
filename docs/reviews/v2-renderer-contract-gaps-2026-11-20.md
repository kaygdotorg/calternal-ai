# What the sealed contract and catalogue do not say enough for a native renderer (2026-11-20)

From the native (SwiftUI) v2 renderer lane, after drawing Threads, Notes, the user-made sections, nested rows, actions with confirmation and Undo, the transcript and the composer against the
step 0 host and a synthetic source. Each item is a proposed numbered-revision note: what the renderer needed, what it did instead, and the smallest definition that would unblock it.
The contract wins on any conflict; nothing here asks the renderer to be smarter than the host.

## A. Needed to draw what the description already declares

1. **Route values for item and collection binds.** A bind with `source.type: "item"` (the header title) or `source.collection: {route: ...}` needs the route value (the open thread, the chosen
   collection) at request time. `WindowRequestPayload` carries only `{bind, from, to, parent?, anchor?}`. The step 0 fixture takes `X-Step0-Route-Item` and `X-Step0-Route-Collection` headers: a
   fixture convention, not the contract. *Proposal:* add an optional `route: {name: ref}` object (at most 8 entries, refs only, host-checked against the Bind's declared route names) to
   `WindowRequestPayload` and to `SelectionRequest`; the host ignores any name the Bind does not declare. Without it the header title of a Threads window cannot be drawn.
2. **What a transcript event carries.** `CommittedTranscriptEvent` is `{event_id, kind, data}` with `data` an open object. *Proposal:* a closed `kind` enum with a typed `data` per kind, at minimum
   `user_message {text, attachments?, quote?}`, `assistant_message {text, final: bool}`, `tool_call {name, summary, status}`, `tool_result {call_id, summary, status}`, `status {state, label}`,
   `request {request}` (a card), each text field with a stated maximum and format (plain text or a closed Markdown subset). The renderer currently draws `kind` plus any `text`.
3. **Transcript paging direction.** `TranscriptPageRequest {thread, after?, limit?}` has no way to read older events. *Proposal:* state that the first read (no `after`) returns the newest page ordered
   oldest to newest, and add `before` (an opaque cursor taken from a new `older_cursor` on `TranscriptPage`) for scrolling up; keep `after` for catching up. State the default `limit`.
4. **Streaming into the transcript.** Rows "grow and change without a description revision": by what channel? *Proposal:* one push stream per open transcript (`GET transcript.stream?thread=`,
   server-sent events or the existing subscription channel) carrying `{event_id, kind, data}` upserts (the same event id replaces the in-flight one) and a `commit` marker with the final
   `committed_cursor`; the host caps the rate (say 10 upserts a second); the renderer drops the stream when the thread route changes.
5. **Composer Send.** The contract says the host owns Send ("`invoke_confirm` carrying `final_text`") but names no `action_id`, no way to obtain the activation context, and the fixture's
   `activation.confirm` rejects `final_text`. *Proposal:* the view-level `ViewOpenResult.activation_contexts` includes `composer.send` and `composer.stop` keys for a composer present in the
   view; Send is `ResolveActivation {action_id: "composer.send", interaction: {draft, options}}` (a resolve only when prepared material changed) then `ConfirmActivation {final_text}`;
   the reply is an `ActionOutcome` whose `created` is the new turn ref. State the retry rule (same `activation_id`).
6. **Composer option pages.** `ComposerOptionRequest {thread, control, after?}` and `ComposerOptionPage` exist, but the `control` enum values, the item shape (`ref`, `label`, `detail?`, `icon?`,
   `disabled?`), what `selected` and `default` hold, and how the chosen refs travel (`Interaction.options` keys) are not worked through in a fixture. *Proposal:* one fixture per control
   (provider, model, reasoning, permission, machine) and a stated dependency order (a model list depends on the chosen provider: the request carries the chosen `options` so far).
7. **Attachments and mentions.** "Reuses the existing host routes for attachment upload and mention lookup": the catalogue has no request or reply shape for either. *Proposal:* add
   `AttachmentUploadRequest/Result` (a multipart or chunked upload, `AttachmentRef`, size and type limits, progress by the HTTP layer) and `MentionLookupRequest/Page` (`{thread, query, after?}`
   to account refs with a display name), or say they are out of the first contract and the composer omits those controls.
8. **Quote.** `Draft.quote` exists; the renderer needs the request that turns a selected passage into a `Quote` ref (the host's Quote control) and what a quote shows in the draft. *Proposal:* a
   `QuoteRequest {thread, event_id, range}` to `Quote {ref, preview}`.
9. **Request card record.** The card is configured with a `request` ref; the renderer needs the record it draws. *Proposal:* a `RequestCardRead {request}` to
   `RequestCardRecord {title, detail?, material: [{name, label, value}], actions: [{id, title, role?}], state}` (the same material shape as a confirmation) and the resolve action ids
   (`approve`, `deny`) with their activation contexts delivered in the record. `RequestCardResolve` exists; its inputs and the record do not.
10. **Item editor.** A note's body in a native text view needs a read and a write protocol: `GET item.body {item}` to `{markdown, revision}`, a revision-checked `PATCH item.body {item, base_revision,
    ops or markdown}`, and for a live room a stated transport. *Proposal:* state the first contract as read plus whole-document revision-checked save (no live room in step 1), with `409
    revision_conflict` carrying the head revision.
11. **Sealed HTML frame content.** `sealed_frame.open/resize/open_link` are specified; how the web view gets its content is not. *Proposal:* `SealedFrameOpen` returns `{html or content_url, csp, max_bytes}`
    and the native renderer loads it with `loadHTMLString` into a non-persistent, network-denied `WKWebView`.

## B. Needed to be correct, not to draw

12. **Image read origin and revalidation.** `query.row_images.read` is `GET /api/v2/row-images/{ref}`: relative to which origin (the host base the renderer already talks to), and does the
    renderer send the lease header (the ref is bound to the live lease)? *Proposal:* "relative to the host origin of the view; send the same bearer and the lease header; honour `404` as
    absent".
13. **`kindIcon`.** "The icon the kind declares" must come from somewhere the renderer can read. *Proposal:* `ViewOpenResult.kind_icons: {kind: icon name}` for the view's kinds.
14. **`viewer` operands.** A row mark `when: {field, op: eq, value: {viewer: person}}` needs the viewer's person id to evaluate on the device. *Proposal:* `ViewOpenResult.viewer: {person: ref}` (an
    opaque account ref the host also puts in `waiting_on`), or say marks never use `viewer` and the verifier rejects it.
15. **`Invoke.disabled` and `showWhen.minRole`.** A predicate `disabled` is evaluated over what? A role gate needs the viewer's role. *Proposal:* a predicate over the row's own fields only
    (and `viewer.role` from `ViewOpenResult.viewer.role`), stated.
16. **Timestamps.** The catalogue's `Timestamp` is an RFC 3339 string (seconds 00 to 59) and the host emits it so, but the sealed reply fixtures (`reply-catalogue-window-ready.json` and others) carry `updated` as an epoch number. Make the fixtures match the type (the renderer reads both today). State that the renderer formats in the device's locale and time zone.
17. **Outcome `created` and `then: {route}`.** After an `ok` outcome with `created`, an invoke's `then: {route: {name}}` should set route `name` to that ref: state it (including for host
    operations such as New thread and New note), and that a missing `created` sets nothing.
18. **Cached rows: the proof.** `MemberRowCacheRecord` carries "the host's proof" that "the shell validates on write and on load". The algorithm and key are not stated. *Proposal:* an HMAC or
    signature scheme with the key delivered in `ViewOpenResult` (per lease partition), or say the proof is opaque and only compared for equality with a value re-issued on `view.open`.
19. **Lease renewal behaviour.** `LeaseRenewResult` returns a grant revision: state what the renderer must drop when it changes (rows, contexts, images, the selection context) and that a lapsed
    lease shows `loading` for authorized parts.
20. **Select all in a long list.** `Interaction.rows` is at most 50. *Proposal:* a `SelectionSetRequest {bind, mode: "all", except?: [ids]}` (the fixture does not serve `selection.set`; step 0
    uses `interaction.rows`) so a bulk action over a 5,000-row list is expressible.
21. **Footer account.** `footer.account {id, title?, menu?}`: the avatar initial, the space switcher entries and sign-out are not defined. *Proposal:* the host supplies `ViewOpenResult.account
    {initial, name}` and the menu is the host's (spaces, settings, sign out), not the description's.
22. **Search flow.** The fixture settles a footer search with `POST /search {type, bind, text}` and then a window request; the sealed `EventSearch` fields after revision 13 (`field_filter`, `match`) are
    not shown in any request fixture against a host. *Proposal:* one event fixture per match shape and the reply that the window request follows.
23. **Counts when not exact.** `SelectorOption.count` and section badges: state what to draw for `estimate` and `none` (nothing; or "about N" for estimate).

## Not a gap, for the record
The window protocol, windows by parent, nested rows, repeat sections, activation (resolve, challenge, confirm, dispatch), outcomes with Undo, bulk activation through `interaction.rows`, the
confirmation material, the secondary line and the miniature fallback were all drawable as written and are drawn against the real step 0 host (revision 13.5).
