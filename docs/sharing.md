# Sharing notes and collections

Share a note or saved Notes collection through a named account, a group, or a public link. A recipient's effective permission is the strongest current grant. Group access follows live membership. An item grant covers that item and, for a saved collection, its explicitly included notes in the same space. The collection owner controls membership. Workspace listings, Trash and unrelated items still require their own authority.

Space Editors can manage account and group grants. Managers and the Owner can manage links and may allow Editors to manage them. A recipient with item Edit can edit shared note content. Receiving a share does not grant share management.

Public links are independently revocable and optionally expire. Omit expiry or choose `null` for Never. A label describes a link in the Share sheet and does not change its URL. Share URLs keep their identity when an item moves. This slice serves view-only guest links on the server's own domain. Thread sharing, custom domains and guest editing follow separately.

The configured caps are **provisional**. An item can have 100 active account and group grants combined and 32 live links. The guest route admits 60 requests per minute per source and 120 per minute per verified link.

## Operation contracts

An item reference is `{"kind":"notes"|"collections","id":"<uuid>"}`. Permissions are `"view"` and `"edit"`. An audience is either `{"account_id":"<uuid>"}` or `{"group_id":"<uuid>"}`. Expiries are Unix milliseconds. All mutations use the standard request ID for retry. Cursor pages return up to 50 rows and an optional `next_cursor`.

| Operation | Input | Result |
| --- | --- | --- |
| `sharing.list` | `item, cursor?` | `permission, can_manage_audiences, can_manage_links, space_access, grants, next_cursor?` |
| `sharing.audiences` | `item, query, cursor?` | `audiences, next_cursor?`. Each row has an audience and display name. |
| `sharing.grant` | `item, audience, permission` | Grant row. The same audience keeps its grant ID when its permission changes. |
| `sharing.link_create` | `item, label, expires_at?` | Link grant row. Use Copy to retrieve the URL. |
| `sharing.link_update` | `item, grant_id, label?, expires_at?` | Updated link grant row. An omitted field stays as written, while a null expiry means Never. |
| `sharing.link_copy` | `item, grant_id` | `url`, for Copy, QR, and the system share sheet. |
| `sharing.revoke` | `item, grant_id` | `grant_id`. Other routes to the item retain their own access. |
| `sharing.shared_with_me` | `kind, cursor?` | `items, next_cursor?`. Each row has `item, title, shared_by, permission, unread`. |
| `sharing.set_link_policy` | `space_id, editors_can_manage_links` | `editors_can_manage_links`. Restricted to space settings authority. |

A grant row has `grant_id` and `permission`. Named grants also have `audience` and `name`. Link grants have `label` and `expires_at`, and report `"view"`. The list contains the space audience summary and management flags separately so the sheet can show effective access without exposing other audiences to ordinary recipients. Open received items at their canonical item address. Missing and hidden addresses have the same response.

| Collection and recovery operation | Input | Result |
| --- | --- | --- |
| `collections.create` | `space_id, title, members` | `id, revision`. Members are same-space live Notes UUIDs. |
| `collections.get` | `id` | `id, title, revision, members`. Rows have `id, title`. |
| `collections.set_members` | `id, base_revision, members` | `id, revision`. Restricted to the collection's space Owner. |
| `sharing.mark_seen` | `item` | `seen` for the signed-in recipient. |
| `sharing.rescue` | `title, changed_blocks` | `note` in the signed-in account's personal space. The client sends only its own changed Markdown blocks. The server uses the local title and supplied blocks to create the copy. |

Putting a note in Trash suspends existing links and grants. Someone with link management permission can revoke a suspended link. Restoring the note makes surviving grants active again. Permanent purge removes authority. Losing group membership or revoking an individual grant removes only the access gained through that grant. Item-only editors can edit the note's Markdown. Renaming uses the space's linked rewrite operation and requires space Edit. Shared Notes search reuses the source space's index with allowed item IDs and checks current access before sending matches.

## Guest reads

`GET /s/<secret>` renders a read-only fallback. Request `Accept: application/json` to get `{title, markdown}` for a note, or `{title, members:[{title, href}], next?}` for a collection. Opening a member's link returns `{title, markdown, back}`. Follow guest member, back and next addresses as returned. A member address is bound to one link and one note. If the collection changes and a different note occupies that slot, the old member address returns 404. Reopen the collection for its current member addresses.

The server adds `item: {kind, id}` only when a valid originating session can independently open the selected note or collection. The client can use that reference for **Open in Calternal**. A link's authority alone never supplies a canonical item address. Invalid, expired and revoked links and hidden items return the same 404. Guest replies use `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, and `X-Robots-Tag: noindex, nofollow`. The HTML document has no scripts or external resources. Reverse proxies must omit the bearer path from access logs. Revocation prevents subsequent reads of the root and its member addresses. Someone who already received the content can still keep a copy. The server stores link lookup digests and encrypted secrets using separate keys derived from the existing installation invite key.

## Storage and upgrade

The production baseline already provides the sharing grant, policy and recipient tables in `auth.db` 14 and the derived reverse membership index in space `content.db` 5. This feature uses both schemas as shipped. Grant, policy and recipient marker changes use the existing auth recovery barrier. The index rebuilds from versioned collection metadata. See [Data-preserving upgrades](upgrades.md).

A typical stored Notes read uses one route lookup, one indexed reverse-membership lookup and one protected authority statement at admission, then a payload read. Delivery uses one fresh reverse-membership lookup and one authority statement. Guest open uses two indexed auth reads for the bearer, one item route lookup, and one note head read or one collection head read. A collection list reads at most 50 member metadata rows. A member open reads one additional note head. The optional item reference adds two credential checks and one current item authorization for a signed-in reader. These reads perform no flushes. Management mutations use an immediate auth transaction with the existing protected receipt and recovery publication. The live rooms integration calls the post-commit eviction trait after last-read loss on explicit grant revoke. Room delivery also uses the shared access revision below.

## Live room authorization

The room service calls `resolve::authorize_item_access` for the current View or Edit decision. A full Notes check binds the supplied space, kind and item with one live-head lookup, reads the reverse membership index once and makes one protected auth statement. The content lock keeps the lifecycle and inherited collection set stable during that check.

`resolve::item_access_revision` returns one process-wide atomic handle. Sampling it costs zero storage reads. Sample with Acquire before and after a full check and cache only an unchanged even epoch. Authority work keeps the value odd through commit or rollback and through recovery publication. Overlapping changes keep it dirty until the last writer retires. There is one retained handle across all rooms. Direct grants, groups and space roles publish through the auth worker. Collections, Trash, restore, purge, path moves, erasure and recovery also invalidate the handle. Plain Notes body writes keep the current epoch. Rooms still check their own session and plugin activation.

## Custom share domains

Keep the canonical link on the installation origin. An optional verified host
can serve `/s/<secret>` and its member addresses using a separate bearer bound
to the verified hostname claim. A request to an expired or transferred
hostname must never disclose a bearer that also works on the installation
origin. Losing the custom hostname leaves the canonical link usable.
Revocation and expiry use the same grant on both hosts.

Start with one exact HTTPS hostname per space, approved by its Owner and
provisioned by the operator. Verify control using a DNS TXT challenge before
enabling the binding. Recheck ownership and withdraw the route on loss of
verification or removal. Refuse reserved hostnames, wildcards, and bindings
already claimed by another space. Scope every lookup to the verified host's
current space and resolve the item by its stable UUID. Moving it across spaces
requires the destination's binding on the custom host.

Serve only the existing read-only guest route on custom hosts. Keep passkeys,
cookies, session-dependent item references, app assets and APIs on the
installation origin. Require explicit proxy trust, strip untrusted forwarding
headers and exclude bearer paths from edge logs. The future Publish plugin can
request an existing verified binding through the host. The core remains
responsible for domain ownership and grant checks.

Custom domains are a design for later. They require a durable host registry and
recovery classification with a versioned migration before any route ships.
