# Notes rebuild read path

Issue #47. The worker pages over eligible item metadata and checks that
each current version exists. It holds one read snapshot through each page
and opens the space directory once per page. It fetches staged bytes only
when it cannot use the file. Journal catch-up, readiness, and index writer
settings are unchanged.

## Integrity

The rowid cap, enabled kind filter, text title requirement, and Trash,
deleted, purging and purged exclusions still apply. The current version
must exist for the item and sequence in the scan snapshot. The file must
pass the descriptor-relative path check and be regular. It must read
successfully within the existing full-file byte limit and match the durable
file hash. The worker hashes the full file before taking the 64 KiB search
prefix. Fallback selects the exact item and sequence from the same snapshot
and normalizes SQLite's empty BLOB slice. Journal replay retains that same
normalization and uses staged content.

Conformance modifies a placed file to contain a different term of the same
byte length, invalidates the generation, and verifies that rebuild finds
the staged term and excludes the file's term. The edited file stays intact.
A second case removes a materialized file whose staged body is empty and
verifies that rebuild still finds its title. The first broad gate exposed
the need to retain the original empty BLOB normalization. The final gate
uses the restored behavior.

## Notes timings

Perf-01, under `/usr/local/bin/perf-lock.sh`. Dev-02 built release binaries
from the pinned x86_64 musl cross shell. Before and after used fresh copies
of the same stopped 100,000-note fixture. Cold arms used
`sudo -n /usr/local/sbin/perf-drop-caches` after syncing. Warm arms read
the source files and databases before launch. The timer spans the worker's
first turn through publication. The perf corpus uses nonempty staged
bodies and measures the optimized verified-file path. The backend gate
also covers empty-body fallback and journal replay after the measurement.

| Cache | Before, s | After, s | Working target, s |
| --- | ---: | ---: | ---: |
| Disk cold | 82.487 | 22.011, 21.137 | 35 |
| Warm | 65.727 | 7.559, 7.088 | 25 |

Each arm produced a 79,015,936-byte index. The production write policy is
unchanged. Normal SQLite automatic checkpointing triggers at 1,000 pages,
about 4 MiB at 4 KiB per page. This threshold does not bound a WAL pinned by a
reader or a transaction. Bulk loading and checkpoint changes were deferred
once the read-path change met the target. The side-by-side proposal specifies
a separate candidate allowance and crash handling.

The perf directory is
`~/calternal-ai-perf/search-thr_yruirs3chu/notes-fast-source`.
`compare.sh`, `compare.log` and `binary.sha256` identify the exact run.
On the dev VM the cross build ran
`cargo build --locked --offline --release --target x86_64-unknown-linux-musl --target-dir target-perf -j6 -p calternal-server`.

## Dev-02 backend gate

The final gate ran with `nix develop -c`, `CARGO_TARGET_DIR=target`,
`CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal`, `CARGO_INCREMENTAL=0`,
and `CARGO_PROFILE_DEV_DEBUG=0`.

| Command | Result |
| --- | --- |
| `npm ci --no-audit --no-fund` and `npm run build:plugins` | Passed |
| `cargo build --locked -j6 -p calternal-server -p calternal-cli -p calternal-perf --features test-plugins` | Passed |
| `cargo test --locked --workspace --all-features --no-fail-fast -j6 -- --skip pool_machines_serve_only_granted_accounts` | 597 passed. The Podman case belongs to dev-01. |
| `cargo test --locked -j6 -p calternal-client --all-features --test real_server real_server_operations_receipts_and_authority -- --ignored` | 1 passed |
| `cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 116 passed, including both fallback cases |
| `npm run test:integration` and `npm run test:plugins` | 2 integration and 27 plugin tests passed. The signed-in test skipped. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` and `python3 contract/verify-cli.py` | Passed |
| `npm run docs:site` and `sh scripts/check-docs.sh` | Passed |
| `cargo fmt --all -- --check` and `rustfmt --edition 2024 --config skip_children=true --check tests/conformance/search_cases.rs tests/conformance/search_cases/live.rs tests/conformance/search_cases/rebuild_source.rs` | Passed |

## Review gate

| Item | Evidence |
| --- | --- |
| 1 | Reuses space IDs, errors, hash and journal positions. |
| 2 | Uses the existing content and filesystem dependencies. |
| 3 | Reads one space under its existing lifecycle guard. |
| 4 | File and SQLite work stays on the search maintenance thread. |
| 5 | One full guarded file read or one staged prefix on fallback. Save I/O is unchanged. |
| 6 | The scan and fallback use cached statements within one bounded page read. |
| 7 | 128 heads per turn, existing row and byte caps, and existing write policy. |
| 8 | Keeps the current derived format and lazy reset lifecycle. |
| 9 | Roots and authority remain keyed by space UUID. |
| 10 | Operation declarations and wire shapes are unchanged. |
| 11 | The worker uses the fast path at every rebuild. |
| 12 | Reuses `Folder::open_file`, the durable hash, the exact staged version and journal catch-up. |
| 13 | Comments and the docs gate cover the snapshot and fallback. |
| 14 | The search guide and generated site copy reflect the implementation. |
| 15 | Workspace and standalone conformance formatting checks passed. |

## Simplification opportunities

The live search and palette cases share the package CLI fixture. Once a
published index path guarantees an initialized schema, the warm reader can
fetch state with one statement while preserving the cold partial response.
