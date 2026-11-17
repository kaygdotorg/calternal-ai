# Performance qualification — 2026-11-17

Lane `lane/perf-budgets`, [issue #25](https://git.kayg.org/kayg/calternal-ai/issues/25),
thread `thr_q9bredmvmy`. This is incomplete qualification until the missing scenarios
below have evidence. No budget is raised to accommodate a breach.

The reference is the existing app at `origin/dev`: `perf/budgets.json`,
`docs/perf/budgets.md`, `crates/perf/src/{stats,server}.rs`, `perf/run-vm.sh`,
and ADR 0062's Performance plan. The runner ports nearest-rank statistics,
disposable roots, real HTTP, controlled concurrency, raw samples, and interleaved
baseline/head comparisons. It reuses the software-passkey flow from this project's
conformance fixture. It introduces no product dependency or product hook.

`perf/budgets.json` records the inherited ceilings and semantic differences.
Read/list p95: 50 ms; save acknowledgement: 40 ms; live round trip: 10 ms;
startup: 300 ms; first space page: 300 ms; idle memory: 64 MiB. Session,
resolve, receipt and account-feed budgets have no equivalent in the reference;
proposals require measured distributions, never guessed comfortable limits.
The current list API returns at most 100 rows, without a cursor; the reference
page returns 50. The 2-second RSS series is before the 600-second idle deadline,
not proof of released spaces. Live write-to-client is measured concurrently with
the HTTP write, from before admission, and is an upper bound on commit-to-client.

## Commands and evidence

Build/test on `claude@calternal-dev-01-compute02`, checkout
`~/Developer/lanes/perf-budgets`, using `nix develop -c` and cargo `-j6`.
Measure only on `kayg@calternal-perf-01-backup01`, under
`/usr/local/bin/perf-lock.sh`, within `~/calternal-ai-perf`.
The runner records uname, CPU, memory, filesystems, devices, binary SHA-256s,
commits, seed, cache policy, all per-arm quantiles and p50 noise. Both roots for
userdata and the appdata root are real disk directories during timing; this does
not establish physical SSD/HDD separation. Raw samples stay on the perf VM.

Coordinator command before merging backend lanes:

```sh
PERF_BASELINE_BIN=/absolute/perf-vm/path/to/baseline/calternal \
PERF_BASELINE_COMMIT=<baseline-sha> scripts/perf-check.sh
```

The script builds release on dev-01, exports runtime libraries, ships executables
and worker bundles, and invokes the locked suite. Exit 1 is a reproduced numeric
breach; exit 2 is failed or incomplete qualification. Three cold pairs use the
host cache-drop helper, followed by five warm pairs. A breach in two head arms
fails regardless of noise; p50 spread above 25% is exposed rather than hidden.
Use identical binaries first as the baseline-vs-baseline control. Full commands
and results will be recorded with the first matrix summary.

## Missing measurements and hooks

- Admission alone: a bounded test-only timing observation between guard/admission
  and dispatch, correlated with request ID. HTTP replay includes those stages and
  is not a standalone receipt lookup benchmark.
- Commit-to-client alone: a post-commit timestamp correlated with the journal
  request ID. Existing `at_ms` precedes commit; the driver reports a conservative
  full write-to-delivery observation instead.
- Three host calls: a digest-pinned benchmark worker/operation in the trusted test
  registry that calls the existing broker exactly three times. The shipped Notes
  worker performs one call, so repeating it three times is not an equivalent test.
- Job/provider distinction: `--jobs-only` uses the existing digest-pinned
  conformance worker with zero delay and one chunk through the real stream. It
  records first-cold and subsequent warm observations, in three independent runs.
  Provider startup still needs the provider lane; this fixture contacts no paid
  provider. There is no need for a new product hook for the fixture job path.
- Idle-released memory: `--idle-only` observes three replicas each at 1, 100 and
  1,000 spaces, after 632 seconds. It checks that all per-space store writer
  threads exited. A read-only open-space count would make the cache-map release
  assertion direct; thread retirement is external evidence rather than a map
  inspection. The normal 2-second observations remain explicitly before release.
- Distribution: add the reference large-note/link/attachment mix and physical
  split disks before claiming ADR 0062 storage qualification. Do not propose 1M
  notes before a measured capacity check.

These are instrumentation/fixture requests, not changes to product behavior.
Options are test-only hooks (small product surface, exact attribution) or an
external upper-bound observation (no product hook, cannot isolate the stage).
The coordinator must assign hooks to their product owners; this lane does not
edit product crates.

## Simplification opportunities

Share an exported conformance login fixture when its owning lane can provide one;
keep one metric catalog and derive report labels from it; replace account-feed
polling with the space feed's existing wake/replay pattern if profiling confirms
the expected 250 ms floor. Product changes require their owning lanes.
