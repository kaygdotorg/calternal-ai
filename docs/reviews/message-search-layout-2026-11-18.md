# Message search layout measurements

Issue #47. These are disposable layout probes built on dev-02 and run on
perf-01 under `/usr/local/bin/perf-lock.sh`. Product message indexing and
client integration await the layout review.

## Layout comparison at 10 million

One disk-cold build per row, using the same 10,000 production-encoded
Markdown thread files and 30 generated fixture terms per record. The new
arms read and parse the thread files, build the index, and checkpoint or
sync it. Index sizes are decimal MB per million messages.

| Layout | Cold seconds | MB/million |
| --- | ---: | ---: |
| Previous 256-position prototype | 1,285.31 | 252.28 |
| Compressed 16,384-position blocks, compact locators, block order, 4 KiB/32 MiB | 345.77 | 121.38 |
| Sorted runs merged into SQLite in key order, 4 KiB/32 MiB | 113.72 | 121.13 |
| Sorted runs, deferred UUID secondary index, 4 KiB/32 MiB | 68.89 | 117.59 |
| Deferred UUID index, 16 KiB/32 MiB | 64.37 | 117.31 |
| Deferred UUID index, 16 KiB/128 MiB | 68.29 | 117.31 |
| Sorted segment file reference | 50.66 | 79.02 |

The block-order arm changes the width, position encoding, and locator
encoding together. The remaining SQLite rows vary the indicated setting.
The new arms create 5,005,312 posting rows and use Elias–Fano to encode
positions. The sorted arms merge bounded in-memory batches through sorted
run files. They spilled 430 MB per 10 million messages in this corpus. One
sample per variant does not resolve the effect of increasing the cache.

The SQLite arms seek by UUID and verify the bounded source byte range. The
fixture resolves compact file ordinals to paths deterministically. A live
layout needs a file identity and revision table as well. The segment
reference uses the fixture's monotonic UUID order for lookup. It would need
a separate directory for arbitrary ID order. It serves as a size and write
cost reference.

Sorted writes and deferred UUID indexing met the five-minute target in this
fixture. The smallest SQLite index was 117.31 MB per million. The segment
reference was the only row below 100 MB per million.

## Scaling by thread shape

The scaling probes use the same SQLite configuration in every row. It has
16,384-position compressed blocks, sorted runs, deferred UUID indexing,
16 KiB pages, and a 128 MiB build cache. This is a fixed comparison setup.
The corpus uses production-encoded byte-counted person records and compact
generated words. All records share a timestamp and use monotonic UUIDs,
so reference checks exercise equal-time order. The common term appears in
half the records. The rare term appears every 8,191 records. The exact
phrase appears every 64 records, with additional non-adjacent occurrences
of the same two words as false-match controls.

The probe verifies the newest 50 and continuation order. Timings cover
the SQLite result and its UUID and timestamp. Phrase timing also includes
locating candidate byte ranges and reading the saved bodies to verify
token adjacency. Warm p95 uses 200 probes on a reused connection. Cold
median and maximum use five OS cache drops and fresh read-only connections,
each opened before timing. The measurements exclude HTTP, policy admission,
late-arrival merging, and HDD-tier service.

Query cells show warm p95 / cold median / cold maximum in milliseconds.

| Messages | Threads | Index GB | Cold build s | Common | Rare | Phrase |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 10M | 100,000 × 100 | 1.173 | 77.74 | 0.132 / 1.23 / 1.44 | 0.128 / 13.97 / 16.10 | 0.484 / 29.91 / 31.70 |
| 10M | 1,436, up to 6,966 each | 1.175 | 54.32 | 0.121 / 1.68 / 2.74 | 0.114 / 14.51 / 16.25 | 0.433 / 28.32 / 59.96 |
| 100M | 100,000 × 1,000 | 11.936 | 775.41 | 0.133 / 2.94 / 5.01 | 0.135 / 19.89 / 28.93 | 0.434 / 21.73 / 49.80 |
| 100M | 14,356, up to 6,966 each | 11.875 | 651.32 | 0.102 / 3.59 / 11.36 | 0.124 / 46.18 / 60.64 | 0.510 / 71.60 / 115.68 |

Cold rare-term and phrase latency exceeded 30 ms in the long 100M shape.
The 100M builds spilled 4.32 GB of sorted-run scratch per shape.

The current thread format caps the entire Markdown at 2 MiB, including
host framing. The compact fixture fits 6,966 records in 2,096,957 bytes.
Payload and provenance sizes change that count. A 100,000-message thread
would require 3.6 MB for the UUID text alone, before body or framing.

The perf volume had about 74 GB free before staging each shape. A
1-billion-message run was excluded by disk preflight. Linear sizing from
the 100-million-message runs puts compact source alone near 301 GB and the
SQLite index near 119 GB, excluding sorted-run scratch, WAL, and a free
space floor. The 10-billion-message request is about 2.52 TB of index with
the original 252 MB/million prototype. The measured SQLite variants imply
roughly 1.2 TB of index at that count, before the source and build reservations.
These are capacity estimates. Query latency was tested through 100 million.

Beyond the measured size, distribute spaces independently, shard a large
space across buildable time partitions, and bound the run-merge fan-in.
Keep a small SSD term and locator directory, and move old sealed partitions
and their bodies to the HDD root. Search the newest partitions first and
skip partitions whose term directories can rule out a match. Cursors need
a global timestamp/UUID continuation across partitions. Cold phrase
verification then needs a separate HDD-tier latency gate.

## Method

The probe sources are retained in the disposable dev-02 build directory
`~/Developer/lanes/search-fast-cross`. It used the pinned x86_64 musl
cross shell and this build command.

```sh
cargo build --locked --offline --release --target x86_64-unknown-linux-musl --target-dir target-perf -j6 -p calternal-server --features rebuild-probe
```

The perf-01 directory is
`~/calternal-ai-perf/search-thr_yruirs3chu/message-layout-review`.
The lock runs `compare.sh` and `scale.sh`. The scripts preserve the raw
reports, binary hashes, per-arm progress, and disk preflight records.
Each build follows `sync` and `sudo -n /usr/local/sbin/perf-drop-caches`.

## Live search WAL

The live writer caps each space's WAL at 64 MiB at the SQLite write
boundary. It also rejects growth through truncate or file-size hints.
Ordinary reads use the platform VFS. At 48 MiB the maintenance worker
attempts a truncated checkpoint before the next write turn. Reader pins
can defer an index update. The affected generation or watermark remains
partial while the worker retries. Notes and journal durability are
unchanged. Reset drops and recreates the FTS tables within its index
transaction to avoid per-row deletion work.

The pinned-reader test generated batches through the actual search FTS
table. With the cap present, SQLite returned disk-full before extending
the WAL past 64 MiB. After the reader released its snapshot, the test
verified truncation, the preserved committed rows, and a successful reset
and write. The uncapped control crossed the limit and failed the test.

The first full gate found a separate issue in the earlier fast read path.
Opening an empty space could create its userdata directory with the
ambient mode. The worker now opens an existing space directory beneath
the userdata root. The focused permissions inventory test and the full
serial rerun passed.

Dev-02 ran `nix develop -c` with `CARGO_TARGET_DIR=target`,
`CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and
`CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal`.

| Command | Result |
| --- | --- |
| `npm ci --no-audit --no-fund` and `npm run build:plugins` | Passed |
| `cargo build --locked -j6 -p calternal-server -p calternal-cli -p calternal-perf --features test-plugins` | Passed |
| `cargo test --locked --workspace --all-features --no-fail-fast -j6 -- --skip pool_machines_serve_only_granted_accounts` | 599 passed. The Podman case stays with dev-01. |
| `cargo test --locked -j6 -p calternal-client --all-features --test real_server real_server_operations_receipts_and_authority -- --ignored` | 1 passed |
| `cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 116 passed |
| `npm run test:integration` and `npm run test:plugins` | Passed, with the signed-in plugin case skipped |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` and `python3 contract/verify-cli.py` | Passed |
| `npm run docs:site`, `sh scripts/check-docs.sh`, `cargo fmt --all -- --check` | Passed |

## Independent thread title lookup

The orchestrator does not opt threads into the full-text projection.
`threads.lookup` now uses the existing content-head title index after the
normal space and package activation checks. It reads committed titles and
keeps stable cross-space cursors, without loading transcript text. The
conformance regression failed on the previous build, then passed with
private-space hiding, case-insensitive ASCII matching, paging, rename,
delete and package-disable assertions. The full dev-02 command set above
passed against the title change after merging `45fa2c0`. It recorded 600
workspace and 118 serial conformance tests.

## Review gate

1. Space identity, journal positions, and caller-safe errors are shared.
2. The VFS uses rusqlite's bundled SQLite support and adds no dependency.
3. One process-lived VFS registration and per-space writers stay outside
   foreground admission.
4. File inspection, writes, and checkpoints run on the background thread.
5. The worker adds a WAL size inspection per write turn. Foreground reads,
   parses, SQL statements, and flush counts are unchanged.
6. The registration is cached for the process lifetime.
7. Live WAL growth is limited to 64 MiB per space, including under a pin.
8. The patch keeps the current derived index schema and lazy rebuild.
9. WAL paths are derived from the existing space UUID.
10. No operation, capability, or role declaration changed.
11. The live writer directly uses the cap.
12. The platform VFS still owns file handling, sync, and locking.
13. The docs gate passed and the worker documents checkpoint behavior.
14. The guide and generated docs describe the shipped limit and retries.
15. Formatting passed and the WAL adapter and test have their own modules.

## Simplification opportunities

Of the measured SQLite settings, the larger cache added no size benefit
in this fixture. Keep the side-by-side upgrade design parked. When it
returns, measure the message candidate's peak WAL under catch-up traffic
and use that measurement for the disk reservation.
