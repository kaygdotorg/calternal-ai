# Plugin workers in sandboxes: Node child vs Podman container vs container in a microVM

Measured 2026-11-17 on `calternal-perf-02-backup01`, issue #14. Measurement and report only; no recommendation. Machine-readable numbers: [plugin-workers-in-sandboxes.json](plugin-workers-in-sandboxes.json). Scripts, driver source, raw latencies (every call) and logs are kept on that VM under `~/calternal-ai-plugin-sandbox` (`scripts/`, `raw/`).

Question: one way to run untrusted plugin code (the machine role's sandbox) instead of today's Node child process. How slow is it for plugin calls, and what does it cost?

## What was measured

The real `plugins/calternal-notes-insights` worker bundle (sha256 `8ef03794…3698`) and the real protocol v2 (`WorkerRuntimeV2`, newline-delimited JSON frames over the worker's stdio), one long-lived worker per (plugin, space). A small Rust driver (`pwbench`) plays the server: it sends `invoke` frames, answers each `host_call` (`read_item`) with a note immediately, and reads `complete`. A bench worker (same SDK, same stdio loop, synthetic operations `bench.hostcalls`, `bench.echo`, `bench.echo_back`) gives the host-call and large-payload cases the notes-insights operation cannot.

| Variant | How one worker is hosted |
| --- | --- |
| A | Node child process with the flags `crates/server/src/plugins/worker.rs` uses: `node --permission --allow-fs-read=<bundle dir> --disallow-code-generation-from-strings --max-old-space-size=128 worker.js`, cleared environment (`LANG` only), cwd the bundle dir |
| A0 | Control: the same without `--permission` and `--allow-fs-read` |
| B-run | `podman run -i --rm` per worker: rootless, `--network none --read-only --read-only-tmpfs=false --cap-drop all --security-opt no-new-privileges --log-driver none --memory 256m --cpus 1 --pids-limit 64`, bundle bind-mounted read-only at `/plugin`; stdio is the `podman run` client's stdio; node flags inside are the A flags except `--permission` (the container is the boundary) |
| B-exec | One long-lived container per worker (same flags), the worker started with `podman exec -i` into it, as the machine role's `Sandbox::exec` does; the container's main process is a busybox `sh -c 'read x'` |
| C-run, C-exec | The B-run / B-exec commands run inside a Firecracker microVM (2 vCPU, 4096 MiB, hybrid lane's guest image: read-only ext4 root with rootless Podman, a read-only ext4 with the Node rootfs used as `podman run --rootfs /toolset:O`, a 2 GiB ext4 data disk holding the bundles, no network device, vsock only). The driver reaches the guest through Firecracker's vsock UDS (`CONNECT 5000`), where the guest's socat starts the command as user `agent` and wires the connection to its stdin/stdout. Podman there uses `cgroupfs`, `--cgroup-parent /agent/ctr` |

Versions and limits: Node v24.21.0 (the flake's pin; also the Node used in every variant, same binary and glibc); Podman 6.1.4 and crun 1.30.1 (the versions of the earlier sandbox measurement); Firecracker v1.16.1; guest kernel 6.1.188+; host kernel 6.12.115. B's image is a `podman import` of the Node closure (285 MB, plus busybox), so A and B run the identical node binary. The container's user is root inside a user namespace, an unprivileged host uid. Worker heap cap 128 MiB, container 256 MiB memory and 1 CPU, 64 pids (`MAX_WORKERS`, timeouts and retirement of the server's pool are not in play: the driver has no deadlines). A worker accepts at most 4096 invocations over its life (`seen` set in the SDK), so each measurement uses a fresh worker with at most about 3,100 calls.

Method: all measurements under `/usr/local/bin/perf-lock.sh`, nothing built on the VM, binaries built on `calternal-dev-01-compute02`. Host and guest page caches are warm (starts are fresh processes, not cold-disk starts). Latency is wall time in the driver from writing the `invoke` frame to reading `complete`, including the host-call round trips the operation makes. Percentiles are nearest-rank over 1,000 calls after 50 warm-up calls (n = 30 for starts and stops). The driver answers host calls instantly, so "host call" numbers are transport cost only; the real broker's work (authority check, store read) comes on top, identically in every variant.

Caveats that apply to everything:
- The VM has 4 cores and the driver shares them with the workers. Parallel runs are CPU-bound, not a property of one variant alone.
- About 3.3 GiB of this VM's RAM is other lanes' leftover `/tmp` (tmpfs), so about 3.4 GiB was available. The 50-worker rows fit in that.
- The calternal-notes-insights worker itself does real work: counting words over a 200 KiB note costs about 4 ms of Node CPU, which is why the 200 KiB row is large in every variant.
- Numbers come from one run of each test on one VM; no confidence intervals.

## 1. Cold start: spawn to first response (n = 30)

"First response" is the time from starting the worker (spawn, or connect plus `podman exec`) until the first `complete` of the unchanged `notes-insights.summarize` operation (which includes one host call). For B-exec and C-exec the container is started first (last column, not included in the first response). C numbers are for a container started in an already-running VM; the VM's own boot is in section 9.

| Variant | first response p50 ms | p95 ms | max ms | 2nd call p50 ms | container start before exec, p50 ms |
|---|---|---|---|---|---|
| A | 35.5 | 37.8 | 98.9 | 0.6 | - |
| A0 (no permission flags) | 34.9 | 39.3 | 48.2 | 0.6 | - |
| B-run | 242.4 | 265.8 | 266.4 | 0.8 | - |
| B-exec | 178.2 | 195.4 | 196.2 | 0.8 | 257.1 |
| C-run | 118.3 | 140.5 | 1780 | 2.1 | - |
| C-exec | 113.9 | 129.9 | 162.7 | 2.1 | 82.1 |

C-run has slow outliers: p95 was 604 ms in the first pass and the final run has a 1.8 s maximum among 30 starts; the cause was not investigated.

## 2. Warm call round trip

One worker, one call at a time (µs; 1,000 calls). The 200 KiB rows are the case that matters most: for a note read, the 200 KiB travels host to worker as the `host_reply` frame; "200 KiB request" sends 200 KiB in the `invoke` input; "both ways" echoes it back.

| Case (p50 / p95 / p99, µs) | A | A0 (no permission flags) | B-run | B-exec | C-run | C-exec |
|---|---|---|---|---|---|---|
| Small request: summarize, 1 KiB note, 1 host call | 122 / 245 / 372 | 136 / 240 / 329 | 310 / 460 / 688 | 301 / 416 / 589 | 879 / 1169 / 1928 | 1045 / 2364 / 8506 |
| 200 KiB: summarize, 200 KiB note returned by the host call | 4023 / 6857 / 10184 | 4117 / 7283 / 10013 | 6061 / 13054 / 19483 | 7168 / 26776 / 29094 | 8014 / 20489 / 21613 | 8450 / 20330 / 21168 |
| 200 KiB request, small reply (bench echo) | 1693 / 1929 / 2077 | 1718 / 2024 / 2212 | 1907 / 2483 / 2877 | 1903 / 2284 / 2737 | 2980 / 15834 / 19215 | 3201 / 12398 / 19038 |
| 200 KiB request and 200 KiB reply (bench echo_back) | 3172 / 5237 / 5989 | 3072 / 4968 / 5732 | 3577 / 5040 / 6574 | 3594 / 4856 / 6377 | 5311 / 15422 / 18844 | 5549 / 13855 / 19985 |

## 3. Host call round trip

The bench worker makes n sequential `read_item` host calls (1 KiB reply each) in one invocation. The slope between 0 and 10 calls is the cost of one worker-to-host-and-back call; the intercept is one bare invocation. A typical operation (1 request and 3 host calls) is the "3 host calls" row.

| Operation (p50 / p95 / p99, µs) | A | A0 (no permission flags) | B-run | B-exec | C-run | C-exec |
|---|---|---|---|---|---|---|
| 0 host calls | 67.0 / 110 / 283 | 49.6 / 99.7 / 207 | 120 / 180 / 236 | 123 / 191 / 269 | 401 / 577 / 859 | 504 / 1017 / 1954 |
| 1 host calls | 97.5 / 210 / 302 | 127 / 226 / 295 | 254 / 394 / 517 | 283 / 419 / 541 | 881 / 1199 / 1768 | 897 / 1612 / 5578 |
| 3 host calls | 216 / 355 / 415 | 234 / 386 / 487 | 489 / 657 / 768 | 521 / 705 / 885 | 1694 / 2262 / 2888 | 1798 / 2461 / 3483 |
| 10 host calls | 566 / 852 / 972 | 700 / 973 / 1285 | 1327 / 1639 / 1943 | 1625 / 2189 / 2777 | 4658 / 6055 / 7870 | 5015 / 7126 / 9766 |
| Per host call (p50 slope, 10 vs 0 calls) | 50 | 65 | 121 | 150 | 426 | 451 |

## 4. Throughput

Calls per second, small request (`notes-insights.summarize`, 1 KiB note). "16 in flight" pipelines 16 invocations on one worker. Eight workers run in eight driver threads at once; the rows with a 4-vCPU VM repeat the C runs in a VM with as many vCPUs as the host has cores (the main C runs used 2 vCPUs).

| Calls per second | A | A0 (no permission flags) | B-run | B-exec | C-run | C-exec |
|---|---|---|---|---|---|---|
| 1 worker, sequential | 7993 (p50 112 µs, p99 241) | 7140 (p50 122 µs, p99 307) | 3829 (p50 249 µs, p99 451) | 2712 (p50 346 µs, p99 864) | 1115 (p50 865 µs, p99 1349) | 1074 (p50 897 µs, p99 1598) |
| 1 worker, 16 in flight | 10799 (p50 1467 µs, p99 2206) | 11303 (p50 1273 µs, p99 2767) | 8028 (p50 1833 µs, p99 14470) | 6983 (p50 2003 µs, p99 10031) | 3438 (p50 4069 µs, p99 13552) | 2893 (p50 5170 µs, p99 11753) |
| 8 workers in parallel, sequential each | 21094 (p50 293 µs, p99 1593) | 20308 (p50 304 µs, p99 1582) | 9667 (p50 631 µs, p99 3202) | 9597 (p50 554 µs, p99 3221) | 1318 (p50 3615 µs, p99 38773) | 1647 (p50 3423 µs, p99 23079) |
| 1 worker, sequential, 4-vCPU VM | - | - | - | - | 958 (p50 915 µs, p99 3777) | 991 (p50 959 µs, p99 1946) |
| 8 workers, 4-vCPU VM | - | - | - | - | 3460 (p50 1843 µs, p99 7936) | 2696 (p50 1762 µs, p99 11708) |

## 5. Host memory per idle worker

PSS in MiB of everything the workers add on the host (processes started by the run, minus the driver), mean of 5 samples taken 8 to 18 s after the last worker was warmed, then the drop in the host's `MemAvailable` over the same interval ("PSS / MemAvailable drop", MiB). The first worker carries the shared file pages of the Node binary (PSS charges them to whichever processes map them), so the marginal figure (1 to 50 workers) is the cost of one more worker. The C row is one VM hosting all N containers (C-exec only; C-run was measured at 10 workers: 565 MiB PSS, the same as C-exec).

| Variant | 1 worker | 10 workers | 50 workers | marginal per worker, 1 to 50 |
|---|---|---|---|---|
| A | 54 / 3 | 160 / 129 | 617 / 627 | 11.5 / 12.7 |
| B-run | 103 / 42 | 319 / 250 | 1294 / 1346 | 24.3 / 26.6 |
| B-exec | 106 / 23 | 368 / 281 | 1322 / 1389 | 24.8 / 27.9 |
| C-exec | 329 / 195 | 565 / 383 | 1604 / 1213 | 26.0 / 20.8 |

What the 50 workers consist of on the host:

| Variant | What the 50 workers add on the host (PSS) |
|---|---|
| A | node 50 procs 617 MiB |
| B-run | conmon 50 procs 22 MiB, node 50 procs 624 MiB, podman 50 procs 648 MiB |
| B-exec | conmon 100 procs 43 MiB, node 50 procs 630 MiB, podman 50 procs 647 MiB, sh 50 procs 2 MiB |
| C-exec | firecracker 1 procs 1604 MiB |

The microVM case on its own (one VM per row, 4096 MiB configured, balloon not used):

| Containers in the VM | empty VM PSS MiB | VM PSS with workers MiB | added per worker MiB | guest used MiB (MemTotal-MemAvailable) |
|---|---|---|---|---|
| 1 | 201 | 329 | 128.1 | 228 |
| 10 | 201 | 565 | 36.3 | 460 |
| 50 | 201 | 1604 | 28.1 | 1489 |

Not measured: memory a plugin actually uses while handling work; per-worker kernel memory beyond what `MemAvailable` shows.

## 6. Idle CPU

CPU time (user plus system, from `/proc/<pid>/stat`, 100 Hz resolution) of every tracked process over a 20 s window with all workers idle and warm. A zero is below that resolution. For C the tracked process is the Firecracker process, so it includes the guest kernel and every process in the guest.

| Variant | Workers | CPU of tracked processes | % of one core | per worker |
|---|---|---|---|---|
| A | 1 | 0.00 s in 20 s | 0.00% | 0.000% |
| A | 10 | 0.00 s in 20 s | 0.00% | 0.000% |
| A | 50 | 0.00 s in 20 s | 0.00% | 0.000% |
| A0 (no permission flags) | 10 | 0.00 s in 20 s | 0.00% | 0.000% |
| B-run | 1 | 0.02 s in 20 s | 0.10% | 0.100% |
| B-run | 10 | 0.16 s in 20 s | 0.80% | 0.080% |
| B-run | 50 | 0.72 s in 20 s | 3.60% | 0.072% |
| B-exec | 1 | 0.02 s in 20 s | 0.10% | 0.100% |
| B-exec | 10 | 0.18 s in 20 s | 0.90% | 0.090% |
| B-exec | 50 | 0.69 s in 20 s | 3.45% | 0.069% |
| C-run | 10 | 1.01 s in 20 s | 5.03% (empty VM alone: 0.13%) | 0.503% |
| C-exec | 1 | 0.15 s in 20 s | 0.75% (empty VM alone: 0.07%) | 0.747% |
| C-exec | 10 | 0.78 s in 20 s | 3.88% (empty VM alone: 0.07%) | 0.388% |
| C-exec | 50 | 2.24 s in 20 s | 11.13% (empty VM alone: 0.13%) | 0.223% |

## 7. Stop and reap

Time from the stop request until the worker is gone (n = 30). Graceful: close the worker's stdin (the worker exits on EOF). Forced: `kill -9` for A; `podman rm -f -t 0` for B and C. For B-run, `--rm` removal is included in graceful (the `podman run` client exits after it). For C the host cannot see the process end: Firecracker's vsock UDS has no half-close, so closing the driver's end ends the guest connection at once; "graceful" for C is the time until a poll running in the guest finds no process mentioning the bundle (it includes one guest rpc round trip, about 3 ms), forced is the guest `podman rm -f -t 0` call's own duration (also one rpc).

| Variant | graceful (close stdin, wait for exit) p50 / p95 ms | forced (kill or `podman rm -f -t 0`) p50 / p95 ms |
|---|---|---|
| A | 3.4 / 3.9 | 1.9 / 2.2 |
| A0 (no permission flags) | 3.5 / 4.0 | 1.9 / 2.1 |
| B-run | 162.3 / 184.2 | 254.0 / 292.9 |
| B-exec | 86.3 / 94.3 | 328.5 / 352.5 |
| C-run | 32.1 / 94.5 | 94.3 / 117.8 |
| C-exec | 26.1 / 46.4 | 109.1 / 560.9 |

A first C-exec run showed 5.6 s for the forced stop. That was a harness artifact (the driver then also waited for the guest connection to close, which lingers about 0.5 s; the rest of the 5 s was not isolated), not Podman: direct timings in the guest of `podman rm -f -t 0` are 84 to 98 ms for an idle container, 88 to 120 ms with a detached exec'd node, and 71 to 92 ms after `podman kill` (`raw/cexec-force-investigation.json`).

## 8. What the worker can reach (probe from inside)

`probe.js` runs as the worker would (same command line as each variant, with the probe in place of the bundle) and tries: reading 13 paths (`/etc/passwd`, `/proc/self/environ`, `~/.ssh/id_ed25519`, cgroup files, its own bundle…), listing `/`, `/proc`, `/home`, `/nix/store`, `/dev`, writing 5 locations (`/tmp`, `/dev/shm`, `/`, the bundle dir, home), spawning `node` and `/bin/sh`, `eval`/`new Function`, and TCP connects to a listener on the host's loopback, the host's LAN address (10.70.3.18) and `1.1.1.1:443`, plus a DNS lookup. In the container variants the loopback address is the container's own (nothing listens there), so a refused connection to it is not a host leak test; the host-LAN and internet connects are.

| Variant | identity, environment | file reads | file writes that worked | processes visible in /proc | network: loopback listener / host LAN IP listener / 1.1.1.1:443 / DNS | spawn |
|---|---|---|---|---|---|---|
| A | uid 1001, pid 476652, env ['LANG'] | 1 of 13 probed paths readable (bundle file only) | none | {'error': 'ERR_ACCESS_DENIED'} | connected / connected / connected / 172.66.147.243 | node child: ERR_ACCESS_DENIED |
| B-run | uid 0, pid 1, env ['PATH', 'container', 'LANG', 'HOME', 'HOSTNAME'] | 10 of 13 probed paths readable | none | 1 | error:ECONNREFUSED / error:ENETUNREACH / error:ENETUNREACH / error:EAI_AGAIN | node child: child ran |
| B-exec | uid 0, pid 2, env ['PATH', 'container', 'LANG', 'HOME'] | 10 of 13 probed paths readable | none | 2 | error:ECONNREFUSED / error:ENETUNREACH / error:ENETUNREACH / error:EAI_AGAIN | node child: child ran |
| C-run | uid 0, pid 1, env ['PATH', 'LANG', 'HOME', 'HOSTNAME'] | 10 of 13 probed paths readable | none | 1 | error:ECONNREFUSED / error:ENETUNREACH / error:ENETUNREACH / error:EAI_AGAIN | node child: child ran |
| C-exec | uid 0, pid 2, env ['PATH', 'LANG', 'HOME'] | 10 of 13 probed paths readable | none | 2 | error:ECONNREFUSED / error:ENETUNREACH / error:ENETUNREACH / error:EAI_AGAIN | node child: child ran |

Facts from the same probes, plain:
- A: the permission model blocks all reads outside the bundle dir, all writes, child processes and `/proc`; it does not block the network. The worker connected to a service on the host's loopback, to the host's LAN address and to the internet, and resolved DNS. Its host has `lo`, `eth0`, `wt0` (NetBird) and `tailscale0`, all visible to it. Environment is `LANG` only. It runs as the server's own uid.
- B: no network interface except `lo`; a read-only root holding only the Node closure (67 store paths) and `/plugin`; no writable location at all (`--read-only-tmpfs=false`); its own PID namespace (1 or 2 processes); cgroup limits are readable (`memory.max 268435456`, `cpu.max 100000 100000`, `pids.max 64`). Node's permission model is not enabled inside, so a plugin can spawn another `node` (it ran; the pids limit bounds it) but there is no shell and no other binary. `eval` stays blocked by the flag.
- C: the same view as B from inside the container, one more layer out: the VM has no network device at all (`/proc/net/dev` shows `lo` only) and only three block devices and a vsock device. The host's files, processes and network are not reachable from the guest except through the vsock channel the harness opened.

## 9. MicroVM start and stop

Fresh VM, 10 samples (disk preparation excluded: a 2 GiB sparse ext4 formatted in about 40 ms): API configuration to the guest rpc answering `true`: min 3.4 s, median 4.3 s, max 5.9 s (the earlier sandbox measurement reported 2.7 s). First container's first response in a just-booted VM: median 596 ms (min 540, max 1890), against about 115 ms in a VM that already ran containers. Killing the VM process and removing its files took 0.24 s per VM including Python start-up and a fixed 0.2 s sleep in the script, so the kill itself is under 40 ms.

## 10. Extra latency of B and C over A

Differences of medians (and p95 where stated) against A, µs unless noted. Typical operation = 1 request plus 3 host calls (bench `hostcalls` with n = 3).

| Variant vs A | typical op (1 request + 3 host calls), p50 µs | same, p95 µs | small request p50 µs | 200 KiB p50 | 200 KiB p99 |
|---|---|---|---|---|---|
| B-run | +273 | +301 | +188 | +2.0 ms | +9.3 ms |
| B-exec | +306 | +350 | +180 | +3.1 ms | +18.9 ms |
| C-run | +1479 | +1907 | +757 | +4.0 ms | +11.4 ms |
| C-exec | +1583 | +2105 | +924 | +4.4 ms | +11.0 ms |

In absolute terms for that typical operation: A 216 µs, B-run 489 µs, B-exec 521 µs, C-run 1694 µs, C-exec 1798 µs (p50). Node without the permission flags (A0) differs from A by at most 0.14 ms at p50 in every row, so the permission model is not what the costs above are made of.

## Commands

On perf-02 (`scripts/env.sh` sets the tool paths; `scripts/pw.sh <variant> <subcommand> …` builds the worker command per variant):

```
/usr/local/bin/perf-lock.sh scripts/run-all.sh        # A, A0, B, C tests, memory, idle CPU
/usr/local/bin/perf-lock.sh scripts/run-fix.sh        # re-run of the C items that failed in the first pass
/usr/local/bin/perf-lock.sh scripts/run-fix2.sh       # C cold start and stop with the final driver
/usr/local/bin/perf-lock.sh scripts/run-probe.sh      # reachability probes A, A0, B
/usr/local/bin/perf-lock.sh scripts/run-probe2.sh     # reachability probes C
# examples of what pw.sh runs:
pwbench warm --load summarize --size 1024 --calls 1000 --warmup 50 --clear-env 1 --cwd $D -- $NODE --permission --allow-fs-read=$D \
    --disallow-code-generation-from-strings --max-old-space-size=128 $D/worker.js                       # A
pwbench warm … --stop-cmd "$PODMAN rm -f -t 0 -i pwb-{id}" -- $PODMAN run -i --rm --name pwb-{id} --memory 256m --cpus 1 --pids-limit 64 \
    --network none --read-only --read-only-tmpfs=false --cap-drop all --security-opt no-new-privileges --log-driver none \
    --env LANG=C.UTF-8 -v $D:/plugin:ro localhost/pwb-node:24 $NODE <node flags> /plugin/worker.js      # B-run
sudo -n python3 scripts/memrun.py <variant> <N> --cpu-window 20                                           # memory and idle CPU
```

`$PODMAN` is `podman --root … --runroot … --storage-driver overlay --runtime <crun>`; `pwbench` subcommands are `cold`, `warm`, `tput`, `stop`, `hold`, `probe`.

## Setup and changes to the VM

Installed through nix on perf-02: Podman 6.1.4, crun 1.30.1, Node 24.21.0, Firecracker 1.16.1, e2fsprogs 1.47.4. System changes: `apt install uidmap` (rootless Podman needs `newuidmap`), `kayg` added to group `kvm`, a permissive `~/.config/containers/policy.json`. The guest kernel and OS image were copied from perf-01's `~/calternal-ai-sandbox-hybrid` (`vmlinux`, `os.ext4`; 650 MB read, `ionice -c3`, hashes checked); the Node rootfs image for the guest was built here. Otherwise only a few small files (harness scripts, configuration) were read on perf-01; nothing was run there.

## What went wrong along the way, and what it affects

- The guest's rpc listener merges stderr into the stream, and Podman prints a one-line warning ("`/` is not a shared mount") at the start of some runs. The first C cold-start, graceful-stop and 4-vCPU throughput runs failed on it; the driver now skips lines that are not frames. Those items were re-run (C cold start, stop, 4-vCPU throughput, probes). The C warm, host-call, 1-worker and 8-worker (2 vCPU) throughput and memory runs came from the first pass, whose driver differed only in not tolerating that line and in retrying vsock connects; the latency path is the same code.
- Eight concurrent vsock connects were sometimes refused by the guest listener; the driver retries up to 6 times (the hybrid lane's harness does the same). Retries happen only before the timed section of throughput runs.
- Closing a vsock UDS in Firecracker ends both directions, so "close stdin" cannot be used as a stop signal for C; see section 7.
- In `run-all.sh` the probe output kept a trailing blank line and was empty; the probes were re-run with `run-probe.sh` and `run-probe2.sh`.
- Not covered: Podman with its default log driver (journald or file) would add disk or journal writes for stdout; here logging is off, which is also the privacy-safe setting. No SELinux/AppArmor/seccomp profile tuning beyond Podman's defaults and `--cap-drop all`. No per-person-VM-with-many-workers CPU contention beyond the 8-worker runs.
