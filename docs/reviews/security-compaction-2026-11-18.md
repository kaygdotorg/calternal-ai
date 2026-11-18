# Smaller security-log compaction threshold — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52) ·
[Revised receipt-delta design](../security-receipt-deltas.md)

One disposable release build of `b876bf2` selected the existing 64 MiB
threshold or 8 MiB. It kept the existing frame format and compaction
protocol. Built on dev-01 with `nix develop` and ran on perf-02 under
`/usr/local/bin/perf-lock.sh`. Each threshold seeded 1,000 spaces and one
Note per space through the real API on tmpfs. Seven AB/BA pairs started
synced copies of the corresponding stopped roots from disk with warm caches.
The clock ran from spawn to the auth status response.

| Threshold | Active segment | Snapshot | Active frames | Warm readiness median | Warm readiness range | Compactions during seeding |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 64 MiB | 61.09 MB | 1.84 MB | 252 | 327.93 ms | 322.15–354.67 ms | 3 |
| 8 MiB | 6.68 MB | 2.08 MB | 26 | 120.29 ms | 108.61–135.74 ms | 30 |

Each pair improved by 186.50–241.14 ms, with a paired median gain of
206.39 ms. Every observed 8 MiB start met 300 ms. These runs characterize warm
readiness for this fixture.

On tmpfs, compaction spans totaled 20.68 ms at 64 MiB and 110.62 ms at 8 MiB.
This measures compaction frequency and the in-memory workload. The real
authority-write cost includes more snapshot file and directory syncs on
disk. Measure the authority write p95 and p99 at threshold crossings
before changing the production setting. The threshold change also leaves
repeated receipt capture cost and idle expiry cleanup in place.

The raw paired starts and compaction spans are under
`~/calternal-ai-perf/profile-52-compaction/` on perf-02. The fixture roots were
removed after measurement.

## Simplification opportunities

Evaluate the byte threshold before introducing a frame format. Reuse the
existing witnessed compaction protocol in both approaches.
