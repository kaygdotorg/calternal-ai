# R1 storage and format lane

## Integration ownership

- S11 normalizes the host-supplied evaluation context at the format boundary.
  The declarations owner can also set nanoseconds to zero in
  `declarations::clock` before formatting RFC 3339. Ledger timestamps keep
  millisecond precision.

## Append evidence

B3 commits one pending row to the private SQLite WAL with FULL
synchronization before appending to the item. The row contains the exact
encoded bytes, next head, record, actor and original receipt hashes. Once the
item is synced, one WAL NORMAL transaction advances the ledger and deletes the
row. WAL autocheckpointing is disabled for this second transaction. If a power
loss discards it, the FULL commit and synced item can replay it. The steady
path issues one WAL sync and one item sync, with SQLite checkpoints adding
periodic syncs. The evidence costs a second SQLite transaction plus one copy
of the encoded record (up to the existing write bound) in the WAL, as well as
the small JSON envelope and WAL page overhead.

Recovery verifies the ledger prefix and existing record hashes before it
truncates an exact partial prefix of the prepared bytes. Nonmatching bytes
remain outside. A complete prepared record followed by outside bytes advances
the ledger only across the prepared record. The host needs exclusive write
access while open. An offline edit that is byte-for-byte identical to the
interrupted prefix is indistinguishable from the interruption. Where evidence
does not match, the stricter choice is to preserve the bytes.

Morning question: should the operator support offline hand-editing of a space
immediately after a host crash, before recovery? Running recovery first avoids
ambiguity about matching prefixes at the cost of an explicit recovery step.
The lane assumes the operator completes recovery before making offline edits.
