# Foreground contention follow-up, 2026-11-17

Issue [#31](https://git.kayg.org/kayg/calternal-ai/issues/31), thread
`thr_i78z9xkxhy`. Measurements ran into November 18 UTC (November 17 local).
This follows the merged [Notes integration report](perf-integration-2026-11-17.md).
Dev `d9e998b` is merged as `2e8c1fb`. The new pin preserves admitted actor attribution. Its concurrency matrix and bounded save/startup/feed control are complete. The performance budget catalog is unchanged.

## Scope and source pins

Phase 1 is in dev. This branch merged `c765490`, then `3118d38`, then pinned
integration of dev `d9e998b4ff9dac5324275e1f5e5849c059172fdd` as `2e8c1fb`.
UI files are merged without lane-authored UI changes. Later measurements retain
their explicit source pins rather than inheriting numbers from older binaries.

The final product SHA-256 is
`d718126d4dfd57c9212426372d89e2dc3b9c527e21714ae67d0d41e318c6b774`;
matching driver SHA-256 is
`923ea5faa910b1cc0085b68fd57b2c773513eae17fef4c17e1fc49f458a22607`.
The before product is immutable `2ab266596862c83f7c612cbb51e7dec38ac70f9c`,
SHA-256 `879aa14d09469186887666b4ab9668550f1e27f0c178ebd1384abd734c01b12f`.
Both arms use the same driver, fixture and conditioning policy. Head uses its
matching plugin bundle; baseline uses its immutable bundle through
`--baseline-plugins`. Trusted-worker checks remain enabled.

The latest integration includes admitted actor attribution, machine verification,
deterministic role-ordering tests, paging, request proof errors and generated-doc
checks. `account` remains authority; optional `via` records trusted admission
provenance and grants nothing. Attribution adds journal metadata without another
durability boundary. The extracted space adapter passes that admitted actor.

The [people lane's observations](https://git.kayg.org/kayg/calternal-ai/issues/37)
used temporary probes on `44c48e4` on dev-02. Its 528/266/46/60/1.7 ms figures
are averages within the slowest five percent, not additive component p95s or
measurements on perf-01. This report reproduces the workload with optimized
binaries built on dev-01 and measured only on perf-01.

## Changes and preserved ordering

`5f9683e` adds a native asynchronous capability read over the existing bounded
`auth.db` mailbox. Synchronous and asynchronous methods share the same SQL
helper. Admission, item resolution and delivery checks await that worker rather
than occupying a general blocking slot while waiting for it. The item route
and Notes state lookup still run on blocking tasks. A resolved capability set
is reused only within that request's admission, as before. Delivery performs a
fresh read. Browser session checks already await their auth service directly;
development token comparison is an in-memory constant-time check, followed by
the existing asynchronous active-account check.

There is no grant cache or invalidation protocol. Direct/group membership,
role changes, account disable and session revocation keep the same ordered
worker and publication barrier. Mutation handlers recheck authority inside
protected transactions. Synchronous-only test directories retain an explicit
blocking adapter. Occupying the only general blocking slot reproduces the old
capability scheduling failure; native authorization now completes. A future
constructed before a revocation and polled afterward sees the revocation.

`e5106b1` tried two shared listener/read slots and six writer slots.
`a8ee0e0` widened writes to 32 and included non-operation HTTP mutations.
The final executor layout in `3ff0ba1` has two dedicated readers, up to 32
writers, and two unscoped listener/background slots: up to 36 blocking threads
across these three executors, versus the previous eight shared slots. Each
foreground lane admits 64 running/queued callbacks; further callers await
permits without taking a thread. Lane threads retire after ten idle seconds.
These are execution limits, not new accepted latency or memory ceilings.

Scheduling classes derive from catalog/manifest `mutates`; no operation list is
duplicated. Native authentication stays outside mutation scopes. Foreground
blocking callbacks do not join either the writer or background queue. The
SSE wrapper in `3f5ff42` retains read context on each later body poll, with one
pinned box per connection and the existing listener/page limits. It forwards
items/errors unchanged. A regression test occupies all 32 writer and both
background slots, then polls a stream outside the original handler scope.
The unscoped adapter times out; the scoped stream returns its item.

Background tasks do not inherit request scopes. Reservations do not bypass
mandatory same-space locks or the protected auth publication barrier. A reader
waiting on those resources can still delay other readers within the two-slot
lane. This is scheduling isolation, not a proof of multi-tenant fairness.

The merge preserves both authorization and pagination modules. Cursor
filter encoding, verification and signing now use the bounded request executor,
with the same signing key, scope and check order. Parsed input/result values move
through the tasks. A paged operation adds two CPU callbacks, no SQL or flushes;
role writes are not paged. The scope owns an ephemeral key clone, not an authority
cache. Existing malformed-token, cross-account, space and filter cases pass.

Recovery full-proposal framing/hashing was already reduced to once per batch
by `50adee8`, before this baseline. Its immutable `Pending` digest is reused
for witnesses, resolution and application; resolution frames contain only the
small decision marker. Auth role batching calls recovery prepare once per
batch. This follow-up does not claim another encoding improvement for code
already present in both products.

## Fixture and before observations

Each caller has its own HTTP connection and target space. The owner changes a
guest between reader/editor roles, or lists that space's members. Bearer
credentials, omitted Origin and the role operations match the people lane's
fixture. Extra development personal/shared spaces are counted separately.
Each arm copies stopped roots, syncs the copy before startup, and performs one
untimed operation per caller. There are three warm AB/BA pairs and 20 measured
requests per caller. Mixed arms use half readers/half writers with synchronized
round boundaries; boundary waits are excluded from individual RPC latency.
All request failures fail the arm; no failed requests are dropped from samples.

128 independent write spaces exceed the unchanged 64-space content cache.
Those arms include reopen/eviction costs and are not a fully resident cache
qualification. The 128-client mixed arm has 64 write spaces. No browser cookie,
provider startup, multi-tenant fairness or cold-disk qualification is implied.

The before-only control used the same immutable product in both arms:
66 arms, zero request failures, exit 2 (observations; no accepted concurrency
latency ceiling). Values below are ranges of individual arm p95s, in ms.

| Clients | Read-only | Write-only | Mixed read | Mixed write |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.505–8.426 | 31.933–36.938 | — | — |
| 8 | 1.545–2.455 | 134.222–197.855 | 83.769–98.280 | 85.607–100.379 |
| 32 | 5.816–7.925 | 593.534–620.374 | 305.420–336.512 | 331.118–370.219 |
| 128 | 25.496–47.234 | 4311.820–5779.534 | 1380.655–1824.535 | 1472.559–1897.765 |

Summary: [before control](../../perf/results/2026-11-17-concurrency-before.json).
Raw vectors/logs are lane-owned on perf-01 under
`~/calternal-ai-perf/perf-fixes-concurrency/`, retained for 30 days. Each arm
removes its three credential-bearing roots; only summaries are committed.

## Final pinned concurrency result

`2e8c1fb` versus `2ab2665`: 66 arms, zero request failures, three warm AB/BA
pairs, exit 2 (observations; no accepted concurrency ceiling). Both products use
the same frozen driver and copied fixture policy. The table shows the median
of three arm p95s in ms.

| Clients | Read-only before → after | Write-only before → after | Mixed read before → after | Mixed write before → after |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.671 → 0.682 | 33.000 → 31.876 | — | — |
| 8 | 1.969 → 2.350 | 168.029 → 147.308 | 87.465 → 85.309 | 92.017 → 91.244 |
| 32 | 7.615 → 7.684 | 616.771 → 244.165 | 318.394 → 197.227 | 346.479 → 197.272 |
| 128 | 27.982 → 27.450 | 4725.206 → 1538.316 | 1427.226 → 511.825 | 1502.978 → 504.023 |

Write-only p95 falls **60.4% at 32 callers** and **67.4% at 128**. Mixed-read
p95 falls 38.1% and 64.1%, respectively. The 32-client write head range is
228.069–245.812 ms; the 128-client range is 1525.884–1806.570 ms.

There is no uniform read-only improvement: eight-client p95 increases 19.3%
(1.969 → 2.350 ms), one-client reads increase 1.6%, and 32-client reads 0.9%.
Eight-client writes improve 12.3% in this final run, unlike the prior `17c43ae`
run's 17.6% regression. Both runs remain recorded; no historical data is
rewritten or discarded. Every arm and quantile is in the
[final summary](../../perf/results/2026-11-17-concurrency-final.json).
Mandatory shared authority publication waits remain.

Eight-client head mixed-read p50 noise is 52.4%. This is a noisy observation,
not an accepted-budget pass. The bounded-repeat gate applies to noisy
**breached** budget groups, and concurrency metrics have no accepted ceiling.
The save/feed control retains its existing budgets and repeat policy.

## Final bounded save/startup/feed control

The same `2e8c1fb` product ran 1 × 10,000 notes and 100 × 100 notes,
three warm AB/BA pairs, 100 samples: **12 arms, zero request failures**.
Head ranges of individual arm p95s (ms) are:

| Metric | 1 × 10,000 | 100 × 100 |
| --- | ---: | ---: |
| Content save acknowledgment | 26.046–29.338 | 28.946–31.366 |
| Draft acknowledgment | 7.709–9.156 | 7.582–10.272 |
| Draft write to feed | 7.887–9.467 | 7.795–10.464 |
| Full content write to feed | 35.709–38.111 | 37.318–39.611 |

Every content-save arm meets 40 ms. Startup observations are 48.340–49.834 ms
at one space and 59.143–78.951 ms at 100 spaces; each arm has one startup sample,
so these ranges do not establish a population p95. The 100-space startup p50
noise is 28.6%, but no startup arm breaches its ceiling. Running
`python3 perf/repeat-plan.py perf/results/2026-11-17-concurrency-final-control.json`
returns an empty plan: no noisy breached metric group requires a repeat.

The control exits **1**, retaining the catalog's 10 ms full-content-write/feed
proxy breaches. Draft-feed p95 is not uniformly below the user's 10 ms target.
Release measurements cannot separate admission or the actual commit timestamp;
this is not a pass for commit-to-client latency. No ceiling is changed.

First-use observations for previously unopened space handles are 17.928–18.913 ms
at one space and 33.350–39.283 ms at 100. These are singleton arm observations,
not cold-disk p95s: OS caches and the eager retention probe can warm reads.
After-two-second RSS is 36.277–36.445 MiB and 64.484–65.285 MiB respectively.
That is not the separately specified post-idle memory qualification; idle memory
after a concurrency burst was not requalified. The 1,000-space and disk-cold
save-tail targets were not rerun by this bounded control and remain open.

Every quantile, breach and failure vector is retained in the
[final control summary](../../perf/results/2026-11-17-concurrency-final-control.json).
Earlier `17c43ae` [concurrency](../../perf/results/2026-11-17-concurrency-merged.json)
and [control](../../perf/results/2026-11-17-concurrency-merged-control.json)
summaries remain historical evidence.

## Exploratory comparisons and diagnostic correction

The earlier [six-writer](../../perf/results/2026-11-17-concurrency-six-writers.json),
[32-writer intermediate](../../perf/results/2026-11-17-concurrency-32-writers-intermediate.json),
and [separate-reader comparison](../../perf/results/2026-11-17-concurrency-reader-isolation.json)
remain pinned to their products. They are not relabeled as measurements of the
final merge. An upload for a later candidate overlapped part of the six-writer
exploration; the final merged run transfers its artifacts under the perf lock
before measurements and performs no further artifact uploads during the run.

The old driver used arm p95s for `noise_pct`, contrary to the documented p50
policy. `23a2cd5` fixes that diagnostic and names the statistic in new JSON.
Historical JSON is unchanged; its actual latency quantiles remain valid.

## Separate CPU profile

A separate locked ten-second baseline profile captured 111 user-CPU samples,
zero lost. Flat sample shares were auth worker 90.0%, journal driver 5.4%, and
Tokio workers 4.5%. Stacks include retained-record capture, canonical JSON row
sorting, and the once-per-batch proposal serialization. This is a small CPU
sample, not wall-clock attribution or a reproduction of the development probe's
98% estimate. Instrumented latencies are excluded from qualification. The source
still captures all retained account receipts; no recovery format change is made.

## Read and durability counts

The scheduling change adds no SQL reads or flushes. A native capability check
still executes one cached indexed grant query, over the same bounded mailbox.
Item admission reuses the resolver's current set; any existing delivery check
still obtains a fresh set. Session checks and handler transaction checks remain.
Space member listing retains its protected snapshot and indexed account/group
page queries. See the people report for those unchanged queries.

An uncontended nonempty role batch retains its proposal file sync, one FULL
auth SQLite commit, and resolution file sync. Every successful request then
retains its own space-journal commit, so a batch of B successes has B+3 normal
request-path durability boundaries, excluding separately triggered checkpoint
or recovery compaction work. The one-client path still has four. Acknowledgment
waits for recovery publication and the space receipt. No receipt is removed or
security check weakened to improve these timings. Content saves retain the
three FULL transactions and bounded deferred placement documented in phase 1.

A dirty account recovery record still captures every retained receipt owned by
that account and sorts canonical rows before proposal framing. Once-per-batch
framing does not remove that work. Further format decisions
are pending; changing receipt ownership, omitting retained receipts or shortening
retention is outside this scheduling change.

The existing role-write boundary still canonicalizes input separately for two
receipt domains: auth hashes the body and records the operation separately;
the space journal hashes operation plus body. Both formats remain unchanged.
Sharing one canonical body encoding across that interface is a remaining
simplification; this report does not claim one digest/encoding for that path.

## Verification and exact commands

All Rust builds/tests run on `claude@calternal-dev-01-compute02` in
`~/Developer/lanes/perf-fixes`, using `nix develop -c`, cargo `-j6`.
No builds or tests run on hd01 or perf-01. Source transfer is:

```sh
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/perf-fixes.tar
ssh claude@calternal-dev-01-compute02 'mkdir -p ~/Developer/lanes/perf-fixes && cd ~/Developer/lanes/perf-fixes && tar xf -' < /tmp/perf-fixes.tar
```

Merged gate results: **579 workspace tests passed, zero failed, one existing
ignored test**; **82 conformance passed**; the explicitly run external fixture
passed. Integration/plugin tests, web build, contract, docs, formatting and
release build all exit 0. The initial docs check failed on 12 stale generated
pages in the VM tree. `npm run docs:site` and a new `scripts/check-docs.sh`
both exit 0; the regenerated tracked pages match the checkout without a source
change. The original failure stays in `status.tsv` alongside the successful
recheck. All four Podman wrappers (focused cursor/purge,
workspace, conformance, external fixture) report no leaks. The final repeated
full gate is `artifacts/gates-concurrency-2e8c1fb/` on dev-01. Earlier failed
regressions and the corrected initial web-script typo remain in their own logs.

Exact commands, through `ssh ... 'cd ~/Developer/lanes/perf-fixes && nix develop -c ...'`:

```sh
npm run build:plugins
cargo fmt -p calternal-auth -p calternal-server -p calternal-perf
cargo fmt --manifest-path tests/conformance/Cargo.toml --all
cargo build --locked -j6 -p calternal-server --features test-plugins
CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" scripts/check-podman-tests.sh cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml purge_blocks_old_body_receipt_replay_with_foreign_isolation -- --test-threads=1
scripts/check-podman-tests.sh cargo test --locked -j6 --workspace --all-features --no-fail-fast -- --test-threads=1
cargo build --locked -j6 -p calternal-server --features test-plugins
CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" scripts/check-podman-tests.sh cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml -- --test-threads=1
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" scripts/check-podman-tests.sh cargo test --locked -j6 -p calternal-client --test real_server -- --ignored --test-threads=1
npm run test:integration
npm run web:build
npm run test:plugins
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
scripts/check-docs.sh # initial stale-generated-page failure retained
npm run docs:site # regenerates, builds and verifies the documentation site
scripts/check-docs.sh # recheck: exit 0
cargo fmt --all -- --check
cargo fmt --manifest-path tests/conformance/Cargo.toml --all -- --check
cargo build --locked --release -j6 -p calternal-server -p calternal-perf
```

The two cursor isolation suites were also run with the same test-plugin server,
`cursor_cases -- --test-threads=1`: 2 passed and the leak wrapper passed.
Native capability, revocation and stream scheduling regressions were observed
failing before their fixes and passing afterward. Focused library result:
24 passed; harness tests: 3 passed. An initial stream-test pinning compile error
was corrected before the actual expected 1.01-second behavioral failure.

Merged performance commands, inside `/usr/local/bin/perf-lock.sh` on perf-01:

```sh
root="$HOME/calternal-ai-perf/perf-fixes-concurrency/merged-2e8c1fb"
base="$HOME/calternal-ai-perf/perf-fixes-merge-ed6f034"
budgets="$HOME/calternal-ai-perf/perf-fixes-concurrency/budgets.json"
"$root/bin/calternal-perf" --bin "$root/bin/calternal" --baseline "$base/bin/calternal" --work "$root/concurrency" --plugins "$root/plugins" --baseline-plugins "$base/plugins" --budgets "$budgets" --concurrency-only --clients 1,8,32,128 --samples 20 --pairs 3 --cold-pairs 0 --commit 2e8c1fb --baseline-commit 2ab2665 --json
"$root/bin/calternal-perf" --bin "$root/bin/calternal" --baseline "$base/bin/calternal" --work "$root/save-control" --plugins "$root/plugins" --baseline-plugins "$base/plugins" --budgets "$budgets" --sizes 10000 --spaces 1,100 --samples 100 --pairs 3 --cold-pairs 0 --commit 2e8c1fb --baseline-commit 2ab2665 --json
```

The used budget file matches `perf/budgets.json` exactly. The separate CPU profile
uses `perf record -e cpu-clock:u -F 99 --call-graph dwarf -p <verified-lane-server-pid>
-- sleep 10`, then `perf report --stdio --no-children`, with the pinned extracted
tools and runtime libraries from the existing performance harness.

All 1,671 tracked files matched the dev-01 snapshot by SHA-256. Product and
driver hashes on perf-01 match the pinned release build; the budget file matches
the repository exactly. A locked scan found zero remaining credential-bearing
fixture roots under this concurrency lane. After all builds, the lane removed
its own `target`, `tests/conformance/target` and `reference-docs/rust` on dev-01
and verified that each path is absent. The cleanup command was:

```sh
ssh claude@calternal-dev-01-compute02 'cd ~/Developer/lanes/perf-fixes && rm -rf -- target tests/conformance/target reference-docs/rust && test ! -e target && test ! -e tests/conformance/target && test ! -e reference-docs/rust'
```
An earlier cleanup before the last dev refresh was premature; the new pin was
rebuilt and qualified.
The original startup/cold-save/feed misses remain open; no budget is raised.

## Commits

The branch's incremental implementation commits are `327b9f9` (harness),
`5f9683e` (native grants), `e5106b1` (initial scheduling partition),
`9370884` (space adapter extraction), `a8ee0e0` (writer sizing),
`23a2cd5` (noise statistic), `3ff0ba1` (dedicated readers), and
`3f5ff42` (stream-poll scope). Integration commits are `893a85e`, `17c43ae`
and the final measured pin `2e8c1fb`. Each commit carries the requested Codex
co-author trailer. The report/evidence commit changes no product code.

## Review gate

1. IDs, capabilities, errors and hashing use existing shared types/helpers.
2. Dependencies still point downward; no helper crate is introduced. Native
   authority scheduling uses the existing auth worker rather than another DB.
3. There is no per-space global mutex or connection. The bounded foreground
   executor replaces scheduling contention; existing per-space admission and
   auth recovery locks retain their documented cross-thread/fsync intervals.
4. SQLite, routing, file work and hashing stay off async workers. Native
   authorization awaits a mailbox, with no blocking cross-thread wait.
5. Grant queries and durability counts are stated above. Input/grant reuse is
   request-local; delivery and transactional authority checks remain fresh.
6. Operation classes derive from immutable catalogs/manifests already cached.
7. Foreground lanes each admit 64 callbacks; readers 2, writers 32, background
   slots 2; foreground thread idle retirement 10 seconds. Auth mailbox 64, role
   batch 32, content cache 64, ten-minute idle
   stores and operator-configurable seven-day receipt retention remain bounded.
8. No compatibility reader, migration chain, alias or ignored test is added.
9. Space selection uses UUIDs. Names in disposable fixture labels grant nothing.
10. Scheduling consumes existing operation declarations and capability SQL.
11. New async methods and executor APIs have production callers; none is a stub.
12. Reuses auth `Db::read`, Tokio blocking callbacks/task scopes, existing
    content/journal lifecycle, and the prior recovery `Pending` digest.
13. Public comments state scheduling, queue bounds, failures and costs. Final
    docs gate exits 0.
14. Server/perf READMEs and this report accompany the change. Historical source
    pins remain historical; the old report is not relabeled as fresh evidence.
15. Both final remote fmt checks exit 0. Space-directory dispatch is extracted
    into its own module to keep the main dispatcher below about 600 lines.

## Unfinished and questions

- Original 1,000-space startup, cold save tail and feed p95 targets remain open.
- Existing full account-record capture can remain as-is (no protocol change,
  work grows with retained receipts), or a scoped/row-delta recovery design can
  be reviewed (bounded work needs new provenance, prune and crash proofs).
  Retention reduction is not an optimization authorized by this work.
- An intentional auth-batch coalescing window could reduce commits at low
  concurrency, at the cost of added acknowledgment delay and fairness policy;
  the existing opportunistic batching is retained.
- Independent authority readers would need a publication visibility design;
  they cannot expose SQLite-committed grants before protected publication.

## Simplification opportunities

- Keep one native authority-query helper shared by synchronous and async APIs;
  no second grant cache or invalidation mechanism is needed.
- Keep operation scheduling derived from catalog declarations rather than
  maintaining read/write operation lists.
- Share canonical body bytes across the two receipt domains while preserving
  their distinct stored fingerprints, if that interface change is reviewed.
- A reviewed recovery receipt boundary could remove repeated full-account
  materialization; retain the existing protocol until that design is settled.
