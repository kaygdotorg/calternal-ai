# Startup and save profiles — November 18, 2026

[Forgejo issue #52](https://git.kayg.org/kayg/calternal-ai/issues/52) ·
`lane/perf-profile`

Opening and validating the security log dominates warm startup and is the
largest single phase cold. The replay step inside authority reconciliation takes
7–8 ms. The three FULL transactions take 37.14 ms of the 40.08 ms profiled
cold Notes save. A single FULL transaction takes 14.57 ms of the 16.03 ms
profiled cold draft-to-feed sample.

The product change is opt-in release timing for the 10 ms
`live.commit_to_client` gate. The proposals below are for review.

## Measurement conditions

- Built the unmodified server at `2ee7c06` with `nix develop -c cargo build
  --locked --release` on `claude@calternal-dev-01-compute02`. Measured on
  `kayg@calternal-perf-02-backup01` under `/usr/local/bin/perf-lock.sh`.
  Perf-02 was available while perf-01 was occupied. Treat these as a control
  for the current fixture on perf-02. The [November 17 perf-01 results](perf-fixes-2026-11-17.md)
  supply historical observations.
- One thousand spaces with 100 Notes each. Three disk-cold and three warm
  AB/BA pairs, each with 100 Notes updates and 100 draft saves. The cold cache
  drop is before server spawn. The driver performs first-use reads and opens
  both SSE subscriptions before writes. Warm runs sync the stopped fixture
  copy and retain the OS page cache.
- The paired server executables have the same SHA-256
  `b5c2f9082bffd27d195ed287d8381dd0e43ea3ed497588b06609d53b900a63bb`.
  One copy carries external entry and return probes. The diagnostic client
  adds same-host monotonic sample boundaries. Its SHA-256 is
  `ee83dbde83782f53177081ff751ec42f9660ba5510c967f1e823151a01a82b37`.
  Twelve arms completed, with zero arm failures and no lost events reported.
  The perf events were detached and every fixture credential root was removed.
- Startup tables use the highest of three traced starts per condition, the
  nearest-rank p95 for three samples. Save tables break down the single traced
  sample at the pooled p95 of 300 samples in the selected condition. Each
  phase is part of that sample's wall time. Percentages use unrounded values.
  The Notes diagnostic brackets HTTP envelope creation through response decode,
  a slightly wider span than the harness's HTTP metric.
- The untraced controls determine whether the numeric startup and Notes misses
  recur. Untraced warm startup had 37.9% noise by the harness's range-over-median
  measure. Probe overhead and between-arm variance affect the traced timings.
  The diagnostic harness used the November 17 catalog. Its exit 1 includes
  the inherited write-to-feed proxy.

The run command was `/usr/local/bin/perf-lock.sh
~/calternal-ai-perf/profile-52/run.sh` on perf-02. The directory retains the
probe map, temporary client sources, raw vectors, `uprobes.data`, its decoded
event stream and the analysis script. Raw diagnostics and dev gate logs use
the 30-day evidence window. The repo carries summaries.

| Metric | Untraced warm p95 | Untraced disk-cold p95 | Traced warm p95 | Traced disk-cold p95 |
| --- | ---: | ---: | ---: | ---: |
| Startup, 1,000 spaces | 554.62 ms | 1,022.35 ms | 399.13 ms | 1,045.47 ms |
| Notes update, range of arm p95s | 31.13–36.48 ms | 28.06–31.99 ms | 32.45–35.43 ms | 36.25–41.01 ms |
| Draft save to feed, range of arm p95s | 8.54–12.52 ms | 17.55–18.74 ms | 10.42–12.31 ms | 11.64–17.95 ms |

All six untraced starts exceed 300 ms. All three untraced disk-cold Notes arms
are within 40 ms p95. The historical 49.93 ms p95 remains a separate observed
miss. The slowest individual traced cold Notes update was 49.76 ms. The draft
path still misses the requested single-digit target in several arms and has
an unbudgeted entry in the catalog.

## Startup, warm

| Phase | ms | Share |
| --- | ---: | ---: |
| Root locks and stores | 11.30 | 2.8% |
| Security log open, load and validation | 271.26 | 68.0% |
| Trusted plugin catalog and worker files | 60.24 | 15.1% |
| Authority database and thread startup | 2.17 | 0.5% |
| Security authority reconciliation | 15.30 | 3.8% |
| Erasure fence and secret configuration | 0.21 | 0.1% |
| Machines and jobs | 13.19 | 3.3% |
| Other startup and readiness polling | 25.46 | 6.4% |
| **Total** | **399.13** | **100%** |

## Startup, disk-cold

| Phase | ms | Share |
| --- | ---: | ---: |
| Root locks and stores | 154.72 | 14.8% |
| Security log open, load and validation | 423.32 | 40.5% |
| Trusted plugin catalog and worker files | 95.78 | 9.2% |
| Authority database and thread startup | 13.94 | 1.3% |
| Security authority reconciliation | 17.35 | 1.7% |
| Erasure fence and secret configuration | 0.67 | 0.1% |
| Machines and jobs | 209.56 | 20.0% |
| Other startup and readiness polling | 130.14 | 12.4% |
| **Total** | **1,045.47** | **100%** |

Within log opening, the snapshot and segment reads take 12.58 ms warm and
141.85 ms cold. The remaining 259 and 281 ms include frame decoding, validation
and reconstructing the in-memory state. The narrower `replay_loaded` call is
7.01 ms warm and 8.28 ms cold within reconciliation. That limits the gain from
changing reconciliation. Cold startup also spends 200.07 ms in jobs opening,
which inventories space directories, and 90.29 ms loading installed and retained
plugin rows. Those calls take 3.60 and 59.24 ms respectively in the warm sample.
Removing the entire security-log contribution alone would still leave about
605 ms in the cold sample.

## Notes update, disk-cold arm

| Phase | ms | Share |
| --- | ---: | ---: |
| HTTP, admission and content preparation | 1.39 | 3.5% |
| Content and outbox commit | 15.49 | 38.7% |
| Publish handoff, journal SQL and token work | 0.60 | 1.5% |
| Journal and receipt commit | 9.91 | 24.7% |
| Broadcast, return and retirement preparation | 0.24 | 0.6% |
| Outbox retirement commit | 11.75 | 29.3% |
| Remaining server work, HTTP response and decode | 0.70 | 1.7% |
| **Total** | **40.08** | **100%** |

The measured commit spans account for 92.68% of the sample. The analysis uses
the transaction span where the release symbol provides it and the VFS sync
span within the commit otherwise. The three VFS sync calls take 37.07 ms. The 49.76 ms individual tail spends
42.56 ms in the same three commits. The trace also contains journal transactions
with two VFS sync calls, including a 30.45 ms journal transaction in a 57.61 ms
warm outlier. File placement runs outside the HTTP acknowledgement span.

## Draft save to account feed, disk-cold arm

| Phase | ms | Share |
| --- | ---: | ---: |
| HTTP, session and worker handoff | 0.27 | 1.7% |
| Account DB open, PRAGMAs and binding | 0.23 | 1.4% |
| Account cleanup and authorization | 0.28 | 1.7% |
| Row, limits, event and commit preparation | 0.27 | 1.7% |
| Draft commit | 14.57 | 90.9% |
| Feed SQLite replay | 0.05 | 0.3% |
| Wake, scheduling, HTTP and client receipt | 0.36 | 2.2% |
| **Total** | **16.03** | **100%** |

### Warm draft save to account feed

| Phase | ms | Share |
| --- | ---: | ---: |
| HTTP, session and worker handoff | 0.26 | 2.2% |
| Account DB open, PRAGMAs and binding | 0.18 | 1.5% |
| Account cleanup and authorization | 0.18 | 1.6% |
| Row, limits, event and commit preparation | 0.20 | 1.7% |
| Draft commit | 10.46 | 89.2% |
| Feed SQLite replay | 0.07 | 0.6% |
| Wake, scheduling, HTTP and client receipt | 0.38 | 3.2% |
| **Total** | **11.72** | **100%** |

The VFS sync takes 14.52 ms cold and 10.40 ms warm. Wake-to-client takes
0.39 ms cold and 0.43 ms warm in these samples. The existing draft driver reads the
account event after the HTTP response. Its measured end-to-end span can
include waiting at the writing client even if the subscribed client already
has bytes buffered.

## Proposals for review

Estimates are the expected gains if a prototype removes the identified work
under this fixture. Remeasure the same paired workload and the affected paths
before accepting a change.

### Startup

| Change | Expected gain | Risk | Cost elsewhere |
| --- | --- | --- | --- |
| Exercise the existing security-log compaction API with a lower segment threshold and compare replayed bytes and frames. | Target 100–180 ms warm and 150–250 ms cold if retained frames account for roughly half of the measured load. The snapshot must still be verified. | Compaction or replacement bugs can interfere with authority recovery. Validate torn writes and exact checkpoint proof across every mutation boundary. | More snapshot serialization on security mutations. A threshold crossing spends two file and three directory syncs. |
| Replace the jobs startup directory sweep with an inventory of spaces that have jobs, backed by the existing job records and verified during recovery. | Target 120–180 ms cold from the 200 ms inventory phase. Warm has about 4 ms of headroom. | Missing or stale routes can hide resumable jobs or erasure work. Verify ownership and crash recovery at inventory publication. | Persist and maintain inventory on job creation and removal. Cold rebuild still pays for the scan. |
| Reuse verified materialized plugin worker files when installed revisions and their digests match. | Target 30–50 ms warm and 45–75 ms cold from the plugin row phase. | Cache replacement or hash mismatch must fail closed. Preserve the trusted manifest and worker checks before serving. | Hold bounded cache files across starts and pay their validation and cleanup cost. |

### Cold Notes save

| Change | Expected gain | Risk | Cost elsewhere |
| --- | --- | --- | --- |
| Fence journal receipt pruning on a durable outbox-retirement checkpoint and allow the retirement flush to finish after acknowledgement. | About 9–12 ms when removing the measured third flush from the acknowledgement span. | Power loss could revive an intent after its receipt expires. Prove the pruning fence and replay semantics before shortening acknowledgement. | Additional ordering and checkpoint work, plus backpressure and disk-retention limits when retirement lags. |
| Evaluate one per-space durable transaction for content plus its publication intent and receipt. | About 15–25 ms if two measured barriers can be removed. | Changes atomicity, replication proof and recovery. Rerun acknowledgement, witness, erasure and crash testing. | More write contention within each space and a broader durable schema and recovery change. |

### Draft save to account feed

| Change | Expected gain | Risk | Cost elsewhere |
| --- | --- | --- | --- |
| Qualify the account WAL on a lower-tail durable volume with the same FULL guarantee. Measure its VFS sync tail first. | If p95 sync reaches 6–7 ms, warm gains about 3–4 ms and cold about 8 ms. That would put the observed samples within the single-digit target. | Keep owner-path isolation, restore ordering and crash guarantees intact. A quiet guest alone cannot prove steady storage tails. | Consumes low-tail storage capacity and adds storage placement and restore checks. |

## Release commit-to-client gate

The clock starts immediately after the journal's FULL SQLite transaction commits.
It ends when the subscriber receives and decodes the matching SSE entry. The
server and client share a nanosecond monotonic clock on the VM. Request IDs match
the two observations. Missing, duplicate or reversed observations fail
qualification. One release arm over 10 ms p95 fails the delivery gate.

With timing enabled, the journal writer reads the clock once per successful
durable transaction and emits a JSON line per request-bearing entry after
broadcast. The logging cost includes formatting and synchronous stderr on the
journal writer. The feature-disabled release compiles the field, branch, clock
and log out. It adds zero request-path clock reads, allocations, locks, tasks,
database work or writes. Debug admission and receipt timing remain debug-only.

The full writer-to-subscriber metric retains its own directly measured 50 ms
p95 ceiling, from the 40 ms save budget plus 10 ms delivery budget. The focused
release gate measures delivery separately from that span.

### Release results on perf-02

The focused gate ran three warm and three disk-cold pairs for each fixture
with 100 correlated events per completed head arm. All 16 completed head arms
exceeded 10 ms p95. The driver exited **1**, so performance acceptance remains
blocked.

| Notes in one space | Disk-cold arm p95 range | Warm arm p95 range | Completed head arms |
| --- | ---: | ---: | ---: |
| 1,000 | 30.08–53.82 ms | 27.29–69.79 ms | 6/6 |
| 10,000 | 28.62–41.77 ms | 97.25 ms | 4/6 |
| 100,000 | 36.57–106.14 ms | 45.74–91.75 ms | 6/6 |

There were 28 completed arms, five arms that failed the existing one-second
file-placement check and three skipped baseline arms under the existing
two-failure stop rule. The file-placement failures involved two 10,000-note
head arms and three plain-release baseline arms. The delivery breaches were
observed in completed arms with all 100 timestamp pairs. All fixture roots
were removed. The plain-matrix 50 ms round-trip ceiling is defined separately
and was outside this focused run.

The locked run used `--commit-timing-only --spaces 1 --sizes 1000,10000,100000
--pairs 3 --cold-pairs 3 --samples 100`. The summary and sample files are in
`~/calternal-ai-perf/profile-52/integrated/commit-timing/` on perf-02.

| Release artifact | SHA-256 |
| --- | --- |
| Server with `perf-timings` | `fa5407bfed3f19e59e033678c42da693173b26bf2201ebbeee9bde1ff98bce86` |
| Plain server | `315ed1e20ca02464fe01d62a6757be492b52ecbace119dd33dcfb43eb4b6c8de` |
| Driver | `20195ace26a5fc33544ae729c2f51d42e88c8ed8994e4efbe74850387ced9efb` |


## Backend verification

The lane advanced from `2ee7c06` to `origin/dev` at `5ac8c7c` before gating
the timing code. That includes the search integration at `4d17fae`. The
benchmark CLI overlap required merging mode declarations and preserving the
search probes. The phase profiles remain pinned to the original release.

All engineering commands ran under `nix develop` in the lane directory on dev-01.
The workspace and conformance suites ran serially with scoped Podman cleanup
checks. The standalone conformance lock was restored to the default dependency
set after the first invocation rejected an extra optional dependency entry.
The generated types reference was refreshed before the docs recheck.

| Gate | Result |
| --- | --- |
| Workspace with all features | 599 passed, one opt-in real-server fixture ignored |
| Test-plugin debug server build | Passed |
| Standalone conformance | 114 passed on corrected lock |
| Opt-in real-server fixture | One passed when explicitly invoked |
| Integration and plugin suites | Passed |
| Exported contract check | Passed |
| Documentation and generated references | Passed after regeneration |
| Workspace and standalone conformance format | Passed |
| Plain, timing-enabled and test-plugin release builds | Passed |
| Binary marker check | Timing marker present in the opt-in release and absent from the two other releases |

Dev logs are under `~/Developer/Calternal-AI-perf-profile/gate-logs/`.

## Simplification opportunities

The draft profile gives priority to one durably synced write. A persistent
writer would save at most the few tenths of a millisecond spent opening and
validating the connection in this fixture. Keep the existing bounded reader and
verify the underlying sync budget before adding a writer cache. The startup
jobs inventory is a candidate for the same lazy per-space pattern already used
by other storage. Replace the directory scan only after a recovery design
accounts for outstanding jobs.
