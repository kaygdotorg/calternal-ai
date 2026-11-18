# Edit-to-ready with draft diagnostics

2026-11-18 · [Lane #50](https://git.kayg.org/kayg/calternal-ai/issues/50)

The enrolled-machine author loop stays under its one-second budget with the
draft diagnostics enabled. Every run had a p95 below 1,000 ms (829 to 925 ms).
The added instrumentation is not separable from run-to-run noise at p95.

| Server | Runs | p50 per run | p95 per run | Median p95 |
| --- | ---: | --- | --- | ---: |
| Before (`a90843fe`) | 3 | 786.0, 795.8, 812.6 ms | 834.8, 832.5, 866.6 ms | 834.8 ms |
| With diagnostics | 4 | 787.9, 797.6, 800.7, 816.8 ms | 829.0, 869.2, 877.6, 925.0 ms | 873.4 ms |

Each run is 30 measured edits after two warm-ups. Edit to ready at p95 and
the other phases are in [the samples](plugin-readiness-2026-11-18.json). Ready
to first result stayed at 6.7 to 8.6 ms p95 in both. The "with diagnostics" set
includes a first run (`new-probe`) taken before the alternating series, because
it passed the same assertions. The after-run p95 rose with run order for both servers
(runs 1 to 3 interleaved base, new), so the 925 ms is partly drift on the
machine. A single run's p95 is the 29th of 30 samples and is itself noisy.

## Method

`calternal-perf-01-backup01` under `/usr/local/bin/perf-lock.sh`, one run per
lock, alternating before and after (`base r1, new r1, base r2, ...`). Both
x86_64-musl release servers (`--features test-plugins,dev-plugin-drafts`) and the
prebuilt `isolation` conformance executable were built on dev-01 inside
`nix develop .#deploy`; nothing was built on perf-01. The test is
`edit_to_first_valid_plugin_result` with `CALTERNAL_MEASURE_RELOAD=remote`,
`CALTERNAL_SERVER_BIN` set to each server and the lane's Podman wrapper and
cached `node:24-alpine` image on `PATH`. It starts an enrolled machine, scaffolds
a watched plugin, edits its TypeScript source and measures from the file rename
to the ready event and to the first correct HTTP result through the real broker.
It checks the unchanged server PID for every sample.

Both servers contain the same production code except the instrumentation: the
before build is the merge `a90843fe`. The instrumentation is three `Instant`
reads and one `OnceLock` write per published revision, plus a bounded
in-memory insert on a failed candidate or failed operation. Nothing is written
to disk, so there is no stored format to migrate.
