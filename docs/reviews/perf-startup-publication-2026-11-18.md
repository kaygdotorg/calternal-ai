# Startup publication timing — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52)

The `99c28cad` build from `4735733e` adds opt-in startup phase markers.
They require the `perf-timings` build feature and `CALTERNAL_STARTUP_TIMINGS=1`.
The security recovery format and SQLite schema versions stay at the baseline.
The v4 receipt and authority comparison lives on the parked receipt-delta branch.

Perf-02 ran under the perf lock with one dev-01-built release executable
(SHA-256 `58029d3e5e978a58d9765c7470f74f9e47a5935073f0f38bae7878681ac1f97e`).
Each arm used five starts from copies of the same stopped 1,000-space v3
installation. Its active security segment was 23,171,190 bytes. Warm runs
primed each copy with a preceding boot. Cold runs dropped guest page caches
before startup. Readiness was the first successful auth-status response.

| Startup | Markers off, median (range) | Markers on, median (range) | ≤300 ms, off / on |
| --- | ---: | ---: | ---: |
| Warm | 276.51 (270.84–281.42) ms | 281.63 (269.11–298.97) ms | 5/5 · 5/5 |
| Disk-cold | 557.20 (548.52–592.49) ms | 520.82 (442.88–640.18) ms | 0/5 · 0/5 |

The five instrumented cold starts spent a median 14.04 ms durably publishing
`startup.json` (13.72–14.41 ms) and 17 µs binding the listener (14–30 µs).
Router setup took 4.24 ms and startup serialization 0.66 ms. The largest
phase was opening stores, the recovery log and plugins at 340.06 ms.
The gap outside the serving phases was 56.13 ms (50.08–65.70 ms), spanning
process and runtime entry plus announcement and readiness detection.
This v3 run measures the named phases without attributing the earlier
79–83 ms residual observed on v4.

The sample log and driver are in
`~/calternal-ai-perf/perf3-startup.JXkgbUSy/` on perf-02. The release build logs are in `~/ca52-perf3.zYnegpNY/` on dev-01. Copied binaries and temporary
fixtures were removed after measurement.

Focused verification at `2493f656` after merging `origin/stack/night-4`
passed 29 tests with zero failures (7 core, 11 authorization, 9 CLI and 2
live HTTP inventory cases). Both formatter checks and the exported contract
passed. The generated-docs check passed for 403 pages. Logs are in
`~/ca52-perf3-stack.p1ohaHR1/` on dev-01. The lane full gate stopped under
the coordinator's new policy.

`lane/perf-2` at `375c576e` holds a docs-only commit that `dev` still lacks.

## Simplification opportunities

Measure process entry and the first accepted connection separately to
locate the remaining startup gap. The ordered one-pass comparison belongs
with the witnessed v4 repair path.
