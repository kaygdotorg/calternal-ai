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
Production callers use `Store::open_roots` with independently validated roots
and a journal-witness root outside appdata backups.

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

Notes current files contain a host-owned `calternal-id` frontmatter key. Titles
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
journal publication barrier, then retires the intent. Checked current-file
placement fsyncs the file and directory before success. Restart handles both
committed-before-placement and placed-before-bookkeeping states. This preserves
the preview's checked-copy contract, including its documented limitation for an
external writer racing the last hash check and replacement. It does not implement
file-only reconstruction, watcher reconciliation, tier moves, backlinks rewriting
or a complete production recovery profile.

`ensure_standard_collection` creates stable, idempotent manifest-keyed space
Collections. Space/account membership helpers store explicit item IDs; those
references never grant access to their targets. Plugin enablement wiring belongs
to the plugin lifecycle lane. Account Collections have library persistence but
cannot yet use the server `resolve` wire response: that response requires a
space ID and has no account-owner alternative. No account ID is substituted for
a space ID. Account-profile recovery remains the host's responsibility.

Main modules are `types` (plain boundary values), `db` (bound metadata/history),
`files` (registered placement and checked materialization), `journal` (direct
calls into each space's inherited journal store), `pool` (pooled read connections),
`collections` (collection records) and `lib` (service/cache/publication
coordination). At most 64 spaces are open, released after 600 idle seconds in the
server, with a 30-second background sweep; each open space owns its journal store. Name caches are capped by directory count, and inherited name
watchers avoid warm Linux directory rescans; their byte size is not separately
metered. There is no performance qualification claim.

Every public operation documents its allocations, locks, I/O and durability cost.
Lists seek an index and load bounded metadata pages. Item/history reads select
one physical database. Resolution uses indexed global identifier routes read through
a connection pool, never membership/content scans. Request IDs are unique per
(account, space) and checked in that space's journal. Writes pay FULL content and
journal commits plus file/directory syncs, and a FULL catalog insert only when an
item is created. Work in one space never waits for another: each space has its own
lock, the cache map lock covers lookups only, and journal calls run on the caller's
thread.

Threads reuse the same Markdown kind storage and guarded move outbox. `admit_item` fences a native engine admission against the current item head; `trash` runs a bounded lifecycle guard before moving an item to its space-local Trash. Callbacks cannot re-enter content.

## Indexed discovery and lifecycle

The current head keeps rebuildable label and link projections. Per-space SQL
indexes provide prefix and name lookups without reading body bytes. A move
between live placement and Trash changes index membership in the same
transaction as the revision and outbox intent. A path-and-link edit stages
changed same-kind sources atomically and finishes their recoverable placement
before publishing the root receipt. Purge removes the guarded Trash file and
its versions while retaining the item identity in the selected space.

See [Notes operations](../../docs/notes-operations.md) for the native
plugin's bounds and retention.
