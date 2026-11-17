# Following server state

Clients do not sync files or merge replicas. The server owns all content and
merge authority. `Client::subscribe` opens one SSE connection following 1–16
space UUIDs, restoring opaque signed bookmarks through `BookmarkStorage`.
`LiveConnection::next` reconnects with exponential 250 ms–30 s backoff; reset,
authorization and protocol failures stop. Cancellation interrupts network reads
and backoff, and dropping the connection releases its socket without background
workers. This preview still accepts loopback HTTP only.

The server subscribes before backlog reads and always reads indexed journal pages;
broadcasts are bounded hints. Sequence positions suppress repeated backlog/live
hints. Every delivery checks current session, membership and per-space item existence.
A bookmark from another space, an expired epoch or rollback produces explicit
`reset`, after which the consumer reloads authorized state and reconnects.

After applying an entry, call `accept` to invalidate its configured item cache and persist
its bookmark. Never accept before application. A crash before acceptance can replay
that event; cache application must be idempotent. `Ready` bookmarks also cover
filtered entries, so clients must not infer global continuity from visible entries.
No atomic snapshot+bookmark or retention lease is implemented: callers must reload
on reset before trusting derived state. `ItemCache` holds bounded UUID-keyed opaque
values and optionally calls platform-provided `PersistentCache`; there is no merge
logic. `FileBookmarks` uses inherited fsio atomic writes and Markdown cache files.
Each storage instance must be scoped to one server/account. Apple supplies durable
storage and restores bookmarks when creating its native stream; NativeLive has
only ephemeral bookmarks, and the platform persists state before accepting events.

Each live connection owns an `OfflineOutbox`, drained after authorized reconnect
before publishing events. It retains ordered immutable request envelopes in memory, with stable
request IDs and no local merge. Draining first queues visible server pending submissions and then submits them
through ordinary operations, rechecking access and replaying receipts. Any refusal or uncertain transport
outcome retains the head and stops the drain. Platform code owns durable offline
input retention and visible pending rendering. Never transfer an outbox into a
different account/session. A sent envelope's identity/input cannot be edited after
an uncertain result. Server drafts and pending submissions now use explicit-delete retention;
input older than seven days needs confirmation. `subscribe_account` follows
content-free invalidations; reload authorized drafts/pending records before
accepting each bookmark. See [account input](../../docs/account-input.md) for
ceilings, target scoping, native support and platform persistence limitations.

Bounds: 16 bookmarks per memory store, 4 KiB per bookmark file and 1024 files per exclusively owned file cache root,
2 MiB SSE buffer,
1–1024 memory cache items of at most 1 MiB each, and 256 offline envelopes of at
most the core's 2 MiB body bound. The caller controls cache cleanup; this lane does
not claim encrypted offline recovery, platform Keychain integration, production
transport or a file sync implementation.
