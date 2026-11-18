# Row-level security receipt deltas

Design for review · November 18, 2026 · [Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52)

A changed account currently puts its entire `space_receipts` set in the next
security-log proposal. On the fresh 1,000-space fixture, repeated receipt rows
occupy 60.48 MB of the 61.72 MB active segment across 254 frames. The log
must retain exact replies for valid retries.

## Capture and apply

Give `space_receipts` one keyed map in the verified image. Keep its schema
in the global record. New proposals carry only the receipt keys affected by
the authority transaction, alongside the existing owner changes. Use the
SQLite primary key `(actor, scope, request_id)` exactly. Each change carries
the committed operation, fingerprint, reply text and timestamp, or indicates
deletion. Capture keys in the existing write transaction using an internal
dirty-key table and triggers for inserts, updates and deletes. A key-changing
update records both keys. Savepoint and transaction rollbacks discard their
dirty changes. Capture final values for the affected keys, omit exact no-ops
and serialize once in deterministic key order. Ordinary owner records no
longer scan or serialize their receipt rows.

Apply owner and receipt changes together only when the matching proposal
resolves as committed. Refuse duplicate keys, invalid fields or bindings and
changes inconsistent with the resulting authority image. Rebuild the existing
SQLite table from the verified map, including original reply bytes. A valid
duplicate returns the original reply, a changed fingerprint conflicts, and a
replay does not extend retention. Expiry transaction deletes become key
deletions. Recovery follows the recorded transaction order and never expires
a receipt using the recovery host's wall clock. Normal authority expiry
resumes after recovery.

## Recovery and retention

Keep the existing proposal sync, SQLite transaction and witness, and
resolution sync order. The SQLite witness binds the new proposal digest
and resolution as part of the same authority transaction. A surviving
original database resolves an interrupted proposal using its witness. An
authority rebuild from the log requires the exact independent installation
and chain checkpoint. An older valid prefix, unresolved proposal or witness
mismatch refuses admission. Aborted proposals contribute no receipt changes.
Handle torn writes under the existing proof: trim only a proven uncommitted
partial proposal and repair a partial resolution only from its matching
witness. Refuse a corrupt complete frame. Use a distinct format version.
Existing roots need a separately reviewed offline conversion that verifies
the old checkpoint and binds the new image to it before admission.

Account and space erasure capture the actual deletions by actor or scope,
including receipts held by other actors for the erased space. Keep the
existing forced erasure compaction. Write and sync the new snapshot and
segment before retiring the previous generation under the existing protocol.
Verify erased rows are absent from both live state and the retained local
chain. Ordinary compaction still folds live rows into one snapshot at the
64 MiB threshold. Small deltas mean more frames can accumulate before the
same byte threshold. Measure that long-running case before changing it.

## Expected effect and acceptance

With one new receipt per proposal and roughly 0.5 KB per row, the measured
fixture's active segment models at about **1.3 MB**, including the existing
1.24 MB of other segment bytes. The snapshot remains on the order of
1.8 MB. Removing roughly 230–250 ms of frame work from the paired-prototype
control models readiness around **205–225 ms warm** and **1.04–1.19 s
disk-cold**, with the cold range allowing for saved segment reads. These
are estimates for that fixture. Cold startup has other measured costs.

Before adoption, compare rebuilt authority and duplicate replies byte for
byte with the current format. Fault-test every proposal, SQLite witness,
resolution and compaction boundary. Cover expiry followed by reuse,
cross-actor erasure, stale external pins, and logs close to the byte
threshold. Re-measure frame count, allocation, segment bytes, save overhead
and readiness. Review this design before implementation.
