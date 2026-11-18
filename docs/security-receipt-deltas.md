# Row-level security receipt deltas

Revised for owner review · November 18, 2026 · [Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52)

An account change currently serializes its entire `space_receipts` set into
the security log. Those repeats account for 60.48 MB of the 61.72 MB active
segment in the 1,000-space profile. Test a smaller compaction threshold on
the current format before choosing a format change.

## Capture and replay

Keep the table schema in the global record and one keyed map of live receipts
in the verified image. Use the exact tuple `(actor UUID, scope UUID, request
ID text)` in the encoding. Each component occupies its own field. The
request ID remains opaque, including punctuation and non-ASCII text. Reject
duplicate tuples and use a deterministic order.

A proposal contains the affected receipt tuples and the existing owner
changes. For an insert or replacement, retain the exact operation,
fingerprint, reply text and timestamp. For a deletion, record the tuple
with a delete marker. Track affected keys in the same SQLite transaction
with triggers for inserts, updates and deletes. A key change records both
keys, savepoint rollback discards its keys, and final comparison omits exact
no-ops. On first attachment, seed every preexisting receipt in the initial
witnessed publication, including receipts whose owner has no other change.

Apply receipt and owner changes together when the matching proposal resolves
as committed. Preserve the SQLite actor foreign key and validate field types.
A receipt's scope may name a deleted space. Space deletion clears prior
receipts and records its own reply under that space in the same transaction.
After a rebuild, a duplicate delete returns that exact reply. Different
operation or fingerprint conflicts. A replay preserves the original age.

## Detection, recovery and erasure

Before admission on startup, compare the resolved logical image with a full
capture of the surviving original authority database after verifying its
witness. Compare normalized authority rows and all receipt tuples and values.
If both integrity checks pass, publish a corrective witnessed delta for
missing, stale or extra receipt rows before serving requests. Fail startup on
an untrusted witness or repair error. This restores the self-healing formerly
provided by recapturing a dirty owner's whole record. Include the full
comparison in the startup measurement.

Keep the proposal fsync, SQLite transaction and witness, and resolution
fsync order. The original database resolves an interrupted proposal. Trim a
partial proposal only when its witness proves it uncommitted. Repair a torn
resolution only from the matching witness. Abort applies no row changes.
Reject corrupt complete frames and unresolved proposals without their
original database. The host has no wired independent chain pin for a rebuild
from a resolved log alone, so this design makes no external freshness claim.

Capture erasure deletes by actor and scope, including cross-actor receipts
for erased spaces. Keep forced erasure compaction and retire the old local
chain only after the replacement snapshot and segment are durable. Verify
erased bytes are absent from every retained local recovery file.

## Expiry, compaction and review gate

Keep the byte threshold for the new-format comparison. While the server is
available, sweep eligible expired receipt rows hourly and record those deletes
through the same authority transaction. Compact within 24 hours of the first
expired reply deletion still represented in a retained image or segment.
This proposes a local physical retention bound of configured receipt age
plus **25 hours** with healthy storage while running. After downtime, run
overdue expiry and compaction before admission. Erasure still compacts
immediately. Fault or storage failures surface as incomplete cleanup.

The 1,000-space model assumes approximately one 0.5 KB receipt per proposal
plus 1.24 MB of other bytes, yielding about **1.3 MB** active and a roughly
1.8 MB snapshot. Removing repeated frame work put the initial warm readiness
estimate at **205–225 ms**. That estimate precedes the new image comparison.
Measure warm readiness again with the comparison, expiry and age maintenance.
Choose the new format version with the upgrade lane through the owner.

Required tests compare image and full database capture after every authority
operation type, including first attach with existing receipts, expiry then
reuse then rebuild, and a retry of space deletion after rebuild. Exercise
every existing recovery `Step` with delete-only and mixed proposals. Check
account erasure across other actors' receipts and scan retained files for the
erased bytes. Measure time per frame, startup and save overhead at steady
state. Format implementation waits for owner confirmation of this revision.

## Smaller-threshold experiment

On perf-02, 8 MiB reduced median warm readiness from 327.93 to 120.29 ms
over seven paired starts at 1,000 spaces. The active segment fell from
61.09 to 6.68 MB. Seeding crossed the compaction threshold 30 times with
8 MiB and three times with 64 MiB. This is a simpler candidate for warm
startup. Measure its authority-write tail on disk before adopting it.
It leaves the repeated receipt capture and idle expiry questions open.
[Compaction experiment](reviews/security-compaction-2026-11-18.md)
