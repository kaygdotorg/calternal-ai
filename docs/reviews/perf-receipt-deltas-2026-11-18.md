# Security receipt deltas and cold startup — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52)

The November 20 owner decision superseded the rollout plan below. V4 ships
on freshly provisioned roots. The v3 conversion and its release-pair test
were removed. The measurements below describe the earlier artifact pair.

The v4 security log keeps the 64 MiB segment threshold and writes one
ordered delta for the receipt tuples affected by a transaction. Authority
changes still cross the proposal sync, SQLite witness commit and resolution
sync. A healthy restart verifies the witness and compares every normalized
authority and receipt row with the surviving database. A discrepancy triggers
SQLite integrity and foreign-key checks, then an ordinary witnessed repair.
First attach seeds existing receipts through that same path.

The server sweeps expired security receipts at boot and hourly. Receipt
retirement records an age in the chain. Compaction becomes due at 23 hours
and happens within 24 hours after deletion with the hourly sweep running.
The healthy-running retention bound for recovery-log copies is the configured age plus 25 hours.
Erasure still compacts synchronously and removes obsolete local segments.
A tombstoned space can return its original delete reply to the same
authenticated actor and exact unexpired request fingerprint. The separate
content journal fence keeps any surviving outbox receipt pinned and reports its age through private admin status.

## Upgrade and proof

The old release refuses v4. The shared runner captures the old snapshot,
segment, database and sidecars before applying the stopped-installation
migration. Its Preparing intent records the new segment as previously absent.
The upgrader verifies the v3 witness and compares complete logical rows
before publishing v4. The operator can restore v3 before commit. Integration
requires link-preserving restore points and the owner's explicit approval.

The deployed baseline was `c98705d74bcf9f9cb2a1292b849b9b44c64b2714`, image
`a0aa9efb6d1198543b772330380ae629efbecb7843cab89ddd092b2e4bb10a89`.
The development image was labeled with source commit
`30b0a5a4`. Its proof uses disposable stores seeded through the baseline's actual
release program and bundled plugin files. The candidate image was
`88466100615bf223587c24719bb50818df0064d51c7f4328bc9f4cd67e33ed11`.
The release-pair proof passed one test. It exercised additive refusal, v3
rollback and removal of the new segment before commit, exact receipt replay
after migration, new acknowledged writes and v3 refusal after commit.

## Authority writes and readiness

Both images ran under the perf-02 lock at the same 64 MiB threshold. A
stopped v3 installation with 1,000 spaces and 2,317 live authority receipts
was copied to both arms. The migrated snapshot accounted for all receipts
including 1,280 seeded role-change receipts and those retained from setup.
Each arm ran 600 serial role changes in three alternating 200-change blocks.
Latency runs through the HTTP response body. Bytes come from the guest's
per-process write counter and include SQLite and incidental writes.

| Format | Writes | p50 / p95 / p99 | Mean process bytes per write |
| --- | ---: | ---: | ---: |
| v3 | 600 | 98.27 / 150.58 / 252.67 ms | 1,297,217 |
| v4 | 600 | 95.79 / 128.73 / 157.35 ms | 335,585 |

V4 reduced mean process writes by 74.1%. Its active segment grew from empty
to 2,199,720 bytes (3,666.2 bytes per measured authority change). At the end,
the v3 snapshot and active segment were 3,770,782 and 28,580,892 bytes. V4
was 3,575,555 and 2,199,720 bytes. These are the retained bytes at that moment
in each log's compaction cycle.

Each readiness cell used five starts from fresh copies of that arm's final
stopped roots. Disk-cold drops guest page caches before launch. Warm trials
first start and stop that copy without timing it. Readiness is the first
successful auth-status request.

| Format | Warm median (range) | Cold median (range) | Warm ≤300 ms | Cold ≤300 ms |
| --- | ---: | ---: | ---: | ---: |
| v3 | 371.03 (364.11–371.69) ms | 577.55 (506.45–611.48) ms | 0/5 | 0/5 |
| v4 | 280.11 (274.97–292.30) ms | 571.67 (517.68–803.89) ms | 5/5 | 0/5 |

Warm median improved by 90.92 ms. Both arms missed 300 ms cold. The cold
medians were 5.88 ms apart, and v4 had the slowest cold sample. An earlier
paired run against `9f800564` showed the same write and warm improvements
and missed every cold start. It is archived separately from the current pair.

## What remains in cold startup

Three disk-cold runs of the instrumented v4 build had readiness
453.79–522.56 ms, median 463.57 ms. The verified worker reuse is enabled.
This release still enumerates the jobs directories. The earlier prototypes
combined worker reuse and an empty jobs inventory to measure that alternative.
Both individual phases improved, with all five readiness runs over budget.

| V4 phase | Median (range) |
| --- | ---: |
| Root validation and opening the initial content store | 56.31 (47.87–88.05) ms |
| Full authority database open and attachment | 193.56 (190.49–198.18) ms |
| ↳ Image-to-database comparison and validation | 148.32 (143.97–151.88) ms |
| Jobs open and discovery | 6.95 (6.60–46.55) ms |
| Plugin store | 42.12 (39.52–49.33) ms |
| Opening and replaying the security log | 42.84 (41.16–46.84) ms |
| Boot receipt sweep | 10.40 (9.74–12.68) ms |
| Other time outside the measured startup phases | 79.71 (79.44–82.80) ms |

The authority database comparison was the largest measured startup cost.
It protects against a missed captured key. Root and content-store open
spent 56.31 ms at the median, followed by recovery-log open and plugin-store
open at about 43 ms each. Jobs discovery varied from 6.60 to 46.55 ms.
The remaining 79–83 ms includes pre-instrumentation startup and the
readiness tail after the final marker.

The exact pair proof and gate logs are in
`~/ca52-v4-dev01-final.plhqZu36/` on dev-01. The paired samples and instrumented
phase logs are in `~/calternal-ai-perf/receipt-v4-production.kEILGU5x/`
on perf-02. The copied binaries, restored fixtures and build outputs were
removed. The release images and measurement evidence were retained.

## Notes durability on perf-02

Keep 40 ms p95 as the product gate. The observed working expectation is
75 ms p95 at 100,000 Notes warm on perf-02, with individual loaded WAL
syncs of 65–76 ms and no sync that slow in the isolated control. In the
complete correlated guest trace, a 36.17 ms WAL sync was dominated by a
35.80 ms ext4/JBD2 commit. The virtual device took 7.41 ms for its cache
flush and 28.15 ms for the 4 KiB FUA write. The longer tail did not recur in
the trace session, which captured 2,733 successful WAL syncs.

Qualification on the production disk class requires a disposable matching
guest or blank volume. For both disks, collect the QEMU block driver and
backing pool, direct I/O and no-flush settings, AIO mode, write cache,
controller and queue count, and I/O thread and throttling configuration.
Correlate guest SCSI submission and completion with virtqueue timing, QEMU
acceptance and backend submission, host block issue and completion, and
FUA or flush completion on a shared clock. Also record queue depth and host
vCPU scheduling delays. [Performance notes](../performance.md)

## Verification

The final full gate ran on dev-01 slot 2 at `30b0a5a4`, which
includes `origin/dev` at `c98705d7`. The first run exposed five CLI golden
failures from startup diagnostics emitted by the all-features build. The
fix requires `CALTERNAL_STARTUP_TIMINGS=1` with `perf-timings` to
print startup measurements. The retry passed with the original CLI goldens.

| Gate | Result |
| --- | --- |
| Plugin build | Passed |
| Workspace and standalone conformance formatting | Both passed |
| Docs site and docs gate | 351 site pages built, 350 docs pages checked |
| Workspace all features, locked, no fail-fast | 673 passed, 0 failed, 2 ignored, including 8 passing CLI goldens |
| Plugin tests | 32 passed, 0 failed, 1 skipped |
| Server build with test plugins | Passed |
| Integration tests | 2 passed, 0 failed |
| Locked serial conformance with the built server | 137 passed, 0 failed, 0 ignored |
| Contract check | Passed |
| Podman, Firecracker and provider sign-in leak checks | Passed |

This transition updates the security recovery file marker from v3 to v4.
The shared runner also updates the auth database's recovery triggers as
part of that transition. The SQLite versions are auth 14, content 5,
and machines 6.

## Simplification opportunities

Walk the ordered authority and receipt maps once during startup
comparison. The current comparison looks up shared keys twice. Preserve
the full database-and-image check and witnessed repair. Break down the
remaining 79–83 ms around startup publication and listener binding before
changing the boot sequence.
