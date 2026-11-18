---
title: "Recovery and backups"
---

<!-- Generated from: crates/recovery/PROTOCOL.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Recovery protocol reference, not a complete backup or restore runbook. A verified operator backup procedure is not yet present in the source material.

Protected append-only security recovery (format 3)
================================================

This Linux coordinator publishes protected account/space authority before an auth
request is acknowledged. It returns validated rebuild input, not a rebuilt DB.
The host supplies a pinned installation UUID, trusted ancestry, root locks and
isolation from tenant/same-UID plugins; every writer must use the auth barrier.
Only an empty mode-0700 directory is provisioned. Files are mode 0600, accessed
through pinned no-follow descriptors. Earlier formats are deliberately unsupported.

Format and records
------------------

`snapshot.json` is a full checksummed record map, installation identity, resolved
chain head, last committed transaction/sequence, and the exact active segment name.
One active `log-<uuid>.bin` follows it. Each frame is an eight-byte little-endian
payload length, a 32-byte BLAKE3 checksum, then canonical JSON. The checksum covers
JSON; the chain digest covers the entire frame, including length and checksum.
Every entry carries installation UUID, strictly consecutive sequence and previous
frame hash. The initial chain head is sequence zero and the all-zero digest.

A proposal contains a UUIDv7 transaction and only affected owner keys: full new
records or explicit deletions. A following small resolution marker says committed
or aborted and binds the proposal hash. No second proposal can precede resolution.
SQLite's singleton witness stores proposal sequence/hash **and** the anticipated
resolution sequence/hash (the current resolved chain head).

The existing transactional dirty-owner triggers and indexed capture remain.
Account records contain credentials, enrollment-token hashes, installations and
revocations, server roles, group membership and retry receipts.
Space records contain only space metadata and memberships. Groups, invites and
owner erasure markers have separate records; global policy holds the closed schema/role inventory and unbound
enrollment tokens. Unknown auth tables refuse configuration. AccountId and SpaceId
remain distinct; a snapshot refuses accounts and spaces sharing a UUID.

Last-used/last-seen telemetry, rolling session-expiry touches and rate limits do
not publish. Recovered expiry is the last protected capture's absolute value;
rebuilding must enforce wall-clock expiry, preserve revocation and force all
devices to resync under current authority. Stored schema text is descriptive, never SQL to execute blindly.
The write audit and regression inventory are in the dated review.

Publication and proof
---------------------

1. The auth gate serializes prepare/SQLite commit/completion. Prepare compares the
   writer's witness with a separate read-only connection: an uncommitted witness
   can never resolve its own proposal. Prior committed work is resolved first.
2. Capture only dirty owners; omit identical records. For a nonempty delta, append
   and **fsync the proposal**, then store its identity and deterministic committed
   marker head in the same SQLite transaction as authority and dirty-key removal.
3. Commit SQLite with its inherited WAL/FULL durability. The witness and authority
   commit together. Run session-revocation cache fences immediately after commit,
   even if filesystem completion fails.
4. Using committed DB visibility, append and **fsync the committed marker**, then
   acknowledge. This second fsync is necessary: the DB witness alone proves commit
   only while that DB survives; a lazy marker could lose an acknowledged change
   from standalone recovery input. A failed response may still require receipt retry.

Before proposal durability, SQLite cannot commit. After proposal durability but
before DB commit, standalone reading refuses. After DB commit but before marker
durability, standalone reading still refuses; the original DB witness completes
the exact committed delta. A synced marker therefore never publishes an
uncommitted grant, and no acknowledged change can disappear after a normal crash.
Multiple affected records are one proposal and become current under one marker.

For a rolled-back proposal, writable autocommit replay first durably advances the
DB witness to the deterministic **aborted** head, then appends/syncs that marker.
The records remain unchanged. Both decisions are repeatable after interruption;
abort-only suffix rollback is also detected once its witness advances. A read-only
prepare cannot make this DB update. If prepare fails, the auth hook rolls back
its transaction before reconciling on its original writable connection; it returns
the failure and a later request can proceed. Direct callers must likewise replay
after rollback. Replay/complete/reconcile refuse active transactions.

Torn tails are never accepted by standalone reading. With the original DB, an
exact prefix of the expected resolution marker can be finished and synced. An
incomplete proposal after a fully resolved DB-matching head cannot have committed;
only that incomplete frame may be trimmed and synced. Valid frames are never
rewritten or removed in place. Other corruption preserves evidence and refuses.
Cold replay flushes the selected file and directory before trusting an uncertain
prior write; steady-state replay uses a private read cache checked against pinned
snapshot/segment inode, size and nanosecond mtime/ctime metadata. Explicit readers
always validate all bytes. Concurrent external filesystem/SQL writers are unsupported.

Compaction
----------

After a resolution, a segment at least 64 MiB (configurable) triggers compaction:
create/fsync a new empty UUID-named segment and sync its directory; atomically
write/fsync a full snapshot naming that segment and the chain head it replaces,
then sync placement; only now unlink the old segment and sync deletion. The DB
witness and external checkpoint do not change. Before snapshot publication the
old snapshot/log remain authoritative; afterward the new snapshot/log do. Crashes
can leave an unused segment, but never require selecting a fallback generation.
Admission removes unused segments under root locks after replay succeeds.
A missing snapshot or selected segment refuses, even if obsolete segments remain.
Atomic snapshot-write interiors use the ported fsio durability primitive.

Owner erasure forces the existing compaction path before clearing its protected
pending flag. The old generation can contain previous authority records for the
erased owner. Recovery keeps the minimal erased UUID and key fingerprint,
while the replacement snapshot captures only current authority. The auth WAL
is checkpointed and truncated before the completion marker. Startup retries
cleanup if that marker is still pending.

Integrity and freshness limits
------------------------------

Validation replays snapshot plus selected log and refuses checksum errors,
installation mismatch, gaps, substitutions, reordering, incomplete frames and
unresolved proposals. A DB witness detects truncation/rollback even to a valid old
resolved prefix. Without the DB, deletion of an interior frame breaks the chain,
but deletion of a **whole suffix**, or replacement of the entire image with an old
consistent image, cannot be distinguished from an installation that stopped there.
The optional `Checkpoint` and `validate_and_read_at` API detect this when the host
retains the latest pin independently before acknowledgement. This crate invents
no external storage. An installation UUID alone is not a freshness pin. Checksums
are not MACs: an operator forging an entirely new consistent image is outside the
threat model. Losing the DB with an unresolved proposal remains ambiguous/refused.

Exact costs (excluding SQLite FULL/WAL syncs)
--------------------------------------------

| Successful steady-state change | File fsyncs | Directory fsyncs |
| --- | ---: | ---: |
| Account credential/profile/session revocation | 2 | 0 |
| Space membership/role/transfer/create/delete | 2 | 0 |
| Combined account + space, including a retry receipt | 2 | 0 |
| No-op, receipt hit, telemetry/delivery touch | 0 | 0 |

Compaction adds **2 file + 3 directory fsyncs**, so a triggering change totals
**4 file + 3 directory = 7**. Initial provisioning costs 2 file + 2 directory;
cold clean replay costs 1 file + 1 directory. Fault recovery may repeat syncs,
append a resolution, trim an incomplete frame or retire orphan segments. Ordinary
changes never create files or sync directories. Capture/write work is proportional
to affected owners' current bytes; cached replay does not scan history per change.
Admission/full validation and compaction cost O(current inventory + segment bytes);
memory retains the current record map. A single large delta can exceed the
threshold before compaction. These are counted durability costs, not benchmarks or
physical power-cut qualification.

Startup validation cost
-----------------------

Opening the coordinator validates the snapshot and segment once and retains the
result in its existing private-file stamp cache. Authority reconciliation reuses
that result only when the snapshot/segment stamps still match, resolves pending
work against the original SQLite witness, establishes required durability, and
validates the complete current record map before admitting requests. No authority
check is deferred to first space use. Independent standalone recovery reads still
reread and validate the complete chain. A replaced snapshot or segment between
open and reconciliation invalidates startup reuse and must pass fresh validation.

A pending proposal retains its canonical frame digest with the decoded entry.
Chain linkage, deterministic resolution and application reuse that one digest;
reading a segment no longer serializes and hashes the same proposal repeatedly.
The digest is derived from the same canonical frame function as publication, so
the persisted format and witness values are unchanged. The extra cache is one
64-character digest for the single pending proposal, not a history cache.
