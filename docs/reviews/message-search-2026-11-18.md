# Message search implementation and gate

Issue #47 approved the SQLite layout with compressed 16,384-position blocks,
sorted writes, a deferred UUID index, 16 KiB pages, and a 32 MiB writer cache.
The production index also records time bounds for blocks with out-of-order
timestamps, the exact thread revision and byte extent, and secondary indexes
for removing old rows. The earlier disposable postings and locator probe
measured 117.31 MB per million and 64.37 s cold at 10 million.

## Locked measurements

The release server and perf driver were built on dev-02 in its pinned
x86_64 musl cross shell. On perf-01, under
`/usr/local/bin/perf-lock.sh`, the driver seeded distinct files with 1,000
production-framed synthetic messages per thread. Every message had the
same saved instant and a distinct UUID. The common token appears in half
of messages. The rare token appears every 8,191 messages. The exact phrase
appears every 64 messages, alongside non-adjacent token decoys. The driver
asserted equal-time order, cursor continuation and opening the exact message
through authenticated HTTP.

| Messages | Common warm p95 | Rare warm p95 | Phrase warm p95 | Phrase cold p95 | Exact open p95 | Continuation | Cold first use | Index MB/million |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1M | 4.88 ms | 8.11 ms | 5.43 ms | 30.69 ms | 1.95 ms | 4.24 ms | 20.82 s | 165.86 |
| 10M | 6.35 ms | 8.47 ms | 5.65 ms | 71.76 ms | 1.77 ms | 5.19 ms | 166.53 s | 167.77 |

Warm p95 uses 200 samples. Cold phrase uses five OS cache drops with the
server process still running, so its nearest-rank p95 is the maximum of five.
Rebuild time starts at the first
search after starting the stopped fixture and includes readiness polling.
The gate thresholds are 25 ms warm and 150 ms cold phrase. The measurements
include authentication, read admission, index lookup, revision verification,
highlight extraction, and the response body. The storage measurement uses
the SQLite index file after build publication. The wired index is 1.43 times
the layout probe's size per million at 10M. Its rebuild takes 166.53 s,
within the earlier five-minute target. SQLite page accounting at 10M showed
659 MB of postings, 418 MB of locators, 282 MB for the UUID index, 163 MB
for the chronological index, and 154 MB for the thread-location index.
The newest-first query uses the term range of the chronological index and
one primary-key block lookup. SQLite's query plan uses no temporary sort.

During a cold phrase read, the search dialog keeps its previous results
until the request completes. With an empty list it currently displays
“No message matches.” While a generation rebuilds, the API sends a partial
page. The web adapter still drops `partial`, `message_id`, and `message_at`.
Adopting those fields and the bounded open request belongs to the web lane.

```sh
cargo build --locked --offline --release --target x86_64-unknown-linux-musl \
  --target-dir target-perf -j6 -p calternal-server -p calternal-perf --features test-plugins

/usr/local/bin/perf-lock.sh bin/calternal-perf --bin "$PWD/bin/calternal" \
  --baseline "$PWD/bin/calternal" --baseline-commit same-binary --commit candidate \
  --work "$PWD/run-10000000" --plugins "$PWD/plugins" --message-search-only \
  --sizes 10000000 --samples 200 --cold-pairs 5 --json
```

Raw measurements and release binary hashes live in the disposable perf-01
lane directory `message-search-final`. The perf binary was built with the
final layout before the fast-forward from the previous dev base to 251303e.
The first attempt revealed a short
SQLite lock during unpublished bulk build. Reads now convert that state to a
partial page. A superseded variant also rewrote existing chronological B-tree
pages during the scan. It had produced 570 MB of its unfinished 10M index at
the 302-second cutoff. The final variant writes primary posting rows in
block and term order, then builds the chronological index once with SQLite's
sorted secondary-index build.

## Backend checks

Dev-02 used `nix develop -c` on the merged 251303e base with
`CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal`. The original HTTP
regression failed on the previous server at the missing message hit.
The extended case verifies hiding unreadable spaces, opening a hit by
ID, exclusive continuation, relocation after a revision changes its
byte offsets, deletion, and package disable. Client integration remains with
the web lanes. Mixed ranking keeps title and item body matches before
messages. A parallel workspace run before the fast-forward
timed out in the provider-readiness test waiting for `interrupted` and saw
`null`. The focused test and the pre-merge workspace rerun both passed. The later
fast-forward to a7cd5dc changed only `docs/open-questions.md`.

| Command | Result |
| --- | --- |
| `npm ci --no-audit --no-fund` and `npm run build:plugins` | Passed |
| `cargo build --locked -j6 -p calternal-server -p calternal-cli -p calternal-perf --features test-plugins` | Passed |
| `cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 124 passed |
| `cargo test --locked --workspace --all-features --no-fail-fast -j6 -- --skip pool_machines_serve_only_granted_accounts` | 604 passed and 1 ignored installed-client case. Podman case stays with dev-01. |
| `cargo test --locked -j6 -p calternal-client --all-features --test real_server real_server_operations_receipts_and_authority -- --ignored` | 1 passed |
| `npm run test:integration` and `npm run test:plugins` | Passed. The signed-in plugin case skipped. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` and `python3 contract/verify-cli.py` | Passed |
| `npm run docs:site`, `sh scripts/check-docs.sh`, and workspace and conformance `cargo fmt --check` | Passed |

## Review gate

1. Message and space identities use existing UUID representations and the
   public caller-safe error family.
2. A host-owned decoder callback points into the content crate without an
   upward dependency. SQLite uses rusqlite.
3. Request reads use per-space handles. The existing background scheduler
   owns projection updates and the journal.
4. Dispatch uses existing blocking admission for SQLite and file reads.
5. There is no search I/O on the save acknowledgment path. A source
   revision is read and decoded once per rebuild or replay turn. Exact open
   uses the locator and current-version seeks, the outbox visibility check,
   and one bounded BLOB read. Search opens the message index only when the
   same content snapshot permits threads.
6. The manifest projection is cached on the immutable catalog. Prepared
   statements are reused within each connection.
7. Live message WAL growth is capped at 64 MiB. The worker reserves
   a 1 GiB disk floor, separately budgets rollback and WAL space, and
   leaves the watermark unchanged when it cannot grow. Coverage and query
   work limits are documented.
8. The message index is derived and generation-bound. Startup uses the
   current schema. Search messages are regenerated from current heads.
9. The file belongs to one space UUID. Filenames can add an interval key
   using the same record format.
10. Thread opt-in and targeted access are declared in the orchestrator
    manifest. The existing thread read serves exact message open.
11. The server configures the journal worker and its decoder. The CLI and
    performance driver exercise the public operations.
12. The worker reuses the durable projection journal, guarded reads and
    hash checks, thread decoder and capped SQLite VFS. Exact revision
    slices use the content SQLite BLOB primitive.
13. The docs gate covers the new public types and file-system helper.
14. The README, content guide, generated contract and docs site describe
    the wire fields, cursor, command, and partial responses.
15. Formatting runs on both manifests. The writer, reader, codec and
    source modules separate rebuild, query, encoding and file access.

## Simplification opportunities

Measure whether the UUID index's thread prefix can also serve thread
replacement and save the dedicated index's 15.4 MB per million. Compare
keeping unchanged postings when a revision only appends or changes file
framing. Either change needs a workload with late timestamps and repeated
thread mutations. The parked upgrade design still needs a measured candidate
WAL peak under catch-up.
