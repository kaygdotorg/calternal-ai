SEAL WITH LISTED FOLLOW-UPS

**1. My major gap (host.item.create cannot place the new item in a collection): CLOSED.**
- Operand: in `contract/host-catalogue-v2.json` the operation now takes an optional `collection` (a `CollectionRef`) alongside `kind`. It accepts only a route operand or a template literal, and an absent route parameter skips placement.
- One transaction: the item, its membership and Undo commit in one FULL content transaction after a live check of authority to edit that collection's membership under the commit fence (catalogue `authority` and `atomic_undo_when`; API §4.4 table row).
- Quotas: the item and membership quotas are checked under the same fence. A refusal returns `create_limit_reached` and commits nothing.
- One refusal: absent, foreign, unreadable and non-editable refs all get the same opaque `collection_unavailable`. It is checked before any quota result could reveal the collection, and it outranks the generic stale outcome if the collection goes away after resolve (description contract §3).
- Undo: it rechecks the item head, the item's membership and sharing revisions, and the membership the create introduced, then removes the item and that membership together.
- No challenge and no wake: `confirmation_when` says no challenge even in a shared collection (API §4.3 item 5), and `worker_wake` is `"never"`. The optional `after` follow-up gets only the created ref and dispatches on its own, so it cannot delay the create or its Undo.
- Notes uses it: the manifest `manifests/notes-app.json` declares `host_create: {defaults: {title: "Untitled"}}` with no operations. The New note invoke in `notes-app.json` is `host.item.create` with `kind: {literal: "notes"}` and `collection: {route: "collection"}`, and §9.2 is rewritten to match.
- Step 0: the case "Host create with collection placement" covers crashes before and after commit, receipt replay, Undo, identical refusals, revoking after resolve, both quotas, a race for the last slot, Undo refusing after a membership or share change, and zero runtime wakes.

**2. The eight follow-ups.**
1. Row-image write path: RECORDED-OPEN. The upload path is still in the OPEN "Derived data" entry. Two parts are DONE in `query.row_images`: the host decides body derivation from where the image came from (a manifest flag cannot), and images are retained with their source revision and purged on revision erasure or item purge.
2. Stale thumbnails: DONE in part. An image from an earlier revision is never shown on a current row, so an edit falls back to the miniature. How often a device uploads stays open with the upload path.
3. Image fetches during key repeat: DONE. New image reads are deferred and reads in flight cancelled. The keyboard-windowing and cold-page gates now count image requests and bytes.
4. Provider icon map: DONE. The host projects `provider_icon` under the `provider_marks` rule with a generic fallback symbol, and the same rule applies to our own providers.
5. Two-collection move: RECORDED-OPEN as a Step 2 gate ("Two-collection move writer").
6. Persisted secondary line: DONE. Cached rows keep only a typed `subtitle_timestamp`, exclude every body-derived part and reserve the line height until the snippet arrives.
7. Consent for a machine a person adds: DONE (§2).
8. Revision number: DONE. Both headers say 13.18.

**3. New major gap: none.**
- Owner decisions: no contradiction found.
  - Host create is generic for any kind that opts in, and the plugin's own logic can run afterwards through `after`.
  - Move to collection challenges only when the destination is shared with someone other than the actor.
  - Search covers all fields by default, with field filters optional.
  - The three VM kinds, the no-KVM labelling, no private items in a shared space, a plugin's scratch database (OPEN), the dropped table store, Craft-style rows and the consent clause all follow the decisions.
- Provider brand marks: not a first-party privilege. Every provider, ours included, needs a verified publisher through a signed gallery entry or an operator pin; without one the row gets the generic symbol.
  - Gap: nothing says the bundled Claude Code and Codex contributions ship their signed gallery entry. Without it, a fresh or offline self-hosted install shows generic thread icons, which a person would notice. Stating that closes it.
- The rules you asked me to check: none makes a legitimate description impossible to write.
  - Shortcut literals restricted to `Id`: `Id` is a lower-case slug of up to 64 characters, so values like `automations` and lower-case UUIDs pass.
  - `fullSwipe`: forbidding it at non-swipe sites, and allowing it only on the first action of a swipe set, matches how the system swipe behaves.
  - `{parent: true}` requiring `source.parent`: legitimate.
  - Persisted rows: excluding body-derived parts and carrying a typed timestamp is consistent with the decision to keep member rows on the device.
  - `Timestamp` without leap seconds: the host clamps a leap second to second 59, which loses nothing that matters.
  - Verifier made normative: acceptable, with a cost noted in item 4.
- Follow-up (owner check): §8 and the Lists bullet now make Home and End scroll without moving the selection too. The owner decided this only for Page Up and Page Down. It matches AppKit, but it changes the web's earlier behaviour and should be confirmed.
- Follow-up (minor): the host-produced `body_excerpt` snippet sits beside the rule that derived data is the plugin's own. It is a generic list projection any searchable text kind can declare, so it is not a privilege, but the text could say why it counts as a projection and not derived data.

**4. Readiness.**
Yes. This is ready to seal. The motivating case (New note in a sleeping, shared collection) now takes one exchange with no wake and no sheet, on a generic built-in a community kind can opt into. Every owner decision from the night is reflected or explicitly recorded as OPEN, and the remaining gaps are named Step 0 or Step 2 gates rather than silent omissions.

One cost to plan for: because the Python verifier is normative, the Rust host and the TypeScript SDK each need a port or a shared conformance run against `contract/fixtures/description-v2/`. Otherwise the three will drift on the verifier-only rules.

The first thing I would expect to change when Notes is rewritten is the thumbnail write path. Every note row declares a `RowImageRef` `thumbnail` field that nothing can write yet, so every row draws the miniature until the device-snapshot upload exists. That design will settle upload frequency, who may write the field, and whether a recent earlier-revision snapshot may be shown. The second is a `host_create.after` follow-up once Notes needs its own create logic (an initial Yjs body or a template). The contract already allows that without changing the create path.