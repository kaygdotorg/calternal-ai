# Server performance gate

## Warm browser-to-worker gate

`plugin.warm_browser_worker` in `perf/budgets.json` is a 15 ms p95 gate.
The measured fixed p95 was 5.2 ms on perf-01 and 10.1 ms on perf-02.
The ceiling leaves headroom for the slower VM and catches even one
reintroduced 20 ms batching wait.
The gate's verdict calculation was checked against the archived perf-01
vectors. The 5.2 ms fixed run passes and the 48.8 ms control fails.
An independent fixed run on perf-02 measured 6.2 ms p50 and 16.7 ms p95,
above the proposed ceiling.

Stage the prebuilt development-mode server, built plugin bundles,
Chromium headless shell, rootless Podman and its cached worker image
on the perf VM. Copy `perf/browser-worker.mjs` and `perf/budgets.json`
into the same directory. Run this from that directory on the perf VM:

```sh
/usr/local/bin/perf-lock.sh node browser-worker.mjs \
  /abs/calternal /abs/plugins /abs/chrome-headless-shell /abs/browser-worker.json
```

The gate provisions a disposable server and an enrolled pool runner.
It measures the complete response for one real broker read in a warm
worker, over loopback. Ten browser warm-ups precede 100 serial samples.
The JSON includes the raw samples and the budget verdict. A breach exits
1, and a fixture or configuration error also fails. Set
`CALTERNAL_PERF_PODMAN` when using the perf VM's lane-owned Podman wrapper.
For controlled A/B diagnosis only, `CALTERNAL_PERF_WORKER_RUNNER` can
select a separately built runner. Leave it unset for qualification.

The coordinator also runs this check in `scripts/perf-check.sh`. Use
`PERF_CHROMIUM` and `PERF_PODMAN` to supply executable paths on the perf
VM when they are absent from its PATH. As with the wider gate, an initial
breach triggers an independent repeat, and a reproduced breach fails the
suite. Each run keeps its JSON evidence. A missing runner or browser
leaves qualification incomplete.

## Wider backend gate

Build on `claude@calternal-dev-01-compute02`; run only on
`kayg@calternal-perf-01-backup01` under `/usr/local/bin/perf-lock.sh`.
The new crate is tooling, never a product dependency. `scripts/perf-check.sh`
is the coordinator's pre-merge gate. It requires a previously built baseline
binary and its commit, runs three cold and five warm AB/BA pairs and preserves raw samples remotely.

The seed controls payload bytes. UUIDs and timestamps are assigned by the real
server. Each size is created once through real HTTP with a software-passkey
registration and login ported from `tests/conformance/isolation.rs`. Stopped roots
are copied for each arm, preserving identical UUIDs and sessions across arms.
Concurrency is deliberately one outstanding request, not an unbounded load test.
Fixtures: 1k, 10k, 100k notes in one space; 100 and 1,000 spaces with 100 notes each.
Untimed provisioning uses private temporary `/dev/shm` roots, then copies stopped
roots to the disk-backed run directory. Timings never use tmpfs roots.
No 1M fixture: establish measured disk/RSS headroom on the 7 GiB VM first.

Every sample includes the complete HTTP response. Raw vectors are kept in
`~/calternal-ai-perf/<run>/<fixture>-<pair>-<arm>/raw.json`; only summary JSON belongs
in git. Roots (including credentials) are deleted after each successful arm.
On error, retain the failing roots for diagnosis, then remove them explicitly
within 24 hours.
Do not reuse a work directory. Keep raw samples for 30 days, scripts and summary
for the issue lifetime; remove expired raw files during the next locked run.

Quantiles use nearest rank, as the reference harness. Report p50/p95/p99/max for
each arm. The gate repeats noisy breached groups above 25% using
`perf/repeat-plan.py`: three warm pairs, plus three cold pairs when needed. A budget breach must occur in at least two head arms to fail.
This reproduction rule applies regardless of noise; never excuse a reproduced
breach because its variance is large. AB/BA alternation limits ordering bias.
A baseline-vs-baseline control is required before interpreting a regression.
The three cold pairs invoke the host-provided `sudo -n
/usr/local/sbin/perf-drop-caches` helper before each server start, while holding
the perf lock. Five warm pairs sync each stopped-root copy before timing and retain the OS
page cache. This excludes fixture-copy writeback from the measured fsync path.
Reported noise is the range of per-arm p50s divided by their median; the gate
adds a bounded repeat for noisy breached groups rather than waiving them.

Null budgets mean no accepted limit; a separate measured proposal may exist. They are
unqualified, never passes. Inherited limits are copied without comfortable
headroom, with semantic differences named in `budgets.json`. `notes.list` is
currently capped at 100 rows without pagination; its inherited page budget exposes that cost without pretending
the responses have equivalent sizes. Session check is `/api/v1/auth/me`; address
resolution is the `resolve` operation, not a fabricated static route.

The coordinator runs ordinary latency on the plain release and the durable
commit-to-client gate on a separate `perf-timings` release. Admission and
receipt lookup remain debug diagnostics. The exact three-host-call worker is
still missing, so the coordinator cannot certify the whole backend gate.

Exit 1 records a reproduced baseline-budget breach or any release delivery arm
over 10 ms p95. Exit 2 records errors or incomplete mandatory coverage. A
completed focused delivery gate exits 0 when all tested head arms meet its
ceiling. A summary is written before a numeric breach or incomplete coverage is
returned from a completed matrix.

The gate also runs `--idle-only`: three independent servers per fixture size
(1, 100, 1,000 spaces, each with 100 notes) wait concurrently for 632 seconds
after all clients close. `/proc` writer-thread counts must reach zero, and RSS
is compared with 64 MiB. These nine servers measure only idle memory; their
startup timings do not enter the latency matrix. A breach must occur in at
least two replicates of the same size.

`perf/profile.sh /absolute/suite-directory <commit>` runs a separate 1k control
with CPU `perf` sampling and syscall wall-time summaries. It requires extracted
Debian perf/strace binaries and their runtime libraries in
`~/calternal-ai-perf/tools/extracted`. Profiles stay on the perf VM; instrumented
numbers are excluded from the uninstrumented gate.

`--jobs-only --test-plugin-root /absolute/tests/plugins` uses the existing
conformance worker and a separately built `test-plugins` server. The production
binary remains separate. Three independent job runs are observations, including
first-cold and subsequent warm behavior; they are not provider-startup proof.

`perf/startup-profile.sh <suite> <commit> <current-driver>` provisions 1,000
spaces of 100 notes with `--prepare-only`, then profiles startup separately.
It removes those large disposable roots after profiling. Failed roots must be
removed within 24 hours after preserving the necessary diagnostic evidence.

`--startup-timeout-seconds` bounds readiness observation (30 seconds by default,
up to 300); it never changes a numeric budget. Arm failures are recorded with
logs and a partial summary. After two failures, that arm is skipped for the
remaining pairs of that fixture, while the other arm and later cases continue.
This allows a repaired head to be measured against a baseline that cannot start.

The normal terminal output reuses `calternal-cli::output` and adapts metric
columns to the terminal width; `--json` retains the complete structured report.
Null thresholds stay unbudgeted. The three Notes paths keep p50 and p95
thresholds in one catalog entry.

The initial run found a product startup stall at 129 spaces. To collect only
the releasable memory cases while that is fixed, select
`--idle-only --spaces 1,100`; the default gate still requests all three sizes.
`startup-boundary.py` independently probes 128 versus 129 one-note spaces.

`perf/summarize-evidence.py <suite>` reproduces the dated summary without exporting
raw vectors. Its source selections intentionally exclude the first run's
unsynced warm copies. See `docs/performance.md` for conditioning and qualifications.

The performance-fixes lane records `startup-state.json` in each arm before first
use. A zero space-writer count, together with lazy content initialization, makes
`space.process_cold_open` a real first activation for that binary. The baseline
may eagerly open spaces; its first-list observation keeps that qualification.

Every measured update checks the matching revision's read-only placement mark,
written after file and directory syncs. `userdata.ack_to_durable_file` starts
at HTTP acknowledgment on the HTTP client thread, independently of the live
subscriber. The separate placement verdict has a 1,000 ms maximum and a
single head breach fails it. If the observer stops without a matching mark,
its measured span is a lower bound. The report retains samples collected before
the miss and identifies incomplete timing arms. An independent
library test checks placement without a follow-up request or receipt replay.

`--fsync-probe` traces one warmed, same-title update using the already extracted
strace on perf-01. Run separately under the lock, once with the baseline binary
and once with the new binary, each in a fresh work directory. It records successful
`fsync`/`fdatasync` calls by descriptor role, with witness/other work separated.
The diagnostic takes four seconds and never contributes latency acceptance data.
It needs the same `sudo -n` profiling permissions as `profile.sh`.

A debug `test-plugins` server emits `CALTERNAL_STAGE` markers for capability
admission, receipt lookup and journal commit. Admission covers resolution and
capabilities and excludes HTTP sign-in and later agent scope review. Receipt
lookup includes its per-space lock and pending outbox check.

Build the instrumented release with `cargo build --release -p calternal-server
--features perf-timings`. Run `calternal-perf --commit-timing-only` with that
binary as `--bin` and a plain release as `--baseline`. The driver requires one
release commit marker per matching head-arm SSE entry, keyed by request ID.
The marker is sampled immediately after the FULL journal transaction commits.
The server and client share a nanosecond monotonic clock on the perf VM. The
client timestamps the decoded event as soon as the subscribed connection
receives it. The instrumented writer reads the clock once per durable commit
and emits one diagnostic line per request-bearing entry after broadcasting.
The build excludes this work entirely when `perf-timings` is off in release.

The delivery p95 ceiling is 10 ms on every head arm. The separate
`live.update_to_client` round trip starts before the HTTP save on the writing
client and ends at the subscribed client. Its p95 limit is 50 ms, the 40 ms
save ceiling plus the 10 ms delivery ceiling. The plain matrix measures that
round trip directly.

`--startup-only` uses the same seeded fixtures, alternating binaries, cache-drop
conditioning, first-use writer evidence and first-list timing, then stops the
scenario before subscriptions and write loops. Use it for a focused startup
comparison; it cannot qualify update, delivery or idle-memory budgets.

When the baseline pins an older trusted worker digest, use
`--baseline-plugins /absolute/matching/baseline/plugins`. Head fixture provisioning
uses `--plugins`; each measured arm validates its own pinned bundle. Both bundle
paths are recorded in the summary. Keep this difference explicit when catalog or
worker changes affect comparability; never bypass trusted-worker validation.

The schema-6 fixture builder reuses the UUID of the personal space created during
account enrollment, then creates the remaining spaces. Nominal fixture counts
therefore include every space in the enrolled account. Historical schema-5
binaries cannot read schema-6 roots; keep the pinned pre-integration comparison
and use schema-compatible controls for an integrated release. No old-schema
reader or migration is added to make a benchmark comparison run.

`--concurrency-only --clients 1,8,32,128 --samples 20 --pairs 3` measures protected
`spaces.members` reads, alternating `spaces.set_role` writes, and mixed waves.
It uses explicit development Owner/Guest bearer credentials, no browser cookie
or Origin, matching `people-development.py`; this is not browser-session
qualification. Each caller has its own connection and fresh UUID target space.
Additional development account/shared spaces are counted separately. Warm-up is
one untimed operation per caller; mixed rounds synchronize outside RPC timers so
fast readers continue overlapping writers. At 128 write callers, the target
working set exceeds the unchanged 64-space content cache, including reopen and
eviction costs. No memory/cache budget is raised to conceal that effect.

The mode uses fresh copied fixtures per arm and syncs them before startup, then
interleaves AB/BA pairs. Counts are bounded to 128 callers, 100 samples/caller,
and the global pair limit. Warm-up/request failures remain failed arms; they do
not disappear into successful latency samples. Partial fixture credential roots
are cleaned on normal error returns, and retained raw evidence has a 30-day
policy. Reuse the same driver and fixture policy for before/after binaries.
There is no accepted concurrency ceiling in the budget catalog; this mode exits
2 for observations with incomplete qualification, or 1 for failed arms.

Concurrency summaries use the range of per-arm p50s divided by the median p50
for `noise_pct`, matching the main suite. The `327b9f9` driver used per-arm p95s
for that diagnostic field; its pinned intermediate summaries retain that field
unchanged, and their p50/p95 latency observations remain valid.
