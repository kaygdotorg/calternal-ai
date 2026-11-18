# File placement gate and startup cache — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52) ·
[Notes save tails](notes-save-tail-2026-11-18.md) ·
[Security receipt delta design](../security-receipt-deltas.md)

## Acknowledgment to durable file

The independent `userdata.ack_to_durable_file` gate uses the maximum across
the observed placements, with a 1,000 ms ceiling from ADR 0062. It starts
in the HTTP client after it receives the save response. The SSE reader runs
concurrently. The file mark must match the acknowledged revision and follows
the file and directory syncs. A missing mark at the limit fails placement
and retains an elapsed lower bound. If placement stops the arm early, the
delivery report retains the correlated observations and marks the arm
incomplete. The Notes save and client-to-client clocks keep their own limits.

| Timing-enabled 1,000-Note arm | Placement p95 | Maximum | Samples |
| --- | ---: | ---: | ---: |
| Disk-cold | 18.48 ms | 25.12 ms | 100 |
| Warm range, three arms | 16.26–17.58 ms | 27.46–32.42 ms | 300 |

The first release harness run on perf-02 used the previously accepted
`b4c5489` server and completed all eight arms. A second run substituted a one
nanosecond placement budget to exercise the failure path. It exited 1
with a hard placement breach, no delivery breach, and incomplete delivery
windows. The real catalog retains 1,000 ms.

The final release run completed 24 arms with 100 updates each and exited 0.
It recorded 1,200 correlated deliveries and 2,400 file observations. Every
arm met its applicable budget. Each fixture had three warm and one
disk-cold pair.

| Notes | Delivery p95, warm | Delivery p95, disk-cold | Placement max, timing build | Placement max, plain control |
| --- | ---: | ---: | ---: | ---: |
| 1,000 | 1.26–1.67 ms | 1.40 ms | 73.64 ms | 158.21 ms |
| 10,000 | 1.67–2.53 ms | 1.38 ms | 80.71 ms | 468.36 ms |
| 100,000 | 1.30–1.34 ms | 1.42 ms | 397.13 ms | 335.10 ms |

## Worker cache and jobs inventory

Startup now reuses current and retained cached workers only when a bounded,
no-follow read from a regular, single-link, mode-0600 file owned by the
service matches the verified archive bytes. The cache directory is private.
Missing or invalid files follow the existing atomic write and sync. Worker
launches still verify the pinned hash. Startup cleans up stale regular
JavaScript cache files. The regression checks inode reuse and stale removal
and exercises permission, content, symlink and hard-link mismatches.

The production path was checked in three warm and three disk-cold paired
starts of a populated worker cache on perf-02. This fixture has one space
and 1,000 Notes. The earlier 1,000-space run isolated the worker phase.

| Cache | Old startup median | Verified reuse median | Verified reuse range |
| --- | ---: | ---: | ---: |
| Warm | 120.42 ms | 66.46 ms | 58.42–79.37 ms |
| Disk-cold | 422.84 ms | 397.61 ms | 389.02–413.50 ms |

The first warm control took 418.66 ms. The two other controls took 98.34
and 120.42 ms. Every verified-reuse warm start was faster than its paired
control. One cold start became slower and all cold starts missed 300 ms.

The earlier 1,000-space prototypes measured the zero-job inventory phase at
under 0.01 ms compared with 185–731 ms for the cold directory sweep. An
adoptable inventory needs more than switching the iterator. Estimate
**280–440 lines including tests**, across inventory storage, first job-store
creation, space deletion and the existing erasure replay. The design needs a
durable membership record before the first jobs database can commit, removal
after the database or space tree is durably removed, and an atomic rebuild
from the current sweep on a missing, corrupt or stale inventory. Startup
must never recreate an erased job store from a stale entry. Deleting an
individual job keeps the space in the inventory while its database exists.
Recovery tests
need to cover both sides of creation and deletion, account-induced space
erasure, and rebuild followed by restart. The directory sweep stays in use
until that work is reviewed.

## Backend verification

Dev-01 completed the full backend gate with 604 all-feature workspace tests,
one additional opt-in real-server test and 117 standalone conformance tests
using `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal`. The Podman leak
checks, integrations, plugin tests, contracts, docs and formatting passed.
Plain, timing-enabled and test-plugin release builds passed. The journal
timing marker is present only in the timing-enabled release.

| Final artifact | SHA-256 |
| --- | --- |
| Plain release | `16f714feb70293d3e06bc2ddef6cfa736e98702c105c008ab562df440fef142d` |
| Timing-enabled release | `a201cc2c2e32c67bf573b5d57ecd4f9a0a0e540a50f28630917f698c4aff3769` |
| Test-plugin release | `d62f05cc6987c1fd8e616d2b4e30e40cded0fe8646403c9db1bc27c2a0e1da14` |
| Release perf driver | `923cfa0853dfc94e8fbf7fecdbf314bb5de8954da142bf0bfd6ded1b4d482bd7` |

The raw runs are in `~/calternal-ai-perf/profile-52-tail/` on perf-02.
The backend logs are in `~/Developer/Calternal-AI-perf-placement/gate-logs/`
on dev-01.

## Simplification opportunities

Use the existing directory sweep as the sole recovery path for any future
jobs inventory. Keep the package catalog as the source of truth for the
worker cache and avoid a second manifest for its files.
