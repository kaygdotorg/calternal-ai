# Performance fixes — issue #31

Thread: `thr_i78z9xkxhy`. Branch: `lane/perf-fixes`. Measurements use only
`kayg@calternal-perf-01-backup01` under `/usr/local/bin/perf-lock.sh`.
Builds and engineering gates use only `claude@calternal-dev-01-compute02`,
`~/Developer/lanes/perf-fixes`, with `nix develop -c` and Cargo `-j6`.
The main matrix meets the save-acknowledgment target, but a bounded repeat
contains a save p95 miss. Startup and account-feed targets also remain partially
unmet, so performance acceptance stays blocked. Numeric budgets have not changed.

## Commits

- `f484570`: lazy startup and blocking initialization.
- `7e9ac54`: durable SSD acknowledgment and bounded placement.
- `d151500`: account commit wakes and replay.
- `2c9ed15`, `50adee8`: validated recovery reuse and canonical hash reuse.
- `9064baf`: deferred placement respects unpublished intents.
- `ba25be2`, `a31ccb1`: sync/stage diagnostics and focused startup driver.
- `42d6a46`: reference TCP_NODELAY policy.
- `32aa64f`: durable crash-fixture baseline before fault arming.
- `1d7709a`: bounded retained account readers and maintenance.
- `7f4880c`: acknowledged replay independent of later userdata conflict.
- `3430102`: matching historical plugin-bundle selection in the driver.
- `071740a`: complete baseline placement before cleanup/tamper fixtures.
- `92b2993`: debug admission stderr emission moves to the blocking pool.
- `244b644`, `0126438`: integrate `origin/dev` at `18a54a2`, preserve threads
  lifecycle placement and consolidate mutation methods.
- `8a7acfa`: observe bounded standard-collection placement in its file fixture.
- `8bcb09e`: merge `origin/dev` at `f56c786`, preserving blocking initialization
  and the explicit local-worker controls.
- `5291228`: schema-6 performance fixtures reuse the initial personal-space UUID.
- `8d7553b`: Notes/plugin canonical hashing on the existing blocking pool.
- `40a3848`: CLI definitions split from the runtime entry point.

- `ae8823a`: pinned original comparison/diagnostic summaries.
- `6328e9b`: integrated matrix, bounded repeats, stage/job and idle summaries.

## Integrated schema-6 artifact

The [current 80-arm matrix](../../perf/results/2026-11-17-perf-fixes-integrated-matrix.json)
pins runtime `40a3848`, after merging `origin/dev` at `f56c786`. Its compatible
baseline `5291228` already contains the three core fixes and the schema-6 fixture
correction. This comparison qualifies the integrated artifact; the earlier
schema-5 before/after matrix below supplies the original cause comparison.
Both current arms use the same trusted plugin bundle. No schema compatibility
reader or migration was added. Fixtures reuse the server-provisioned personal
space UUID and assert exact requested cardinality.

Five warm and three disk-cold pairs per fixture, 100 updates and draft saves per
arm: 80 completed arms, zero arm failures. All 40 head-arm update p95s satisfy
40 ms (24.216–36.230 ms). Highest individual update is 100.063 ms. File durability
p95 is 38.846–57.033 ms across arms, with highest sample 119.119 ms; every measured
placement meets the one-second bound. Full-write-to-space-feed proxy p95 remains
35.623–49.050 ms, over its unchanged 10 ms ceiling in every head arm. Exit 1.

| Fixture | Update p95 range, head arms | Draft-to-account-feed p95 range |
| --- | ---: | ---: |
| 1 × 1k notes | 24.216–29.189 ms | 8.153–12.533 ms |
| 1 × 10k notes | 27.460–32.089 ms | 8.259–12.420 ms |
| 1 × 100k notes | 29.231–34.398 ms | 8.901–11.701 ms |
| 100 × 100 notes | 27.605–29.180 ms | 7.888–11.757 ms |
| 1,000 × 100 notes | 28.202–36.230 ms | 9.362–12.854 ms |

The single-digit draft-to-feed target remains unmet. This client span includes
the durable HTTP save; it is not an isolated post-commit latency. Its catalog
budget remains null rather than silently adopting a new accepted ceiling; the
user's single-digit target is assessed explicitly here.

Startup p95 pools singleton starts (maximum of five warm or three cold starts):

| Spaces, 100 notes each | Baseline ready warm / cold | Head ready warm / cold | Head genuine first-open warm / cold |
| --- | ---: | ---: | ---: |
| 100 | 157.748 / 229.017 ms | 71.657 / 245.078 ms | 16.302 / 25.376 ms |
| 1,000 | 519.140 / 848.874 ms | 377.200 / 802.131 ms | 18.066 / 24.256 ms |

All eight 100-space head starts meet 300 ms; all eight 1,000-space starts miss.
Each of the 40 head arms has zero space writers before first use. The one-space
100k-note first-open p95 is 49.099 ms warm and 666.055 ms cold, exceeding its
existing 300 ms harness ceiling in the cold group. That group's 65.53% noise,
1,000-space cold startup's 39.62%, and its warm full-feed proxy's 29.31% select
bounded repeats for those two fixtures. No result is waived for noise.

The [bounded 100k-note repeat](../../perf/results/2026-11-17-perf-fixes-integrated-repeat-1x100000.json)
completes twelve arms (three warm and three cold pairs), zero arm failures, exit
1 for the unchanged full-write/feed proxy. No head metric has noise over 25%.
Cold genuine first-open p95 across its singleton starts is 243.250 ms (individual
starts 197.298–243.250), versus 49.724 ms warm. All six head update p95s are
30.856–33.491 ms, draft-to-account-feed 7.894–9.435 ms, and file-durability p95
49.026–51.828 ms. This repeat passes those numeric targets without deleting or
waiving the original cold-open and feed observations.

The [bounded 1,000-space repeat](../../perf/results/2026-11-17-perf-fixes-integrated-repeat-1000x100.json)
completes twelve arms, zero failures, exit 1. Head startup p95 is 319.499 ms warm
and 858.852 ms cold, versus the compatible baseline's 416.745/828.423 ms. Every
head start misses 300 ms. Genuine first-open head p95 is 17.392 ms warm and
52.059 ms cold. Cold startup noise remains 56.16%; one bounded repeat is complete
and that uncertainty remains, rather than rerunning until a passing sample.

The third cold head arm has update p95 **49.928 ms**, above 40 ms; the other two
cold head arms are 28.092 and 37.132 ms. All three warm head update p95s are
28.437–32.507 ms. Only one of the six head arms misses 40 ms, so this is not two
reproduced save breaches under the harness's repeated-breach rule, but it prevents
a claim that every measured arm meets the requested target. The slow arm also
has slower startup and draft-save observations; that alone does not establish a
cause. Draft-to-account-feed p95 ranges 7.991–13.310 ms cold and 8.969–10.150 ms
warm. Original-matrix passes and repeat misses are both retained.

The [current debug stages](../../perf/results/2026-11-17-perf-fixes-integrated-stages.json)
use the same `40a3848` debug feature artifact against itself, six warm arms,
zero arm failures. Per-arm p95 ranges across all six: admission 0.377–0.403 ms
(200 samples/arm), receipt lookup 0.419–0.464 ms (200), and space-journal
post-commit marker to client 10–21 ms (100). The latter has 1 ms clock precision;
it is neither release acceptance nor the private-account commit stage. Exit 1
retains the unchanged full-write/feed proxy. These observations do not add any
release clock, allocation, output, or task.

The [three worker-fixture observations](../../perf/results/2026-11-17-perf-fixes-integrated-jobs.json)
use release `test-plugins` with the explicit `CALTERNAL_WORKER_TEST_BACKEND=local`
mode. First-cold chunk latencies are 216.133, 126.986 and 130.434 ms; subsequent
99-sample warm p95s are 39.777, 41.302 and 38.494 ms. Each exits 2 for incomplete
qualification. The accepted catalog has no numeric job ceiling. This is the
trusted, digest-pinned conformance worker with no provider or artificial delay;
it proves neither a production sandbox nor provider startup nor a three-host-call
operation. Production worker controls stay explicit.

The [current idle suite](../../perf/results/2026-11-17-perf-fixes-integrated-idle.json)
activates every space, retains at most 64 writers per server, then waits the
production 600-second deadline plus sweep allowance. All nine independent
servers have zero writers after the shared 632-second wait:

| Spaces, 100 notes each | Released RSS range | Writers after idle |
| --- | ---: | ---: |
| 1 | 24.141–24.230 MiB | 0 / 0 / 0 |
| 100 | 48.230–48.754 MiB | 0 / 0 / 0 |
| 1,000 | 57.352–57.891 MiB | 0 / 0 / 0 |

All observations meet 64 MiB. Exit 2 means broader backend qualification remains
incomplete; it is not an idle numeric breach. No latency workload overlaps this
suite. The [integrated diagnostic summary](../../perf/results/2026-11-17-perf-fixes-integrated-diagnostics.json)
records all 58 head-side startup observations with zero pre-use writers, artifact
hashes, every phase's exit code, and **zero remaining credential-root directories**
in this run. The wrapper exits 1 for the retained performance breaches; engineering
gates remain separate and pass. Raw vectors/logs, copied binaries and dev gate
logs have a 30-day retention; only credential-free summary JSON enters git.

A [same-artifact 1k control](../../perf/results/2026-11-17-perf-fixes-integrated-control.json)
uses `5291228` against itself, three warm pairs, 100 samples per arm. It is a
repeatability observation, not a product baseline without these fixes. The
summary retains all quantiles/noise and the unchanged proxy breach.

Current binary SHA-256 values:

| Artifact | SHA-256 |
| --- | --- |
| Plain server, `40a3848` | `527f2e41e881bf462473397268d986607d75d788f86a9d1047adfdc741ac3180` |
| Plain baseline, `5291228` | `597f894aa9871ae7f6e56268b80f2dde7a91f6b1c7c00d66cca1f81605bf3df7` |
| Driver | `099460bf9f4224e55e254455136d31e99cee80d00210d2daefd3b2dae13a91a6` |
| Release test-feature server | `06d5b81062dceeb59e98a23b08e29f7eb9164cd655517092c2bc66eb5bcc3e2b` |
| Debug test-feature server | `fb4416fb02cc1dfea1ede519931d13498e555811a1fc84083cb0e49574b37ab9` |

The later documentation/comment clarification adds no runtime change. Measurements
stay pinned to these artifacts, rather than relabeled as a newly built binary.

## Integrated measurement commands

In `~/calternal-ai-perf/perf-fixes-integrated` on perf-01, all phases run
sequentially inside one `/usr/local/bin/perf-lock.sh` invocation; `root=$PWD`:

```sh
./bin/calternal-perf --bin "$root/bin/calternal-baseline" \
  --baseline "$root/bin/calternal-baseline" --work "$root/control" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes 1000 --spaces 1 --pairs 3 --cold-pairs 0 --samples 100 \
  --commit 5291228 --baseline-commit 5291228 --json
./bin/calternal-perf --bin "$root/bin/calternal" \
  --baseline "$root/bin/calternal-baseline" --work "$root/matrix" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes 1000,10000,100000 --spaces 1,100,1000 \
  --pairs 5 --cold-pairs 3 --samples 100 \
  --commit 40a3848 --baseline-commit 5291228 --json
python3 repeat-plan.py "$root/matrix/summary.json" > repeat-plan.txt
./bin/calternal-perf --bin "$root/bin/calternal" \
  --baseline "$root/bin/calternal-baseline" --work "$root/repeat-1x100000" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes 100000 --spaces 1 --pairs 3 --cold-pairs 3 --samples 100 \
  --commit 40a3848 --baseline-commit 5291228 --json
./bin/calternal-perf --bin "$root/bin/calternal" \
  --baseline "$root/bin/calternal-baseline" --work "$root/repeat-1000x100" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes '' --spaces 1000 --pairs 3 --cold-pairs 3 --samples 100 \
  --commit 40a3848 --baseline-commit 5291228 --json
```

The repeat planner selects a fixture only when a head metric has noise above
25% and at least two breached arms. Cold selection keeps three cold and three
warm pairs; there is only one bounded repeat per selected fixture.

Additional sequential commands inside that same lock:

```sh
./bin/calternal-perf --bin "$root/bin/calternal-debug" \
  --baseline "$root/bin/calternal-debug" --work "$root/stages" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes 1000 --spaces 1 --pairs 3 --cold-pairs 0 --samples 100 \
  --commit 40a3848 --baseline-commit 40a3848 --json
# Execute once for each repeat=0,1,2:
CALTERNAL_WORKER_TEST_BACKEND=local ./bin/calternal-perf \
  --bin "$root/bin/calternal-test" --baseline "$root/bin/calternal-test" \
  --work "$root/jobs-$repeat" --plugins "$root/plugins" \
  --test-plugin-root "$root/tests/plugins" --budgets "$root/budgets.json" \
  --commit 40a3848 --baseline-commit 40a3848 --samples 100 --jobs-only --json
./bin/calternal-perf --bin "$root/bin/calternal" \
  --baseline "$root/bin/calternal" --work "$root/idle" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --spaces 1,100,1000 --commit 40a3848 --baseline-commit 40a3848 --idle-only --json
```

## Implementation and recovery ordering

Startup moves store, root recovery, catalog/registry and authority initialization
onto blocking workers. The synchronous journal bridge parks on real wake-ups
instead of repeatedly yielding while polling. Installation locks, root bindings, security-log validation,
authority reconciliation and trusted plugin validation remain eager: serving
before them could admit requests under stale grants or wrong installation state.
Content initialization does not enumerate/open spaces. First use opens that
space's journal and content database, awaits initial receipt pruning, cleans owned
temporary files, publishes unfinished intents, then finishes placement before
exposing the space. The existing cache retains at most 64 spaces, evicts unused
entries under pressure, and closes idle entries after 600 seconds with a 30-second
sweep. Pending placement pins its existing slot. Root reconciliation reuses its
validated stamp-checked image; each immutable canonical proposal is hashed once.
The on-disk protocol and witness checks are unchanged.

Save acknowledgment follows three FULL WAL commits: content/history and outbox,
journal event/receipt, then outbox retirement. Under independent receipt pruning,
retirement cannot simply become NORMAL:
a power loss could resurrect a retired intent after its receipt expires and
change request-ID reuse semantics. Userdata placement uses the existing lifecycle
worker, awakened by commit. It fsyncs the file and parent, then marks placement
with replayable NORMAL bookkeeping. The oldest pending age never resets; a
one-second miss refuses further writes until placement recovers. Failed placement
retries every 50 ms without a separate task queue. Worker I/O retains installation
root locks. A subsequent write to the same item first finishes its pending
placement, preserving outside-edit checks. Trash retains explicit placement before
thread authority cleanup. The additive `Content::retain_root_guard` lets the host keep validated installation
locks alive during background work; it does not change existing call signatures.
Explicit `Content::materialize` remains for threads;
the obsolete server adapter and receipt association are removed. A replay returns
the original acknowledged SSD result even if later outside edits block userdata.

The account feed subscribes before head/replay and treats its capacity-one wake
channel as a hint. Successful commits send hints; replay determines actual events,
including lag/reset. The wake sender map uses weak references and removes unused
entries. Each admitted stream retains the existing SQLite connection, avoiding
last-connection checkpoint/delete/reopen for every save. Its page-cache setting
is 64 KiB, not a claim about total connection memory. Active and closing readers
share the existing listener ceiling; connection close runs on the blocking pool
while holding the admission lease and root guards. No transaction is held across
an async wait. Ordinary commit wakes skip cleanup already done by the mutation;
authority changes and an absolute one-second heartbeat retain cleanup/age checks.
Actual content operations always reauthorize. TCP_NODELAY follows the reference
app's listener policy; the isolated control did not identify it as the dominant
feed cost.

Debug `test-plugins` builds alone emit admission, receipt lookup and post-commit
markers correlated by request ID. Every hook is cfg-eliminated from all release
builds, including test-plugin release builds. Debug admission emission is awaited
on the existing blocking pool so stderr I/O stays off async threads; its elapsed
span is captured before that diagnostic task. Admission excludes HTTP sign-in and
later agent scope review. Receipt lookup includes pending publication and the
space lock. Post-commit/client correlation has 1 ms clock precision. Instrumented
debug results are separate from plain-release acceptance measurements.

## Read and durability accounting

A warm same-title ordinary content update reads the existing file once for its
outside-edit guard. `Content::write` performs 12 logical SQL SELECTs: nine content,
one catalog route and two journal receipt reads, including empty outbox checks.
The Notes adapter adds one prior head/body read to preserve title, the outer
receipt replay guard adds two reads, and item resolution adds one: 16, excluding
authentication/policy and plugin metadata. Kind registration/encoding definitions
reuse the bounded 32-entry cache. Request fingerprinting uses the existing canonical helper once on a blocking worker;
the same parsed object returns without an additional body clone for handoff.
The existing shared canonical encoder makes two JSON-tree copies before encoding;
this lane reuses its byte-stable digest semantics. Metadata is serialized once for the transaction;
publication reads kind rather than rereading the body. First activation, pending
placement, a rename, new item routing and checkpoint thresholds add work and are
reported separately rather than charged as steady-state reads.

Before this lane, the warmed update had four WAL syncs (content commit, journal
commit, outbox retirement, placement mark), plus userdata file and directory
syncs before acknowledgment: six durability calls. Afterward, three WAL syncs
precede acknowledgment; file and directory sync follow, totaling five. NORMAL
bookkeeping does not add a per-update WAL sync. The independent journal witness
flush is delayed/batched and remains counted separately. First WAL creation,
checkpoints, new directories and renames add boundaries. Exact traced observations
are recorded below; traced timings are never used for latency acceptance.

## Pinned pre-schema-6 plain-release matrix

[Complete summary](../../perf/results/2026-11-17-perf-fixes-final.json): 80 arms,
zero arm failures, five warm and three disk-cold pairs at each fixture. Exit 1
records reproduced numeric breaches. Each arm has 100 timed updates and 100 draft
saves. JSON retains p50/p95/p99/max, noise, sample counts and metadata. Tables below
show median per-arm p95s in ms; the last column preserves the highest head-arm
p95 across both cache conditions. This does not hide a breached arm behind its
median. Startup uses pooled singleton observations, so p95 is the maximum of
five warm or three cold starts.

| Fixture | Update before warm / cold | Update after warm / cold | Highest after p95 |
| --- | ---: | ---: | ---: |
| 1 × 1k notes | 47.916 / 51.001 | 25.796 / 24.613 | 28.907 |
| 1 × 10k notes | 50.119 / 52.006 | 29.163 / 27.781 | 33.720 |
| 1 × 100k notes | 57.434 / 58.449 | 33.029 / 32.091 | 35.495 |
| 100 × 100 notes | 57.092 / 56.746 | 30.149 / 28.839 | 34.004 |
| 1,000 × 100 notes | 57.207 / 60.152 | 31.875 / 29.248 | 33.722 |

**Save target met on the measured artifact:** all 40 head-arm update p95s are
below 40 ms, ranging 24.492–35.495 ms. Individual update max is 118.029 ms;
the p95 claim is not a maximum-latency claim. Conservative write-to-durable-file
p95s range 39.265–55.436 ms, with maximum 133.003 ms. No observed update misses
the one-second materialization bound. Each update checks the read-only mark
written after both file and directory fsyncs; this span includes HTTP and feed
observation before polling, making it an upper bound rather than a subtracted
background-placement estimate.

| Fixture | Draft-to-feed before warm / cold | After warm / cold | Highest after p95 |
| --- | ---: | ---: | ---: |
| 1 × 1k notes | 253.211 / 253.040 | 8.911 / 7.931 | 9.671 |
| 1 × 10k notes | 253.119 / 253.265 | 10.381 / 9.547 | 11.826 |
| 1 × 100k notes | 253.189 / 253.224 | 9.348 / 8.120 | 23.494 |
| 100 × 100 notes | 253.186 / 253.198 | 9.230 / 8.384 | 11.925 |
| 1,000 × 100 notes | 253.041 / 253.220 | 9.511 / 10.057 | 13.417 |

**Single-digit feed target remains unmet.** The 250 ms poll is removed. At 10k,
paired client time outside the saved HTTP acknowledgment span has per-arm p95
0.222–0.279 ms, while durable draft-save acknowledgment p95 is 8.001–11.638 ms.
That residual includes client preparation/parsing/feed reading; it is not an
isolated server post-commit measurement. Slow full-feed arms already have slow
HTTP acknowledgment. No FULL durability was weakened to improve this result.

| Spaces (100 notes each) | Ready before warm / cold p95 | Ready after warm / cold p95 | Genuine first-open after warm / cold p95 |
| --- | ---: | ---: | ---: |
| 100 | 201.305 / 396.704 | 87.132 / 369.306 | 17.007 / 24.717 |
| 1,000 | 1,003.011 / 1,543.318 | 328.960 / 1,078.785 | 18.888 / 25.390 |

**Startup target remains unmet.** Warm 100-space readiness passes; cold 100-space
readiness and both 1,000-space conditions miss 300 ms. Two of five warm
1,000-space head starts breach; all three cold starts breach. This comparison's
baseline already includes the first lazy/blocking fix. The separately retained
original `e712ff4` comparison has 100-space ready p95 2,233.318 ms warm and
2,994.008 ms cold, versus `f484570` 126.823/418.835 ms. At 1,000 spaces the
original baseline failed two 30-second readiness observations; no successful
percentile is invented. `f484570` reached readiness at 944.416/1,399.903 ms.
Its first-open observations are genuine, with zero startup writers before use.

For the single-space 100k-note fixture, genuine first-open after p95 is 62.253 ms
warm and 202.671 ms cold; the complete matrix preserves smaller fixtures too.
No 1M-note, long-note, attachment or physical two-disk qualification is claimed.

The existing full-write-to-space-feed proxy still breaches 10 ms in all 40 head
arms: p95 34.488–52.144 ms. That span starts before HTTP work. Its budget is
unchanged, and it cannot substitute for the debug post-commit stage below.
The only noisy breached group selected by `repeat-plan.py` is cold 1,000-space
startup (28.8% range/median noise); its bounded repeat is recorded separately.
Other noisy groups are retained, including nonbreached and unbudgeted metrics.

Measured product SHA-256:
`f5df68d453aba06f8712348ff7386a3b6163621600d19c53af78ca35b8a8dda2`.
Baseline:
`4704638272f60bcea2ad3b0ef9cb8a89e5dab799b11970c2eaa97fbfb35d41ed`.
Driver:
`19654337071f74bd1942dd7eb82accc8346ba2f2edc5956ca8840a4675423dc0`.

The later rebuilt release after the debug-only emission offload has SHA-256
`669f37adcb472180d31fb04239b7d57e371f669acb2a4a3c25e331c73d61b5a6`.
It is **not byte-identical**. The 80-arm results stay pinned to `0126438`, rather
than being relabeled as measurements of that later artifact. A separate matched
warm 1k comparison checks the rebuilt artifact; it does not requalify every size.
Both plain releases exclude the stage marker; a release test-plugin build is
checked separately too. The offload is entirely cfg-eliminated from release.

## Historical bounded repeat and idle lifecycle

The [repeat summary](../../perf/results/2026-11-17-perf-fixes-repeat.json)
retains all twelve arms: three warm and three cold pairs at 1,000 spaces, 100
notes/space, 100 timed samples/operation/arm. Zero arm failures; exit 1. The
repeat independently reproduces the startup breach with cold noise reduced from
28.8% to 9.47%; warm noise is 24.86%. No noisy result is waived.

| Repeated 1,000-space observation | Before warm / cold | After warm / cold |
| --- | ---: | ---: |
| Ready p95 (pooled singleton starts) | 1,018.676 / 1,744.698 ms | 380.185 / 881.145 ms |
| Median per-arm update p95 | 55.080 / 56.994 ms | 27.976 / 30.164 ms |
| Highest update p95 | 56.407 / 57.473 ms | 30.023 / 30.945 ms |
| Median per-arm draft-to-feed p95 | 253.089 / 253.284 ms | 10.694 / 11.880 ms |
| Genuine first-open p95 | 18.948 / 23.375 ms | 17.877 / 31.352 ms |

All six repeated head update p95s satisfy 40 ms. Every repeated head startup
exceeds 300 ms. Feed p95 remains over single digits in multiple repeated arms;
its highest head p95 is 12.589 ms. One bounded repeat is complete; it is not an
invitation to keep rerunning until a favorable sample appears.

Locked repeat command, with `root=$PWD` in the same perf run directory:

```sh
./bin/calternal-perf-final --bin "$root/bin/calternal-final" \
  --baseline "$HOME/calternal-ai-perf/perf-fixes-startup/bin/calternal" \
  --work "$root/repeat-final-1000x100" --plugins "$root/plugins" \
  --baseline-plugins "$root/plugins-baseline" --budgets "$root/budgets.json" \
  --sizes '' --spaces 1000 --pairs 3 --cold-pairs 3 --samples 100 \
  --commit 0126438 --baseline-commit f484570 --json
./bin/calternal-perf-final --bin "$root/bin/calternal-final" \
  --baseline "$root/bin/calternal-final" --work "$root/idle-final" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --spaces 1,100,1000 --commit 0126438 --baseline-commit 0126438 \
  --idle-only --json
```

The [released-memory summary](../../perf/results/2026-11-17-perf-fixes-idle.json)
records three independent servers per size after the shared 632-second interval:

| Spaces, 100 notes each | Released RSS range | Writers after idle |
| --- | ---: | ---: |
| 1 | 23.301–23.379 MiB | 0 / 0 / 0 |
| 100 | 47.727–48.250 MiB | 0 / 0 / 0 |
| 1,000 | 56.078–56.613 MiB | 0 / 0 / 0 |

All nine observations are below 64 MiB and every journal writer retired. Exit 2
retains incomplete broader backend qualification; it is not an idle numeric
breach. No latency measurement overlapped these nine idle servers. This suite is
pinned to `0126438`, before the later schema-6 integration.

## Historical rebuilt-artifact comparison and debug stages

The [separate warm 1k comparison](../../perf/results/2026-11-17-perf-fixes-artifact-control.json) (`artifact-control`, three AB/BA pairs, 100
samples/arm) has zero arm failures. Original `0126438` update p95 is
24.130–25.946 ms; rebuilt `92b2993` is 24.093–25.857 ms. Draft-to-feed p95 is
8.931–14.866 ms original and 7.868–9.442 ms rebuilt. These observations do not
show a warm 1k save regression; they do not establish equivalence at other sizes
or relabel the 80-arm matrix. Its exit 1 retains the full-write/feed proxy breach.

The [debug stage run](../../perf/results/2026-11-17-perf-fixes-stages.json) (`stages-final`) compares the same debug artifact against
itself, three warm AB/BA pairs, 100 updates and matching receipt replays per arm:

| Debug observation, all six arms | Per-arm p95 range | Samples per arm |
| --- | ---: | ---: |
| Resolution/capability admission | 0.375–0.425 ms | 200 |
| Receipt lookup, initial plus matching replay | 0.419–0.465 ms | 200 |
| Durable journal commit marker to client observation | 9–12 ms | 100 |

The journal marker is emitted after durable commit and before broadcast, on its
existing writer thread. Admission stderr emission is awaited on the blocking pool.
Lookup/admission samples include both original write and matching replay;
post-commit samples correlate only actual commits by request ID. Clock precision
is 1 ms. This timestamp measures the space-journal feed, not the private-account
commit path. Instrumentation/debug optimization costs remain in these numbers;
there is no claim of release post-commit acceptance. Zero arm failures; exit 1
reflects the unchanged full-write/feed proxy. Both plain release and separately
built release `--features test-plugins` binaries exclude `CALTERNAL_STAGE`.
No clock read, allocation, stderr write or diagnostic task survives cfg pruning
in either release configuration.

Exact additional locked commands, with `root=$PWD`:

```sh
./bin/calternal-perf-final --bin "$root/bin/calternal-rechecked" \
  --baseline "$root/bin/calternal-final" --work "$root/artifact-control" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes 1000 --spaces 1 --pairs 3 --cold-pairs 0 --samples 100 \
  --commit 92b2993 --baseline-commit 0126438 --json
./bin/calternal-perf-final --bin "$root/bin/calternal-debug" \
  --baseline "$root/bin/calternal-debug" --work "$root/stages-final" \
  --plugins "$root/plugins" --budgets "$root/budgets.json" \
  --sizes 1000 --spaces 1 --pairs 3 --cold-pairs 0 --samples 100 \
  --commit 92b2993 --baseline-commit 92b2993 --json
```

## Measured sync boundaries

[Diagnostic summary](../../perf/results/2026-11-17-perf-fixes-diagnostics.json)
records all three corrected probes per binary and startup-writer evidence.
All 40 main head arms and six repeat head arms have zero space writers before
first use. Three independent corrected probes per binary agree exactly. Each performs an
untraced update and waits for placement before attaching strace; a list alone
would leave the first journal-WAL directory creation inside the trace.

| Successful calls per traced update | Before (`f484570`) | After (`0126438`) |
| --- | ---: | ---: |
| Content WAL | 3 | 2 |
| Journal WAL | 1 | 1 |
| Userdata file | 1 | 1 |
| Userdata directory | 1 | 1 |
| Delayed witness, separate | 1 | 1 |

Before acknowledgment: six calls before, three afterward. Afterward the file and
directory boundaries run on the lifecycle worker. The trace includes the later
witness flush; no witness guarantee was removed. All six probes returned 0.
Evidence remains under `~/calternal-ai-perf/perf-fixes-final/sync-corrected-*`.
Earlier probes that warmed only with a list include one extra first-journal-write
directory sync and are excluded from this steady-state table.

## Historical schema-5 measurement commands and scope

Plain product source: `0126438`; driver source: `3430102` (the latter changes only
the historical bundle selector). Baseline `f484570` is this lane's isolated lazy
startup fix, before save/feed/recovery optimizations. The original `e712ff4`
startup comparison is retained separately in
[`2026-11-17-perf-fixes-startup.json`](../../perf/results/2026-11-17-perf-fixes-startup.json).
The final comparison uses matching trusted plugin bundles because the integrated
SDK changes its pinned worker digest. The old baseline bundle has the earlier
orchestrator catalog; this is a recorded comparability difference, not a bypass
of digest validation. These are within-product comparisons, not measurements of
the reference application.

At `~/calternal-ai-perf/perf-fixes-final` on perf-01, through the lock:

```sh
./bin/calternal-perf-final --bin "$PWD/bin/calternal-final" \
  --baseline "$HOME/calternal-ai-perf/perf-fixes-startup/bin/calternal" \
  --work "$PWD/matrix-final" --plugins "$PWD/plugins" \
  --baseline-plugins "$PWD/plugins-baseline" --budgets "$PWD/budgets.json" \
  --sizes 1000,10000,100000 --spaces 1,100,1000 \
  --pairs 5 --cold-pairs 3 --samples 100 \
  --commit 0126438 --baseline-commit f484570 --json
```

For each `repeat` 0, 1 and 2, the corrected probes used the already built driver:

```sh
./bin/calternal-perf --bin "$HOME/calternal-ai-perf/perf-fixes-startup/bin/calternal" \
  --baseline "$HOME/calternal-ai-perf/perf-fixes-startup/bin/calternal" \
  --work "$PWD/sync-corrected-before-$repeat" --plugins "$PWD/plugins-baseline" \
  --budgets "$PWD/budgets.json" --commit f484570 --baseline-commit f484570 \
  --fsync-probe --json
./bin/calternal-perf --bin "$PWD/bin/calternal-final" \
  --baseline "$PWD/bin/calternal-final" --work "$PWD/sync-corrected-after-$repeat" \
  --plugins "$PWD/plugins" --budgets "$PWD/budgets.json" \
  --commit 0126438 --baseline-commit 0126438 --fsync-probe --json
```

## Engineering commands

Source transfer from the lane worktree:

```sh
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/perf-fixes.tar
ssh claude@calternal-dev-01-compute02 'mkdir -p ~/Developer/lanes/perf-fixes && cd ~/Developer/lanes/perf-fixes && tar xf -' < /tmp/perf-fixes.tar
```

All following commands execute in that dev directory through `nix develop -c`:

```sh
npm run build:plugins
cargo test --locked -j6 --workspace --all-features --no-fail-fast -- --test-threads=1
cargo build --locked -j6 -p calternal-server --features test-plugins
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml -- --test-threads=1
npm run test:integration
npm run test:plugins
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
scripts/check-docs.sh
cargo fmt --all -- --check
cargo build --locked -j6 --release -p calternal-server -p calternal-perf
cargo build --locked -j6 --release -p calternal-server --features test-plugins
```

Final integrated engineering results (all commands above executed on dev-01):

| Gate | Result |
| --- | --- |
| Workspace, all features, corrected full rerun | Exit 0; 546 passed, 0 failed, 1 ignored |
| Test-feature server build | Exit 0 |
| Plain release server/driver and release test-feature server builds | Both exit 0 |
| Conformance, serial | Exit 0; 58 passed |
| Ignored external real-server fixture, explicitly invoked | Exit 0; 1 passed |
| Integration and plugin tests | Both exit 0 |
| Contract export/check | Exit 0 |
| `scripts/check-docs.sh` | Exit 0 |
| Workspace format and standalone conformance format | Both exit 0 |

The explicit external fixture command is
`CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 -p calternal-client --test real_server -- --ignored --test-threads=1`.
Standalone formatting uses
`rustfmt --check --edition 2024 --config skip_children=true tests/conformance/isolation.rs tests/conformance/drafts_cases.rs tests/conformance/perf_cases.rs`.
The final runtime is `40a3848`; serial workspace and conformance gates, all
other required gates and release builds passed after the `f56c786` integration.
Logs/status remain on dev-01 under `artifacts/gates-serial-final/`. Documentation
clarifications made afterward do not change runtime code. Their final docs and
format checks also passed (`docs-report-comments.log`,
`format-report-comments.log`). Both plain-release and
release test-feature binaries contain no stage marker; the debug feature build does.

A prior concurrent gate attempt exhausted the dev account's rootless-container
keyring allowance (`maxkeys=200`, `maxbytes=20000`, `gc_delay=300`), while the
filesystem still had 68 GiB free. Crun reported `join keyctl ... Disk quota
exceeded`; affected worker tests failed. No allowance was raised and no global
prune or other lane's process was stopped. Seven stale containers were removed
only after their bind mounts were positively matched to this lane's directory or
the unique Nix-shell prefixes in this lane's gate logs (six before the serial
rerun, one afterward). The serial gates then passed. Matching/cleanup evidence is
in `artifacts/container-cleanup-{candidates,result,final}.json`. Earlier failed
attempts remain separately identifiable and are not presented as passing runs.

After the gates, only this lane’s `target`, `tests/conformance/target` and
`reference-docs/rust` directories were removed on dev-01; gate logs remain.

Earlier runs found and corrected a crash fixture that armed the next fault before
its baseline file was durable. The first integrated workspace run found that
assumption in the temporary-file cleanup fixture and a direct-SQLite tamper
racing baseline placement in the reserved-path fixture. Both now finish only
their baseline placement before testing the intended boundary. A later run found
the same immediate-file assumption after standard-collection enablement; that
fixture now observes its one-second placement policy (`8a7acfa`). These were fixture changes and did not alter product durability boundaries. An earlier whole-workspace
run also exposed an intermittent machine-pool assignment timeout; its unchanged
isolated rerun and the final whole-workspace run passed. Earlier failures are
retained, not counted as clean full runs.

## Review gate

| Item | Evidence / limit |
| --- | --- |
| 1. Shared types | Existing IDs, caller-safe errors, clock and hash primitives reused; no parallel copies. |
| 2. Dependencies | No new crate; perf tooling adds existing types and rusqlite dependencies downward. |
| 3. Request-path globals | Map locks only lookup/update; no I/O while held. Existing per-space lock spans that space's durability work with documented purpose. Authority's existing installation reconciliation gate remains unchanged. |
| 4. Blocking calls | Changed startup, feed replay/close and Notes/plugin request hashing use blocking workers; parsed input returns by move, with one canonical hash. Journal pruning has an awaited readiness signal. Inherited `turns::ManualAdmission::new` still hashes from async callers; this separate turns-area exception is recorded on #31 rather than changing another lane’s admission seam. |
| 5. Parse/load/flush | Counts above; cached kinds, one transaction metadata serialization, no publication body reload; traced sync evidence separate. |
| 6. Constant work | Registry/validators built once; kind cache bounded 32; recovery image stamp-checked and proposal hash reused. |
| 7. Bounds | 64 spaces, 600 s idle/30 s sweep; 1 s pending placement backpressure, 50 ms failed-placement retry; existing listener limit includes closing readers; receipts retain operator-configured seven-day default. Raw perf evidence, copied binaries and dev gate logs: 30 days; credential roots removed after runs or within 24 h on failure. |
| 8. Compatibility | No new migration, legacy reader or alias; obsolete replay adapter removed. Existing root legacy-layout presence fence remains to preserve protection and is a recorded inherited exception. |
| 9. UUID spaces | Content, journal and wake keys retain shared UUID types; no space names introduced. The inherited root-layout refusal fence still names old notes/history paths; removing that protection requires its own decision (items 8/9 exception). |
| 10. Single declarations | No new operation, capability or role; budgets remain one catalog and numeric limits unchanged. |
| 11. Wiring | New lifecycle, recovery, reader, diagnostics and harness modes have callers. Explicit materialization remains called by integrated threads. |
| 12. Reuse | ADR 0062/tiering-v3, existing fsio/names, cache/sweeper, journal wake/replay and reference TCP_NODELAY policy considered and reused. |
| 13. Public docs | Purpose/failure/cost comments accompany changed APIs; workspace all-feature documentation gate required. |
| 14. Matching docs | Content/server READMEs, recovery protocol and contract acknowledgment updated with code; this dated report links evidence. |
| 15. Format/size | Startup, lifecycle, mutation/publication, recovery sink, request hashing and CLI definitions are split by concern; `main.rs` is 479 lines after the latest extraction. Formatting gates pass on the integrated base. Existing large integration fixtures and the 791-line store public facade remain recorded exceptions: this lane adds readiness/cfg wiring there without splitting its existing public API surface. |

## Unfinished work and questions

The lane does not certify complete
file-only recovery, physical SSD/HDD tiering, provider startup, multi-tenant
isolation or a three-host-call benchmark worker. Those missing scenarios remain
explicit in the existing performance gate.

The original cold 100-space startup and multiple feed arms missed their
requested targets; integrated results are kept separately above. No safety check, durability boundary or budget is removed to
make those numbers pass. Choices the current documents do not settle:

| Question | Options and costs |
| --- | --- |
| How to reduce security-log startup replay further? | Keep the current 64 MiB compaction threshold and report the cold miss; or qualify a smaller threshold using the existing compaction API, trading shorter startup replay for more snapshot work and two file plus three directory syncs on threshold-crossing security mutations; or redesign checkpoint/replay granularity with new crash/provenance review. No option is selected here. |
| Whether to change inherited root-layout refusal checks? | Keep the presence fence and its directory I/O; or explicitly retire it after reviewing install/adoption refusal semantics. Removing it as a perf shortcut would weaken a protection this lane was told to preserve. |
| How to close the repeated cold save-tail miss? | Keep the three required FULL commits and profile their syscall latency on the same cold fixture, leaving acceptance blocked; or fence receipt pruning on a proven durable outbox-retirement checkpoint, trading an acknowledgment sync for a new cross-store pruning protocol; or design co-located durable content/receipt intents or bounded commit grouping. The protocol alternatives require atomicity, receipt-expiry, backpressure and crash review and are not selected here. Independent pruning makes a simple NORMAL retirement unsafe. |
| How to reduce remaining durable draft-save tails? | Profile per-request account connection/schema/binding costs separately from its FULL commit, then consider reusing an existing writer primitive with a new explicitly bounded account lifetime/eviction policy. That requires a cache/admission design decision and memory/isolation review. Keep the current durable path meanwhile; waking before commit or weakening FULL is not an acceptable substitute. |


## Simplification opportunities

The Notes update adapter still loads the old body only to preserve its title; a
metadata-only access path in the shared content service could remove that allocation.
The shared canonical encoder could also serialize borrowed sorted values, removing
its two existing JSON-tree copies while preserving every digest byte.
Consolidate repeated SSE test frame parsing when the feed test helpers are next
changed. Recovery checkpoint granularity should be evaluated only with protocol
review and measured mutation costs, rather than silently changing its threshold.
