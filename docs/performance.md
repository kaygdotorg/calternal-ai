# Performance qualification — 2026-11-17

The performance-fixes follow-up is tracked in
[issue #31](https://git.kayg.org/kayg/calternal-ai/issues/31) and its
[implementation/review report](reviews/perf-fixes-2026-11-17.md). It adds lazy
space recovery, blocking startup initialization, durable SSD acknowledgment with
bounded userdata placement, commit-driven account feeds, and debug-only stage
observations. The historical control and missing-path analysis below remain
unchanged evidence. Updated target measurements and remaining decisions are in
the follow-up report. The integrated matrix passes all 40 head-arm save p95s,
but one cold bounded-repeat arm misses 40 ms; 1,000-space startup and some
account-feed arms also miss their targets. Idle-close and engineering gates pass.
This page does not turn a partial qualification into a pass.


Performance acceptance is **blocked**. The tooling and first control matrix are
complete enough to reproduce breaches, but not to certify every requested path.
[Issue #25](https://git.kayg.org/kayg/calternal-ai/issues/25), thread
`thr_q9bredmvmy`, owns the evidence and instrumentation requests. No product
crate was edited and no inherited budget was raised.

## What was measured

The full cardinality control measured product commit
`e7f94dec65aaede2c08410bc4798c442b5f72d21`, identical binaries in baseline and head
arms (SHA-256 `6ae339d3111b11a2fe65d371b5f340140f70a054559967810e33fe2e5b49a0b0`).
This establishes repeatability, not a regression against the old application.
The lane subsequently merged `origin/dev` at
`f8c4ac30e0e67c0fe860b97fc81d05d80aadcb4a` in `e712ff4`; engineering gates were
repeated on that merged tree. The separate merged-server control is described below.

The port-first reference is the existing app at
`ea57b84b9c9e40e8b67b7d1e1f1805915123b040`: `perf/budgets.json` (blob
`aaf25a669ca2b362ed9d0cb1921bc702cfb8cd12`), `docs/perf/budgets.md`,
`crates/perf/src/{stats,server}.rs`, `perf/*.sh`, and ADR 0062's Performance plan.
The new harness ports nearest-rank statistics, disposable roots, real HTTP,
interleaved comparisons and the noise policy; login reuses the conformance
software-passkey flow. The old app itself was not benchmarked in this run.

Machine: `calternal-perf-01-backup01`, KVM, four vCPUs reporting AMD Ryzen 7 3700X,
8,262,774,784 bytes RAM, no swap, Debian kernel `6.12.115+deb13.1-amd64`.
All measurements held `/usr/local/bin/perf-lock.sh`. Appdata and both userdata
roots were separate directories on `/dev/sda1` (ext4, nonrotational guest device).
The HDD root was empty: this is not physical SSD/HDD or attachment qualification.
`GLIBC_TUNABLES` was unset; the reference pins two malloc arenas, so memory
comparisons have that qualification. Build tools ran only on dev-01.

Fixtures use seed 20261117, short Markdown, real server-issued UUIDs and real
sessions. Provisioning is untimed in private tmpfs roots; stopped roots are copied
to disk for every arm. Warm copies are synced before starting the timer. Cold
pairs additionally run the host cache-drop helper. Concurrency is one outstanding
operation, with two live subscriptions. This is not a saturation test.
The matrix covers 1k/10k/100k notes in one space and 100 spaces of 100 notes.
The 1,000-space fixture was built, but readiness failed after both 30- and
300-second observation windows. A 1M-note fixture is not proposed without capacity
and startup fixes.

## Results

[Summary JSON](../perf/results/2026-11-17.json) contains both control arms,
p50/p95/p99/max, sample counts, noise, machine metadata and binary hashes.
For sampled operations the table reports the median of head-arm p95s, in ms.
Startup and RSS have one sample per arm, so their quantiles pool those singleton
values instead. All maxima remain visible in JSON.

| Fixture | Get warm/cold | Update warm/cold | List first 100 warm/cold |
| --- | ---: | ---: | ---: |
| 1 × 1,000 | 0.479 / 0.473 | 47.787 / 49.524 | 0.705 / 0.703 |
| 1 × 10,000 | 0.474 / 0.524 | 50.177 / 52.213 | 0.706 / 0.762 |
| 1 × 100,000 | 0.502 / 0.514 | 57.276 / 57.371 | 0.743 / 0.755 |
| 100 × 100 | 0.495 / 0.477 | 51.335 / 50.756 | 0.687 / 0.742 |

Inherited p95 budgets are get 50 ms (`open.api_get`), update 40 ms
(`sync.save_ack`) and list 50 ms (`home.page`). Update breaches reproduce in
all four fixtures. The reference page has 50 rows; this API returns at most 100
and has no cursor. The save APIs also differ; those differences do not authorize
raising the inherited ceilings.

| Observation | Result | Inherited p95 budget / qualification |
| --- | ---: | --- |
| Startup, 1 space / 1k notes, warm p95 | 60.55 ms | 300 ms |
| Startup, 1 space / 10k notes, warm p95 | 82.67 ms | 300 ms |
| Startup, 1 space / 100k notes, warm p95 | 120.68 ms | 300 ms |
| Startup, 100 spaces, warm p95 | **2,300.37 ms** | **300 ms: reproduced breach** |
| Startup, 100 spaces, cold p95 | **4,377.73 ms** | 300 ms; noisy, repeated, still unqualified |
| Startup, 1,000 spaces | Not ready within 300 s | 300 ms; no invented percentile |
| RSS, 1 space, 632 s idle, p95 | 21.680 MiB | 64 MiB; three replicas, writer threads retired |
| RSS, 100 spaces, 632 s idle, p95 | 62.375 MiB | 64 MiB; three replicas, writer threads retired |
| RSS, 1,000 spaces, released | Blocked by startup | 64 MiB; no value |
| Full write-to-subscribed-client, warm p95 range | 47.73–57.28 ms | 10 ms proxy ceiling breached; cannot isolate post-commit delivery |
| Draft save to account feed, warm p95 range | 253.17–253.39 ms | No reference limit |
| Session check, warm p95 range | 0.240–0.267 ms | No reference limit |
| Resolve `/notes/<UUID>`, warm p95 range | 0.312–0.332 ms | No reference limit |
| Receipt HTTP replay, warm p95 range | 0.868–0.909 ms | Includes guard/admission; not lookup alone |
| Fixture job first chunk, warm median per-run p95 | 75.959 ms | Three runs, 297 samples; no provider contacted |
| Fixture job first-cold p95 | 291.964 ms | Three first-job observations; not an OS cache-drop test |

The first list after restart is around 1 ms warm, but eager startup already opens
spaces. It does **not** establish genuinely cold-space latency; the catalog states
that limitation. Two-second RSS observations are retained separately and do not
claim idle release. Six servers idled concurrently for the released-memory test;
no latency samples were taken during that test.

New-path and Notes p50 proposals in `perf/budgets.json` are explicitly unaccepted
observations, derived from the summary with no added headroom. Existing p95
ceilings remain unchanged. Missing-path proposals remain null. Options for the
owner: adopt measured control limits after the missing cases are qualified
(earlier regression protection, but narrow fixture coverage), or keep observations
until the broader mix is measured (better coverage, later enforcement). Neither
choice is made by this lane.

## Merged-server control

A fresh plain release build from `e712ff4a7ac5d9ec6a188db1d21734c6c6030cc2`
(SHA-256 `fe4886d34a9ebaa0a25c1983c2dfe4c89484d0423fa8c0b725a919f6b151f40f`)
ran a separate identical-binary control: one space / 1k notes, three warm AB/BA
pairs, 100 samples per arm, no cold pairs. Its median head-arm p95 is get
0.528 ms, update **46.834 ms**, list 0.714 ms; startup p95 across head arms is
59.886 ms. All six arms completed, exit 1: the update breach persists.
[Separate summary](../perf/results/2026-11-17-merged-control.json) preserves the
exact hashes and distributions. This smaller check does not qualify the merged
product at every cardinality or replace the pinned full control.

Build command on dev-01: `nix develop -c cargo build --release --locked -j6 -p
calternal-server -p calternal-perf` (the plain server was copied before a separate
`--features test-plugins` build). Workers came from `npm run build:plugins`.
At `~/calternal-ai-perf/20261117-merged` on perf-01:

```sh
/usr/local/bin/perf-lock.sh ./bin/calternal-perf \
  --bin "$PWD/bin/calternal" --baseline "$PWD/bin/calternal" \
  --work "$PWD/control" --plugins "$PWD/plugins" --budgets "$PWD/budgets.json" \
  --sizes 1000 --spaces 1 --pairs 3 --cold-pairs 0 --samples 100 \
  --commit e712ff4a7ac5d9ec6a188db1d21734c6c6030cc2 \
  --baseline-commit e712ff4a7ac5d9ec6a188db1d21734c6c6030cc2 --json
```

## Reproduction, noise and exclusions

Quantiles use nearest rank. AB/BA order alternates. At least two breached head
arms constitute a reproduced breach. Noise is `(max arm p50 - min arm p50) /
median arm p50`; above 25% triggers one bounded additional three-pair run, with
cold pairs repeated when affected. High noise never excuses a reproduced breach.
Corrected warm update noise is 1.5–20.9%. Warm 100-space startup noise is 13.3%.
Cold 100-space startup noise was 53.8%; its repeat remained noisy at 34.9%.
The independent low-noise warm runs prove the startup breach; cold precision
remains unqualified.

The dated summary selects 24 initial cold arms and 40 corrected warm arms.
Original warm arms and their early warm repeats are excluded because stopped-root
copy writeback had not been synced. The corrected harness adds that sync before
timing. Preliminary smoke metadata, failed request-ID job attempts, cancelled
idle attempts and all instrumented latency samples are excluded. The request-ID
collision was fixed with a fresh per-client nonce. Failed-arm continuation was
checked at 129 spaces: two failures per arm, four total, exit 1 with preserved
failure/partial-summary evidence.

Raw samples, failure logs and profiles remain at
`kayg@calternal-perf-01-backup01:~/calternal-ai-perf/20261117-control`.
Raw retention is 30 days; scripts and summaries stay for the issue lifetime.
Credential-bearing roots are removed after collection. Summary-only JSON is in
git; no sessions, credentials, raw vectors or binary profiles are committed.

Exact main commands, run on perf-01 under the lock (paths below are relative to
`~/calternal-ai-perf/20261117-control`; each uses absolute paths in execution):

```sh
/usr/local/bin/perf-lock.sh ./bin/calternal-perf-clean \
  --bin "$PWD/bin/calternal" --baseline "$PWD/bin/calternal" \
  --work "$PWD/verified-warm" --plugins "$PWD/plugins" \
  --budgets "$PWD/budgets.json" --sizes 1000,10000,100000 --spaces 1,100 \
  --pairs 5 --cold-pairs 0 --samples 100 --seed 20261117 \
  --commit e7f94dec65aaede2c08410bc4798c442b5f72d21 \
  --baseline-commit e7f94dec65aaede2c08410bc4798c442b5f72d21 --json
```

The initial `full` invocation used the same sizes, `--spaces 1,100,1000`,
`--pairs 5 --cold-pairs 3 --samples 100`; it stopped at the 1,000-space startup.
Only its cache-dropped cold arms qualify. The longer retry used
`--sizes '' --spaces 1000 --startup-timeout-seconds 300`.
Released memory used `--idle-only --spaces 1,100`, other paths unchanged;
job runs used `--jobs-only --test-plugin-root "$PWD/tests/plugins"`, a separately
built `test-plugins` binary and fresh `jobs-fixed-0`, `-1`, `-2` work roots.
Normal/jobs/idle returned 1/2/2 respectively: reproduced breach / incomplete
coverage / fixture-only coverage. None is a backend acceptance pass.

Coordinator gate before merging backend lanes:

```sh
PERF_BASELINE_BIN=/absolute/perf-vm/path/to/baseline/calternal \
PERF_BASELINE_COMMIT=<baseline-sha> scripts/perf-check.sh
```

That script syncs tracked/untracked source to dev-01, runs `nix develop -c` release
builds with `cargo -j6`, ships plain and test-feature binaries, their Nix runtime
closures and built workers, then runs all measurements under the perf lock. The
runtime closure is relayed through the coordinator because dev-01 cannot resolve
perf-01. It repeats noisy cases, runs idle and fixture-job checks, preserves raw
evidence and removes its dev targets. Exit 1 means a reproduced breach; exit 2
means failed or incomplete qualification. Mandatory missing hooks keep it nonzero.
Engineering gate commands and all 15 review items are in the
[dated review](reviews/perf-budgets-2026-11-17.md).

## Breach causes and the three most useful changes

1. **Move blocking initialization off the async root, then open spaces lazily.**
   A separate boundary probe found 128 one-note spaces ready in 2.982 s (0.32 CPU
   seconds), versus 129 unready after 10.006 s (7.36 CPU seconds). Startup profiling
   of 1,000 spaces attributes 88.36% of sampled CPU to wake/park inside
   `calternal_content::journal::block_on`. `async serve` calls `Store::open`; the
   custom synchronous bridge polls Tokio join handles without yielding the root
   task. The pinned Tokio 1.54.1 source starts a cooperative budget of 128. Once
   exhausted, its join-handle poll wakes this loop without completing. This
   explains the sharp boundary. Offloading blocking initialization fixes the
   scheduler violation; lazy per-space recovery/open is also needed for the
   300 ms cardinality budget. A blanket unconstrained future leaves blocking I/O
   on the async thread and is not an adequate fix. Evidence:
   `startup-profile-20261117T165246Z/{perf.data,perf-report.txt}` and
   `startup-boundary-3111966/summary.json`.
2. **Reduce durability work before the save acknowledgement within ADR 0062.**
   The clean separate write profile recorded 1,207 `fsync` calls taking 7.028 s,
   95.22% of selected syscall wall time, across its mixed workload. CPU stacks
   include `Content::write`, placement/atomic-file writes and SQLite preparation;
   this is evidence of a durability-bound path, not a precise per-update fsync
   count. Profile file `profile-20261117T175337Z/syscalls.txt`; CPU profile has
   40 samples, zero lost, so fine-grained CPU percentages are weak evidence.
   Reuse the accepted SSD durable journal acknowledgement and bounded userdata
   materialization policy; the owning lane should count fsyncs and remove duplicate
   durability boundaries without weakening recovery guarantees.
3. **Wake account-feed subscribers after commit.**
   Draft-feed p95 stays near 253 ms, matching the current 250 ms polling interval.
   Reuse the journal feed's wake/replay approach instead of a separate polling
   loop. This cause is a source-and-latency inference, not a measured post-commit
   span; the owner must preserve replay/overflow semantics and verify it with the
   exact commit marker.

Profiles were collected with `perf/profile.sh <suite> <commit> <clean-driver>`
and `perf/startup-profile.sh <suite> <commit> <driver>`, both acquiring the lock.
Tools were extracted from Debian packages built elsewhere; no compilation or
system tool installation occurred on perf-01. `perf record/report` succeeded;
`strace` ended at its intentional timeout (124). Instrumented driver exit 1 is
expected from inherited breaches; its latency values do not enter the baseline.

## Missing measurements and owner decisions

- Admission and receipt lookup alone need bounded, test-only stage timings
  correlated by request ID; HTTP replay cannot separate them.
- Exact commit-to-client needs a timestamp after durable commit correlated with
  SSE. Existing `at_ms` precedes commit. The broader write-to-client proxy exceeds
  10 ms, but that does not prove the isolated post-commit stage does.
- A digest-pinned trusted benchmark worker must make exactly three host calls.
  Today's Notes worker makes one; three independent invocations are not equivalent.
- True cold-space opening needs a controlled lazy-open/reset hook or proof that
  startup did not open it. Otherwise report only first-list-after-restart.
- A read-only cache-open count would strengthen idle release verification; writer
  retirement is external evidence. The 1,000-space case awaits the startup fix.
- Provider startup, large notes, links, attachments, physical tier movement and
  real-provider first chunks remain unqualified.

For timing hooks, options are small test-only product observations (precise
attribution, a maintained test surface) or external upper-bound measurements
(no product surface, cannot certify the isolated budget). The coordinator must
assign these to product owners; this lane's scope excludes product edits.

## Simplification opportunities

Export one conformance software-passkey helper for tool consumers. Keep one metric
catalog and derive labels from it. Reuse the existing feed wake/replay primitive
and lazy space lifecycle rather than adding another polling or cache mechanism.
