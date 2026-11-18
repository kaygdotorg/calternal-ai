# Live delivery, placement and startup follow-up — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52) ·
[Initial profiles](perf-profile-2026-11-18.md)

The Notes subscriber waited inside its item-visibility check. That check took
the space writer's lock while retirement was still in progress. If the
placement worker acquired the lock next, the subscriber could also wait for
the file and directory sync. The fix uses an indexed read-only SQLite WAL
snapshot for already recovered spaces. The existing owner admission, session
and space checks, kind check, journal replay and event order still apply.
The writer keeps the same FULL content, journal and retirement commits.

## Commit to client, before the fix

Built release artifacts on dev-01 with `nix develop`. Ran on perf-02 under
`/usr/local/bin/perf-lock.sh`. The diagnostic build stamped the durable journal
return, broadcast, SSE task, journal replay and visibility worker. The
subscriber stamped receipt of the SSE data line and subsequent JSON decode.
A loopback capture recorded the outgoing TCP packet at the kernel timestamp,
translated to the VM's monotonic clock. Times below are phases of the single
nearest-rank p95 sample in each group. The cold 100,000-note arm provided 16
deliveries before a placement timeout, so its row describes a partial
diagnostic sample. The release gate itself uses completed arms.

| Phase | 1,000 warm | 1,000 cold | 100,000 warm | 100,000 cold |
| --- | ---: | ---: | ---: | ---: |
| Samples in diagnostic group | 90 | 30 | 90 | 16 |
| Commit return to broadcast marker | 0.107 ms | 0.037 ms | 0.030 ms | 0.042 ms |
| Broadcast marker to SSE wake | −0.030 ms | 0.035 ms | 0.039 ms | 0.052 ms |
| SSE wake to frame ready | 43.748 ms | 63.225 ms | 30.378 ms | 442.754 ms |
| Frame ready to TCP egress | 0.052 ms | 0.043 ms | 0.058 ms | 0.045 ms |
| TCP egress to client data line | 0.155 ms | 0.089 ms | 0.097 ms | 0.092 ms |
| Client JSON decode | 0.014 ms | 0.011 ms | 0.017 ms | 0.015 ms |
| **Total** | **44.046 ms** | **63.439 ms** | **30.619 ms** | **442.999 ms** |

The broadcast marker is sampled after sending. An already scheduled receiver
can record its wake just before that marker. Frame-ready includes the
diagnostic's server-side log handoff before the kernel egress capture.

| Detail within the trace | 1,000 warm | 1,000 cold | 100,000 warm | 100,000 cold |
| --- | ---: | ---: | ---: | ---: |
| Journal replay | 2.597 ms | 0.461 ms | 0.248 ms | 0.427 ms |
| Item visibility check | 35.419 ms | 61.578 ms | 29.539 ms | 439.985 ms |
| Admission to visibility worker | 0.022 ms | 0.017 ms | 0.012 ms | 0.014 ms |
| Visibility worker overlapping retirement | 35.184 ms | 6.093 ms | 9.573 ms | 439.730 ms |
| Visibility worker after retirement | 0.214 ms | 55.469 ms | 19.954 ms | 0.241 ms |

In the 1,000-note cold p95 sample, the placement worker logged 55 ms after
retirement and immediately before the SSE frame. In the 100,000-note cold
sample, the 442 ms retirement span explains almost the whole stall. This
accounts for the difference from the earlier account draft path, whose
reader does not acquire the space writer's lock.

The regression test parks the space writer with a committed row and a live
journal. It timed out with the original visibility check and passed with the
read-only path. The recovered slot retains the same bounded reader pool type
already used for storage reads. A visibility probe performs one indexed query
and zero flushes. A cold probe still opens and recovers the space first.

## Release delivery remeasurement

Each timing-enabled arm recorded 100 matching Notes updates. The paired plain
release used the same source changes. The warm column is the range across
three arms. The disk-cold column is one arm after dropping OS caches.

| Notes | Warm p95 | Disk-cold p95 | Timing-enabled arms above 10 ms |
| --- | ---: | ---: | ---: |
| 1,000 | 1.40–1.72 ms | 1.41 ms | 0 of 4 |
| 10,000 | 1.16–1.24 ms | 1.20 ms | 0 of 4 |
| 100,000 | 1.50–3.68 ms | 3.04 ms | 0 of 4 |

An earlier run before the final dev merge also measured 1.43–1.91 ms on
the six 1,000-note head arms and 1.79 ms on the one completed 100,000-note
head arm. The conservative placement cutoff interrupted the remaining
100,000-note pairs in that run. The merged release completed all 12 planned
timing-enabled arms, 1,200 saves, below 10 ms p95. The 10,000-note run exited
0. One plain 100,000-note warm control hit the conservative cutoff, leaving
the combined 1,000- and 100,000-note driver run incomplete (exit 2).

The separate 1,000-note draft matrix also measured the full Notes round trip.
Timing-enabled save-to-subscriber p95 was 35.32 ms disk-cold and 54.59–63.12 ms
warm against its own 50 ms budget. The plain release ranged from 28.37 to
57.38 ms. Every Notes acknowledgement arm on both releases exceeded its
40 ms budget. This matrix still has performance failures outside the
commit-to-client and draft-to-feed spans.

## File-placement failures

| Original arm(s) | Check reported | What the evidence establishes |
| --- | --- | --- |
| 10,000 notes, cold baseline pairs 1 and 2 | Request-to-file exceeded one second | An observation eventually succeeded. The measured span includes the original save, socket delivery and scheduling before observation. |
| 10,000 notes, warm head pair 5 | Request-to-file exceeded one second | Same conservative check. |
| 100,000 notes, warm baseline pair 5 | Request-to-file exceeded one second | Same conservative check. |
| 10,000 notes, warm head pair 4 | Placement observer timed out | The read-only observer never saw the matching materialized revision during its one-second wait, which starts after acknowledgment and delivery. |

The historical logs lack separate acknowledgment and placement timestamps.
New 100,000-note traces reproduced both classes. One sample took 1,010.48 ms
from request to file, with 966.13 ms in the HTTP save and 44.21 ms after
acknowledgment. The conservative harness cutoff would flag this sample even
though the placement interval passed. Another sample acknowledged in 52.36 ms
and still timed out 1,000.48 ms after acknowledgment. After stopping its
disposable server, SQLite still showed the revision pending and the outbox
empty. The latest bytes were visible in the file. Completion of the sync and
bookkeeping boundary remained outstanding. The first class combines save and
placement time. The second class can reveal a genuine product placement delay.

A separate 10,000-note repeat completed 1,200 saves across 12 arms. All
matching revisions were observed. The maximum request-to-file span was
322.93 ms and the maximum post-acknowledgment observation was 206.11 ms.
The new traces establish both failure classes, with no per-failure
classification for the four historical conservative failures beyond their
broader measurement. The product placement bound and harness checks were
preserved.

## Draft durability

After completing the delivery breakdown, the accepted draft-only policy was
implemented as `DRAFT_SAVE_SYNCHRONOUS`. It selects SQLite WAL NORMAL for
`drafts.save` after the existing FULL housekeeping step. An acknowledged
unsent edit survives application process death. Power loss or a system crash
can discard recent unsent edits. Invalidation is committed with the edit.
The draft writer skips a checkpoint on connection close and keeps SQLite's
normal WAL autocheckpoint threshold. Other operations keep their established
durability. [SQLite synchronous modes](https://sqlite.org/pragma.html#pragma_synchronous) and [WAL checkpointing](https://sqlite.org/wal.html).

The restart conformance case kills the process with a live account reader
still open, then checks both the saved draft and replay of its invalidation.
The draft-to-account-feed p95 ceiling is 9 ms.

The first release comparison ran six arms per variant with 100 saves per
arm on the 1,000-note fixture. Every NORMAL arm passed the new budget. The
merged tree repeated the test with three warm and one disk-cold arm, again
with 100 saves per arm. The last column shows the timing-enabled release.

| Cache | FULL baseline p95 range | Draft-only NORMAL p95 range | Merged tree NORMAL p95 range |
| --- | ---: | ---: | ---: |
| Warm | 10.09–29.04 ms | 1.28–1.35 ms | 1.14–1.21 ms |
| Disk-cold | 9.09–13.96 ms | 1.27–2.69 ms | 1.35 ms |

## Startup prototypes

Three rotated trials per prototype and cache condition used disk-backed copies
of the same 1,000-space, 100-Notes-per-space seed. The fixture had zero job
databases. The jobs prototype loaded an inventory generated from the frozen
seed and called the existing recovery for any listed space. Worker reuse
opened the cached file without following its final component, checked that
it was a mode-0600 regular file, and compared bounded bytes to the verified
archive. A missing or mismatched file followed the existing atomic write.

| Startup variant | Warm readiness median | Disk-cold readiness median | Warm targeted phase | Disk-cold targeted phase |
| --- | ---: | ---: | ---: | ---: |
| Existing jobs sweep | 454.57 ms | 1,418.47 ms | 3.00–5.37 ms | 185.10–731.05 ms |
| Jobs inventory | 452.89 ms | 1,059.08 ms | <0.01 ms | <0.01 ms |
| Existing worker rewrites | 454.57 ms | 1,418.47 ms | 43.41–295.30 ms | 81.84–225.97 ms |
| Verified worker reuse | 435.83 ms | 2,032.49 ms | 3.13–4.34 ms | 72.48–202.47 ms |

Cold readiness changes versus the paired control were −642, +1,536 and
−1,725 ms for jobs and +614, −31 and −355 ms for worker reuse. Warm changes
were −269, +5 and +11 ms for jobs and −195, +167 and −6 ms for worker reuse.
The jobs sweep was removed in every trial. Warm worker verification improved
its measured phase in all three trials. End-to-end readiness moved both ways
and every trial exceeded 300 ms. The prototypes remain in the disposable
benchmark build. An authoritative jobs inventory would move maintenance to
job creation, deletion and erasure. A missed entry can skip a job on restart,
so those transitions need recovery coverage before adoption. Worker reuse
trades rewriting and syncing for a bounded private-file read and comparison
on each cache hit. Adoption requires keeping the existing bounded cleanup of
stale revisions and strict private-file checks.

## Security-log diagnostics

The unchanged compaction configuration produced a 1,839,753-byte snapshot
and a 61,722,172-byte active segment with 254 frames. This is an average
of 243 KB of segment data per frame, counting short resolution frames
together with proposals. The 127 proposal frames occupy 61,693,216 bytes
and the 127 resolution frames occupy 28,956 bytes. `space_receipts` alone
accounts for 60,478,715 bytes of serialized rows, 98.0% of the segment.
The timings below are medians of three baseline starts in each cache
condition. The per-frame line divides each run's timed frame work by its
frame count before taking the median.

| Active-segment phase | Warm ms | Disk-cold ms |
| --- | ---: | ---: |
| BLAKE3 payload checksum | 17.71 | 16.37 |
| JSON decode, including allocations | 134.24 | 128.43 |
| Frame and key checks | 0.13 | 0.12 |
| Canonical proposal re-encoding, including its checksum | 74.46 | 71.02 |
| Whole-frame chain hashing | 15.37 | 14.35 |
| Apply resolved record changes | 14.94 | 13.79 |
| **Timed work per frame** | **1.011 ms** | **0.961 ms** |

The separate warm CPU recording had 271 samples and zero lost samples.
Allocator functions accounted for about 15.5% of sampled user CPU across
the whole startup. That sampling estimate includes allocation and release
in parsing, serialization and state construction. It cannot be added to
the phase spans. The largest single sampled leaf was the BLAKE3 AVX2 batch
function at 14.4%. JSON escaping took 10.7% and string parsing 7.0% as
individual leaves. Parsing and canonical serialization together are the
largest timed component. The segment uses checksums and chain hashes and
performs zero signature checks.

Prototype a decoder that can reuse canonical proposal bytes for chain
verification. Preserve the currently accepted representation and test exact
digest equivalence, malformed and noncanonical inputs, torn boundaries and
external witness recovery. The 71–74 ms re-encoding phase is the ceiling
for that change. Each changed account record currently serializes its entire
receipt set. Prototype row-level receipt deltas. This targets the 60.48 MB
dominated by repeated rows and shifts work to recording inserts and deletes
and replaying them in order. Verify duplicate-request replies, expiry, erasure
and witness recovery against the existing guarantees. The compaction threshold
remained in force.

## Verification

The follow-up branch merged `origin/dev` at `45fa2c0` without conflicts.
The final-source release runs include that merge because its search worker
can rebuild an active space in the background. Dev-01 ran the all-feature
workspace suite with 601 passed and one opt-in fixture, which also passed
when run separately. Standalone conformance ran with
`CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal` and passed all 117 tests,
including the draft kill-and-restart case. The integration and plugin suites,
contract export, docs and both format gates passed. Scoped Podman checks
passed. Plain, timing-enabled and test-plugin release builds passed. The
plain release omits the journal timing code at compile time, with zero
request-path clock, branch or logging work when the feature is off.

Locked raw vectors and arm results are under
`~/calternal-ai-perf/profile-52-followup/` on perf-02. The `current-live`,
`current-live-10k` and `current-drafts` directories hold the merged release
measurements. Dev logs are under
`~/Developer/Calternal-AI-perf-fix-45fa2c0/gate-logs/` on dev-01.

| Merged release binary | SHA-256 |
| --- | --- |
| Timing-enabled | `c54b872c38e7cf476444157beb7bd2b1623485cff2b4aeceed1a9c0ddf133445` |
| Plain | `91cb64d16461d2385d151145991a4cada3b5bb6cc3fa04cc3c44b586d40e6ac4` |
| Test plugins | `8344738482e5426219c2ae2ae314c6645996ee338cfd81644dae9cfd651a0890` |

## Simplification opportunities

Record request-to-acknowledgment and acknowledgment-to-file separately, and
retain their samples alongside the commit-to-client result. This gives the
placement check its own verdict and shows which phase consumed the budget.
