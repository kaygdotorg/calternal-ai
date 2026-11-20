# Splitting plugins without the memory cost

Measured on 2026-11-20 on `calternal-perf-01-backup01` (kernel 6.12.115+deb13.1-amd64, 4 AMD Ryzen 7 3700X cores under KVM, 7.7 GiB RAM, no swap), every run under `/usr/local/bin/perf-lock.sh`. Nothing was built on the perf VM. The guest image was built on `calternal-dev-01-compute02` and copied with SHA-256 checks. [Forgejo #103](https://git.kayg.org/kayg/calternal-ai/issues/103). The [JSON companion](plugin-split-memory-2026-11-20.json) has the raw samples. The bench is in [`bench/plugin-memory/`](../../bench/plugin-memory/README.md). It is not product code.

## Question

The Chat app (until today Threads) may be split into several replaceable plugins ([proposal](https://git.kayg.org/kayg/calternal-ai/src/branch/lane/threads-split/docs/proposals/threads-plugin-split-2026-11-20.md)). Today every plugin with code is its own sandboxed Node process in the space's plugin microVM ([`plugin-api-v2.md` §2](https://git.kayg.org/kayg/calternal-ai/src/commit/3e62ca07/docs/plugin-api-v2.md)). The proposal estimated about 25 MiB per process by dividing an earlier total. This lane measured the real cost and three ways to reduce it:

1. **Interface-only plugins run no process.**
2. **Start on first call, stop when idle.**
3. **One shared runtime per publisher**, each plugin in its own worker thread (its own V8 isolate and heap).

## Short answer

- One more plugin process costs **about 15–17 MiB** of guest memory, not 25 MiB. Seven processes add 105 MiB over a space with no plugin processes.
- **Approach 1** costs nothing: no process, no memory, no wake work.
- **Approach 2** is the useful saving. With an idle stop of 15 s, seven plugins of which one is in use averaged 63 MiB instead of 120 MiB of plugin PSS. A stopped plugin's next call takes **35–51 ms** p50 to start when its files are cached and **86–157 ms** p50 when they are not.
- **Approach 3** saves almost nothing and costs isolation. Seven plugins in one runtime used 95 MiB against 105 MiB as processes (−10%). With one or two plugins it uses more. Calls take about twice as long. One plugin's native crash or memory exhaustion killed all plugins in the runtime. One plugin could send frames that the host took as another plugin's. Do not adopt it.

## Method

The guest is the same arrangement as the [space runtime walls](space-runtime-walls.md) arm A: Firecracker 1.16.1, the same guest kernel (`vmlinux` SHA-256 `f5735774…`), two vCPUs, a read-only ext4 root, vsock to the host, no network. The guest had **512 MiB** (the walls used 256 MiB; seven processes needed more room). Node is 24.21.0, as in the walls.

Inside the guest a bench supervisor (`guest/host.mjs`) stands in for the plugin supervisor. It starts the plugins, answers their broker calls from memory (fixed replies, no I/O), and reads memory from `/proc`. It starts each plugin process with the product's flags: `--permission --allow-fs-read=<package> --disallow-code-generation-from-strings --max-old-space-size=128`. Everything in the guest ran as root without the product's in-guest seccomp and user separation. That changes neither memory nor start time much, but it is not the full product wall.

Image `root.ext4` SHA-256 `35ad2a68…` ran the memory, latency, wake and idle-stop series. The fault series ran on a repacked image that only adds OOM protection for the relay and resets each plugin's OOM score to the default. Without that, plugins inherited the supervisor's protection and the guest kernel panicked when it ran out of memory. The first two fault attempts are kept on perf-01.

Workload, in the order used for 1, 2, 4 and 7 plugins:

| # | Plugin | Source | Call used |
| --- | --- | --- | --- |
| 1 | Threads (Chat core) | `origin/lane/threads-plugin` at `ae428ee6`, bundled by its own build | `threads.refresh` on a version 1 fixture conversation: reads the file through the broker, parses it and builds a page |
| 2 | Notes Insights | same checkout, `dist/worker.js` | `notes-insights.summarize`, one `read_item` broker call |
| 3 | Plugin Guide | same | `calternal-plugin-guide.explain` |
| 4 | Threads Claude Code harness | same | `threads-claude-code.run` with a fake provider, two broker calls |
| 5 | Plugin Submission Guide | same | `calternal-plugin-submission-guide.explain` |
| 6 | Composer Settings fixture | same | `composer-settings.label`, one broker call |
| 7 | Trivial | bench | echo, no imports |

The Threads bundles export handlers rather than a serve loop, so a 40-line bench adapter (`guest/runner.mjs`) serves them on the same newline wire. The other workers run unchanged.

Measures:

- **Plugin PSS**: sum of `Pss` from `smaps_rollup` over plugin processes (or the one shared runtime). PSS splits shared file pages (the Node binary) between processes, so one process alone looks bigger than its real share.
- **Private**: sum of `Pss_Anon`, the memory no other process shares.
- **Guest used**: `MemTotal − MemAvailable` in the guest. **Added** is guest used minus the same value with the supervisor running and no plugin process (5 boots, 35.8 MiB). This is the clearest cost per plugin.
- **VMM PSS**: PSS of the Firecracker process on the host, which is what the host machine pays.
- Medians use the midpoint. p95 is nearest rank.

Memory was read 2 s after each plugin had answered 100 to 1,050 calls. Every layout row used 5 fresh boots.

## Baseline: one process per plugin

| Plugins | Boots | Plugin PSS MiB | Private MiB | Guest used MiB | Added MiB | Added per plugin MiB | VMM PSS MiB | Start all plugins p50 / p95 / max ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 5 | 0 | 0 | 35.81 | 0 | — | 123.37 | — |
| 1 | 5 | 39.03 | 14.33 | 52.77 | 16.96 | 16.96 | 145.43 | 61.37 / 72.80 / 72.80 |
| 2 | 5 | 60.61 | 28.87 | 69.24 | 33.43 | 16.71 | 163.30 | 63.51 / 76.21 / 76.21 |
| 4 | 5 | 93.90 | 55.99 | 100.71 | 64.90 | 16.23 | 193.38 | 98.82 / 107.24 / 107.24 |
| 7 | 5 | 135.13 | 93.71 | 141.15 | 105.34 | 15.05 | 235.38 | 155.36 / 157.16 / 157.16 |

Per process with seven plugins (median of 5 boots, MiB):

| Plugin | PSS | Private | Shared file share | RSS |
| --- | --- | --- | --- | --- |
| Threads | 22.32 | 14.39 | 7.93 | 60.18 |
| Notes Insights | 20.14 | 14.78 | 5.40 | 56.65 |
| Plugin Guide | 17.91 | 12.36 | 5.56 | 54.57 |
| Threads Claude Code | 20.40 | 14.18 | 6.23 | 57.98 |
| Submission Guide | 17.78 | 12.35 | 5.44 | 54.43 |
| Composer Settings | 19.50 | 14.09 | 5.44 | 56.17 |
| Trivial | 16.88 | 11.44 | 5.44 | 53.35 |

- Even the trivial plugin costs 11.4 MiB private. Most of a plugin's cost is Node itself, not plugin code. Threads costs about 3 MiB more than the trivial plugin.
- RSS overstates the cost about fourfold because every process counts the whole Node binary.
- Boot to a running supervisor took 720–730 ms p50 whatever the plugin count. Starting all plugins at once after boot grew from 61 ms (1) to 155 ms (7) on two vCPUs.
- The VMM's host PSS grew by 112 MiB for seven plugins, in line with the guest.

### Wake from a full snapshot against the number of processes

Snapshot taken after warm-up, VMM stopped. A wake is launch, restore, vsock connect and one Threads call, timed from the driver. *Cold* evicted the snapshot and image files from the host page cache before each wake. 20 wakes per row.

| Plugins | Warm p50 / p95 / max ms | Cold p50 / p95 / max ms | VMM PSS after wake MiB (warm) |
| --- | --- | --- | --- |
| 1 | 357.70 / 464.67 / 509.18 | 520.66 / 603.79 / 612.20 | 64.97 |
| 2 | 358.81 / 469.45 / 484.27 | 563.86 / 699.40 / 733.87 | 67.03 |
| 4 | 622.51 / 925.26 / 952.99 | 753.09 / 848.68 / 980.05 | 72.54 |
| 7 | 803.36 / 930.07 / 978.07 | 864.37 / 989.63 / 1035.06 | 74.88 |

Wake time roughly doubles from 2 to 7 processes. All rows stay inside the contract's three-second wake allowance. The first release boots fresh rather than restoring (`plugin-api-v2.md` §2). For a fresh boot the cost is boot (about 725 ms) plus starting all plugins (61–155 ms above), or only the called plugin with approach 2.

## Approach 1: interface-only plugins

Measured: nothing to measure. A package without code starts no process, so it adds 0 MiB and no wake work. The bench confirms the supervisor starts processes only for listed workers. In the product, `worker` is already optional in `contract/plugin-manifest-v2.schema.json`, and `app.open` already serves a pinned description "with no worker wake".

What the host needs, as contract text:

- A package without `worker` never starts a process, in any placement.
- Install checks that every action in its descriptions is something the host runs itself (engine operations, navigation, settings) or an operation declared by a named other package. A description that needs its own code is refused at install with a plain reason.
- Its permissions are checked against the description at install, because nothing runs later to check.

## Approach 2: start on first call, stop when idle

### First call after a stop

The plugin was stopped (SIGKILL after its last call). The timer starts at the call and ends at the reply, so it includes process start, Node start, module load and the call. *Warm*: the plugin's files are in the guest page cache. *Cold*: the guest dropped its caches (`drop_caches`) and the host evicted the image before each sample. 40 samples per row; 1,120 in total for processes.

| Plugin | Warm p50 / p95 / max ms | Cold p50 / p95 / max ms |
| --- | --- | --- |
| Threads | 50.75 / 54.81 / 56.39 | 156.89 / 175.51 / 179.84 |
| Threads Claude Code | 45.75 / 51.86 / 66.45 | 106.08 / 117.26 / 187.77 |
| Notes Insights | 36.85 / 40.68 / 44.01 | 93.44 / 100.79 / 103.15 |
| Plugin Guide | 35.88 / 38.27 / 40.78 | 93.12 / 101.67 / 102.83 |
| Submission Guide | 34.93 / 37.82 / 38.93 | 87.88 / 100.88 / 102.86 |
| Composer Settings | 34.68 / 38.73 / 40.31 | 86.25 / 96.06 / 98.72 |
| Trivial | 34.17 / 36.82 / 38.11 | 89.15 / 98.30 / 103.19 |

- About 34 ms of every start is Node itself. Plugin code adds 1–17 ms.
- Node's on-disk compile cache (`NODE_COMPILE_CACHE`) made no measurable difference (Threads warm 48.32 vs 50.75 ms, cold 151.66 vs 156.89 ms; 40 samples each). It is not worth the writable cache directory it needs.
- **Lost warm state**: calls 2, 3, 6 and 21 after a start all took 0.1–1.0 ms, the same as long-running processes. These plugins keep no in-memory cache, so a stop loses nothing measurable. A plugin that builds a large cache in memory would lose it. The contract already says nothing in memory must survive sleep.

### Memory over time

All seven plugins called at 0 s, then only Threads every 2 s, and Notes Insights once at 30 s. Idle stop after 15 s. Plugin PSS in MiB, live processes in brackets, sampled every 0.5 s for 60 s (one run each):

| Layout | 1 s | 10 s | 20 s | 31 s | 40 s | 50 s | 59 s | Mean |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Always on, processes | 119.9 (7) | 120.1 (7) | 120.2 (7) | 120.4 (7) | 120.4 (7) | 120.7 (7) | 120.7 (7) | 120.3 |
| Idle stop, processes | 119.5 (7) | 119.3 (7) | 38.0 (1) | 55.5 (2) | 55.5 (2) | 38.2 (1) | 38.4 (1) | 62.9 |
| Idle stop, threads in one runtime | 108.0 (7) | 107.8 (7) | 88.8 (1) | 67.2 (2) | 67.5 (2) | 64.7 (1) | 61.1 (1) | 78.5 |
| Always on, threads in one runtime | 107.8 (7) | 107.5 (7) | 107.4 (7) | 107.5 (7) | 107.6 (7) | 107.9 (7) | 108.7 (7) | 107.7 |

A stopped process gives all its memory back at once. A stopped worker thread does not: the runtime kept most of the memory for some time after its threads ended.

## Approach 3: one shared runtime, a worker thread per plugin

One Node process (`guest/shared-runtime.mjs`) per space with the product flags plus `--allow-worker`. Each plugin is a `worker_threads` Worker with `resourceLimits.maxOldGenerationSizeMb = 128`, the same heap limit as a process. The supervisor talks to the runtime over one pipe, and the runtime passes frames to each thread.

**Which Node mechanism separates heaps?** A worker thread is a separate V8 isolate: it has its own heap, its own heap limit, and `worker.terminate()` stops it even inside an endless loop. `node:vm` contexts share one isolate and one heap: no separate limit, and Node says `vm` is not a security mechanism. The `isolated-vm` package gives separate isolates with a memory limit but without Node's APIs, so plugins would need a new host bridge. Only worker threads fit the existing workers, so only they were measured.

### Memory and start

| Plugins | Boots | Runtime PSS MiB | Private MiB | Added MiB | Added per plugin MiB | vs processes | VMM PSS MiB | Start all p50 / p95 / max ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 5 | 52.21 | 25.40 | 28.55 | 28.55 | +11.59 | 157.40 | 129.89 / 134.67 / 134.67 |
| 2 | 5 | 63.97 | 37.11 | 41.78 | 20.89 | +8.35 | 171.37 | 130.42 / 139.50 / 139.50 |
| 4 | 5 | 85.55 | 58.30 | 65.47 | 16.37 | +0.57 | 195.42 | 182.53 / 198.14 / 198.14 |
| 7 | 5 | 114.71 | 87.46 | 95.08 | 13.58 | −10.26 | 223.41 | 300.77 / 326.76 / 326.76 |

The runtime itself costs about 17 MiB, and each further thread about 11 MiB, because every isolate has its own heap (6–8 MiB used and about 10 MiB reserved per thread at seven plugins) and its own copy of Node's internal modules. Threads also started more slowly than processes.

Wake from a snapshot was no faster: seven threads woke at 910.59 / 1122.47 / 1209.99 ms warm and 996.74 / 1494.28 / 2016.62 ms cold (p50 / p95 / max, 20 each), against 803 and 864 ms p50 for seven processes. With approach 2, a thread in an already running runtime started in 34–42 ms p50 even after the guest dropped its caches, because the runtime keeps Node loaded. That only helps when the runtime is already running.

### Added call latency

1,000 calls per plugin after 50 warm-up calls, seven plugins live, timed in the guest:

| Plugin | Processes p50 / p95 µs | Shared runtime p50 / p95 µs |
| --- | --- | --- |
| Threads | 223.3 / 367.9 | 461.0 / 793.9 |
| Notes Insights | 189.9 / 320.0 | 412.7 / 643.1 |
| Plugin Guide | 87.4 / 168.3 | 193.9 / 322.8 |
| Threads Claude Code | 211.4 / 337.0 | 472.9 / 685.1 |
| Submission Guide | 100.6 / 181.4 | 198.8 / 334.8 |
| Composer Settings | 153.0 / 280.9 | 341.3 / 584.0 |
| Trivial | 63.5 / 189.7 | 143.1 / 215.6 |

The extra hop through the runtime's main thread about doubles each call (+80 to +260 µs p50). That is small against a network round trip, but it is pure cost.

### One misbehaving plugin

Layout: Threads, Notes Insights, Trivial and a fault plugin. The supervisor called the three good plugins in a loop for 3 s (8 s for the memory case) while the fault plugin misbehaved. Fresh boot per row, one run each.

| Fault | Processes: other plugins | Shared runtime: other plugins |
| --- | --- | --- |
| Uncaught exception | unaffected (342 calls, 0 failed) | unaffected (297, 0); only the thread ended |
| `process.exit(3)` | unaffected | unaffected; only the thread ended |
| Endless loop, then stop | unaffected; SIGKILL took 3.6 ms | unaffected; `terminate()` took 2.3 ms |
| JS heap growth | unaffected; V8 had not stopped the plugin within the 3 s window (in the dev-01 smoke run it stopped at 0.5 s); other calls' p95 rose to 31.7 ms on the two shared vCPUs | unaffected; the thread stopped with `ERR_WORKER_OUT_OF_MEMORY` after 2.2 s |
| Memory outside the JS heap (16 MiB buffers every 20 ms) | unaffected; the guest OOM killer killed only the fault process | **all failed** (293 of 1,822 calls): the OOM killer killed the whole runtime |
| Native crash (SIGSEGV to its own process) | unaffected | **all failed** (291 of 376): the runtime died |
| `process.abort()` | unaffected; only the plugin died | refused inside a thread; nothing happened |
| Write a frame on descriptor 1 naming another plugin | rejected as malformed; it can only write its own pipe | **accepted as a Threads reply** |
| Read `/proc/self/maps` and `/proc/self/environ` | refused (`ERR_ACCESS_DENIED`) | **both readable** (3 of 3 boots) |

The last row is a Node finding. In Node 24.21.0, a worker thread can read any file below the process's current directory even though `--allow-fs-read` does not grant it and the main thread is refused. The guest runs with `/` as its current directory, so a plugin thread could read every file in the guest. This was reproduced on dev-01 with a minimal script. The product does not use worker threads today, so it is not exposed. Node's documentation also says its permission model is a "seat belt" and does not protect against malicious code.

### What isolation is lost against separate processes

- **Crash containment.** A native crash, a fatal signal or the OOM killer takes every plugin in the runtime with it. Uncaught exceptions, `process.exit`, endless loops and JS heap growth stay contained.
- **Memory limits.** The heap limit is per thread, but buffers and other memory outside the JS heap are not limited per thread. Only the whole runtime can be limited, so one plugin can use the others' memory.
- **Accounting.** The kernel accounts memory per process. Per plugin there are only V8 heap statistics. CPU can be read per OS thread, but not limited per thread.
- **Identity on the wire.** All threads share one descriptor table, so any plugin can write to the runtime's pipe as any other plugin. The host must treat the whole runtime as one identity: a ticket given to one plugin is available to all.
- **OS permissions.** seccomp, the user ID, cgroup limits and the Node permission model all apply to the whole process. The permission model also has the worker-thread read gap above.
- **Memory between plugins.** A memory-safety bug in V8 or Node gives one isolate the others' memory. With processes the same bug stays inside one process.

Limiting the runtime to one publisher's plugins does not fix this: the host still could not tell which plugin acted, and a bug in one plugin still breaks the publisher's others.

## Not measured

- The product's in-guest sandbox (unprivileged user, seccomp, `ExecRequest`), the real broker, SQLite and real user data. Broker replies came from memory.
- The heavy plugin VM, several spaces waking at once, and memory pressure from other spaces on the host.
- Fresh-boot wakes with the full product supervisor. Boot to a running bench supervisor (720–730 ms) stands in for it.
- Node startup snapshots (`--snapshot-blob`) and `isolated-vm`.
- Stopping the shared runtime itself when it has no threads.
- Mac (`mba`) and the no-KVM self-host path. Per-process numbers should be similar there, but that is untested.
- One run per fault and per timeline layout. Their results are pass/fail observations, not distributions.

## Recommendation

1. **Adopt approach 1.** Interface-only parts (the list, composer and people panel in the split proposal) cost nothing. The contract must state that a package without `worker` starts no process and is fully checked at install.
2. **Adopt approach 2 for every plugin process, any publisher.** The host starts a plugin process on its first admitted call and stops it after an idle period. This keeps memory close to the plugins actually in use. A split Chat with four coded parts then costs about 15–17 MiB per part only while that part is in use. A first call costs about 35–50 ms p50 warm and 90–160 ms p50 cold, well inside the 3 s wake allowance. The contract would have to say:
   - The host may start a plugin process on its first admitted call and stop it after it has had no active invocation, job or alarm for an idle period the host chooses. Plugins must not rely on in-memory state between calls (this extends today's "nothing in memory survives sleep" from the placement to each plugin).
   - Process start counts inside the wake allowance when the placement is waking, and inside the dispatch allowance otherwise. The coordinator has to choose which (question below).
   - Accounting and limits stay per plugin process.
3. **Do not adopt approach 3.** It saves 10 MiB at seven plugins, costs more than separate processes at one or two, doubles call latency, and loses crash containment, per-plugin memory limits, per-plugin identity and OS permissions. Keep one process per plugin in `plugin-api-v2.md` §2, and state that plugins never share a process.
4. **Correct the split proposal's cost table.** Use about 15–17 MiB per process (about 105 MiB for seven, about 50 MiB for four) rather than 25 MiB, and note that with approach 2 only parts in use cost memory.

## Questions for the coordinator

- Should a cold plugin start count against the three-second wake allowance or the separate interactive cutoff, when the placement is already awake?
- Which idle period should the host use before stopping a plugin process? 15 s was the bench value. A longer period trades memory for fewer first-call delays.
- Should the Node worker-thread permission gap be reported upstream? This lane did not contact anyone outside the project.
