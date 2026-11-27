# Live window wire protocol · 2026-11-27

Slice 4 of [chat-first-build-plan-2026-11-27.md](../proposals/chat-first-build-plan-2026-11-27.md).
One WebSocket per browser session carries every live subscription for a space:
collection/ad-hoc queries and single-item record streams anchored by record
anchor. The protocol is generic: nothing in it names Chat or any plugin.

## Endpoint and authentication

- Route: `GET /v1/spaces/{space}/live` (WebSocket upgrade). `{space}` is a
  space UUID.
- The browser sends the Secure, HttpOnly, SameSite=Strict
  `__Host-calternal_session` cookie on the HTTP upgrade. The host checks its
  configured public origin and rejects an inconsistent `Sec-Fetch-Site`.
- The host verifies the session and personal-space access before upgrading.
  Missing credentials get HTTP 401. An inaccessible space gets HTTP 404.
- On success the host replies with `{"type":"ready","account":"<uuid>","space":"<uuid>"}`.
  Authentication and space access are checked again for live delivery. Passive
  delivery and expiry checks leave the session idle deadline unchanged.

## Client to host frames

All frames are UTF-8 JSON text frames, bounded to 64 KiB each. A session may
hold at most 64 concurrent subscriptions.

| `type` | Shape | Purpose |
| --- | --- | --- |
| `subscribe` | `{"type":"subscribe","sub":"<id>","query":{...}}` | Subscribe to a collection (`{"collection":"plugin.id","limit?":200}`) or ad-hoc query (`{"from":"plugin.kind","where?":"...","sort?":[...],"limit?":200}`). Either accepts `records: true` for records in each row. |
| `subscribe` | `{"type":"subscribe","sub":"<id>","records":{"item":"<uuid>","after?":"<anchor>"}}` | Subscribe to one item's records and metadata. When `after` is present and found in the item's ledger, the host sends only records with a strictly higher ordinal in ascending order; otherwise it sends the full live snapshot. |
| `unsubscribe` | `{"type":"unsubscribe","sub":"<id>"}` | Remove an active subscription. |
| `ping` | `{"type":"ping"}` | Keepalive check; host replies `{"type":"pong"}`. |

Reusing an existing `sub` ID replaces that subscription.

## Host to client frames

| `type` | Shape | Purpose |
| --- | --- | --- |
| `ready` | `{"type":"ready","account":"<uuid>","space":"<uuid>"}` | Session authenticated and ready for subscriptions. |
| `rows` | `{"type":"rows","sub":"<id>","rows":[...],"totals":{...},"cursor":null}` | Initial or updated query window. Row shape matches `s.query` (`id`, `kind`, `fields`, `computed`, viewer `state`, server-evaluated `when` and `version`). |
| `records` | `{"type":"records","sub":"<id>","item":"<uuid>","version":N,"fields":{...},"computed":{...},"state":{...},"when":{...},"mode":"snapshot"\|"delta","after":"<anchor>"\|null,"records":[...],"last_anchor":"<anchor>"\|null}` | Initial snapshot, reconnect catch-up from `after`, or resync after edit/redaction/lag. `records` are always in ascending ledger ordinal order. |
| `record_append` | `{"type":"record_append","sub":"<id>","item":"<uuid>","version":N,"record":{...}}` | Incremental push when one or more new records are appended strictly after the subscription's last sent anchor. |
| `item_change` | `{"type":"item_change","sub":"<id>","item":"<uuid>","version":N,"fields":{...},"computed":{...},"state":{...},"when":{...}}` | Pushed to a `records` subscriber when the item's fields or computed values change (e.g. a rename or unread count change) without record edits. |
| `error` | `{"type":"error","sub":"<id>"\|null,"code":"...","message":"...","next":"..."}` | Caller-safe error using the ten revision 9 codes. |
| `pong` | `{"type":"pong"}` | Reply to `ping`. |

Records in `records`, `record_append` and opt-in `rows` frames use the
[same `writer` object as `s.read`](host-call-route-2026-11-27.md). Read
`writer.name` for a person, or resolve `writer.plugin` in `s.declarations`
for a run. `writer.starter` identifies the starting person. Older run stamps
may have `writer.plugin: null`. Unverified outside records carry
`writer.type: "outside"`.

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
3. **Lag recovery:** each connected account in a space has its own bounded
   invalidation channel (capacity 256). The existing space lock orders the
   nonblocking publication with the commit. If a slow client
   misses notifications, the session re-evaluates its active subscriptions from
   each subscription's tracked `last_anchor` (records) or current query result
   (collections) and pushes the exact diff or snapshot.
4. **Viewer-aware evaluation:** every notification re-evaluates through the
   same authorized `s.query` and `s.read` paths as the HTTP call route, so
   per-viewer formulas (`state.pinned`, `unread`) stay isolated per actor. A
   state write queues an event for that viewer only. Shared row and record
   data are checked through the receiving actor’s authorized read path before
   serialization.

## Run logs

Subscribe with `{"type":"subscribe","sub":"answer","run_log":{"run":"RUN_UUID","after":0}}`.
The run ID comes from Chat's send result. `after` is the last received chunk
offset, exclusive. Omitting it starts at the first retained chunk. The socket uses the same authenticated
person and rechecks that person's read access to the run target before sending
each batch.

The `run_log` response contains `sub`, `run`, `offset`, `chunks`, and
`stopped`. Each chunk has an increasing offset and either
`event: "data"` with its stream and base64 bytes, or `event: "exit"` with
the process status. A subscription starts with a batch even when the run
has yet to emit output. The terminal batch sets `stopped`. Reconnect with
the last offset to receive only the suffix. An offset older than retained
output returns `conflict`. The host retains up to 256 chunks or 1 MiB per
run, and removes the in-memory logs on restart.
