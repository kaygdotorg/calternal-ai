# Server performance gate

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

Still missing: isolated admission timing, exact server-commit-to-SSE timing,
and a three-host-call worker. See `docs/performance.md`. Until those are implemented,
this is a partial qualification and cannot certify the whole backend merge gate.

Exit 1 means a reproduced numeric breach. Exit 2 means an error or incomplete
mandatory coverage; neither is green. A summary is written before either numeric
breach or incomplete coverage is returned.

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
