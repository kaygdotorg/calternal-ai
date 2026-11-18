# Authority compaction and Notes storage — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52)

Keep the 64 MiB security-log threshold and take the
[receipt-delta design](../security-receipt-deltas.md) to the owner for approval.
At day 30 the 8 MiB threshold met the 300 ms warm startup budget in all five
trials. It missed in all five disk-cold trials and raised measured authority
write p95 from 104.67 to 163.91 ms over the month. Receipt deltas address the
repeated receipt serialization visible at both thresholds. The design still
needs a post-migration benchmark that includes full image comparison, expiry,
and a breakdown of disk-cold startup. The 300 ms budget remains open.

## Authority writes and startup

The prior lane's disposable release capture used one month-clock and
byte-instrumented binary for both thresholds. The two arms started from
the same stopped seed with 1,000 spaces, one owner, Notes content, and
40 spaces with a second member. Over 30 simulated days they ran 20, 80,
then 160 serial role changes daily on those spaces, with the normal
seven-day receipt age. This models a concentrated active installation,
with synthetic activity rather than production telemetry. The day-30
retained working set was 1,280 receipts, after 3,260 measured changes
per threshold. The writes ran on disk. A request timed HTTP through
the response body. The thresholds alternated order between some day
blocks. Within a block writes were contiguous, so time-varying VM I/O
can affect the measured latency difference.

| Sample window | Threshold | Changes | p50 / p95 / p99 (ms) | Log append B/change | Snapshot B/change | Process write B/change | Compactions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Days 1–30 | 64 MiB | 3,260 | 67.11 / 104.67 / 161.80 | 265,080 | 7,550 | 477,459 | 13 |
| Days 1–30 | 8 MiB | 3,260 | 84.07 / 163.91 / 249.41 | 265,080 | 58,566 | 528,873 | 101 |
| Days 24–30 | 64 MiB | 1,120 | 65.02 / 91.42 / 135.39 | 328,709 | 8,576 | 499,123 | 5 |
| Days 24–30 | 8 MiB | 1,120 | 71.67 / 143.03 / 211.98 | 328,709 | 73,764 | 564,542 | 43 |

Percentiles use nearest rank. Bytes are arithmetic means. Logical
append plus snapshot bytes rose from 272,630 to 323,645 B/change over
the month. Process `/proc/PID/io` write attribution rose 10.8% and
includes SQLite, allocation effects and incidental server writes. This is
a guest process I/O counter. Compaction-crossing
requests had p50/p95/p99 of 179.15/251.80/251.80 ms at 64 MiB (13 samples)
and 206.97/287.82/385.25 ms at 8 MiB (101 samples). Uncompacted request
p95 also differed (103.72 and 143.96 ms), so the full latency difference
cannot be assigned to compaction alone.

Each cell below has five starts from fresh, synced copies of the stopped
roots. Threshold and cache order alternate. Disk-cold drops guest
page caches before spawn. Readiness is the first successful auth-status
response. Each time is median (min–max), in milliseconds.

| Day | Retained receipts before boot | Active segment MB, 64 / 8 | 64 MiB warm | 8 MiB warm | 64 MiB disk-cold | 8 MiB disk-cold |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1,057 | 58.75 / 0.98 | 348 (333–818) | 123 (102–300.44) | 640 (579–1,726) | 591 (351–667) |
| 7 | 140 | 42.20 / 0.65 | 284 (276–362) | 101 (99–121) | 675 (492–800) | 442 (346–1,354) |
| 14 | 580 | 32.96 / 7.80 | 266 (253–298) | 131 (130–144) | 608 (563–1,232) | 480 (408–622) |
| 30 | 1,280 | 38.54 / 2.45 | 305 (292–317) | 113 (110–119) | 580 (460–777) | 376 (317–382) |

From 140 to 1,280 retained receipts in steady activity, the warm median
rose about 20 ms at 64 MiB and 11 ms at 8 MiB. Day 14 and the initial
seed show why a per-receipt linear slope would mislead. The log's bytes
since the last compaction vary with activity and expiry, so more receipts
can coincide with a smaller segment. Snapshot sizes ranged from
1.63 to 2.08 MB. Warm startup met 300 ms in 10/20 starts at 64 MiB
and 19/20 at 8 MiB. Disk-cold met it in 0/20 starts at either threshold.

The capture is at `~/calternal-ai-perf/profile-52-receipt-month/`
on perf-02. It retains the fixture driver, clock, and 6,520 write and
80 startup samples. Release binary SHA-256
`f5edd64da02b584a59d468625d0769142b7d975ce42f8a096d898cfbeb93e8b4`.

## Notes storage on perf-02

The guest ext4 filesystem was `/dev/sda1` for the server roots and the
isolated files. A probe rewrote 12,288 bytes at a seeded random offset
and timed only the next sync. It preallocated each test file and excluded
30 warmups. The WAL-sized cases used the modal lengths from the separate
100,000-Note syscall calibration (4,169,472 bytes for the journal and
6,109,992 for content). File size matches those WALs. The rewrite is
an isolated sync control, without the app's checkpoint and concurrent
writeback patterns.

| Probe | Samples | p50 / p95 / p99 (ms) | Max (ms) |
| --- | ---: | ---: | ---: |
| 4 MiB, 12 KiB dirty, `fsync`, first run | 3,000 | 9.37 / 14.47 / 18.20 | 42.39 |
| 4 MiB, 12 KiB dirty, `fsync`, isolated repeat | 3,000 | 6.77 / 11.58 / 14.91 | 22.92 |
| 4 MiB, 12 KiB dirty, `fdatasync`, first / repeat | 3,000 each | 4.72 / 7.95 / 9.87 and 3.28 / 5.41 / 7.81 | 30.44 / 12.69 |
| 4 MiB, 1 MiB dirty, `fdatasync`, first / repeat | 1,000 each | 7.59 / 10.51 / 12.88 and 6.79 / 9.35 / 10.67 | 37.71 / 13.18 |
| Journal WAL-sized, `fsync` | 3,000 | 6.62 / 10.14 / 13.77 | 36.44 |
| Content WAL-sized, `fsync` | 3,000 | 6.51 / 8.84 / 12.01 | 23.37 |

Zero of 20,000 isolated syncs reached 65 ms. Earlier workload traces put
76.45 ms of one save inside a single retirement WAL sync, and another
save spent 69.81 ms inside its journal SQLite commit. The application
workload can therefore spend 65–70 ms or more in the sync path. The
cause within that path remains unresolved across guest writeback,
virtual disk contention, and backing storage.
[Syscall trace](notes-save-tail-2026-11-18.md) · [Commit stages](notes-save-stages-2026-11-18.md)

Use **75 ms p95** as a provisional perf-02 expectation for warm
100,000-Note acknowledgment with the three FULL commits. It covers
the worst observed warm arm p95 (70.92 ms) with limited headroom. Qualify
the 40 ms product gate on a confirmed production-class SSD under the
same save workload before drawing a hardware or production conclusion.
The raw syncs and calibration are at
`~/calternal-ai-perf/profile-52-storage/` on perf-02.

## Receipt health and container tests

`calternal receipt-status SPACE_UUID --appdata DIR --json` asks the
existing private admin socket to read one space, including a dormant
space. It reports the age of the oldest expired pinned receipt using
the receipt timestamp and the configured retention. When SQLite cannot
read the outbox or journal, it reports the reason and the age is unknown.
An uninitialized content DB has no outbox yet. An inspection can race
retirement and should be interpreted as a point-in-time observation.

The check uses the existing content read opener, binding checks and
journal key encoder. It adds one outbox scan and one indexed journal
query on demand, with no recovery, schema migration, or new durability
boundary. The worker tests serialize their Podman fixtures and retain
each original isolation, resource-limit, and cleanup assertion.

## Verification

Built and tested via SSH on `claude@calternal-dev-01-compute02` under
`nix develop`, in a disposable checkout based on `11e78abc1`.

| Gate | Result |
| --- | --- |
| `npm run build:plugins` | Passed |
| Workspace and standalone conformance `cargo fmt --check` | Both passed |
| `npm run docs:site` then `scripts/check-docs.sh` | Both passed, 345 source pages verified |
| `cargo test --locked --workspace --all-features --no-fail-fast` | 638 passed, 0 failed, 2 ignored across 85 harnesses |
| `npm run test:plugins` | 29 passed, 0 failed, 1 skipped |
| `cargo build --locked -p calternal-server --features test-plugins` | Passed |
| `npm run test:integration` | 2 passed, 0 failed, 0 skipped |
| Standalone conformance with `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal` and `--test-threads=1` | 133 passed, 0 failed, 0 ignored |
| `cargo run --locked -p calternal-core --bin export-contract -- --check` | Passed |
| Opt-in real-server test using the same built server | 1 passed |

The parallel workspace suite passed the container isolation probe and
the crash recovery test that inspects the receipt before first-use recovery.
The separate opt-in run covered one of the two workspace ignores. The
other is a MicroVM performance fixture. The plugin skip is the signed-in
Claude opt-in.

## Simplification opportunities

Keep the existing witness and compaction protocol if receipt deltas are
approved. Reuse the outbox pin scan and the journal's receipt-key encoder
for operator diagnostics.
