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

S8 borrows the growing reload catalog as a slice and indexes it after
validation. The live install path still clones the existing map into a slice.
To make both paths borrow their catalog directly, the declarations and format
owners can change `declarations::parse_plugin` and
`format::InstallOptions::installed` to accept a slice of borrowed definitions
or a borrowed keyed catalog. New SQLite ledgers use an integer primary key
for order. Existing add-only ledgers keep their recorded SQLite row order.

S1 pins the operator-selected base for opening a space and walks both space
roots through no-follow directory descriptors. The host configures an
operator-owned base and runs with exclusive write access to its storage.
Other processes using the host Unix identity are trusted to stay outside the
tree while it is open. The private SQLite ledger requires a stable pathname.
The operator must keep the protected path's ancestors stable during service.

## Per-space retention and residency

S4 maintains receipts, recovered receipts and Trash by their time indexes,
in batches of 64 per class, on the existing storage maintenance path. It
also uses a ledger counter for a combined 16,384-receipt maximum. New writes
can allocate at most 10,000 item and Trash entries combined. The installer
caps loaded bundles at 32 per space. Existing per-item and per-bundle byte
limits continue to bound the size of each entry. The caps reject new entries
and allow operators to remove or expire existing entries.

Integration needs a host-owned timer and slot lifecycle in
`crates/host/src/lib.rs`. That file is outside this lane's allowlist. The
timer should call `Space::expire_trash` on its blocking maintenance path, even
for spaces without new calls. It should sweep dormant ledgers by UUID in
bounded rounds (without loading plugin catalogs or rebuilding heads). For
open spaces, retain active holders and WebSocket subscribers, then remove
idle slots once both have drained. Coordinate the subscriber check through
the live lane's `Hub`. A registry slot must stay discoverable until the same
critical section proves that only the registry holds its Arc, to avoid a
second space lock while a request still holds the old one.

Morning question: are 10,000 item/Trash entries, 32 installs and 16,384
receipts acceptable for the preview? For example, bulk importing a larger
library would require higher aggregate limits and a storage budget. Keep
these ceilings until the operator reviews observed disk and memory use.

## Format work limits

S6 allows 256 KiB and 4,096 physical lines of frontmatter. Definitions
allow 2,048 logical entries, including definitions with continuations.
Item YAML allows 4,096 aggregate mapping and list entries and 256 bytes
per structural key after decoding. Literal Markdown body text stays intact.
Formula evaluation charges one step per 64 bytes of cloned fields or
searched text, in addition to collection entries and formula nodes.
