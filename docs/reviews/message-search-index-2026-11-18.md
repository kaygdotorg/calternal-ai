# Message index footprint

[Forgejo issue #47](https://git.kayg.org/kayg/calternal-ai/issues/47).
The UUID index already starts with the thread slot. Bundled SQLite
uses the covering `locations_id(thread,id)` prefix for
`DELETE FROM locations WHERE thread=? RETURNING position` after
removing `locations_thread`. Edits and deletes keep the same SQL
and update postings even when a block contains several threads.

Generation 2 rebuilds from current authoritative thread revisions, using
the existing worker. A single SQLite transaction replaces the old schema
objects and commits the new unpublished state. Search reports partial
coverage until the new generation catches up. The SQLite schema version
and authoritative formats are unchanged.

## Page accounting

The previous [layout probe](message-search-layout-2026-11-18.md) reproduced
1,173,078,016 bytes for 10M messages. This table compares that replay with
both fresh wired arms at 10M. Measurements are decimal MB per million
messages. Rows are rounded independently.

| SQLite object | Layout probe | Wired before | Wired after | Before less probe |
| --- | ---: | ---: | ---: | ---: |
| Primary postings | 51.33 | 65.86 | 65.86 | +14.53 |
| Message locator rows | 40.79 | 41.79 | 41.79 | +1.00 |
| UUID index | 25.19 | 28.23 | 28.23 | +3.04 |
| Chronological postings index | 0 | 16.35 | 16.35 | +16.35 |
| Thread-location index | 0 | 15.37 | 0 | +15.37 |
| Dictionary, thread rows, block bounds and SQLite bookkeeping | <0.01 | 0.18 | 0.18 | +0.17 |
| **Total** | **117.31** | **167.77** | **152.40** | **+50.46** |

The primary posting increase is 12.92 MB of record payload, 1.57 MB of
unused page space and 0.05 MB of page and cell overhead per million.
A posting's newest timestamp and UUID take 22 bytes per wired row, or
11.29 MB per million messages. The remaining 1.63 MB of posting payload
includes 127,638 more term/block rows in the wired corpus and changes
in encoded positions. The chronological index seeks by term and newest
key across overlapping timestamp ranges. The probe assumed chronological
ingest and could use block order for that lookup.

The locator stores the source UTC offset. The offset's SQLite record
type costs 1 MB per million in this UTC fixture. The UUID index adds thread
scope, which accounts for 3.01 MB of payload and 0.03 MB of page and cell
overhead. The thread table stores identity and revision once per thread.
Together with its index, the term dictionary, block bounds and state, it
accounts for 0.17 MB of the difference.

Every retained SQLite object occupied exactly the same number of bytes
in the two wired arms at both scales. The 10M file shrank by **153,665,536
bytes** on a fresh build. A 1M in-place rebuild of a populated generation-1
file took **16.31 s**, returned partial coverage on the first call, and
dropped the old index. The file stayed **165,855,232 bytes**, with
**14,876,672 bytes** on SQLite's reusable freelist.

## Locked HTTP measurements

Both release servers were built from the same source base on dev-01
using the pinned x86_64 musl shell. The candidate applies the generation
change and drops the redundant secondary index. Perf-01 ran each arm
under the perf lock on fresh fixtures with 1,000 framed messages per
thread. Warm p95 uses 200 authenticated requests per query. Cold phrase
uses five OS cache drops and reports the highest sample. Rebuild starts
with the first search on a stopped, seeded fixture and includes readiness
polling. The driver also checks result ordering, continuation and
opening the exact message by ID.

| Metric | 1M before | 1M after | 10M before | 10M after | Accepted budget |
| --- | ---: | ---: | ---: | ---: | ---: |
| Index MB per million | 165.86 | 150.98 | 167.77 | 152.40 | — |
| Cold first-use rebuild, s | 16.81 | 15.29 | 170.73 | 163.76 | — |
| Common warm p95, ms | 4.48 | 4.55 | 5.96 | 5.91 | 25 ms |
| Rare warm p95, ms | 7.54 | 6.57 | 7.64 | 7.90 | 25 ms |
| Phrase warm p95, ms | 4.74 | 4.03 | 5.76 | 6.84 | 25 ms |
| Phrase cold p95, ms | 29.49 | 27.26 | 83.04 | 66.01 | 150 ms |

All eight candidate latency checks in [perf/budgets.json](../../perf/budgets.json)
passed. The measurements use one build per arm.

## Verification

Dev-01 reproduced the redundant-index plan on the baseline and passed
three focused message search tests on the candidate. Serial HTTP
conformance covered restart with an existing generation-1 layout and
subsequent edits, deletion, private space filtering, and exact opens. The
search-only Rust workspace ran 638 passed and 2 ignored. Its serial HTTP
conformance ran 133 passed.

## Simplification opportunities

The generation reset now lets SQLite drop indexes with their table.
The chronological access path is the largest remaining secondary index.
Any change to its layout needs a new comparison on overlapping timestamp
ranges because that path lets warm queries stop after the newest matches.
