# Plugin reload topology measurement

2026-11-18 · [Lane #50](https://git.kayg.org/kayg/calternal-ai/issues/50)

Select **a fresh container per revision** for the first live implementation.
The measured p95 leaves about 537 ms of the one-second budget for the
unmeasured parts of the author loop. Starting a standby at validation did
not shorten the measured path. Reusing a sandbox reduced p95 by about
139 ms, with additional isolation and lifecycle work.

| From edited bundle availability to correct worker result | Samples | p50 | p95 | p99 |
| --- | ---: | ---: | ---: | ---: |
| Fresh container | 30 | 441.7 ms | 463.3 ms | 471.2 ms |
| Fresh process in the existing sandbox | 30 | 290.7 ms | 323.8 ms | 356.6 ms |
| Standby container started at package verification | 30 | 442.3 ms | 464.5 ms | 470.5 ms |
| Standby from container ready to result | 30 | 180.5 ms | 200.4 ms | 203.3 ms |

The reused-sandbox path includes retiring a cooperative previous worker
process before launching the next one. Fresh-container and standby
measurements end at the successful new result, after which the harness
closes the process and removes the candidate container. In the intended
live implementation the previous revision can continue running while the
candidate uses a separate container.

## Method

Run on `calternal-perf-01-backup01` under `/usr/local/bin/perf-lock.sh`.
Podman 6.1.4 used rootless crun and overlay in a dedicated benchmark store.
The cached `docker.io/library/node:24-alpine` image reported Node v24.22.3.
Its image ID, kernel version, worker digest and per-sample results are in
[the measurements](plugin-reload-2026-11-18.json).

Copy a prebuilt Notes Insights worker from the dev-02 build, its manifest,
the shipped worker seccomp rules, and the [harness](../../scripts/bench-plugin-reload.py).
Prepare the cache before timing. Run the harness as `python3 bench.py`
under the lock with `PLUGIN_BENCH_PODMAN` and `PLUGIN_BENCH_STORAGE`
pointing to the benchmark Podman tool and store. The harness ran two
warm-up rounds, then 30 serial rounds with one sample from each candidate
per round. It sends worker-v2 frames and serves one fixed Markdown reply.
The result must match the edited bundle. Percentiles use nearest rank.

The timer includes manifest JSON decoding, hashing the prepared bundle,
Podman lifecycle calls and Node startup, then one worker invocation with
one fixture broker call. Source compilation, full host JSON Schema
compilation, the HTTP and machine transports, readiness handshake, and
broker storage I/O are outside the timer. Container creation alone was
258.2 ms p50 and 282.8 ms p95. The [earlier dev-01 cold HTTP measurement](../reviews/dev-e2e-2026-11-17.md#enrolled-worker-cold-starts-on-dev-01)
reported 656 ms p50, 1,156 ms p95 and 2,184 ms p99 under a different load,
with an enrolled machine and a real broker write.

## Isolation and activation

- A fresh container starts with a separate process, mount and IPC namespace.
  Each deployed revision receives only its own immutable source mount.
  Its bounded readiness handshake can run before publication while the
  previous revision remains available.
- A standby uses the same separate-container boundary. It has a prepared
  namespace during validation. It executes the candidate only after
  verification. The early provisioning cost is still part of the author
  path and consumes a second sandbox seat while the old worker runs.
- A fresh process reuses the container namespaces, init process, mounts and
  resource limits. The current worker profile has a read-only root and code
  mount, no network, denied child-process permission and no writable worktree.
  Reuse would require proof that the prior process and descendants are gone,
  that no old broker calls retain authority, and that the old process cannot
  observe newly staged source. Any persistent namespace state stays confined
  to the same plugin and space. The harness used fixture code in a single
  read-only directory for this candidate. It does not prove a secure source
  update or teardown path for that topology.

A widening update keeps old work on its old revision and never lets it use
the widened grant. A narrowing update or removal refuses remaining calls at
the next broker boundary. The bounded readiness handshake commits activation.
A fixture health action is optional in author checks. Draft loading requires
a development-only build boundary. The production binary must reject drafts
regardless of runtime configuration.

The measured topology is the starting point. The implementation gate remains
edit-to-first-valid-result p95 under one second in the full development loop.

## Full author loop

The fixture starts the server and enrolls a machine on perf-01. It uses
the `node:24-alpine` image recorded above. The release server and standalone
conformance binary were cross-built for `x86_64-unknown-linux-musl` on dev-02.
The benchmark runs prebuilt executables under the perf lock. Inside the
benchmark, the development CLI bundles each edited fixture. The clock starts
at the source rename and ends after the expected generated view returns
through the authenticated HTTP route and a brokered note read.

The first run (server at `d4fa600`) had 30 samples after two warm-up edits.

| Phase | Stop before first invocation, p95 | Stop after first result, p95 |
| --- | ---: | ---: |
| Manifest and bundle build | 176.2 ms | 190.5 ms |
| Package read, readiness and publication | 489.9 ms | 642.0 ms |
| Edit to CLI reports ready | 691.6 ms | 855.1 ms |
| Ready to correct first result | 419.1 ms | 50.3 ms |
| Edit to correct first result | 1,084.1 ms | **903.0 ms** |

The phase percentiles are computed independently. The full path was
1,030.7 ms p50 and 1,086.3 ms p99. The first run missed the one-second p95
gate. The machine processes sandbox stops and input in message order.
Stopping the previous draft as part of publication made the first invocation
wait behind cleanup of that container. The follow-up fences the old draft
at publication and defers physical retirement until the replacement answers.
Both sandbox permits count against capacity until authenticated teardown.
Idle eviction covers a draft whose replacement is never invoked.

The second run passed with 861.0 ms p50, 903.0 ms p95 and 915.9 ms p99
over 30 samples. CPU and machine time from a prior teardown can still
overlap the following activation. The build's p95 increased by 14.3 ms.
Scheduling the current result before teardown reduced end-to-end p95 by
181.0 ms.

Run the prebuilt conformance binary with `CALTERNAL_MEASURE_RELOAD=remote`,
`CALTERNAL_SERVER_BIN` set to the release binary built with
`test-plugins,dev-plugin-drafts`, and `CALTERNAL_CONFORMANCE_SOURCE_ROOT`
set to the copied plugin and SDK sources. Put the isolated Podman store's
wrapper on `PATH`. The gate command was:

```sh
/usr/local/bin/perf-lock.sh ./bin/isolation edit_to_first_valid_plugin_result --test-threads=1 --nocapture
```

The follow-up binary linked the same OpenSSL release as the first run
using existing x86_64-musl static archives. Rust compilation and linking
ran on dev-02.

[Raw full-loop samples](plugin-reload-author-loop-2026-11-18.json)
