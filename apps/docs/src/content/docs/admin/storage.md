---
title: "Storage layout"
---

<!-- Generated from: crates/roots/src/lib.rs, docs/glossary.md, crates/content/README.md, crates/server/README.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

- **appdata**: the server's own databases and keys, on SSD. Never the only copy of a user's file.
- **userdata**: real, readable files, in two trees: `userdata-ssd` for text and `userdata-hdd` for everything else.
- **Root guard**: the startup check that the three storage roots are present, correct and not swapped.
- **Recovery log**: an append-only protected file in userdata recording every change to who has access to what, so access survives losing appdata.
- **Kind**: a type of item, such as `notes` or `threads`. A plugin declares its kinds.
- **Item**: one thing of a kind, with a permanent ID and an address `/<kind>/<id>`.
- **Collection**: a stored list of items with its own ID, addressed as `/collections/<id>`.
- **Journal**: a space's ordered record of changes, which clients follow to stay in sync.


## Root guard

Three-role storage startup guard, ported from Calternal's storage startup.

The caller pins an installation UUID in configuration outside appdata and
explicitly opts into first setup. All roles and identities are checked before
any lock sentinel or store is created. Existing installations require a
nonempty regular `system/auth.db`; this is not SQLite integrity validation or
FO1 reconstruction. This library never creates a database.

On Linux, Android and Apple, every path component is opened with `NOFOLLOW`
and directories are retained by descriptor. Markers, locks and setup writes
resolve through those descriptors, so path symlink substitutions cannot redirect
writes. Lexically overlapping paths and equal device/inode roots are refused.
Bind-mount ancestry under different path spellings is not detected. These checks do not
prove separate devices, mount IDs, SSD/HDD hardware, mount continuity, or
provenance of copied markers/auth files. No device separation is enforced:
userdata-ssd and appdata are allowed on the same filesystem. An unavailable
mount is detected through missing roots/markers/identity, not a mount-table
check. Hostile concurrent mutation of real directories or marker bytes is
outside the cooperative root-lock contract; this is not a tenant sandbox.

Incomplete first setup is preserved and refused on subsequent opens; there
is no automatic repair, adoption of legacy single-root storage, or automatic re-pair. Deliberate offline re-pair requires explicit confirmation.

## Content storage

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


## Preview durability


Each space's `appdata/spaces/<space-uuid>/content.db` (WAL, `synchronous=FULL`) is authoritative for its revisions and history, and its `index.db` journal for its request receipts; plugin activation is a control value in that space's `content.db`, so no installation-wide database is written for it. Reads come from SQLite. `userdata-ssd/spaces/<space-uuid>/Notes/<title>.md` is a checked, portable copy of each note's head revision, named from its title, with the item ID in its frontmatter.

A write proceeds as follows:

1. **Commit.** One transaction commits the full revision, the receipt `(principal, request_id) → operation, SHA-256 fingerprint of canonical input, result` and `mat_state=pending`.
2. **Check the existing file.** It must be missing on a first write, or match the SHA-256 recorded at the last placement. Symlinks and non-regular files never pass.
3. **Place the new bytes.** Write a temp file, fsync it, rename it over the target, then fsync the directory. Newly created directory ancestry is also fsynced.
4. **Mark and acknowledge.** Set `mat_state=materialized` with the new hash, then return the acknowledgement.

Recovery and retries:

- **Startup.** The server takes an exclusive lock on the data directory (a second instance exits with code 2). It removes only its own `.<uuid>.<12hex>.tmp` files, then replays pending notes before serving.
- **Crash after rename.** If replay finds the new bytes already in place, it fsyncs the file and the directory before marking.
- **Retry with the same input.** It returns the original result, finishing any pending placement. It is reauthorized against current grants and does not re-run the old base-revision check.
- **Retry with different input.** It returns 409 `request_id_reused`.
- **Plugin enable/disable.** These are receipted in the same way, so retrying an old enable cannot undo a later disable.
- **Mutating plugin operations.** Receipted after delivery: a retry with the same request ID and input returns the original result without running the worker; different input is 409. Each brokered `write_item` also carries its own receipt (`plugin/<plugin>/<request>/<n>`), so a retry after an undelivered result replays those writes instead of duplicating them.

**External changes.** A changed, deleted or replaced note file is never overwritten. The note is blocked: updates return 409 with `details.reason = "external_change"`, the external bytes are preserved, and reads still serve the SQLite revision. There is no import and no unblock command tonight. The hash check detects changes that happened before a write. It does not lock out an external writer that races the check and the rename. Concurrent external editing is out of scope.

**Crash tests.** In debug builds only, `CALTERNAL_FAULT=after_commit|before_placement|after_placement|before_reply` aborts the process at that point. `tests/http.rs` kills the server at each point, restarts it, and checks the placed bytes, the idempotent retry, that history has no duplicates and that no temp files are left.
