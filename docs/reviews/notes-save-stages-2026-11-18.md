# Release Notes save stage timing — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52) ·
[Historical Notes tail](notes-save-tail-2026-11-18.md)

The `perf-timings` release now emits one `notes_save` record per content
commit and an exact SQLite commit duration in the existing journal marker.
Admission covers the HTTP guard through the start of the foreground write
callback. The final executor wait is a subspan. Slot wait sums acquisition of
the space mutex across the receipt lookup, metadata lookup and write.
Content timing covers the explicit SQLite commit. Retirement covers the
autocommit outbox delete. Journal round trip includes the writer's
separately timed SQLite commit. File guard
and filename selection are also recorded. The callback emits after releasing
the slot, and a receipt replay produces no second save record.

An ordinary Notes save adds about 21 monotonic clock reads across the HTTP
callback and journal writer, plus one structured log line. The storage spans
use thread-local counters without a new worker, lock or fsync. Release
builds with the feature off omit this diagnostic path.

## Per-fixture budgets

Built both variants on dev-01 and ran on perf-02 under
`/usr/local/bin/perf-lock.sh`. The matrix used `b876bf2` with the permanent
timing changes applied. Both binaries include the placement retry fix.
Each fixture ran three warm pairs and one disk-cold pair from the same
stopped seed. Cold refers to dropping OS caches before server launch.
There were 100 saves in each 1,000- and 10,000-Note arm and 750 in each
100,000-Note arm.

| Notes and cache | Plain acknowledgment p95 vs 40 ms | Plain round-trip p95 vs 50 ms | Timing build acknowledgment p95 | Timing build round-trip p95 |
| --- | ---: | ---: | ---: | ---: |
| 1,000 warm | 26.08–29.78 ms, pass | 18.91–21.08 ms, pass | 27.44–30.90 ms | 19.67–21.73 ms |
| 1,000 disk-cold | 32.40 ms, pass | 24.60 ms, pass | 29.06 ms | 21.86 ms |
| 10,000 warm | 30.62–32.61 ms, pass | 22.87–23.26 ms, pass | 27.63–36.78 ms | 20.68–24.67 ms |
| 10,000 disk-cold | 31.81 ms, pass | 23.20 ms, pass | 26.98 ms | 20.70 ms |
| 100,000 warm | 33.94–45.64 ms, 2 of 3 breached | 34.19–37.79 ms, pass | 33.90–43.89 ms, 1 of 3 breached | 35.72–41.81 ms |
| 100,000 disk-cold | 36.10 ms, pass | 33.80 ms, pass | 34.09 ms | 33.98 ms |

The round trip ends when the subscriber receives the event. It can finish
before the writing client receives its HTTP acknowledgment. These are
per-arm p95 verdicts. Individual samples crossed the respective thresholds.
All 24 arms completed and every file-placement check passed.

After integrating the standalone pruning fence, the same instrumentation
correlated every save in an eight-arm release smoke check at 1,000 Notes.
One 30-save disk-cold plain arm breached both budgets at 107.99 and
74.89 ms p95. In a separate repeat with the same artifacts, all six plain
and six instrumented 100-save arms passed. The repeat's plain disk-cold
acknowledgment p95 ranged from 28.06 to 31.66 ms and the round trip from
20.36 to 24.06 ms. The initial cold miss did not repeat.

The final binaries also ran eight 100,000-Note pairs at 100 saves per arm.
This added five independent disk-cold starts per variant. Acknowledgment
missed in two of the three warm plain arms, as in the longer matrix. All
round-trip, commit-to-client and file-placement gates passed in this repeat.

| 100,000 Notes, final source | Plain acknowledgment p95 vs 40 ms | Plain round-trip p95 vs 50 ms | Timing build acknowledgment p95 | Timing build round-trip p95 |
| --- | ---: | ---: | ---: | ---: |
| Warm | 33.98–60.26 ms, 2 of 3 breached | 22.51–42.80 ms, pass | 31.40–38.42 ms | 22.06–27.85 ms |
| Disk-cold | 30.00–36.60 ms, pass | 22.84–25.89 ms, pass | 30.41–35.82 ms | 22.26–26.02 ms |

Across the 36 final-source arms, commit-to-client p95 ranged from 1.04 to
1.37 ms and maximum acknowledgment-to-durable-file was 80.49 ms. Those
separate gates are 10 ms p95 and 1,000 ms maximum, respectively. The integrated
evidence is retained under `~/calternal-ai-perf/profile-52-stages-final/`.

The dev-01 backend gate passed 609 workspace tests, 123 conformance tests
and one opt-in real-server test. Integration, plugins, contracts, formatting
and Podman leak checks passed. The docs gate passed in a fresh target after
the shared target supplied stale store metadata.

## Long-tail attribution

The 100,000-Note matrix logged 3,000 instrumented saves and 3,000 plain
controls. Every timed save correlated to exactly one journal and one save
record. The two slowest instrumented saves came from the same warm arm.

| HTTP acknowledgment | Admission | Slot wait | Content commit | Journal handoff and commit | Outbox retirement |
| --- | ---: | ---: | ---: | ---: | ---: |
| 136.81 ms | 0.24 ms | 0.0001 ms | 38.46 ms | 70.66 ms, including 69.81 ms in SQLite commit | 25.85 ms |
| 134.17 ms | 0.23 ms | 0.0001 ms | 86.49 ms | 30.36 ms, including 29.95 ms in SQLite commit | 15.75 ms |

Across all 3,000 instrumented saves, the largest summed space-slot wait
was 0.0007 ms, and the largest HTTP admission interval was 2.73 ms.
The initial cold maximum was 79.94 ms, with 55.08 ms in first-save filename
selection. Among the 800 additional instrumented 100,000-Note saves on the
final source, the cold maximum was 99.86 ms, including 65.48 ms in filename
selection on the first save. The warm maximum was 91.58 ms, with 47.39 ms
in the content commit, 21.38 ms in journal publication and 21.24 ms in
retirement. Maximum space-slot wait stayed below one microsecond.

The historical 966 ms event did not recur in the 3,800 instrumented saves
and 3,800 plain controls at 100,000 Notes. Its cause remains unresolved.
Durable SQLite work accounted for the observed warm tail. First-save
filename selection accounted for the slowest observed cold save. The
space-slot wait was negligible in both.

Raw arm vectors and marker correlations are retained in
`~/calternal-ai-perf/profile-52-stages/` on perf-02.

## Simplification opportunities

Use the existing request ID and journal event to associate one save record
with the HTTP and live measurements. A single storage callback record keeps
the log write outside the space lock.
