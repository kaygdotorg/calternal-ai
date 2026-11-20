# Per-space content

`calternal-content` supplies opaque item versions, per-space history, checked
userdata placement, collection records and durable request publication. The
server Notes adapter uses the same service exposed to trusted integrations by
`Store::content()`. Authorization remains at the mandatory host boundary.

The roots are **appdata** (databases and state), **userdata-ssd** (text files),
and **userdata-hdd** (other originals). The current `calternal-roots::Roots`
field names are used at its API boundary until that crate's separate rename.
Callers retain its root guards. The disposable server helper places the two
userdata trees inside its data directory; it is not production mount validation.
The host adapter `Store::open` validates and locks the supplied roots and retains
those guards during background placement. The witness root is `<appdata>/secrets`;
production backup policy must keep independent witnesses outside restored state.
This preview is not proof of complete file-only or rollback recovery.

| Location | Contents |
| --- | --- |
| appdata/spaces/<space-uuid>/content.db | One space's kind registrations, item metadata, immutable versions, control values (such as plugin enablement), pending publication intents and ID-to-path mapping |
| appdata/spaces/<space-uuid>/index.db | That space's inherited journal and account-qualified request receipts |
| appdata/system/content-index.db | Item-to-space/kind routes, request-to-space routes and account-collection ID routes; no titles, bytes, history or response payloads |
| userdata-ssd/spaces/<space-uuid>/<registered folder>/ | Current real files, with human-readable names |
| userdata-ssd/spaces/<space-uuid>/.calternal/<registered plugin>/<kind>/ | Hidden internal files, when the kind registration chooses hidden placement |
| appdata/accounts/<account-uuid>/content.db | Protected account-owned Collection definitions and membership |

A kind must be registered before its first write. Registration declares visible
placement (including a safe visible subtree) or hidden placement, plus Markdown
or opaque encoding. The library has no hard-coded Notes or chat layout. Notes
registers visible `Notes`, Markdown. A later chat provider can register a visible
`AI` folder with `Chats` as its subdirectory without changing storage mechanics.
Hidden placement uses the same plugin/kind namespace documented by the manifest
contract. Collections placement is also registered by their provider before standard
collections are provisioned; the library does not choose a provider's placement.

Notes current files contain a host-owned `calternal-id` frontmatter key and a
`calternal-note/v1` format tag when the document has no format of its own. Titles
use inherited NFC normalization, case/normalization collision checks and numbered
names. The per-space index stores the selected relative path; identity is never
derived from that filename. A title update commits its fixed rename intent with
the revision. Creation and renames use fsio exclusive destination placement;
case-only renames use a fixed temporary name so restart can find either half.
Existing outside bytes block the operation instead of being replaced. Markdown
operation bodies retain the existing wire format; identity frontmatter is added
when the file is materialized.

Writes commit metadata, immutable bytes and an outbox intent with FULL SQLite
synchronization. Recovery publishes its journal event and receipt, uses a FULL
journal publication barrier, then durably retires the intent. Acknowledgement
requires FULL content/history, journal/receipt and outbox-retirement commits on SSD.
The retirement barrier prevents power loss from resurrecting an intent after its
receipt expires. Before startup replay or hourly maintenance, content reads the
outbox under the space slot and asks the journal writer to prune only expired,
unpinned receipts. If the outbox cannot be read, that prune is skipped.
A surviving intent protects its receipt through durable retirement, even past
the seven-day default eligibility age. The existing lifecycle worker wakes after
commit, fsyncs the current file and directory, then
marks placement with NORMAL bookkeeping. Lost bookkeeping is replayed from durable
intents and file hashes. Pending heads pin their existing cache slots; no second
cache, queue, per-write thread or unbounded task list is added. The worker pins the
host's root locks while doing I/O. The inherited 30-second idle sweep remains.
A failed placement or a busy slot schedules another pass after 50 ms.
ADR 0062's materialization maximum is one second after acknowledgement. The oldest
pending age never extends when newer edits arrive. A missed deadline refuses new
writes in that space until placement succeeds; failed/outside-changed files remain
preserved and blocked. An update to a pending head first finishes its placement,
keeping the existing checked outside-edit and rename guards. Crash recovery opens
and recovers affected spaces lazily before serving their first operation. Restart handles both
committed-before-placement and placed-before-bookkeeping states. This preserves
the preview's checked-copy contract, including its documented limitation for an
external writer racing the last hash check and replacement. It does not implement
file-only reconstruction, watcher reconciliation, tier moves, general backlinks reconciliation
or a complete production recovery profile.

`ensure_standard_collection` creates stable, idempotent manifest-keyed space
Collections. Space/account membership helpers store explicit item IDs; those
references never grant access to their targets. Plugin enablement wiring belongs
to the plugin lifecycle lane. Account Collections have library persistence but
cannot yet use the server `resolve` wire response: that response requires a
space ID and has no account-owner alternative. No account ID is substituted for
a space ID. Account-profile recovery remains the host's responsibility.

Unfiltered `notes.list` uses the live Notes heads and keeps its existing
timestamp and UUID continuation. The content window API serves explicit
Notes collections in title order or a provisioned member-only newest order.
Registered claimed-text kinds can publish one scalar order in either
direction. A registered parent relation gets a separate covering scope for
each parent. Each space stores a covering key per effective row set and a
maintained count and version per nonempty scope. The per-space journal drives
background updates for item, membership, Trash and restore changes. Collection
grants to people, groups and public links refer
to the same explicit collection projection and retain their independent live
authority checks. A guest window exposes title and a view-scoped row selector.
Exact counts require explicit disclosure authority.

The content window API takes the catalogue request and returns loading while
the projection catches up, a ready or empty result, or a typed plan refusal.
Sequential starts seek a checked
boundary. Arbitrary exact-count jumps walk the covering index from the nearer
end. The host resolves the Bind and rechecks authority before delivery. See
the [window engine lane report](../../docs/reviews/window-engine-2026-11-19.md)
for statement counts and rollout requirements.

Main modules are `types` (plain boundary values), `db` (bound metadata/history),
`files` (registered placement and checked materialization), `journal` (direct
calls into each space's inherited journal store), `pool` (pooled read connections),
`collections` (collection records) and `lib` (service/cache/publication
coordination). At most 64 spaces are open, released after 600 idle seconds in the
server, with a 30-second background sweep; each open space owns its journal store.
Startup opens only the identifier catalog. First use validates that space and
replays its outbox before pending file placement, under the existing slot lock;
failed recovery never exposes an opened slot. Root locks and authority recovery
still complete eagerly before the server admits requests. Name caches are capped by directory count, and inherited name
watchers avoid warm Linux directory rescans; their byte size is not separately
metered. There is no performance qualification claim.

Every public operation documents its allocations, locks, I/O and durability cost.
Lists seek an index and load bounded metadata pages. Item/history reads select
one physical database. Resolution uses indexed global identifier routes read through
a connection pool, never membership/content scans. Request IDs are unique per
(account, space) and checked in that space's journal. Writes pay three FULL SSD commits before acknowledgement (content, journal and
outbox retirement),
plus a FULL catalog insert only for a new item. The former FULL placement mark is now NORMAL, safe-to-replay bookkeeping.
File/directory syncs follow off the acknowledgement path. SQLite may still sync at
WAL creation/checkpoint; those maintenance costs are measured separately. Work in one space never waits for another: each space has its own
lock, the cache map lock covers lookups only, and journal calls run on the caller's
thread.

Threads reuse the same Markdown kind storage and guarded move outbox. `admit_item` fences a native engine admission against the current item head; `trash` runs a bounded lifecycle guard before moving an item to its space-local Trash. Callbacks cannot re-enter content.

## Indexed discovery and lifecycle

The current head keeps rebuildable label and link projections. Per-space SQL
indexes provide keyset pages in update, folded title or tag name order without
reading body bytes. Each warmed-space page uses one indexed query. A move
between live placement and Trash changes index membership in the same
transaction as the revision and outbox intent. A path-and-link edit stages
changed same-kind sources atomically and finishes their recoverable placement
before publishing the root receipt. Purge removes the guarded Trash file and
its versions while retaining the item identity in the selected space.

See [Notes operations](../../docs/notes-operations.md) for the native
plugin's bounds and retention.

Warm ordinary writes load kind, revision, sequence, path, placement state, prior
path and expected file hash in one shared header query. Admission reuses it for
the checked file read; a pending prior revision is placed and then reread before
another edit. Placement reuses its header path instead of selecting it again.
No previous body is loaded on the ordinary acknowledgment path. The two checked
file reads (admission and later placement) remain; only repeated SQL header
reads were removed. Linked path edits and Trash/restore finish placement before
returning, and purge finishes removal before publishing its receipt.

An opted-in Markdown kind is searchable through a separate per-space database.
Threads have a chronological message index that follows the same journal and
locates one byte-counted message in the exact committed revision.
A bounded worker advances its projection and journal watermark outside the
acknowledgment path. A new generation becomes visible when it has scanned
the retained heads and caught up. List and ID reads use the retained metadata.
See [search and palette lookups](../../docs/search.md).

V2 windows use a separate rebuildable per-space WAL database. The shared
projection worker follows the durable item journal and publishes keys,
scope totals and an epoch/watermark in one NORMAL transaction. The reader
checks live authority before storage access and before delivery. A missing
or relevant lagging publication returns loading without rows, a count or a
cursor. A root Notes window can keep serving exact results across up to 128
proved unrelated journal entries. The native Notes cursor remains a head
query. The worker reuses its per-space handles and skips idle published
heads in memory. Bootstrap, epoch reset and collection replay use bounded
resumable pages. A broken sidecar produces typed unavailability while the
worker retries with backoff and rebuilds derived files. Replay records and
omits inherited over-limit notes until their memberships are repaired.
A Notes save adds no synchronous index I/O or worker request.

Space plugin settings use a versioned file in userdata. A single durable,
settings-only intent in the existing per-space database carries the file
exchange and journal receipt across process death. Recovery runs on the next
settings access. A file that needs repair blocks settings writes in that space.
