# Notes search worker and online rebuild

Issue #47. The rebased independent lookup and palette commit is
`70d81a5a07b4f77f758c3b76d5639bda048d4532`. The operation schema remains
`search.query` with input `query`, optional `space_id`, `kinds`, `limit`
and `cursor`, and result `hits`, optional `next_cursor`, and `partial`.
The Notes projection is the first runtime producer. Message indexing follows
the byte-counted parser on issue #46.

## Save gate and perf VM noise

Use the production passkey driver with identical stopped roots, one binary,
an alternating on/off pairing and the same manifest bundle apart from the
search opt-in. Wait for file placement after every acknowledged save and
before sending the next update to that note. Time the HTTP acknowledgment
before the placement wait. Run a burst workload separately if consecutive
saves are the user scenario. That workload can put the prior file
placement on the next save's acknowledgment path.

The previous perf-01 identical-binary control at 100 spaces × 100 notes
spanned 26.2–164.6 ms warm. Reads, draft writes, and placement also slowed
in the outlying arm. The 100,000-note historical artifact reached 56.2 ms
against 33.0 ms in its identical-binary arm. These observations are
consistent with shared host resource contention, including storage.
They do not identify a process or establish a disk cause. The lock uses
two advisory files to coordinate cooperating build and perf jobs.
Kernel writeback, ordinary services, the hypervisor, and backing storage
can still affect timing. The available VM timer records did not identify
a matching scheduled task. Historical per-second disk or process data
were unavailable. The follow-up collects `vmstat` and `iostat` under its
lock for correlation.

## Work and durability

The acknowledged save retains its existing content, journal, and outbox
durability boundaries. Search adds zero save-path SQL statements, filesystem
reads, parses, or flushes. A separate bounded worker reads the durable
journal's epoch and position and uses the stored title and staged text.
It commits projection mutations with their watermark. It reads files only
during reconstruction and prefers materialized bytes whose hash matches
the durable head. A pending or changed file falls back to the durable
version. Delete, linked rename, and restore reach the projection
through their item journal changes.

A release, schema generation, kind configuration, or journal-epoch change
invalidates the index. One 128-head turn at a time runs outside foreground
admission, with four warm writer connections. A new generation stays hidden
until the snapshot and journal catch-up complete. List, ID reads and saves
use retained content heads meanwhile. Search reports `partial` for repair,
lag or capped coverage. Healthy lag has a one-second target. This applies
to the search projection. Other derived indexes still use their current
lifecycle.

## Paired matrix on the search-worker build

The initial qualification used on/off manifests with an identical
`67254f0a7420232c3e1b59756f810afacee334afa37986f8c7b99173f3f9b040` server
on perf-01. The driver seeded the shared fixture with indexing off. One
OS-cold pair and three warm pairs ran at each size, 100 updates per arm
with the placement wait. All 32 arms completed. The matrix exited 1 for
retained catalog budget breaches. Indexing-on save p95 stayed under 40 ms
in all 16 enabled arms.

| Fixture | On p95 range, ms | Off p95 range, ms | Median paired on minus off, ms |
| --- | ---: | ---: | ---: |
| 1 × 1,000 | 27.53–30.75 | 26.44–27.86 | +1.06 |
| 1 × 10,000 | 27.36–32.10 | 29.02–32.54 | −1.69 |
| 1 × 100,000 | 29.57–36.08 | 32.63–40.84 | −2.00 |
| 100 × 100 | 26.62–30.72 | 29.12–33.78 | +0.11 |

The measured directions vary by fixture. The paired runs found no
reproducible save regression. They used no predefined equivalence margin.

| Fixture | First cold list, ms | Search ready, cold / warm | List and ID read during rebuild, ms | Peak RSS, MiB | Maximum ack-to-index, ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 × 1,000 | 28.83 | 219 ms / 97–115 ms | 1.43–1.61 / 0.86–1.42 | 37.06 | 27.10 |
| 1 × 10,000 | 46.81 | 2.03 s / 1.31–1.37 s | 1.52–2.21 / 0.89–1.45 | 45.75 | 30.58 |
| 1 × 100,000 | 304.89 | 79.86 s / 66.39–67.57 s | 1.60–2.32 / 0.85–1.78 | 58.47 | 42.82 |
| 100 × 100, first selected space | 64.02 | 143 ms / 94–121 ms | 1.43–4.36 / 0.83–2.55 | 36.25 | 28.02 |

The readiness clock starts after the initial Notes list. It includes the
foreground operations and search polling until publication. The 100,000-note
save made during rebuild took 92.2–101.6 ms on and 91.6–152.7 ms off. These
are first-use samples. Steady-state p95 uses the separate 100-save loop.
The 100,000-note cold first list was 338.3 ms with indexing off. Search
remains visibly delayed during reconstruction at that size.

| Rebuilds pending together | Healthy list, ms | Healthy ID open, ms | Healthy saves, ms | Time to search ready | Peak RSS, MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 1.36–2.19 | 0.58–0.86 | 22.31–31.70 | — | 44.95 |
| 1 × 10,000 notes | 1.43–2.44 | 0.72–1.09 | 26.81–67.36 | 1.58 s | 51.32 |
| 2 × 10,000 notes | 1.41–2.16 | 0.62–0.92 | 23.06–36.66 | 2.75 s each | 54.76 |

Each queue wave collected 12 request sets in a third space. The two
rebuilds were both pending for all 12 probes and shared one bounded
maintenance thread. Requests used their ordinary foreground admission.

## Package-store repeat

The integrated repeat uses the same placement-spaced driver and verifies
the loaded opt-in before each arm. Both perf-01 runs are under
`~/calternal-ai-perf/search-thr_yruirs3chu/`. The initial run is in
`notes-stage-two-qualified`. The integrated run is in `notes-stage-two-dx`.
The integrated server SHA-256 is
`f3c1d7709f09a1856a1570fefea6f208f81bd5946b19aa1a63eb6a9a28d14741`
and the driver SHA-256 is
`81d61a64bf674807a851b08a647db4fd5ac3750b81d2267743fad723d78fcdc5`.

| 100,000-note pair | On save p95, ms | Off save p95, ms | On minus off, ms |
| --- | ---: | ---: | ---: |
| Disk cold, off first | 31.67 | 30.15 | +1.52 |
| Warm, on first | 31.38 | 33.47 | −2.09 |
| Warm, off first | 29.91 | 30.41 | −0.50 |
| Warm, on first | 29.37 | 36.06 | −6.70 |

The median paired change was −1.30 ms. All eight arms completed and met
the 40 ms steady-state save budget. The matrix exited 1 for the feed
proxy and first-cold-open budgets in both arms.

At 100,000 notes, the first cold Notes list took 311 ms with the rebuild
enabled and 326 ms with it disabled. Once the first list returned, search
published at 79.85 s cold and 66.49–67.76 s warm. During reconstruction,
repeat lists took 1.62–1.89 ms, ID opens took 0.97–1.83 ms, and the first
save took 91.80–104.30 ms. The corresponding first saves with indexing
disabled took 88.00–101.49 ms. Peak server RSS with rebuild enabled was
59.86 MiB. The maximum steady-state acknowledgment-to-index sample was
38.82 ms.

| Pending rebuilds | Healthy list, ms | Healthy ID open, ms | Healthy saves, ms | Search ready | Peak RSS, MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 | 1.32–1.77 | 0.61–0.99 | 21.99–29.02 | — | 46.70 |
| 1 × 10,000 notes | 1.40–2.27 | 0.66–1.11 | 22.73–89.02 | 1.45 s | 50.90 |
| 2 × 10,000 notes | 1.37–2.02 | 0.61–0.99 | 24.13–29.40 | 2.71 s each | 58.03 |

Each wave had 12 request sets. One save reached 89.02 ms in the
single-pending wave. Both rebuilds had pending work for all 12 probes in
the two-pending wave. Those measurements qualify this level of overlap.

The new I/O logs include storage activity under the lock. They do not
identify the owner of block-level I/O or attribute the older p95 outliers.

## Remaining work

Search remains visibly incomplete for over a minute after the first open
at 100,000 notes. The Notes path can already serve foreground operations
during that window. Making search usable on the first visit requires a
measured reduction in file and projection rebuild work, or preparing
and catching up a new generation before switching releases. The first
option preserves lazy rebuild but needs a cold-file throughput gate.
The second increases disk and upgrade orchestration cost and needs an
owner decision on when to build. These measurements cover the Notes search
projection. Tags and thread lists still need online rebuild and
qualification. The [initial combined measurement](search-2026-11-18.md#first-use-rebuild-performance)
covers the earlier blocking lifecycle.

Message ingestion and one-read message open await issue #46. The
[standalone postings benchmark](https://git.kayg.org/kayg/calternal-ai/src/commit/1f1a8f9432d7dd0615c3ae97f494f73d6ef27924/docs/reviews/search-postings-2026-11-18.md)
qualified 10 million synthetic messages at 0.040 ms warm p95 for the
newest 50 hits, about 2 ms cold and 169 MB per million messages. Before
allocating a new generation, preflight free space for the entire new index,
peak WAL and temporary bytes, and the configured free-space floor.
Capacity planning also accounts for any live generation. The 100-million
case remains unmeasured.

Notes retains its existing title-first ranking. Ranking choices, recents,
frecency, and typed filter tokens are recorded with options in
[`docs/search.md`](../search.md#pending-product-decisions).

## Commands

On dev-02 the release bundle used the pinned musl cross shell and
`cargo build --locked --release --target x86_64-unknown-linux-musl
--target-dir target-perf -j6 -p calternal-server -p calternal-perf`.
On perf-01 the launch script ran these commands under one outer
`/usr/local/bin/perf-lock.sh`. The off bundle differs only by removal of
the Notes manifest's `searchable` opt-in. The integrated package-store driver
resets only the disposable copied package catalog before each arm seeds it.

```sh
./bin/calternal-perf --paired-search --bin "$PWD/bin/calternal" --baseline "$PWD/bin/calternal" \
  --plugins "$PWD/plugins" --baseline-plugins "$PWD/plugins-off" --budgets "$PWD/budgets.json" \
  --sizes 1000,10000,100000 --spaces 1,100 --pairs 3 --cold-pairs 1 --samples 100 \
  --commit notes-projection-on --baseline-commit notes-projection-off --work "$PWD/matrix" --json
./bin/calternal-perf --paired-search --search-queue --bin "$PWD/bin/calternal" --baseline "$PWD/bin/calternal" \
  --plugins "$PWD/plugins" --baseline-plugins "$PWD/plugins-off" --work "$PWD/queue" \
  --commit notes-projection-on --baseline-commit notes-projection-off --json
```

For the package-store repeat the matrix flags were `--sizes 100000
--spaces 1 --pairs 3 --cold-pairs 1 --samples 100`. The queue command
was unchanged.

Dev-02 gates on the package-store integration ran through `nix develop -c`:

| Command | Search worker result |
| --- | --- |
| `npm ci` and `npm run build:plugins` | Passed |
| `cargo build --locked -j6 -p calternal-server -p calternal-cli -p calternal-perf --features test-plugins` | Passed |
| `cargo test --locked --workspace --all-features --no-fail-fast -j6 -- --skip pool_machines_serve_only_granted_accounts` | 590 passed |
| `cargo test --locked --workspace --all-features -j6 real_server_operations_receipts_and_authority -- --ignored` | 1 passed |
| `cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 103 passed |
| `cargo test --locked -j6 -p calternal-perf` after the driver-only readiness update | 3 passed |
| `npm run test:integration` and `npm run test:plugins` | 1 integration and 26 plugin tests passed. One signed-in Claude test skipped. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` and `python3 contract/verify-cli.py` | Passed |
| `npm run docs:site` and `sh scripts/check-docs.sh` | Passed |
| `cargo fmt --all -- --check` and `rustfmt --edition 2024 --config skip_children=true --check tests/conformance/search_cases.rs tests/conformance/isolation.rs` | Passed |

For the independent rebase, use the same conformance, integration, plugin,
contract, docs and format commands, with
`cargo test --locked -j6 -p calternal-core -p calternal-cli -p calternal-auth -p calternal-plugin-host -p calternal-server --all-features -- --skip pool_machines_serve_only_granted_accounts`.
Dev-02 passed 324 targeted and 101 serial conformance tests. The earlier
conformance run encountered a disappearing file while the existing provider
sign-in case scanned its fixture. That case passed in isolation and on the
unchanged serial rerun. The named Podman-only test stays with the
coordinator's dev-01 gate.

The earlier clean-`origin/dev` failure was
`threads_cases::saved_thread_events_replay_and_stay_in_the_owning_space`
at `threads_cases.rs:518:5`. Its assertion was
`assertion failed: std::fs::read_to_string(chats.join("Renamed.md")).unwrap().contains("title: \"Renamed\"")`.
The current serial gate passed the case.

## Review gate

| Item | Evidence |
| --- | --- |
| 1 | Shared space UUID, caller-safe errors, and journal positions. |
| 2 | Journal projection reader lives in the shared store. No new crate dependency. |
| 3 | Journal and search writers remain separate. Existing bounded reader-cache ownership transfers hold a short mutex and release it before SQLite or filesystem work. |
| 4 | Worker I/O uses a dedicated blocking thread. Requests use the existing foreground blocking pool. |
| 5 | Search adds zero reads and zero flushes to the save acknowledgment path. The worker reuses staged title and text. |
| 6 | Manifest approval and kind configuration serialize once. SQL statements cache on reused connections. |
| 7 | 250,000 head slots per space, text bounds, 128 heads per turn, 128 queued spaces and four retained writers. |
| 8 | A changed generation discards its derived rows. The current schema is the only reader. |
| 9 | Projection files and authorization are keyed by space UUID. |
| 10 | Existing declarations supply wire schemas, capabilities and operation targets. |
| 11 | Server startup wires the worker and the existing query and picker handlers use it. |
| 12 | Uses read-only WAL and journal indexes, owner lifecycle guards, and descriptor-relative Markdown reads. |
| 13 | Docs gate passed. Public projection reader APIs document their purpose, bounds and I/O. |
| 14 | Search docs, content README, generated reference and this report accompany the code. |
| 15 | Rust and standalone conformance formatting checks passed. The worker, read source and perf probes are separate modules. |

## Simplification opportunities

Fetch the retained head fields once during rebuild and read staged text
only when the materialized file needs fallback. Reusing a directory
descriptor, read handle and statement across a large scan can also remove
per-page setup and duplicate body reads. Measure these independently of
file-copy conditioning. Keep burst saves and placement-spaced saves in the
same fixture driver as distinct measurements.
