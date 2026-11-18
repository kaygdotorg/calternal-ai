# Performance integration follow-up — issue #31

Thread `thr_i78z9xkxhy`, branch `lane/perf-fixes`. This follows the coordinator's
merge request after the [pinned performance report](perf-fixes-2026-11-17.md).
The integration base is `origin/dev` at `ed6f034`, after lane head `802096c`.
The next concurrency investigation is separate from this merge handoff.

## Shared write path

The existing per-space lock, 64-space cache, 600-second idle deadline and
30-second sweep remain. Ordinary content saves acknowledge after three FULL SSD
commits: content/history/indexes and intent, journal/receipt, and outbox retirement.
The existing lifecycle worker performs guarded file/directory synchronization
within one second, then NORMAL replayable placement bookkeeping. The oldest
pending deadline never extends; overdue placement still blocks new writes.

Trash, restore and linked rename finish their required placement synchronously.
The publisher carries the Notes lane's `outbox_placement` dependencies, guarded
purge finalization and standing-instruction intent handling. Pending related
placements finish before their root receipt is published. Purge retains its
FULL intent/finalization/receipt/retirement boundaries. No durability boundary
was weakened to obtain latency numbers.

One shared header carries kind, revision, sequence, path, placement state,
expected file hash and prior path. A warm update uses that one query for revision
and lifecycle admission plus the outside-file guard, replacing four separate
header/path queries. A pending older revision is placed first and then its
changed header is reloaded. Placement also uses its loaded path, removing another
path query. The admission and final placement file checks both remain.

For a warm ordinary same-title update with an empty outbox and no pending prior
revision, the content mutation's explicit SELECT statement calls are: initial
empty outbox, journal receipt, shared header, populated outbox, catalog route,
publication receipt, final empty outbox (seven). Joins/subqueries within a
statement and write-trigger JSON projections are not counted as additional
statement calls. The Notes adapter adds one live-metadata query, its outer
receipt guard adds empty-outbox and receipt queries, and item resolution adds
one existence query (four more). Authentication, policy/plugin metadata,
PRAGMAs/schema work, name collisions, linked sources and background placement
are separate costs. This is a statement-call accounting scope; the historical
report's broader logical-read total is not reused for this merged implementation.

The Notes engine loads no previous body for a content-only save. It parses the
submitted Markdown's tag/link projection once. There are two checked userdata
reads across the complete path: admission before acknowledgement and final
placement afterward. File and directory each sync once for ordinary replacement;
SQLite checkpoint/WAL creation and the independent delayed witness add their
existing maintenance costs. Linked/Trash/purge costs differ as described above.

## Retention, readiness and replay

The Notes startup/hourly sweep previously activated every space through the
content service, undoing lazy startup. It now probes one closed SQLite database
at a time read-only and activates the existing content lifecycle only for
expired work. Purge eligibility is still rechecked under the space lock. No
second space cache or journal is introduced. Read-only SQLite probes may warm
metadata or map WAL sidecars; first-open observations retain that qualification.

Account-feed readiness notifications use the existing wake/replay loop alongside
commit hints and authority changes. A readiness wake loops through current
session validation before emitting its hint. Reader admission still includes
blocking close, replay and overflow semantics remain, and the 250 ms poll does
not return.

A reconciliation test found that removing the old forced-materialization replay
check could expose an older body receipt after permanent purge. The HTTP replay
boundary now checks the associated item's deletion state, without forcing file
placement or rejecting an acknowledged result merely because a later outside
file edit blocks placement. Purge's own body-free result remains idempotently
replayable. The server reexports the shared content receipt rather than copying
its fields into another type.

## Verification and measurement

All engineering commands run on `claude@calternal-dev-01-compute02` in
`~/Developer/lanes/perf-fixes`, through `nix develop -c`, Cargo `-j6`.
The full workspace and conformance runs are serial and wrapped in
`scripts/dev-e2e/check-podman-tests.sh`. The wrapper uses a private TMPDIR,
checks labeled container roots and fails the command if containers remain.
No other lane's containers or processes are pruned.

The initial merged workspace gate passed with the leak check. The new purge
regression then failed as expected: 75 conformance cases passed and the new case
returned HTTP 200 instead of 404. After the tombstone fix its focused rerun passed,
along with its leak check. Initial logs remain in
`artifacts/gates-reintegrated-initial/`; final logs are separate under
`artifacts/gates-reintegrated-final/`.

The reused VM source snapshot also retained obsolete `plugins/notes` and
`plugins/notes-insights` directories after the official rename. Duplicate npm
workspace package names produced a misleading `npm ci` lockfile EUSAGE error.
Only those two proven stale lane-local source copies were removed; `npm ci`
and `npm run build:plugins` then passed.

Final engineering results: **571 workspace tests passed, zero failed, one
ignored; 76 conformance tests passed; the explicit ignored real-server fixture
passed**. Integration, plugin, contract, docs, workspace/standalone conformance
formatting and release builds all exited zero. Workspace, full conformance,
focused conformance and real-server Podman leak checks all passed. The source
candidate is merge commit `2ab2665`.

The [bounded warm control](../../perf/results/2026-11-17-perf-fixes-reintegrated-control.json)
finished all twelve arms with zero arm failures. Both arms use the same artifact;
the ranges below retain all six observations per fixture, not only a median:

| Fixture | Update p95 range | Ready range | First content activation range | Draft-save-to-account-feed p95 range |
| --- | ---: | ---: | ---: | ---: |
| 1 × 10k notes | 27.380–30.172 ms | 44.877–58.950 ms | 15.389–18.030 ms | 8.136–12.041 ms |
| 100 × 100 notes | 25.135–27.399 ms | 58.143–86.161 ms | 27.462–34.186 ms | 11.106–14.686 ms |

All twelve update p95s satisfy 40 ms and every readiness observation satisfies
300 ms. Maximum individual update is 50.007 ms; the claim is about p95, not a
maximum. File-durability p95 is 40.946–48.769 ms, with maximum 78.142 ms, below
the one-second placement bound. Feed's single-digit target remains unmet.
The full-write-to-space-feed proxy remains 35.432–40.424 ms p95 in all arms,
over its unchanged 10 ms ceiling, so the control exits **1**, not green.
100-space head startup noise is 36.71%, but none of those arms breaches 300 ms;
there is no noisy repeated-breach group requiring another run. This is the one
bounded control requested by the coordinator.

The [cleanup/startup diagnostic](../../perf/results/2026-11-17-perf-fixes-reintegrated-diagnostics.json)
records all twelve startup observations with zero pre-use space writers and zero
remaining credential-root directories. First activation is a process's first
content/journal activation, with the read-only maintenance probe qualification
above; this is not an OS-cold-space measurement. There is no new cold,
1,000-space or post-idle memory qualification.

Plain server SHA-256:
`879aa14d09469186887666b4ab9668550f1e27f0c178ebd1384abd734c01b12f`.
Driver SHA-256:
`5ed0970abb4a98cb98fa549adafa47ddcc910b2380735d25a368d4d19d9155c9`.

Exact perf command, inside the lock at
`~/calternal-ai-perf/perf-fixes-merge-ed6f034`, with `root=$PWD`:

```sh
./bin/calternal-perf --bin "$root/bin/calternal" \
  --baseline "$root/bin/calternal" --work "$root/control" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes 10000 --spaces 1,100 --pairs 3 --cold-pairs 0 --samples 100 \
  --commit 2ab2665 --baseline-commit 2ab2665 --json
```


Exact commands, each run through `nix develop -c` on dev-01 with
`CARGO_BUILD_JOBS=6` (plugins built before Cargo tests):

```sh
npm ci
npm run build:plugins
cargo fmt --all
cargo fmt --manifest-path tests/conformance/Cargo.toml --all
cargo build --locked -j6 -p calternal-server --features test-plugins
CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" \
  scripts/dev-e2e/check-podman-tests.sh cargo test --locked -j6 \
  --manifest-path tests/conformance/Cargo.toml \
  purge_blocks_old_body_receipt_replay_with_foreign_isolation -- --test-threads=1
scripts/dev-e2e/check-podman-tests.sh cargo test --locked -j6 \
  --workspace --all-features --no-fail-fast -- --test-threads=1
CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" \
  scripts/dev-e2e/check-podman-tests.sh cargo test --locked -j6 \
  --manifest-path tests/conformance/Cargo.toml -- --test-threads=1
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" \
  scripts/dev-e2e/check-podman-tests.sh cargo test --locked -j6 \
  -p calternal-client --test real_server -- --ignored --test-threads=1
npm run test:integration
npm run test:plugins
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
scripts/check-docs.sh
cargo fmt --all -- --check
cargo fmt --manifest-path tests/conformance/Cargo.toml --all -- --check
cargo build --locked -j6 --release -p calternal-server -p calternal-perf
```

The requested control uses one artifact against itself, 1 × 10k notes and
100 × 100 notes, three warm pairs, 100 timed samples per operation/arm, under
`/usr/local/bin/perf-lock.sh`. This avoids opening schema-8 fixtures with the
older schema-6 binary; no migration reader is added. The earlier measurements
remain pinned to their own binaries and source commits. No budget changes.

## Review gate

| Item | Evidence or recorded limit |
| --- | --- |
| 1 | Shared IDs/errors/digests retained; duplicate server receipt replaced with the shared content type. |
| 2 | No new crate/dependency; the header stays in content storage and retention uses existing rusqlite. |
| 3 | Header/guard/placement share the existing space lock; no new installation mutex. Protected auth publication ordering remains unchanged. |
| 4 | Changed storage/maintenance remains on blocking workers; Notes mutation hashing uses the existing blocking helper. Synchronous directory checks and inherited turns hashing remain recorded issues for the ordered concurrency follow-up. |
| 5 | Statement/file/flush counts above; header is reused, pending state reload occurs only after placement changes it. |
| 6 | Existing kind/schema caches and prepared projections retained; no new per-request constant construction. |
| 7 | 64 spaces, 600-second idle/30-second sweep, one-second placement bound; one retention probe connection at a time. Receipt retention stays operator-configured, seven days by default. Raw evidence/binaries/logs retained 30 days; fixture roots removed after runs. |
| 8 | No migration/alias/old reader added. Existing root-layout refusal fence remains an explicitly retained protection. |
| 9 | Space routing remains by UUID; concrete Notes retention code uses its declared kind. The inherited layout fence's old path names remain the existing 8/9 exception. |
| 10 | Operations, risks, capabilities and role declarations remain authoritative and unchanged by this integration. |
| 11 | Shared header, replay tombstone check, readiness wake and retention probe have runtime callers; obsolete internal path accessor removed. |
| 12 | Existing content lifecycle, guarded fsio placement, Notes indexes, journal wake/replay and protected recovery reused. |
| 13 | Changed public receipt and save behavior carry purpose/failure/cost comments; docs gate required. |
| 14 | Content/server READMEs and this report describe the merged behavior; historical reports retain their artifact scope. |
| 15 | New concerns remain split; source formatting gates required. The integrated store facade is now 558 lines (its earlier size exception is resolved); main is 539. Existing large integration fixtures remain the recorded exception. Dispatch is 608 lines, close to the approximate guide, with native Notes helpers already in their own module. |

## Remaining work and decisions

The original 1,000-space startup, occasional cold save tail and single-digit feed
target remain open; this bounded warm control does not recertify those cases.
The next ordered investigation reproduces the people lane's concurrency evidence
on perf-01 with 8/32/128 clients, then addresses current session/capability
checks, blocking read/write scheduling, and recovery encoding once per batch.
Authority checks and protected-publication ordering cannot be relaxed as a queue
optimization. Receipt-capture record boundaries require a separate decision if
changing them is needed; encoding/copy reuse within the existing format is the
first option.

## Simplification opportunities

The Notes port already removes the prior body load needed only for title lookup.
Borrowed canonical JSON encoding could remove its existing tree copies. The shared
header could serve additional storage callers as they are changed, without adding
another cache. Keep receipt-pruning and durable-outbox coordination explicit rather
than weakening retirement synchronization independently.
