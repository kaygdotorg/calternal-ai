
# Hybrid sandbox: one microVM per person-project, one rootless container per thread

Measured 2026-11-17 on `calternal-perf-01-backup01`, same machine as [sandbox-containers-vs-microvms.md](sandbox-containers-vs-microvms.md) (the "previous report"). Measurement and report only; no technology selection is made. Raw results and scripts stay on that VM under `~/calternal-ai-sandbox-hybrid`; the machine-readable summary is [sandbox-hybrid.json](sandbox-hybrid.json). All measurements ran under `/usr/local/bin/perf-lock.sh`; nothing was built or measured on hd01. perf-02 was not used.


## Machine, setup and what was reused

Host: Debian 13.7, Linux `6.12.115+deb13.1-amd64`, 4 exposed cores (Ryzen 7 3700X model), 7.7 GiB RAM, no swap, KVM nested on Proxmox. **Firecracker here is nested virtualization**; no bare-metal comparison exists.

Reused from the previous run (same Nix store paths, so identical binaries): Podman 6.1.4, crun 1.30.1, conmon 2.2.1, Firecracker 1.16.1 (default seccomp, no jailer), the Firecracker CI guest kernel 6.1.188+, the toolset closure (`config.json` closure list of 174 store paths: git 2.55.0, Node 24.21.0, Rust 1.99.1, ripgrep 15.2.0, C compiler, prefetched Cargo sources), `/sbin/workload` (idle: shell plus a Node timer; light: adds `rg --hidden -n 'pub|async|space' /work` every 3 s), the item 8 first command and dummy-credential method, the sparse 8 GiB writable ext4 per VM (`mkfs.ext4 -E lazy_itable_init=0,lazy_journal_init=0`), ballooning configuration (`amount_mib=0`, `deflate_on_oom`, `free_page_reporting`, 1 s statistics), and the quantile method (median and nearest-rank p95).

Rebuilt (the previous run deleted its images): the toolset ext4 (2.2 GiB tree, 4 GiB logical, same content, now owned by uid 1000), the source snapshot (this worktree at `496d001d5aa36f6c5141013757f6261b28160925`, 4.6 MiB archive, `cargo fetch` resolved against it; the previous run used a different, dirty checkout), and a fan-out fixture (source plus a fresh Git repository, 6.6 MiB archive). Source and Rust version differ from the previous run, so build-heavy figures are not directly comparable; start, memory and fan-out figures use the same method.

New for this run:

- **Guest image**: `os.ext4` (read-only root, 0.6 GiB allocated) holds BusyBox, Podman and its closure (123 store paths), socat (vsock listener), setuid `newuidmap`/`newgidmap`, `/init`, `/sbin/rpc`. A non-root guest user `agent` (uid 1000, subuid/subgid 100000+65536) runs rootless Podman with `cgroup_manager=cgroupfs`, `events_logger=file`, `--network none`. No systemd, no SSH.
- **VM disks**: `vda` os (read-only, root), `vdb` toolset (read-only), `vdc` data (sparse 8 GiB ext4, read-write; Podman graphroot and `/work` layers). The toolset is the read-only lower directory of each container: `podman run --rootfs /toolset:O` (kernel overlay, `userxattr`), not an image in a Podman image store. Reason: an imported store records the importing user's id mapping; the same ownership trick applies to a directory owned by the guest `agent`.
- **Cgroups in the guest**: init enables `cpu memory pids` on `/sys/fs/cgroup/agent` and delegates it to `agent`; containers use `--cgroup-parent /agent/ctr`. `--memory`, `--cpus` and `--pids-limit` were confirmed to take effect (cgroup `memory.max` 1073741824; throttling counters in the noisy-neighbour results).
- **Control channel**: Firecracker vsock (host Unix socket, `CONNECT 5000`) to `socat VSOCK-LISTEN:5000,fork`; one connection per command, run as `agent` inside the delegated cgroup. Container commands are `podman run/inspect/exec` run in the guest. Times are host-observed (include one vsock round trip). Connect attempts that the host-side vsock muxer closed were retried (up to 6 times, 50 ms apart); retries are inside the timings. 27 retries occurred in the 4-vCPU parallel-start run (the only run that counted them); the other stages did not count them.
- **Default VM**: 2 vCPUs, 4096 MiB configured RAM (RAM is allocated lazily). Variants are named where used (4 vCPUs; 1 GiB).
- **Serial control path** (added after the first stages, see item 6): `/init` also reads `ID|command` lines from the serial console. The OS image was rebuilt for this; the starts, memory, disk, noisy-neighbour and A-vs-B stages ran on the image without it; the freeze and fan-out stages ran on the image with it.

Container start in the guest mirrors the previous run: `podman run -d` (`--network none --pids-limit 256 --memory 1024m --cgroup-parent /agent/ctr`, checkout archive mounted read-only), `podman inspect`, `podman exec … true`. Fan-out spawn adds checkout extraction, the 0600 dummy credential and the first Git/Node command, as in item 8 of the previous report.


## 1. Cold VM and container starts

Cold VM: the timer starts before data-disk preparation (sparse file plus mkfs); "agent ready" is the first successful vsock `true`; "runtime ready" adds `podman info`. The first container in the VM follows, still cold (empty guest page cache). Seconds.

| VM | n | disk prep p50 | boot to agent p50 / p95 / max | `podman info` p50 | total to runtime p50 / p95 | first container p50 / p95 | total to first container p50 / p95 / max |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2 vCPU, 4 GiB | 30 | 0.044 | 4.73 / 8.41 / 8.77 | 1.03 | 5.80 / 9.50 | 5.24 / 8.82 | 11.20 / 15.93 / 17.96 |
| 4 vCPU, 4 GiB | 10 | 0.043 | 2.88 / 2.95 / 2.95 | 0.71 | 3.65 / 3.77 | 1.77 / 4.33 | 5.37 / 7.96 / 7.96 |
| 2 vCPU, 1 GiB | 10 | 0.044 | 3.69 / 4.81 / 4.81 | 1.03 | 4.87 / 6.24 | 1.73 / 3.47 | 6.69 / 8.06 / 8.06 |

Previous report, per-thread microVM (2 vCPU, 1 GiB, minimal init, one disk): 2.68 s p50 / 3.60 s p95 to a serial shell including disk formatting. The guest kernel reports its own `Run /init` at about 3.2 s (4 GiB) and 2.0 s (1 GiB) after start; the rest is init (mounts, cgroup delegation, socat). Boot time varies run to run (2 vCPU p95 8.4 s vs p50 4.7 s); no cause was isolated.

Container start inside an already running VM (two warm-up cycles excluded; removal outside the timed region). Seconds.

| VM | Concurrency | Individual n | p50 | p95 | max | Batch p50 / p95 / max |
| --- | --- | --- | --- | --- | --- | --- |
| 2 vCPU | 1 | 30 | 0.230 | 0.295 | 0.326 | same |
| 2 vCPU | 10 | 300 | 1.51 | 5.92 | 24.84 | 1.67 / 6.13 / 24.85 |
| 4 vCPU | 1 | 30 | 0.257 | 0.297 | 0.304 | same |
| 4 vCPU | 10 | 300 | 0.953 | 1.64 | 6.12 | 1.11 / 1.89 / 6.12 |

Previous report, containers only on the host: 0.633 s single, 3.01 s p50 / 3.91 s p95 at 10 parallel. The in-VM figures are lower in the 4 vCPU case and single case; why was not isolated (differences include the previous run's `cgroup_manager=systemd`, an image store instead of `--rootfs`, and the CLI being invoked from a shell in the guest instead of from Python). Parallel individual observations within a batch are correlated.


## 2. Fan-out to first command output

Method as item 8 of the previous report: request timer starts before dispatch, ends when the last child prints `FIRST_COMMAND_OUTPUT`; each child gets a fresh container, checkout (with `.git`), dummy credential and the Git/Node command; cleanup outside the timer; 30 requests per case. 2 vCPU, 4 GiB VM; no other threads in the VM ("0 existing"). The 4 vCPU and 10-existing-idle variants of the previous report were not run in this stage; section 8 has fan-out with 4 and 10 existing threads in the VM.

| Case | New subagents | Requests | p50 s | p95 s | max s |
| --- | --- | --- | --- | --- | --- |
| warm VM | 3 | 30 | 1.31 | 2.36 | 2.48 |
| warm VM | 10 | 30 | 4.40 | 8.31 | 33.56 |
| cold VM (VM start included) | 3 | 30 | 31.20 | 42.03 | 45.92 |
| cold VM (VM start included) | 10 | **13** | 75.09 | 89.94 | 89.94 |

In the cold rows the VM was ready (agent and `podman info`) after p50 5.34 s (3) and 5.35 s (10); the remainder is the first containers of a fresh VM reading the toolset through an empty guest page cache. Only 13 of the planned 30 cold requests for 10 subagents were collected: the run was stopped for time. Previous report for comparison (containers only, no idle neighbours): 2.95 s (3) and 10.13 s (10); per-thread microVMs: 3.74 s (3) and 9.08 s (10).


## 3. Host memory

Method as the previous report: fresh VM per row, threads populated in parallel (checkout extracted), five seconds wait, five samples two seconds apart; PSS/RSS from host `/proc/<firecracker>/smaps_rollup` (VMM process only; excludes host block cache). Host available delta is `MemAvailable` before minus mean after, including the whole guest. Allocated data disk is host blocks of the sparse file.

| Activity | Threads | VM PSS MiB | PSS per thread MiB | Host available delta MiB | Guest container cgroup memory MiB | Allocated data disk MiB |
| --- | --- | --- | --- | --- | --- | --- |
| idle | 0 | 270.8 | n/a | 249.1 | 0 | 68.5 |
| idle | 1 | 353.9 | 353.9 | 351.0 | 81.4 | 73.6 |
| idle | 3 | 406.6 | 135.5 | 385.6 | 117.1 | 83.7 |
| idle | 10 | 634.2 | 63.4 | 623.2 | 241.4 | 120.1 |
| light | 0 | 268.4 | n/a | 234.7 | 0 | 68.5 |
| light | 1 | 356.8 | 356.8 | 362.3 | 87.6 | 73.6 |
| light | 3 | 440.0 | 146.7 | 423.2 | 123.8 | 84.6 |
| light | 10 | 608.3 | 60.8 | 591.9 | 251.6 | 119.9 |

Fixed cost of a VM with the runtime booted and no thread: about 270 MiB PSS (guest kernel, guest page cache of the OS image and Podman). Marginal cost of threads 2 to 10, idle: (634.2 − 353.9) / 9 = 31 MiB; light: (608.3 − 356.8) / 9 = 28 MiB. The first thread costs about 83 MiB (Podman, conmon, first reads of the toolset into guest cache). RSS equals PSS here (single process). VMM CPU over the 10 s window: 0.8% of one core with 0 threads; 45.5% (idle) and 22.4% (light) with 1 thread, an unexplained spike; 1.1% (idle) and 5.5% (light) with 10 threads; short interval and timer resolution limit these.

Previous report, per thread at 10 threads, same activities: containers only 14.2 MiB PSS (idle) and 14.4 MiB (light) (sum of container cgroup processes; excludes conmon, CLI and host page cache); microVM per thread 123.7 / 136.9 MiB PSS. The hybrid per-thread total at 10 threads (63.4 / 60.8 MiB) mixes fixed and marginal cost; the host-visible accounting differs (here the guest kernel and page cache are inside the VMM PSS).

**Ballooning and free-page reporting** (VM with 10 idle threads, then threads removed; `free_page_reporting=true`, `deflate_on_oom=true`). VMM PSS:

| Step | PSS MiB | Guest MemAvailable MiB | Guest Cached MiB |
| --- | --- | --- | --- |
| 10 idle threads running | 634.2 | 3581 | 173 |
| threads removed, +1 s | 634.2 | 3718 | 122 |
| +11 s (free page reporting only) | 634.2 | 3717 | 122 |
| after guest `drop_caches`, +3 s | 461.1 | 3784 | 7 |
| balloon inflated to 1024 MiB, +5 s | 247.3 | 2759 | 8 |
| inflated to 2048 MiB / 3072 MiB, +5 s | 247.4 / 247.3 | 1735 / 711 | 8 |
| deflated to 0, +3 s | 247.5 | 3756 | 8 |
| three light threads started again | 443.3 | 3638 | 143 |

Result: ending the threads returned no host memory by itself within 11 s (free page reporting did not reduce PSS until the guest page cache was dropped); dropping guest caches released 173 MiB; explicit balloon inflation released a further 214 MiB, to 247 MiB PSS, and further inflation did not lower it. Deflation returned guest-visible memory immediately without raising PSS (memory is re-faulted on use). Balloon statistics at 3072 MiB: target 786432 pages = actual. Ballooning needs the guest to cooperate and does not cap a hostile guest; reporting depends on guest free-page order and was not examined further.


## 4. Disk

| Shared once across VMs | Allocated MiB | Logical GiB |
| --- | --- | --- |
| `os.ext4` (guest root, read-only) | 599.4 | 2.00 |
| `toolset.ext4` (read-only lower of every container) | 2273.2 | 4.00 |

Two VMs started at the same time resolve the same device/inode for both base disks (`/proc/<pid>/fd`), and the SHA-256 of both base images was unchanged after a thread in one VM wrote 1 MiB; the other thread reported `ABSENT`.

Per VM: a blank formatted data disk allocates 68.5 MiB (journal and metadata; the previous report measured 68.2 MiB per thread for a per-thread microVM). Per thread inside the VM: the container upper layer after checkout extraction is 5.4 MiB (source archive; 9.1 MiB with `.git`, measured in the visibility run), and the data disk grew from 68.5 MiB (0 threads) to 120.1 MiB (10 idle threads), 5.2 MiB per thread including Podman metadata. Writing 1 MiB with `dd` plus `sync` grew the data disk by 16.2 MiB (journal commits). Cargo build growth was not measured here because builds were stopped at 120 s (previous report: about 1.3 GiB per thread after a full build, containers and microVMs alike).


## 5. Noisy neighbour (one thread builds, siblings run light loops)

Setup: three threads, each with the fan-out checkout, in one 2 vCPU VM (layout B in section 8). Thread `build` runs `cargo build --workspace --locked --offline -j2` (fresh target directory, the same flags as the previous report); `sib2` and `sib3` run the `light` workload and are probed every 0.5 s (plus the probe time) by the fan-out first command (`git rev-parse` and a Node check) through `podman exec`; the latency is host-observed. The probe window is the 120 s after the build's first five seconds. The build is not run to completion. "Limits" means `--cpus 1 --memory 1024m` on the builder and `--cpus 1 --memory 512m` on each sibling; "no limits" passes no CPU or memory flag (PIDs limit 256 stays). Sibling latency, seconds, both siblings pooled:

| VM | Build | Container limits | Runs | Probes | p50 | p95 | max | Per-run p50 | "Compiling" lines at 120 s per run |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 vCPU | no | no | 1 | 329 | 0.137 | 0.487 | 6.56 | 0.137 | n/a |
| 2 vCPU | yes | no | 3 | 130 | 3.32 | 17.16 | 32.24 | 0.42 / 7.42 / 7.89 | 88 / 6 / 6 |
| 2 vCPU | yes | yes | 3 | 101 | 5.28 | 22.52 | 27.91 | 5.47 / 4.31 / 5.14 | 6 / 6 / 6 |
| 4 vCPU | no | no | 1 | 336 | 0.142 | 0.217 | 10.01 | 0.142 | n/a |
| 4 vCPU | yes | no | 2 | 277 | 0.248 | 6.57 | 16.62 | 0.236 / 0.268 | 154 / 148 |
| 4 vCPU | yes | yes | 2 | 418 | 0.186 | 2.04 | 17.08 | 0.188 / 0.182 | 131 / 130 |

No probe failed. In the 2 vCPU VM the siblings' latency rose from 0.14 s to a median of 3.3 s (about 24 times) with the build running, with or without limits; limits did not improve it (with limits the median was higher). The first no-limit run behaved differently (0.42 s median and 88 compile lines) from the other two (about 7.4 s and 6 lines), so the 2 vCPU result is bimodal; no cause was found. Cgroup counters in a limited run: the builder was throttled 1142 times (101 s total) and its memory peak hit the 1 GiB limit without an OOM kill (`oom_kill 0`); siblings were throttled 43 and 68 times. The memory hog case (a thread allocating beyond the guest's RAM, with and without a memory limit) was not measured.

Diagnostic (3 runs, one build thread and one light sibling, 2 vCPU, no probes, no limits, guest `/proc/stat` over 85 s): the build saturated both vCPUs; guest-reported jiffies user 3759–4124, system 6904–7514, iowait 34–57, steal 62–71, idle 8–12 (system time about twice user time; jiffy totals reach about 64% of 85 s × 2 vCPUs, so guest tick accounting is incomplete under this nesting). Compile-line counts after 85 s were 6, 51 and 43. The guest's PSI files were not available, so no pressure figures exist.


## 6. Freeze and resume of the whole VM with 3 threads

The VM holds three idle threads (fan-out checkout). Pause and resume use the Firecracker API (`PATCH /vm`); the timer for "resume to first output" starts before the resume call and ends when the first command output from one thread arrives. 30 samples with an immediate resume and 10 with five seconds paused. Milliseconds unless noted.

| Paused for | n | Freeze p50 / p95 / max | Resume call p50 / p95 / max | Resume to first output s p50 / p95 / max |
| --- | --- | --- | --- | --- |
| immediate | 30 | 0.40 / 0.46 / 2.18 | 0.26 / 0.34 / 0.41 | 0.152 / 0.221 / 0.363 |
| 5 s | 10 | 0.42 / 0.44 / 0.44 | 0.63 / 5.38 / 5.38 | 0.157 / 0.226 / 0.226 |

**Finding that changed the method:** after a pause and resume, the vsock control channel did not answer again (every connection was closed or timed out, for minutes, while the VMM kept running and the guest was alive). First outputs were therefore read over the serial console (`/init` reads `ID|command` lines). The vsock failure after resume was not diagnosed. Previous report: Podman pause/unpause 97.9 / 96.2 ms; Firecracker 0.18 / 0.19 ms; resume to first output 350.7 ms (container) and 48.2 ms (microVM) p50. Pause leaves memory resident; no checkpoint or snapshot of the VM with threads was taken.


## 7. Limits and unmeasured

- Nested virtualization (Proxmox, KVM on KVM): guest CPU, page-table and virtio-block paths cross two layers; numbers are not bare-metal microVM performance and no correction factor exists. Guest system time dominated the build diagnostic. Single host, one repetition of each setup; build-related rows have 1 to 3 runs.
- Guest boot times (3 to 9 s) are slower than the previous run's 2.7 s mainly from more RAM (kernel boot scales with configured RAM: about 2.0 s to `/init` at 1 GiB, 3.2 s at 4 GiB), three disks, and the guest init; the first containers in a cold VM are slow because the guest page cache is empty (a 2.2 GiB toolset read through nested virtio-block).
- Toolset used as `--rootfs DIR:O`, not as an image from a Podman store; image pull/import time inside the VM is not included. Per-thread checkouts are extracted per thread into the container layer (as the previous report); a shared per-project checkout with Git worktrees was not measured.
- VM sizes tested: 2 vCPU/4 GiB for all stages; 4 vCPU and 1 GiB only for cold boot, container starts and the noisy-neighbour table. No VM-size sweep, no jailer, no host cgroup limits on the VMM, no networking (all containers `--network none`).
- Cold fan-out for 10 subagents has 13 of 30 requests; fan-out with a 4 vCPU VM and with 10 existing idle threads was not run in the single-VM stage (partially covered in section 8); vsock connect retries are uncounted outside one run. The first memory pass (four idle rows) was superseded by a rerun after a harness error; its events remain in `raw/events.jsonl` before the final rows and are excluded here.
- Not measured: full VM snapshots with threads, restore from disk, host-OOM density, memory-hog neighbour, network egress, Cargo build to completion inside the hybrid, Cloud Hypervisor, idle VM suspend to disk, free-page reporting internals.
- Isolation properties are the documented ones from the previous report; this run adds no penetration test.


## 8. A vs B: VM per project versus one VM per person

Same 10 threads in both layouts, from three projects with 4, 3 and 3 threads. Each project has its own checkout in each thread's container (fan-out archive) and its own dummy credential (`BENCH_DUMMY_PROJECT_ONE/TWO/THREE`, mode 0600).

- **A**: three VMs (2 vCPU, 4 GiB each), threads 4 + 3 + 3.
- **B**: one VM (2 vCPU, 4 GiB), all ten containers. For the noisy-neighbour table a 4 vCPU variant is also given.

Host memory, VMM PSS summed over the VMs, MiB; three fresh setups per cell, 5 samples each, runs listed:

| | 0 threads (runtime booted) | 10 idle | 10 light | Data disk allocated (10 idle / 10 light) |
| --- | --- | --- | --- | --- |
| A, 3 VMs | 793.1 (6 setups) | 1349.9 (1346.4, 1344.7, 1358.5) | 1383.9 (1378.8, 1378.2, 1394.6) | 278.9 / 284.6 MiB over 3 disks |
| B, 1 VM | 272.2 (6 setups) | 714.6 (700.5, 708.8, 734.6) | 698.8 (686.6, 725.5, 684.1) | 160.5 / 162.6 MiB over 1 disk |

The difference at 10 idle threads is 635 MiB, about 2 × 270 MiB of fixed VM cost plus the page cache each VM keeps for its own toolset reads.

Disk: both layouts share `os.ext4` and `toolset.ext4` once (section 4: same inode in every VM); per-VM overhead is one data disk, 68.5 MiB blank; allocated after the fan-out runs: A 219.0 + 97.0 + 97.0 MiB, B 292.5 MiB (the first A disk and B hold the project-1 fan-out layers; removal of containers does not shrink the sparse file).

Time for a new thread (spawn with checkout, credential and first command output), 30 samples each, seconds. "Warm" means the project's VM is already running with its other threads (A: the project's 4 threads; B: the 10 existing threads):

| Case | p50 | p95 | max |
| --- | --- | --- | --- |
| A, VM already running | 0.61 | 1.72 | 1.91 |
| B, VM already running | 0.60 | 0.73 | 4.07 |
| A, project needs a cold VM (VM start, runtime ready and first thread) | 15.52 | 27.28 | 27.95 |

In the cold case the VM was ready (agent and `podman info`) at p50 5.78 s, p95 11.12 s, max 12.24 s; the rest is the first thread in a VM with an empty page cache. B has no cold-VM case after the person's first project.

Fan-out within one project (project 1; the other projects' threads idle), request to last first output, 30 requests each, seconds:

| Layout | Subagents | p50 | p95 | max |
| --- | --- | --- | --- | --- |
| A (VM with 4 existing threads, other two VMs idle) | 3 | 1.30 | 9.48 | 12.15 |
| A | 10 | 4.32 | 11.46 | 70.04 |
| B (VM with 10 existing threads) | 3 | 1.32 | 3.59 | 3.92 |
| B | 10 | 4.40 | 11.52 | 53.81 |

Noisy neighbour across projects. Layout A: the build runs in project 1's VM, `sib2` runs in project 2's VM and `sib3` in project 3's VM (three 2 vCPU VMs on four host cores). Layout B: the same three threads in one VM (rows from section 5). Sibling probe latency, seconds, pooled over both siblings:

| Layout | Build | Container limits | Runs | Probes | p50 | p95 | max | Compiling lines per run |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A, 2 vCPU per VM | no | no | 1 | 373 | 0.131 | 0.179 | 2.99 | n/a |
| A | yes | no | 3 | 1006 | 0.168 | 0.446 | 1.17 | 87 / 88 / 88 |
| A | yes | yes (builder `--cpus 1`) | 3 | 1093 | 0.144 | 0.214 | 2.23 | 11 / 9 / 18 |
| B, 2 vCPU | no | no | 1 | 329 | 0.137 | 0.487 | 6.56 | n/a |
| B | yes | no | 3 | 130 | 3.32 | 17.16 | 32.24 | 88 / 6 / 6 |
| B | yes | yes | 3 | 101 | 5.28 | 22.52 | 27.91 | 6 / 6 / 6 |
| B, 4 vCPU | yes | no | 2 | 277 | 0.248 | 6.57 | 16.62 | 154 / 148 |
| B, 4 vCPU | yes | yes | 2 | 418 | 0.186 | 2.04 | 17.08 | 131 / 130 |

In A the build in one VM did not measurably raise the latency of siblings in other VMs (the 2-vCPU build uses two of the four host cores). The A builder runs the full 2 vCPUs in the unlimited case, so its compile progress (87–88 lines) is above B's 2-vCPU runs (6–88). The limited A builder progressed to 9–18 lines in the same window (`--cpus 1`).

**What a container can see of other projects' threads in B** (factual listing from a thread of project 1 in a VM running six threads of three projects, plus the VM-level view for contrast; full output is in the JSON under `a_vs_b.visibility`):

- Mounts in the container: overlay root (lower `/toolset`, upper in the container's own directory under the agent's Podman storage), `/checkout.tar` (read-only bind of the shared archive, the same file in every thread), `/proc`, `/dev` tmpfs, `/sys`, `/dev/shm`, `/sys/fs/cgroup`, `/etc/hosts`, `/etc/hostname`. No other container's layer, `/data`, `/opt` or `/run/user` is mounted (`ls /data` and `ls /run/user` fail with "No such file or directory").
- Processes: the container's own PID namespace lists 4 processes (workload shell, Node timer, sleep, the listing command); `/proc/1/root` is its own root.
- Files: `find / -xdev -name .agent-credential` inside the container returns only `/work/.agent-credential`, containing its own project's dummy value. The container's `/work` holds its own checkout; the file is mode 0600, owned by container root (host uid 1000).
- Network: only `lo` (`--network none`). Capabilities: effective set `0x800405fb`, including `CAP_SYS_CHROOT` and `CAP_SETUID`; uid map `0 → 1000 (1)`, `1 → 100000 (65536)`. Kernel: the same guest kernel `6.1.188+` as every other container and the guest user `agent`.
- At VM level, the `agent` user (the one that runs Podman, and the one the vsock command channel runs as) sees all six containers (`podman ps`), reads every container's `.agent-credential` under `~/.local/share/containers/storage/overlay-containers/*/rootfs/upper/work/` (6 files; values `BENCH_DUMMY_PROJECT_ONE`, `_TWO`, `_THREE`, two each), and has 104 processes in its PID view. A project's container does not have this view; anything the guest agent itself runs outside a container does.
- In A, a container sees none of these other projects' paths for the simpler reason that they live in other VMs; the same listing was not repeated there.

This listing is mount and process visibility only; no escape, privilege-escalation or kernel-attack testing was done.


## Exact commands

```sh
# hd01: read/copy only
git ls-files -co --exclude-standard -z | tar --null -T - -cf - | ssh kayg@calternal-perf-01-backup01 'cd ~/calternal-ai-sandbox-hybrid && mkdir -p source && tar xf - -C source'
# perf-01, every run under the lock
cd ~/calternal-ai-sandbox-hybrid
/usr/local/bin/perf-lock.sh nix shell nixpkgs#socat nixpkgs#shadow nixpkgs#iproute2 --command bash -c 'command -v socat newuidmap newgidmap ip > tools2.paths'
curl -o dl/vmlinux-6.1.188 https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/20261111-778a26c0594c-0/x86_64/vmlinux-6.1.188
/usr/local/bin/perf-lock.sh python3 setup.py                 # toolset.ext4, os.ext4, checkout archives
/usr/local/bin/perf-lock.sh python3 m.py starts_par4 mem disk freeze fanout
HYBRID_EVENTS=events2.jsonl /usr/local/bin/perf-lock.sh python3 m2.py noisy ab visibility diag
```

`starts` (cold, 2/4 vCPU and 1 GiB, single and 10-parallel container starts) ran first and its 4 vCPU parallel part crashed on a vsock connect error; `starts_par4` redid it after adding the connect retry. Scripts: `setup.py` (images), `hlib.py` (VM and Podman wrappers, exact Firecracker API calls), `m.py` (starts, fan-out, memory, balloon, disk, freeze), `m2.py` (noisy neighbour, A vs B, visibility, diagnostic), `rebuild-os.sh`, `patch-init.sh` and `srpc` (serial path); the guest `/init` and `/sbin/rpc` texts are inside `setup.py`; `build_json.py` and `an*.py` produced the summary from the raw events. The staged `os/` tree and all images were deleted after the runs. Raw events: `raw/events.jsonl`, `raw/events2.jsonl`; per-VM logs were removed with the VMs. Failed or superseded attempts kept in the logs: `run1.log` (crash at 4 vCPU parallel start), `run1b.log` (balloon statistics GET carried a body), `run2-visibility.log` (missing `ps`), `fz.log`/`fz2.log` (vsock after resume).
