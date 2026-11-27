# Live window wire protocol · 2026-11-27

Slice 4 of [chat-first-build-plan-2026-11-27.md](../proposals/chat-first-build-plan-2026-11-27.md).
One WebSocket per browser session carries every live subscription for a space:
collection/ad-hoc queries and single-item record streams anchored by record
anchor. The protocol is generic: nothing in it names Chat or any plugin.

## Endpoint and authentication

- Route: `GET /v1/spaces/{space}/live` (WebSocket upgrade). `{space}` is a
  space UUID.
- Authentication:
  - Non-browser clients may send `Authorization: Bearer <token>` on the HTTP
    upgrade request.
  - Browser clients (which cannot set custom headers on `WebSocket`) send an
    `auth` frame as their first message:
    ```json
    {"type":"auth","token":"<bearer>"}
    ```
  - Bearer tokens are never placed in query parameters.
  - The host resolves the session through `calternal-auth` and verifies access
    to `{space}` before accepting any subscription.
- On success the host replies:
  ```json
  {"type":"ready","account":"<uuid>","space":"<uuid>"}
  ```
- On authentication failure the host sends an `error` frame with code
  `needs-sign-in` or `not-found` and closes the socket.

## Client to host frames

All frames are UTF-8 JSON text frames, bounded to 64 KiB each. A session may
hold at most 64 concurrent subscriptions.

| `type` | Shape | Purpose |
| --- | --- | --- |
| `auth` | `{"type":"auth","token":"..."}` | Authenticate the WebSocket session (first frame when no Bearer header was sent). |
| `subscribe` | `{"type":"subscribe","sub":"<id>","query":{...}}` | Subscribe to a collection (`{"collection":"plugin.id","limit?":200}`) or ad-hoc query (`{"from":"plugin.kind","where?":"...","sort?":[...],"limit?":200}`). |
| `subscribe` | `{"type":"subscribe","sub":"<id>","records":{"item":"<uuid>","after?":"<anchor>"}}` | Subscribe to one item's records and metadata. When `after` is present and found in the item's ledger, the host sends only records with a strictly higher ordinal in ascending order; otherwise it sends the full live snapshot. |
| `unsubscribe` | `{"type":"unsubscribe","sub":"<id>"}` | Remove an active subscription. |
| `ping` | `{"type":"ping"}` | Keepalive check; host replies `{"type":"pong"}`. |

Reusing an existing `sub` ID replaces that subscription.

## Host to client frames

| `type` | Shape | Purpose |
| --- | --- | --- |
| `ready` | `{"type":"ready","account":"<uuid>","space":"<uuid>"}` | Session authenticated and ready for subscriptions. |
| `rows` | `{"type":"rows","sub":"<id>","rows":[...],"totals":{...},"cursor":null}` | Initial or updated query window. Row shape matches `s.query` (`id`, `kind`, `fields`, `computed`, `version`). |
| `records` | `{"type":"records","sub":"<id>","item":"<uuid>","version":N,"fields":{...},"computed":{...},"mode":"snapshot"\|"delta","after":"<anchor>"\|null,"records":[...],"last_anchor":"<anchor>"\|null}` | Initial snapshot, reconnect catch-up from `after`, or resync after edit/redaction/lag. `records` are always in ascending ledger ordinal order. |
| `record_append` | `{"type":"record_append","sub":"<id>","item":"<uuid>","version":N,"record":{...}}` | Incremental push when one or more new records are appended strictly after the subscription's last sent anchor. |
| `item_change` | `{"type":"item_change","sub":"<id>","item":"<uuid>","version":N,"fields":{...},"computed":{...}}` | Pushed to a `records` subscriber when the item's fields or computed values change (e.g. a rename or unread count change) without record edits. |
| `error` | `{"type":"error","sub":"<id>"\|null,"code":"...","message":"...","next":"..."}` | Caller-safe error using the ten revision 9 codes. |
| `pong` | `{"type":"pong"}` | Reply to `ping`. |

## Ordering, reconnects and bounds

1. **Authoritative order:** record order is always the host ledger's ascending
   `ordinal`. Every `records` and `record_append` frame delivers records in
   strict ordinal order.
2. **Reconnect from anchor:** the client remembers the latest `anchor` it has
   applied for each open records view. On reconnect it sends
   `subscribe` with `records: { item, after: lastAnchor }`. The host returns
   `mode: "delta"` containing only records after that anchor, so no record is
   duplicated or reordered. If the anchor was redacted or no longer exists,
   the host falls back to `mode: "snapshot"` so the client replaces its window
   cleanly.
3. **Lag recovery:** each space has its own bounded `broadcast` channel
   (capacity 256). No global lock sits on the request path. If a slow client
   misses notifications, the session re-evaluates its active subscriptions from
   each subscription's tracked `last_anchor` (records) or current query result
   (collections) and pushes the exact diff or snapshot.
4. **Viewer-aware evaluation:** every notification re-evaluates through the
   same authorized `s.query` and `s.read` paths as the HTTP call route, so
   per-viewer formulas (`state.pinned`, `unread`) stay isolated per actor.
