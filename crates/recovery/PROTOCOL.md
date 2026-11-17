Protected incremental security recovery
======================================

This crate implements the security publication portion of ADR 0062 FO1, adapted
to spaces. It supplies validated rebuild input; it does not rebuild auth state,
recover namespace operations, validate mount roles or certify tenant sandboxing.
The host must pin the installation UUID externally, validate SSD/HDD roles and
all root locks, and configure every auth writer before admitting requests.
Without a sink, the existing auth transaction behavior remains available.
Version 2 deliberately does not read or migrate version-one recovery trees.

Location, ownership and records
------------------------------

The server provisions `<ssd-space-root>/.calternal-recovery` mode 0700, outside
all tenant content roots. The Linux implementation pins no-follow directory
handles. All files, including staging files, are mode 0600 and atomically placed
with the ported fsio writer. Trusted ancestor directories, root locks and protection
from same-UID plugins are host requirements; mode bits alone are not a sandbox.
A genuinely empty root may be provisioned. A partial tree or missing head is
refused, never adopted or repaired by guessing.

`accounts/` contains protected account, group, invite and global-policy records;
`spaces/` contains only a space row and that space's memberships. Account and
space UUIDv7 identities remain distinct, including their typed APIs. Global
policy holds the fixed protected schema inventory, role/capability definitions,
development fixture bindings and unbound first-account tokens. It is not a bundle
of every account. Group and invite records have their own UUID namespaces. The
explicit mappings are in `state.rs`; unknown tables refuse configuration until
reviewed. Token hashes, public passkeys and all intents remain host-private.

Each current account record owns its user/profile/server role/quota row, passkeys,
bound setup/recovery tokens, installations (including revoked state), group
memberships, operation receipts, installation Notes scopes, owned share-authority
rows and recipient grant-authority rows. Group records own group definitions;
invite records own invite state and group targets. Sharing authority is retained
conservatively even though its API is not ported. No credential enters a space
record. SQL schema text is descriptive input: future rebuilding must install its
own validated schema and enforce all owner/identity/FK invariants.

Recoverable versus disposable state
----------------------------------

Credential additions/removals/counters, enrollment-token issuance/replacement/
consumption, session creation/revocation, account metadata, authority/role changes,
space lifecycle/memberships and retry receipts publish before acknowledgement.
SQLite AFTER triggers enqueue old/new owner keys **in the modifying transaction**.
Capture uses indexed owner predicates, including an index on setup-token ownership;
it never captures every auth table for an ordinary mutation. Bootstrap seeds all
current owner keys once. The first protected transaction publishes that initial
inventory; an empty genesis head is not usable rebuild input. Hosts attaching to
preexisting state must finish that first protected publication before claiming
recoverability of operations that predated configuration.

Hourly session `last_seen_at` and rolling `expires_at` touches do not dirty records.
Passkey `last_used_at` touches do not dirty records either. Last-seen is represented
as creation time and last-used as NULL; these values carry no authorization.
A recovered expiry is the last protected capture's absolute expiry, potentially
shorter than the live database's extended expiry. Unrelated later protected account
changes can capture the then-current absolute expiry. Rebuild must enforce actual
wall-clock expiry and explicit revocations, never extend expiry during import or
resurrect a revoked installation. Touches cannot clear `revoked_at`.

Rate-limit buckets, failure backoff and pending WebAuthn ceremonies are memory-only
and excluded. Sharing journal/delivery/ack/reset bookkeeping is excluded:
`note_journal`, `share_fanout`, `recipient_control`, `recipient_control_acks`,
`recipient_share_state`, `share_fanout_acks`, `recipient_share_fences`,
`sharing_installation_resets`, `sharing_reset_cursors`, `share_fanout_deliveries`.
**Future rebuild rule: discard all sharing delivery state and conservatively force
every device to resync under current authority. Never trust restored delivery,
acknowledgement, cursor, fence or reset state.** Dormant-device touches do not
publish. Sharing remains unported; this representation does not enable it.

The auth write audit and exclusions are in REVIEW.md. Runtime mutations use the
barrier; startup schema migration remains outside runtime publication, under root
locks before admission. Migrations 1–15 were not changed. No invite consumption
API is currently ported; any future implementation must use this same barrier.
Direct external SQL writers are not authorized runtime writers merely because
triggers enqueue their dirty keys: they must also execute the commit barrier.

Authenticated index and head
----------------------------

`head.json` is a bounded-size checksummed installation head. It binds format,
installation UUID, sequence, transaction UUID, the current intent digest/name,
the authenticated index root, an optional pending intent and a collection flag.
It never derives freshness from directory filenames. `index/` holds immutable
BLAKE3-addressed 16-way radix nodes. Keys are one namespace digit plus 32 UUID
hexadecimal digits; a lookup/replacement touches at most 34 nodes, independent
of account/space count. Each terminal contains the full key and record-file hash.
Every root-to-record edge authenticates exact serialized bytes. The global record
contains the closed schema inventory, allowing a full read to assemble one
coherent logical Snapshot. Full validation/reconstruction necessarily costs O(N)
records; it runs for explicit rebuild reads and once at configured admission,
not for each mutation. Mutation replay/capture/collection touch bounded affected
paths. No full-table integrity scan or history scan occurs per change.

This intentionally uses separate small node files, reusing fsio without extending
its durability primitive or maintaining a growing full manifest. The tradeoff is
a relatively high fixed fsync cost, quantified below, rather than installation-
sized rewrites. It has not passed a production latency budget.

Publication protocol
--------------------

The auth commit gate serializes prepare/commit/completion across workers;
SQLite IMMEDIATE transactions serialize writers. Direct trait callers must hold
the same interval themselves. Installation root locks exclude other processes.

1. While the authority write transaction is active, read the singleton committed
   witness through a **separate read-only connection**, never the writing
   transaction. A transaction already containing its own unpublished witness
   cannot prepare again. Reconcile any prior pending head using only committed
   visibility, then capture the transaction's dirtied owner records. A true no-op
   clears the dirty queue in SQLite and returns an empty publication token.
2. Compute only replaced radix paths and their exact retirement inventory. Write
   one checksummed delta intent in `transactions/`, containing changed records,
   new nodes, resulting root and its exact base head. Sync its file and directory.
3. Atomically replace and sync the fixed head with that pending intent reference.
   Only after this barrier may the authority transaction commit. Replace the
   singleton `(transaction UUID, intent digest)` witness **in the same transaction**
   as the authority change and dirty-queue deletion; commit with SQLite FULL WAL
   durability. The witness does not accumulate with history.
4. Using an autocommit connection, resolve the pending intent. A witness matching
   the pending transaction means commit: write only changed record versions and
   new index nodes durably, then atomically publish the new clean head. A witness
   matching the base head means rollback: restore the base head and remove the
   aborted intent. Any other witness, missing pinned intent or digest mismatch
   refuses operation. No uncommitted witness can publish authority.
5. After the clean head is durable, reclaim only old files listed by that intent:
   replaced path nodes, replaced/deleted record versions, and its predecessor's
   intent. Sync every affected directory. Durably mark collection complete in the
   head, then acknowledge. A failure prevents acknowledgement. Existing session
   revocation cache fences still run immediately after SQLite commit even when
   publication fails.

The head itself is the completion marker; there is no `.complete` sidecar.
The current intent remains pinned as a manifest until the next head supersedes it.
An intent orphaned by failure before pending-head placement cannot have committed;
a surviving authoritative DB and clean matching witness allow its removal.
Unknown extra transaction files cause a standalone snapshot reader to refuse.

Retention and crash proof
-------------------------

A single correctly hooked transaction cannot commit before its pending head and
intent are durable. Before commit, loss of execution rolls back both authority
and witness. Replay sees the base witness and cannot publish the proposed grant.
After commit, replay sees the pending witness and must finish that exact state,
including revocations. A pending head always blocks standalone snapshot reading.
Thus deleting a newest pending intent cannot make the reader choose the base.
After publication, the exact manifest/root/record hashes are mandatory; deleting
newest files or substituting an older record/index version refuses rather than
restoring authority. Two or more affected records become visible under one head;
there is no accepted mixed-version intermediate inventory.

A published replacement path cannot remain reachable from another current key:
leaves bind complete keys and branches bind their children. Collection therefore
unlinks only files unreachable from the new root. It starts after new-head
placement and can never delete a current dependency. Interrupted deletion repeats
idempotently; the head's collection flag is set only after all deletion directory
syncs finish. Until then the current intent retains the complete retirement list.
A crash during the flag write leaves either safe value. Ordinary collection does
not sweep the installation or copy unrelated live records.

For L current logical records there are at most 34*L live index files, one current
version per record, one retained intent, and a fixed head. During interruption
there is at most one additional proposal's records/path nodes and one predecessor
intent. Bootstrap and an explicitly large multi-record operation can have large
intents proportional to their changes; superseded history does not accumulate.
The dirty queue can hold only distinct affected owners; the DB witness has at
most one row. Current authority rows and retry receipts within an owner's record
remain proportional to that owner's current DB state, not installation history
of recovery versions. External writers that bypass the coordinator invalidate
these bounds and are outside the configured-writer contract.

State loss while a head is pending is ambiguous and **refused**: without the
committed DB witness it is impossible to choose between an uncommitted grant and
a committed revocation. This is not full FO1 availability across two-store loss.
A committed but unacknowledged change can require a receipt retry. The next hooked
request resolves it before publishing its own change; restart must reconcile
before admitting authenticated requests. Ordinary session reads do not replay.

Rollback threat boundary and external pin
----------------------------------------

Checksums detect missing, truncated, mixed or substituted bytes. They are not MACs
and do not protect against a privileged operator forging a whole new consistent
image. A tenant must have no access to the tree. Deleting just the newest intent,
current record, current index node or fixed head is detected. Replacing individual
files with old versions is detected by the unchanged current head/index hashes.
Compaction also removes obsolete current dependencies; the validator never searches
for a fallback generation.

A complete, internally consistent old recovery-tree image **can** be accepted by
`validate_and_read` when no independent freshness evidence survives. An installation
UUID does not prove freshness. With the original DB surviving, replay compares the
head to its singleton committed witness and rejects a tree-only rollback. If the
DB is also lost or rolled back, detection requires a separately durable,
rollback-resistant `Checkpoint { installation, sequence, digest }`, retained at
or beyond the last acknowledged change. `checkpoint()` supplies that binding and
`validate_and_read_at()` checks it together with the snapshot under one gate.
The host must store/update that external pin before acknowledging changes if it
requires this stronger guarantee; this crate does not implement the external
store or claim it is already integrated. Keeping a pin beside the tree or only
pinning the installation UUID does not detect complete-tree rollback.

Exact filesystem durability costs
---------------------------------

For an ordinary nonempty delta, let R be changed live record versions, N the new
index nodes, and F the number of directories with retired files (0–4). All atomic
writes use fsio: one file fsync plus one placement-directory fsync. The exact total
on the successful fresh publication path is:

`8 + 2*R + 2*N + F` filesystem fsyncs, plus SQLite FULL/WAL durability.

The fixed eight are intent placement (2), pending head (2), clean published head
(2), and collection-complete head (2). Collection unlinks do not fsync files;
each distinct affected directory is synced once. Replay after an injected fault
may repeat durable writes/syncs. One-time private-directory provisioning and the
genesis-head placement are additional startup work, excluded from mutation costs.

| Change | R / N / F | Exact normal-path filesystem fsyncs |
| --- | --- | --- |
| Existing account credential/label/session revocation, no receipt | 1 / 34 / 3 | **81** |
| Existing space membership/role/owner change, no receipt | 1 / 34 / 3 | **81** |
| New account or space record, without another affected record | 1 / 34 / 2 | **80** after initial inventory exists |
| Existing account plus existing space (for example a space change with an account receipt) | 2 / 67 / 4 | **150** |
| Delete a space record | 0 / N<=33 / 3 | **11+2*N**, at most **77** |
| No-op/receipt hit with no protected changes | 0 / 0 / 0 | **0** |
| Session touches, last-used touches, rate limits, sharing delivery/reset state | no publication | **0** |

A batch costs O(affected record bytes + 34*affected identities), with shared index
prefixes written once. A single-owner cost is constant in unrelated accounts and
spaces; tests count actual completed fsio file/directory barriers at both sizes.
This is a work/durability bound, not a benchmark pass or an interactive budget.

Fault injection
---------------

`Step` covers intent, pending head, commit, records, index nodes, published head,
each retired unlink and each collection directory sync. Tests interrupt every
index-write/unlink/sync position in a one-record replacement, primary/replay
boundaries for grants/revocations/roles/transfers/passkeys, partial multi-record
publication, aborted-intent collection, next-request completion, and concurrent
space mutation pairs. Atomic file-fsync/rename/directory-sync interiors rely on
the ported fsio primitive tests; coordinator observation surrounds the whole
atomic call and reports its two completed durability calls. This is deterministic
fault testing, not physical power-cut qualification.
