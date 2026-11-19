# Per-space plugin runtime wall measurements

Measured on 2026-11-19 on `calternal-perf-01-backup01`. The VM exposed four AMD Ryzen cores and 7.70 GiB RAM with no swap. The backing filesystem was ext4 on the VM's `/dev/sda1`. Firecracker ran on nested KVM. The [JSON companion](space-runtime-walls.json) includes counts and numeric results. Scripts, run logs and raw samples are retained under `~/space-runtime-measure/` on perf-01. They were prepared against `badbb9f09fb8`. [Forgejo #77](https://git.kayg.org/kayg/calternal-ai/issues/77)

## Workload and walls

Each runtime used Node 24.21.0 with five protocol-v2 workers. The bundled workers were Notes Insights (two independent processes), Plugin Guide and Plugin Submission Guide. The fifth process was the existing Composer Settings fixture, which declares the setting-read broker method. The host waited until all five workers had completed an initialization call before answering. The simple operation returned the submission guide's 3.3 KiB Markdown document. The settings operation crossed the external broker boundary with `read_setting_v1` and validated the returned marker in the GenUI badge. The broker served a fixed setting from memory. Application routing, grant checks, SQLite, and real plugin data were outside the timer.

The fixtures and image were built on dev-01 in `nix develop` and copied with SHA-256 verification. This baseline used only the prebuilt fixtures on perf-01. The test reused the Firecracker kernel version, runtime versions, rootless overlay and Unix HTTP/vsock setup from the earlier experiments. The retired guest and container images had been cleaned up, so the dev VM repacked the workload into a read-only ext4 image for A and a rootfs archive for the host-kernel cases. The host and workers used anonymous pipes internally, including inside the container where the worker seccomp profile denies socket creation. [Per-thread sandbox measurements](sandbox-containers-vs-microvms.md) [Plugin worker measurements](plugin-workers-in-sandboxes.md)

| Arm | Sleeping state | Timed wake path |
| --- | --- | --- |
| A | 256 MiB full, file-backed snapshot, VMM exited | Launch Firecracker 1.16.1, load state, resume, connect via vsock bridge, answer invocation |
| B | Stopped process tree | Start bubblewrap 0.13.1 and Node with new namespaces, read-only root, no network, no capabilities, and a seccomp filter denying socket and io_uring syscalls |
| B frozen | Live tree in a frozen systemd user scope | Request thaw using `systemctl --user thaw` and answer via the live pipes |
| C | Precreated, stopped rootless Podman container | `podman start -ai` through crun, start Node and workers, answer over the attached pipe |

A used two vCPUs and no virtual network adapter. The benchmark used Firecracker without the jailer. C used Podman 6.1.4, crun 1.30.1, and the current worker limits (256 MiB, one CPU, 64 PIDs). It reused the worker profile with no network, read-only root, no added capabilities, no new privileges, no logging, and the product seccomp allowlist. The lab's Nix Node rootfs took the place of the product's Alpine image. C used one container per space holding the shared host and five workers, a different process layout from the product's per-worker launch path. B cold had no separate CPU or memory quota. The tests characterize the configured walls and transport paths.

## Wake

Each cell uses independent wakes for each operation and cache condition, measured from request arrival until the broker validated the successful reply. Process cleanup, precreation, and cache preparation were outside the timer. Warm caches were primed by reading the backing files. Cold caches used file-specific `POSIX_FADV_DONTNEED`, checked with `mincore` immediately before entry. Container metadata, shared host binaries, filesystem metadata, and any hypervisor disk cache remained outside this cache control. A frozen tree can keep mapped pages resident and continue using its anonymous memory after file eviction. Eviction removed the frozen workers’ script pages but left 42.27% of their mapped Node binary resident. The stopped B and C cases reached 0% residency for the checked files. All wall-clock measurements ran under `/usr/local/bin/perf-lock.sh`. B and C's cached trials were repeated after all guest image staging finished. The final numbers use that repeat. The median uses the usual midpoint and p95 is nearest rank among successful calls.

| Arm | Cache | Op | Attempts | Failed | p50 / p95 / max ms |
| --- | --- | --- | --- | --- | --- |
| A | warm | simple | 100 | 0 | 198.74 / 239.01 / 278.01 |
| A | warm | setting | 100 | 0 | 202.19 / 269.12 / 279.87 |
| A | cold | simple | 100 | 0 | 409.88 / 458.81 / 634.61 |
| A | cold | setting | 100 | 0 | 433.26 / 488.74 / 862.76 |
| B | warm | simple | 100 | 0 | 101.01 / 111.34 / 141.19 |
| B | warm | setting | 100 | 0 | 100.36 / 111.10 / 132.15 |
| B | cold | simple | 100 | 0 | 349.58 / 373.62 / 487.60 |
| B | cold | setting | 100 | 0 | 348.15 / 374.82 / 395.89 |
| B-freeze | warm | simple | 100 | 0 | 5.85 / 6.73 / 7.29 |
| B-freeze | warm | setting | 100 | 0 | 6.02 / 6.83 / 7.49 |
| B-freeze | cold | simple | 100 | 0 | 6.00 / 6.62 / 7.72 |
| B-freeze | cold | setting | 100 | 0 | 6.12 / 6.85 / 33.99 |
| C | warm | simple | 100 | 0 | 371.42 / 413.54 / 457.45 |
| C | warm | setting | 100 | 0 | 371.56 / 412.13 / 499.36 |
| C | cold | simple | 100 | 0 | 649.45 / 699.23 / 730.29 |
| C | cold | setting | 100 | 0 | 639.40 / 690.44 / 717.52 |

## Twenty simultaneous wakes

Three batches per cache condition. Each batch released twenty threads together with one settings read per space. A used twenty separate physical copies of the 256 MiB snapshot from one disposable template. Shared base image bytes were primed or evicted once per batch. B and C each used separate trees or containers. The p95 pools successful workers across the three batches, whose measurements are correlated. Cleanup started after each runtime answered, so cleanup for earlier responders could overlap the slower wakes.

| Arm | Cache | Batches | Attempts | Failed | p50 / p95 / max ms |
| --- | --- | --- | --- | --- | --- |
| A | warm | 3 | 60 | 0 | 837.42 / 922.41 / 942.22 |
| A | cold | 3 | 60 | 0 | 2334.60 / 2518.85 / 2532.32 |
| B | warm | 3 | 60 | 0 | 1199.37 / 1355.50 / 1367.09 |
| B | cold | 3 | 60 | 0 | 1391.82 / 1451.11 / 1461.68 |
| B-freeze | warm | 3 | 60 | 0 | 43.69 / 67.18 / 73.03 |
| B-freeze | cold | 3 | 60 | 0 | 43.65 / 63.14 / 69.46 |
| C | warm | 3 | 60 | 0 | 2339.06 / 5476.01 / 6525.33 |
| C | cold | 3 | 60 | 0 | 2135.35 / 4755.89 / 6375.69 |

## Idle footprint and stored density

The PSS of an awake A space is the VMM's host process, after restore and one successful worker invocation. The mapped portion of the restored guest depends on which pages were touched. The B process-tree PSS includes the bubblewrap helpers and Node processes. C includes the attached client, container descendants and conmon. Sample medians come from three readings. The host column uses `MemTotal - MemAvailable`, an estimate that includes background services and accounts for reclaimable cache. The disk column counts allocated image, sandbox and fixture blocks. It excludes the benchmark archive, tool installation, and other experiment artifacts.

Stopped spaces have no live workers or VMM process. Snapshot files, shared images, metadata, and their reclaimable Linux page cache remain. Frozen trees retain PSS and their suspended worker state. For B cold and B frozen, a stopped entry is a fixture space record. There is no persisted process image. For C a stopped entry also has the Podman container record and overlay metadata. Every population size included a measurement with one awake space and a measurement with all spaces asleep. Copies of A's full snapshot allocate distinct disk blocks while holding identical template data. The disk guard reserves 8 GiB for the shared VM. The memory guard stopped adding frozen trees at 68. Remaining B frozen entries in the 100- and 1,000-space rows were stopped.

| Arm | Created | Frozen when all asleep | Awake PSS MiB | PSS per frozen tree MiB | Host used all asleep MiB | Host used one awake MiB | Disk allocated MiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A | 1 | 0 | 43.30 | 0.00 | 1018.18 | 1008.00 | 527.66 |
| A | 10 | 0 | 44.35 | 0.00 | 1042.89 | 1024.52 | 2831.92 |
| A | 100 | 0 | 43.41 | 0.00 | 934.32 | 940.04 | 25874.46 |
| B | 1 | 0 | 127.41 | 0.00 | 955.40 | 1051.55 | 286.79 |
| B | 10 | 0 | 127.40 | 0.00 | 1009.28 | 1059.41 | 286.82 |
| B | 100 | 0 | 127.40 | 0.00 | 997.82 | 1064.99 | 287.18 |
| B | 1000 | 0 | 127.44 | 0.00 | 1008.31 | 1053.11 | 290.77 |
| B-freeze | 1 | 1 | 127.39 | 127.39 | 1054.65 | 1068.19 | 286.79 |
| B-freeze | 10 | 10 | 83.21 | 83.22 | 1868.00 | 1876.49 | 286.82 |
| B-freeze | 100 | 68 | 78.95 | 78.97 | 6375.98 | 6390.78 | 287.18 |
| B-freeze | 1000 | 68 | 78.95 | 78.97 | 6347.53 | 6360.62 | 290.77 |
| C | 1 | 0 | 177.31 | 0.00 | 1016.57 | 1099.43 | 288.27 |
| C | 10 | 0 | 174.84 | 0.00 | 1068.31 | 1086.75 | 288.75 |
| C | 100 | 0 | 176.06 | 0.00 | 974.01 | 1084.50 | 294.36 |
| C | 1000 | 0 | 182.50 | 0.00 | 988.53 | 1110.83 | 352.29 |

A reached 100 stored snapshots (25.00 GiB allocated) with 19.94 GiB free. Those snapshots held 6.00 GiB in the reclaimable file cache before the targeted population eviction, and zero resident snapshot pages afterward. The fresh-boot VMM control was 180.89 MiB PSS, while the restored VMM touched fewer guest pages. The additional distinct snapshot files for 1,000 spaces would exceed the free disk reserve.

## Firecracker full snapshots

The timed snapshot step includes pausing the VM, creating the full memory and state files, and fsyncing both files and their directory. It starts after any guest balloon request has reached the target according to Firecracker's reported actual balloon size. The root image is shared and its allocation is excluded. The timer excludes VM boot and workload initialization.

| Guest MiB | Balloon MiB | Attempts | Failed | Snapshot p50 / p95 / max ms | Balloon p50 / p95 / max ms | Memory logical MiB | Memory allocated MiB | State KiB | VMM PSS reduction MiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 256 | 0 | 3 | 0 | 530.93 / 554.27 / 554.27 | — | 256.00 | 256.00 | 25.31 | -0.00 |
| 256 | 64 | 3 | 0 | 605.04 / 606.74 / 606.74 | 51.13 / 52.56 / 52.56 | 256.00 | 256.00 | 25.32 | 0.86 |
| 512 | 0 | 3 | 0 | 1163.36 / 1347.92 / 1347.92 | — | 512.00 | 512.00 | 25.31 | -0.00 |
| 512 | 320 | 3 | 0 | 1282.30 / 1305.78 / 1305.78 | 51.06 / 51.09 / 51.09 | 512.00 | 512.00 | 25.33 | 0.76 |
| 1024 | 0 | 3 | 0 | 3713.19 / 3800.73 / 3800.73 | — | 1024.00 | 1024.00 | 25.31 | -0.00 |
| 1024 | 832 | 3 | 0 | 3178.00 / 3181.49 / 3181.49 | 353.79 / 555.73 / 555.73 | 1024.00 | 1024.00 | 25.34 | 42.77 |

Full memory files occupied their configured guest size in every run, including after ballooning.

## Snapshot randomness

The Node host initialized its in-process crypto RNG before snapshot. Each attempt independently restored the same snapshot and read 32 bytes directly from `/dev/urandom` and 32 bytes from the restored process's synchronous `crypto.randomBytes`. One explicit option sent 32 fresh bytes from the broker, mixed them into the guest kernel's pool and called `RNDRESEEDCRNG` inside the guest. A second option also passed a separate 32 bytes to OpenSSL's `RAND_seed()` in the long-lived Node host. The guest time includes the seed helper and both draws. The Node stage column starts after the kernel helper returns and includes both draws. It measured the effect on this kernel and Node process. Other plugin processes, cached random values and previously issued tokens were outside the probe. [Firecracker snapshot and VMGenID documentation](https://github.com/firecracker-microvm/firecracker/blob/v1.16.1/docs/snapshotting/snapshot-support.md) [OpenSSL random generation](https://docs.openssl.org/3.5/man7/EVP_RAND/)

| Mode | Attempts | Failed | Distinct kernel draws | Distinct Node draws | Broker p50 / p95 / max ms | Guest reseed + draws p50 / p95 / max ms | Node stage + draws p50 / p95 / max ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| default | 20 | 0 | 20 | 1 | 89.12 / 114.64 / 119.06 | — | — |
| kernel | 20 | 0 | 20 | 1 | 194.86 / 281.60 / 306.81 | 156.22 / 239.37 / 268.38 | 4.27 / 4.95 / 5.62 |
| kernel_and_node | 20 | 0 | 20 | 20 | 218.80 / 315.57 / 341.49 | 178.09 / 267.18 / 288.94 | 6.07 / 6.25 / 7.04 |

The first two restores produced different kernel bytes and identical Node bytes. The snapshot's ACPI `VMGENCTR:00` device had no bound driver in this guest, so the default kernel result does not isolate VMGenID behavior. The Node stage measures host-isolate reseeding plus sampling, with the kernel-only draw path shown separately.

## Steady broker round trip

One awake space per wall, one invocation at a time. After 50 warmup calls, the driver timed 1,000 calls of each kind on the same connection. B and C were remeasured because the host, number of workers, and operation shape changed from #14. The B frozen variant uses B's same path once thawed.

| Arm | Op | Calls | p50 / p95 / max µs |
| --- | --- | --- | --- |
| A | simple | 1000 | 437.56 / 1858.61 / 171093.27 |
| A | setting | 1000 | 693.53 / 1148.05 / 121313.79 |
| B | simple | 1000 | 172.38 / 273.41 / 5469.31 |
| B | setting | 1000 | 280.32 / 463.89 / 9081.97 |
| C | simple | 1000 | 287.71 / 399.35 / 36414.69 |
| C | setting | 1000 | 411.28 / 573.98 / 7980.80 |

## Agent containers inside the space microVM

The guest ran the same five plugin workers plus one or three rootless Podman agents. It had two vCPUs and separate read-only OS and toolset drives plus a private writable ext4 drive. Agents used native rootless overlay, crun, delegated cgroup v2, and no network. The idle agent was a shell keeping a Node process alive. The light agent added a repository search every three seconds. The build was an offline `cargo build --workspace --locked -j2` in each agent with a fresh target. It used the earlier report's source revision `c9aebce00982` and a locked dependency cache prepared on dev-01. The earlier report's working tree also contained local changes. The inherited interference series ran Cargo as the measured workload inside the disposable guest. All benchmark artifacts were prepared on dev-01.

With an already-awake guest, the timer measured one plugin broker round trip on the existing connection. Waking from an idle snapshot opened a fresh VMM and broker connection and timed a settings operation. It then checked that Podman listed the exact saved cohort as running, outside the timer. A failed cohort check failed the trial. Each restore began from the same saved writable disk contents. The selected series measured five cached and five evicted-file attempts for each guest size, cohort size and FPR setting. A full 8 GiB memory file cannot fit in this VM's file cache, so its warm trials use the restore working set and report observed residency. The first 8 GiB run hit a host mapping error in the cache sampler after the idle snapshot. The retry used 64 MiB mappings for `mincore`. The snapshot timer excludes the guest sync and balloon wait. The original agent runs requested all but 640 MiB. Later runs requested half the configured RAM after the first 4 GiB three-agent balloon reached 2,037 MiB of a 3,456 MiB request in the observation window. The table shows the requested amount and the driver’s actual amount when sampled. FPR is the Firecracker free-page-reporting setting selected at guest boot. The guest used column estimates non-reclaimable usage as `MemTotal - MemAvailable` minus the inflated balloon. The snapshot rows also record host VMM PSS and physical allocation of the memory file, which include different kinds of pages. The JSON includes guest page cache, dirty pages, cgroup charges, VMM PSS, and balloon driver results. Tables use the original series for 2 GiB and 4 GiB with one agent, and the final series for 4 GiB with three agents and 8 GiB. The raw series and retries are retained on perf-01.

All agent measurements below ran on `calternal-perf-01-backup01`. The idle and light rows each represent one guest boot and cohort, followed by the indicated number of broker calls. Charges include cgroup-accounted file pages.

| Guest GiB | Agents | FPR | Load | Calls | Failed | Settings p50 / p95 / max ms | Guest used MiB | Plugin / agent charge MiB | VMM PSS MiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 | 1 | off | idle | 100 | 0 | 2.07 / 11.64 / 33.02 | 240.5 | 137.1 / 112.2 | 353.3 |
| 2 | 1 | off | light | 100 | 0 | 1.34 / 11.05 / 18.94 | 243.5 | 137.4 / 85.5 | 331.3 |
| 2 | 1 | on | idle | 100 | 0 | 1.63 / 8.76 / 23.16 | 242.9 | 137.0 / 89.0 | 335.0 |
| 2 | 1 | on | light | 100 | 0 | 1.11 / 30.34 / 151.77 | 250.9 | 136.8 / 127.3 | 365.1 |
| 2 | 3 | off | idle | 100 | 0 | 0.88 / 2.08 / 6.29 | 276.2 | 137.4 / 160.4 | 407.5 |
| 2 | 3 | off | light | 100 | 0 | 0.94 / 2.32 / 237.65 | 279.9 | 136.9 / 161.8 | 412.9 |
| 2 | 3 | on | idle | 100 | 0 | 1.21 / 2.73 / 24.74 | 290.0 | 137.2 / 162.4 | 423.4 |
| 2 | 3 | on | light | 100 | 0 | 0.96 / 2.03 / 3.15 | 281.4 | 136.4 / 169.8 | 413.2 |
| 4 | 1 | off | idle | 100 | 0 | 2.55 / 52.46 / 318.43 | 309.1 | 137.8 / 90.4 | 438.1 |
| 4 | 1 | off | light | 100 | 0 | 1.48 / 5.25 / 67.76 | 306.2 | 136.9 / 86.4 | 432.0 |
| 4 | 1 | on | idle | 100 | 0 | 1.96 / 17.48 / 66.00 | 312.1 | 137.5 / 95.8 | 442.5 |
| 4 | 1 | on | light | 100 | 0 | 1.55 / 11.60 / 123.77 | 309.8 | 137.6 / 92.1 | 437.9 |
| 4 | 3 | off | idle | 100 | 0 | 0.86 / 2.05 / 10.27 | 344.6 | 137.2 / 161.2 | 515.9 |
| 4 | 3 | off | light | 100 | 0 | 0.94 / 17.11 / 83.45 | 346.4 | 138.0 / 161.3 | 517.6 |
| 4 | 3 | on | idle | 100 | 0 | 0.92 / 12.00 / 86.91 | 344.7 | 137.2 / 160.5 | 515.4 |
| 4 | 3 | on | light | 100 | 0 | 0.85 / 1.84 / 2.53 | 344.8 | 136.9 / 162.1 | 517.1 |
| 8 | 1 | off | idle | 100 | 0 | 1.45 / 12.07 / 24.20 | 338.3 | 137.2 / 83.3 | 520.2 |
| 8 | 1 | off | light | 100 | 0 | 1.31 / 10.55 / 49.22 | 350.7 | 136.9 / 131.0 | 556.5 |
| 8 | 1 | on | idle | 100 | 0 | 1.29 / 3.48 / 22.46 | 341.2 | 136.5 / 88.1 | 524.9 |
| 8 | 1 | on | light | 100 | 0 | 1.16 / 37.55 / 147.40 | 347.4 | 138.1 / 118.5 | 545.6 |
| 8 | 3 | off | idle | 100 | 0 | 1.01 / 17.31 / 143.35 | 383.2 | 137.8 / 167.6 | 598.0 |
| 8 | 3 | off | light | 100 | 0 | 0.90 / 4.04 / 68.62 | 385.0 | 137.5 / 168.5 | 599.8 |
| 8 | 3 | on | idle | 100 | 0 | 0.90 / 2.53 / 6.57 | 384.5 | 137.3 / 167.6 | 599.6 |
| 8 | 3 | on | light | 100 | 0 | 1.21 / 3.08 / 65.26 | 387.3 | 137.1 / 177.9 | 602.9 |

| Guest GiB | Agents | FPR | Cache | Attempts | Failed | Wake p50 / p95 / max ms | Memory file resident |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2 | 1 | off | warm | 5 | 0 | 1074.68 / 1954.19 / 1954.19 | 100.0–100.0% |
| 2 | 1 | off | cold | 5 | 0 | 946.49 / 1438.28 / 1438.28 | 0.0–0.0% |
| 2 | 1 | on | warm | 5 | 0 | 1093.60 / 1154.38 / 1154.38 | 100.0–100.0% |
| 2 | 1 | on | cold | 5 | 0 | 685.47 / 748.73 / 748.73 | 0.0–0.0% |
| 2 | 3 | off | warm | 5 | 0 | 307.75 / 358.11 / 358.11 | 100.0–100.0% |
| 2 | 3 | off | cold | 5 | 0 | 508.83 / 746.77 / 746.77 | 0.0–0.0% |
| 2 | 3 | on | warm | 5 | 0 | 364.88 / 422.39 / 422.39 | 100.0–100.0% |
| 2 | 3 | on | cold | 5 | 0 | 595.79 / 618.80 / 618.80 | 0.0–0.0% |
| 4 | 1 | off | warm | 5 | 0 | 818.21 / 966.80 / 966.80 | 100.0–100.0% |
| 4 | 1 | off | cold | 5 | 0 | 819.22 / 878.33 / 878.33 | 0.0–0.0% |
| 4 | 1 | on | warm | 5 | 0 | 719.02 / 1014.95 / 1014.95 | 100.0–100.0% |
| 4 | 1 | on | cold | 5 | 0 | 742.66 / 756.58 / 756.58 | 0.0–0.0% |
| 4 | 3 | off | warm | 5 | 0 | 290.16 / 396.52 / 396.52 | 100.0–100.0% |
| 4 | 3 | off | cold | 5 | 0 | 505.35 / 556.15 / 556.15 | 0.0–0.0% |
| 4 | 3 | on | warm | 5 | 0 | 421.90 / 448.23 / 448.23 | 100.0–100.0% |
| 4 | 3 | on | cold | 5 | 0 | 518.10 / 545.74 / 545.74 | 0.0–0.0% |
| 8 | 1 | off | warm | 5 | 0 | 938.47 / 1220.08 / 1220.08 | 71.8–72.0% |
| 8 | 1 | off | cold | 5 | 0 | 700.29 / 718.23 / 718.23 | 0.0–0.0% |
| 8 | 1 | on | warm | 5 | 0 | 810.52 / 1099.63 / 1099.63 | 70.8–70.9% |
| 8 | 1 | on | cold | 5 | 0 | 725.24 / 843.61 / 843.61 | 0.0–0.0% |
| 8 | 3 | off | warm | 5 | 0 | 286.04 / 388.64 / 388.64 | 69.5–69.6% |
| 8 | 3 | off | cold | 5 | 0 | 636.16 / 761.23 / 761.23 | 0.0–0.0% |
| 8 | 3 | on | warm | 5 | 0 | 357.24 / 414.42 / 414.42 | 70.2–70.3% |
| 8 | 3 | on | cold | 5 | 0 | 603.04 / 655.38 / 655.38 | 0.0–0.0% |

Each snapshot row represents one save, so the percentiles and maximum coincide for successful saves. The data drive is sparse and logically 6 GiB. OS and toolset allocation total 4.56 GiB once per host. Each state file allocated 28 KiB.

| Guest GiB | Agents | FPR | Phase | Attempts | Failed | Save p50 / p95 / max ms | Balloon requested / actual MiB | Guest used MiB | VMM PSS MiB | Memory file MiB | Data drive MiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 | 1 | off | idle | 1 | 0 | 5687.07 / 5687.07 / 5687.07 | 0 / 0 | — | — | 2048.00 | 10.87 |
| 2 | 1 | off | postbuild | 1 | 0 | 5381.60 / 5381.60 / 5381.60 | 0 / 0 | 313.7 | 2003.7 | 2048.00 | 1407.63 |
| 2 | 1 | off | postbuild ballooned | 1 | 0 | 4704.40 / 4704.40 / 4704.40 | 1408 / 1408 | 271.8 | 635.3 | 2048.00 | 1407.66 |
| 2 | 1 | on | idle | 1 | 0 | 6226.15 / 6226.15 / 6226.15 | 0 / 0 | — | — | 2048.00 | 10.34 |
| 2 | 1 | on | postbuild | 1 | 0 | 4946.96 / 4946.96 / 4946.96 | 0 / 0 | 306.0 | 2004.1 | 2048.00 | 1408.07 |
| 2 | 1 | on | postbuild ballooned | 1 | 0 | 4248.19 / 4248.19 / 4248.19 | 1408 / 1408 | 266.8 | 634.7 | 2048.00 | 1408.07 |
| 2 | 3 | off | idle | 1 | 0 | 4623.07 / 4623.07 / 4623.07 | 0 / 0 | — | — | 2048.00 | 20.02 |
| 2 | 3 | off | postbuild | 1 | 0 | 5487.35 / 5487.35 / 5487.35 | 0 / 0 | 340.5 | 2015.6 | 2048.00 | 4191.84 |
| 2 | 3 | off | postbuild ballooned | 1 | 0 | 4585.06 / 4585.06 / 4585.06 | 1408 / 1408 | 301.7 | 620.8 | 2048.00 | 4191.84 |
| 2 | 3 | on | idle | 1 | 0 | 6435.98 / 6435.98 / 6435.98 | 0 / 0 | — | — | 2048.00 | 20.08 |
| 2 | 3 | on | postbuild | 1 | 0 | 6069.61 / 6069.61 / 6069.61 | 0 / 0 | 319.1 | 2010.1 | 2048.00 | 4211.76 |
| 2 | 3 | on | postbuild ballooned | 1 | 0 | 4416.58 / 4416.58 / 4416.58 | 1408 / 1408 | 298.9 | 617.5 | 2048.00 | 4211.76 |
| 4 | 1 | off | idle | 1 | 0 | 11523.10 / 11523.10 / 11523.10 | 0 / 0 | — | — | 4096.00 | 10.39 |
| 4 | 1 | off | postbuild | 1 | 0 | 8453.67 / 8453.67 / 8453.67 | 0 / 0 | 370.8 | 2693.3 | 4096.00 | 1379.51 |
| 4 | 1 | off | postbuild ballooned | 1 | 0 | 16799.52 / 16799.52 / 16799.52 | 3456 / 3456 | 336.6 | 615.5 | 4096.00 | 1379.51 |
| 4 | 1 | on | idle | 1 | 0 | 12551.94 / 12551.94 / 12551.94 | 0 / 0 | — | — | 4096.00 | 10.29 |
| 4 | 1 | on | postbuild | 1 | 0 | 9452.45 / 9452.45 / 9452.45 | 0 / 0 | 371.9 | 2610.6 | 4096.00 | 1404.41 |
| 4 | 1 | on | postbuild ballooned | 1 | 0 | 8100.60 / 8100.60 / 8100.60 | 3456 / 3456 | 336.7 | 609.9 | 4096.00 | 1404.48 |
| 4 | 3 | off | idle | 1 | 0 | 22500.59 / 22500.59 / 22500.59 | 0 / 0 | — | — | 4096.00 | 20.08 |
| 4 | 3 | off | postbuild | 1 | 0 | 9110.06 / 9110.06 / 9110.06 | 0 / 0 | 410.2 | 4040.9 | 4096.00 | 4140.38 |
| 4 | 3 | off | postbuild ballooned | 1 | 0 | 8109.26 / 8109.26 / 8109.26 | 2048 / 1700 | 450.5 | 2308.2 | 4096.00 | 4140.38 |
| 4 | 3 | on | idle | 1 | 0 | 8503.26 / 8503.26 / 8503.26 | 0 / 0 | — | — | 4096.00 | 19.99 |
| 4 | 3 | on | postbuild | 1 | 0 | 15358.63 / 15358.63 / 15358.63 | 0 / 0 | 414.1 | 3950.4 | 4096.00 | 4120.95 |
| 4 | 3 | on | postbuild ballooned | 1 | 0 | 8784.16 / 8784.16 / 8784.16 | 2048 / 1459 | 474.8 | 2532.0 | 4096.00 | 4120.95 |
| 8 | 1 | off | idle | 1 | 0 | 18030.78 / 18030.78 / 18030.78 | 0 / 0 | — | — | 8192.00 | 10.31 |
| 8 | 1 | off | postbuild | 1 | 0 | 24792.68 / 24792.68 / 24792.68 | 0 / 0 | 411.6 | 2752.9 | 8192.00 | 1362.13 |
| 8 | 1 | off | postbuild ballooned | 1 | 0 | 17588.35 / 17588.35 / 17588.35 | 4096 / 4096 | 394.9 | 2369.3 | 8192.00 | 1362.16 |
| 8 | 1 | on | idle | 1 | 0 | 24229.76 / 24229.76 / 24229.76 | 0 / 0 | — | — | 8192.00 | 10.29 |
| 8 | 1 | on | postbuild | 1 | 0 | 17429.30 / 17429.30 / 17429.30 | 0 / 0 | 418.4 | 2710.5 | 8192.00 | 1367.42 |
| 8 | 1 | on | postbuild ballooned | 1 | 0 | 17003.21 / 17003.21 / 17003.21 | 4096 / 4096 | 380.1 | 2379.2 | 8192.00 | 1367.42 |
| 8 | 3 | off | idle | 1 | 0 | 59042.79 / 59042.79 / 59042.79 | 0 / 0 | — | — | 8192.00 | 20.08 |
| 8 | 3 | off | postbuild | 0 | 0 | — | — | — | — | — | — |
| 8 | 3 | off | postbuild ballooned | 0 | 0 | — | — | — | — | — | — |
| 8 | 3 | on | idle | 1 | 0 | 22064.13 / 22064.13 / 22064.13 | 0 / 0 | — | — | 8192.00 | 20.13 |
| 8 | 3 | on | postbuild | 0 | 0 | — | — | — | — | — | — |
| 8 | 3 | on | postbuild ballooned | 0 | 0 | — | — | — | — | — | — |

The build interference comparison alternated five-second windows in each build. The reservation pinned plugin processes to guest vCPU 0 and the agent cgroup to vCPU 1, with `memory.low=192 MiB` for the plugin cgroup. The other windows let both groups use both vCPUs and cleared the memory protection. Results include settings calls from windows in which every agent was still building at both endpoints. Each Cargo process changes compilation phase over the run. The comparison uses the first 50 windows. The final harness also stopped sampling at that cap. Alternation limits timing drift, and each reservation pair comes from one build run with that guest configuration. The first 4 GiB three-agent run exceeded the original completion wait. The extended runs preserved the sampling schedule and let builds finish with the reservation cleared after the sampling cap. The earlier trial is retained separately in the raw evidence. The table reports agent cgroup CPU seconds per sampling window. Peak sampled agent cgroup memory, observed OOM kills and build exit status are in the JSON. The raw per-window samples are on perf-01. The shared guest kernel lacked pressure-stall reporting in `/proc/pressure/memory`.

| Guest GiB | Agents | FPR | Reserved | Windows | Calls | Failed | Settings p50 / p95 / max ms | CPU sec/window p50 / p95 / max | Peak agent MiB | OOM kills |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2 | 1 | off | off | 16 | 800 | 0 | 1.49 / 27.77 / 198.49 | 9.79 / 10.28 / 10.28 | 1731 | 0 |
| 2 | 1 | off | on | 15 | 750 | 0 | 1.13 / 40.34 / 1148.55 | 5.00 / 6.24 / 6.24 | 1687 | 0 |
| 2 | 1 | on | off | 15 | 750 | 0 | 1.99 / 30.23 / 171.23 | 9.70 / 10.22 / 10.22 | 1733 | 0 |
| 2 | 1 | on | on | 15 | 750 | 0 | 1.13 / 18.25 / 117.48 | 5.00 / 5.08 / 5.08 | 1720 | 0 |
| 2 | 3 | off | off | 25 | 1250 | 0 | 1.17 / 17.94 / 335.62 | 9.92 / 10.06 / 10.13 | 1783 | 0 |
| 2 | 3 | off | on | 25 | 1250 | 0 | 1.17 / 8.91 / 131.49 | 4.99 / 5.00 / 5.06 | 1788 | 0 |
| 2 | 3 | on | off | 25 | 1250 | 0 | 1.11 / 32.53 / 140.94 | 9.91 / 10.12 / 10.32 | 1779 | 0 |
| 2 | 3 | on | on | 25 | 1250 | 0 | 1.11 / 2.01 / 48.71 | 4.99 / 5.01 / 5.05 | 1779 | 0 |
| 4 | 1 | off | off | 17 | 850 | 0 | 1.57 / 31.23 / 150.06 | 9.70 / 10.23 / 10.23 | 2114 | 0 |
| 4 | 1 | off | on | 16 | 800 | 0 | 1.19 / 45.33 / 557.82 | 4.99 / 5.30 / 5.30 | 2025 | 0 |
| 4 | 1 | on | off | 16 | 800 | 0 | 2.27 / 41.45 / 150.50 | 9.78 / 10.20 / 10.20 | 2053 | 0 |
| 4 | 1 | on | on | 15 | 750 | 0 | 1.12 / 25.12 / 482.56 | 5.00 / 5.57 / 5.57 | 2010 | 0 |
| 4 | 3 | off | off | 25 | 1250 | 0 | 1.24 / 40.35 / 1126.77 | 9.94 / 10.34 / 10.34 | 3563 | 0 |
| 4 | 3 | off | on | 25 | 1250 | 0 | 1.11 / 25.87 / 228.48 | 4.99 / 5.18 / 5.30 | 3544 | 0 |
| 4 | 3 | on | off | 25 | 1250 | 0 | 1.19 / 28.54 / 192.00 | 9.91 / 10.01 / 10.17 | 3609 | 0 |
| 4 | 3 | on | on | 25 | 1250 | 0 | 1.09 / 13.40 / 500.41 | 5.00 / 5.08 / 5.28 | 3603 | 0 |
| 8 | 1 | off | off | 15 | 750 | 0 | 1.74 / 38.92 / 235.98 | 9.69 / 10.18 / 10.18 | 2061 | 0 |
| 8 | 1 | off | on | 14 | 700 | 0 | 1.12 / 17.53 / 108.65 | 5.00 / 5.08 / 5.08 | 1968 | 0 |
| 8 | 1 | on | off | 15 | 750 | 0 | 1.35 / 33.79 / 295.47 | 9.68 / 10.34 / 10.34 | 1986 | 0 |
| 8 | 1 | on | on | 15 | 750 | 0 | 1.17 / 34.95 / 110.15 | 5.00 / 5.22 / 5.22 | 2141 | 0 |
| 8 | 3 | off | off | 25 | 1250 | 0 | 1.35 / 35.22 / 401.63 | 9.99 / 10.26 / 10.27 | 3882 | 0 |
| 8 | 3 | off | on | 25 | 1250 | 0 | 1.09 / 28.91 / 167.74 | 4.99 / 5.09 / 5.21 | 3739 | 0 |
| 8 | 3 | on | off | 0 | 0 | 0 | — | — | — | — |
| 8 | 3 | on | on | 0 | 0 | 0 | — | — | — | — |

Build completion covers the full cohort, including work after sampling. The 8 GiB run with FPR off reached the 1 GiB host reserve and lost its guest connection. With FPR on, the fresh build-phase guest timed out during boot before any build began. Neither case reached a post-build save.

| Guest GiB | Agents | FPR | Builds started | Failed / incomplete | Cohort build wall seconds |
| --- | --- | --- | --- | --- | --- |
| 2 | 1 | off | 1 | 0 | 222.6 |
| 2 | 1 | on | 1 | 0 | 240.8 |
| 2 | 3 | off | 3 | 0 | 478.8 |
| 2 | 3 | on | 3 | 0 | 462.7 |
| 4 | 1 | off | 1 | 0 | 263.2 |
| 4 | 1 | on | 1 | 0 | 274.0 |
| 4 | 3 | off | 3 | 0 | 610.3 |
| 4 | 3 | on | 3 | 0 | 594.1 |
| 8 | 1 | off | 1 | 0 | 249.2 |
| 8 | 1 | on | 1 | 0 | 260.7 |
| 8 | 3 | off | 3 | 3 without a result | — |
| 8 | 3 | on | 0 | — | — |

## Self-host on the existing kernel

The self-host tests used rootless Podman without KVM. The nested layout attempted to launch an inner container inside the outer space container. The sibling layout launched the plugin and agent containers separately with either crun or gVisor.

In Podman 6.1.4, the working crun sibling case used a common user namespace with separate PID, mount and network namespaces and one per-space cgroup. Rootless nesting with the worker profile stopped at an operation-permission error. The seven other nested configurations stopped at the inner UID-map write, including trials with FUSE overlay and relaxed outer policies. Private sibling UID mapping failed in the tested crun and runc configurations on startup, timeouts, or mounts. That leaves UID separation between spaces unproven in this setup.

gVisor `runsc release-20261116.0` used Systrap and no KVM device. Each sibling had its own Sentry. The successful run configured no networking and put the cohort under one user systemd scope capped at 2 GiB and two CPUs. It used `runsc --ignore-cgroups` inside that scope. Per-container limits from the Podman profile were therefore outside the comparison. The Sentry applied its own default sandbox policy. No build or checkpoint/restore was attempted for gVisor. [Rootless gVisor modes](https://gvisor.dev/docs/user_guide/rootless/) [Systrap](https://gvisor.dev/docs/user_guide/platforms/)

| Rootless launch probe on perf-01 | Attempts | Failed | Result |
| --- | --- | --- | --- |
| Podman/crun nested | 8 | 8 | Worker policy denied the launch; other profiles failed at inner UID mapping |
| Podman/crun or runc siblings with private UID mapping | 8 | 8 | Timeouts, missing `/run/.containerenv` or denied `/etc/hostname` mount |
| Podman/crun siblings with default UID mapping | 1 | 0 | Settings operation and agent readiness passed |
| Podman/runsc under the inherited systemd settings | 4 | 4 | Interactive authentication required |
| Podman/runsc with Podman cgroups disabled | 4 | 4 | Podman rejected the runtime with NoCgroups |
| Podman/runsc with cgroupfs under a delegated scope | 4 | 4 | Host cgroup subtree control permission denied |

Wake below is a fresh launch with existing images and uncontrolled cache residency. Plugin wake ends at the validated settings reply. Agent startup is timed separately through readiness. The combined column sums the two timed stages per trial and excludes the standalone memory sample between them. PSS includes the plugin and agent runtime processes accounted for by the harness. Each row ran on `calternal-perf-01-backup01`.

| Runtime | Agents | Load | Attempts | Failed | Plugin wake p50 / p95 / max ms | Agent start p50 / p95 / max ms | Combined stages p50 / p95 / max ms | Cohort PSS p50 / p95 / max MiB | Calls | Failed calls | Settings p50 / p95 / max ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Podman/crun siblings | 1 | idle | 3 | 0 | 465.40 / 503.30 / 503.30 | 579.65 / 638.77 / 638.77 | 1082.95 / 1104.16 / 1104.16 | 214.65 / 217.66 / 217.66 | 600 | 0 | 0.69 / 1.13 / 2.32 |
| Podman/crun siblings | 1 | light | 3 | 0 | 424.79 / 549.19 / 549.19 | 553.58 / 572.58 / 572.58 | 973.58 / 1102.77 / 1102.77 | 214.64 / 215.96 / 215.96 | 600 | 0 | 0.68 / 1.11 / 5.81 |
| Podman/crun siblings | 3 | idle | 3 | 0 | 475.63 / 480.87 / 480.87 | 1712.78 / 1785.66 / 1785.66 | 2181.76 / 2266.53 / 2266.53 | 242.05 / 243.52 / 243.52 | 600 | 0 | 0.68 / 1.07 / 3.56 |
| Podman/crun siblings | 3 | light | 3 | 0 | 446.11 / 469.91 / 469.91 | 1694.35 / 1959.46 / 1959.46 | 2140.46 / 2429.37 / 2429.37 | 251.89 / 252.35 / 252.35 | 600 | 0 | 0.69 / 1.13 / 8.29 |
| Podman/runsc siblings | 1 | idle | 3 | 0 | 548.18 / 1006.49 / 1006.49 | 497.55 / 544.21 / 544.21 | 1036.15 / 1550.70 / 1550.70 | 734.91 / 748.81 / 748.81 | 600 | 0 | 1.09 / 1.78 / 3.32 |
| Podman/runsc siblings | 1 | light | 3 | 0 | 510.13 / 518.44 / 518.44 | 534.26 / 558.01 / 558.01 | 1042.27 / 1068.15 / 1068.15 | 715.43 / 722.66 / 722.66 | 600 | 0 | 1.11 / 1.74 / 2.67 |
| Podman/runsc siblings | 3 | idle | 3 | 0 | 485.90 / 515.75 / 515.75 | 1551.68 / 1554.71 / 1554.71 | 2040.62 / 2067.43 / 2067.43 | 848.36 / 878.71 / 878.71 | 600 | 0 | 1.12 / 1.73 / 6.25 |
| Podman/runsc siblings | 3 | light | 3 | 0 | 518.17 / 541.21 / 541.21 | 1659.12 / 1705.52 / 1705.52 | 2200.33 / 2223.68 / 2223.68 | 830.20 / 881.51 / 881.51 | 600 | 0 | 1.16 / 1.93 / 6.16 |

## What this means for the design

These conclusions describe the initial wall experiments. The later XFS tests use the owner’s separate plugin and agent VMs.

1. **Wake budget.** With a 300 ms allowance, a single cached 256 MiB space met the budget in all 100 settings wakes (p95 269 ms, max 280 ms). With the backing files evicted, p95 was 489 ms and max 863 ms. Twenty simultaneous settings wakes raised p95 to 922 ms cached and 2,519 ms evicted. These timings end at the fixture's validated reply and precede any explicit reseed.
2. **Space capacity.** An exited VMM consumes 0 MiB of process PSS. Its snapshot can occupy reclaimable page cache. Disk below counts the state and memory image plus any private idle data drive when sleeping. Awake disk assumes the old snapshot is discarded. Keeping the old snapshot adds its full allocation to awake disk. Larger guests with agents used 10–21 MiB of private data blocks when idle, growing to roughly 1.33–1.38 GiB after one build or 4.02–4.11 GiB after three where the build completed.

   | Guest | Awake VMM PSS MiB | Awake private disk MiB | Sleeping private disk MiB | Sleeping disk ceiling | Fully backed awake ceiling |
   | --- | --- | --- | --- | --- | --- |
   | 256 MiB, plugins | 43.30 restored, 180.89 fresh | Fixture metadata | 256.03 | 149 (100 measured) | 22 |
   | 512 MiB, plugins | 187.02 fresh | Fixture metadata | 512.03 | 74 | 11 |
   | 1 GiB, plugins | 199.02 fresh | Fixture metadata | 1,024.03 | 37 | 5 |
   | 2 GiB, agents | 335–423 idle | 10.34–20.08 | 2,058–2,068 | 16 | 2 |
   | 4 GiB, agents | 438–516 idle | 10.29–20.08 | 4,106–4,116 | 8 | 1 |
   | 8 GiB, agents | 520–600 idle | 10.29–20.13 | 8,202–8,212 | 4 | 0 |

   These arithmetic ceilings use the initial 45.55 GiB available disk and 6.51 GiB MemAvailable on this 7.70 GiB host. Disk subtracts the 8 GiB guard and shared fixture allocation (about 272 MiB for plugins or 4.56 GiB for agents). RAM subtracts the 1 GiB guard and divides by configured guest RAM, before VMM overhead. The 512 MiB and 1 GiB fresh PSS values are medians from the pre-snapshot samples. The 8 GiB guest booted and restored because its resident pages occupied a fraction of configured RAM. Fully backed admission would exclude it on this host. The benchmark exercised one agent space at a time. Simultaneous agent spaces, mixed populations and production density remain unmeasured.
3. **Agents in the space microVM.** Use 2 GiB as the smallest tested candidate for this workload. All 120 idle restore trials passed the cohort check. At 2 GiB the one- and three-agent builds completed, idle full saves took 4.62–6.44 s, and restore p95 ranged from 358–1,954 ms cached and 619–1,438 ms evicted. At 4 GiB all builds completed, with 8.50–22.50 s idle saves. At 8 GiB one-agent builds completed and idle saves took 18.03–59.04 s. Three-agent build completion at 8 GiB is unverified. Three-agent settings p95 improved with the reservation in every measured pair (including 17.94 → 8.91 ms and 32.53 → 2.01 ms at 2 GiB). The agents received about half the CPU time per window, and the one-agent p95 sometimes worsened. The experiment applied CPU pinning and memory protection together, so it gives no separate effect for either control.
4. **Self-host isolation.** Label native rootless Podman siblings **weaker isolation** than Firecracker. Siblings separate PID, mount and network namespaces but share the host kernel and, in this fixture, the same user namespace. Nested native Podman would also share the host kernel, and the tested configurations failed to start. gVisor adds a userspace kernel that handles workload syscalls before they reach the Linux host. Its measured deployment used a shared cohort resource cap. With idle cohorts, the median for both timed stages combined was 1.04 s for one agent and 2.04 s for three, and PSS was 735 and 848 MiB. Native siblings took 1.08 and 2.18 s, with 215 and 242 MiB PSS. [gVisor architecture](https://gvisor.dev/docs/)
5. **Randomness on restore.** Synchronous Node crypto emitted the same 32 bytes in all 20 default restores, despite 20 distinct kernel draws. Kernel-only reseeding still returned one distinct Node result. Kernel plus Node/OpenSSL reseeding gave 20 distinct draws from each, with broker p50/p95/max of 219/316/341 ms versus 89/115/119 ms by default. The difference between broker medians was 130 ms. The Node stage including both draws measured 6.07 ms p50 with both reseeds versus 4.27 ms with kernel-only seeding. The result supports a wake gate that refreshes each resumed long-lived RNG before it serves requests. The probe covered one Node process.

## Follow-up on XFS snapshots

The owner split the runtime after the first wall tests. Each space has a small plugin VM, a larger plugin VM on demand, and a separate on-demand agent VM. The combined plugin and agent guest above is historical. The follow-up used the five-worker fixture in a 256 MiB guest and three idle rootless Podman containers in a separate 2 GiB guest. The agent guest used a private ext4 scratch disk for container state. No VM mounted userdata. [Runtime decisions](../open-questions.md)

Both snapshot guests had two vCPUs. All timed runs used the perf lock on `calternal-perf-01-backup01`. Snapshots were written on the perf VM’s existing 100 GiB XFS filesystem at `/srv/homes` on `/dev/sdb1`, in the disposable `/srv/homes/calternal-space-runtime-xfs-77` directory. The filesystem used 4 KiB blocks, reflinks enabled and `noatime`. Firecracker and `snapshot-editor` were version 1.16.1. The guests used Node 24.21.0. The agent guest used Podman 6.1.4 and crun. Starting with a disposable template, each plugin session issued 50 broker calls across the runtime with all five workers resident and set an in-memory marker. Each agent session set a marker in the live Node process in each of its three containers and then synced its scratch disk.

- **Diff.** Save dirty guest pages and the matching device state. At wake, create a temporary XFS reflink of the template memory file and apply the delta with `snapshot-editor`.
- **Merged dirty reflink.** Apply the Firecracker delta to a template reflink before sleep, and retain the merged inode and matching device state. XFS FIEMAP counts the private extents. The inode’s `st_blocks` also includes shared extents.
- **Full-scan reflink.** Save full memory, compare it against the template in 4 KiB pages and rewrite only differing pages into a new reflink. Retain the merged inode and the state from the full save.
- **Reporting and hole punch.** Enable free-page reporting at boot, save a full memory file, then use `fallocate --dig-holes` to deallocate zero blocks.
- **Balloon and hole punch.** Use the reporting configuration and request a 64 MiB or 1,024 MiB balloon before saving. Punch zero blocks and request deflation on wake.

The save timer covers pause, snapshot writes and fsync. The reflink arm includes merging and syncing the final inode, and the hole-punch arms include scanning, punching and syncing. Balloon settle and scratch-disk sync are outside the save timer. The harness compares the first Firecracker diff at each size with two full snapshots taken during the same pause. It checks all 64-byte blocks where the two full saves agree. The full-scan reflink and hole-punch controls require byte-for-byte agreement with their input. Firecracker describes diff snapshots as a developer preview. [Firecracker snapshot support](https://github.com/firecracker-microvm/firecracker/blob/v1.16.1/docs/snapshotting/snapshot-support.md)

The wake timer starts at release of the request and ends after reseeding and checking a restored session marker. For plugins it also checks five live worker PIDs and a successful settings call. For agents it requires the expected three running containers and a reply from each saved Node process. Every restore creates a fresh VMM. Agent restores use a fresh private copy of the saved scratch-disk generation. The raw diff includes its per-wake merge.

Evicted runs used file-specific `POSIX_FADV_DONTNEED` on the memory inputs, state and backing drives. The table reports `mincore` residency of the first snapshot memory input per batch. For a raw diff, the materialized memory inode is created inside the timer. Cached runs either read the files in advance or replayed the wake working set on each inode. The 2 GiB raw diff, merged dirty and hole-punch runs replayed the working set for singles as well. The residency column shows the cache that remained at launch. The 20-way batches release 20 independent VMM launches together, with independent memory inodes and private agent scratch disks. Wake attempts within a batch are correlated. Cleanup after a successful wake can overlap the other wakes.

Sizes count private filesystem extents for the memory and state files. Add the separate private scratch allocation for the agent guest. Shared template memory and shared OS/tool images are charged once per generation. The punched and full-scan arms measured five independently prepared sessions per size and made additional independent files for the 20-way batches. A punched copy keeps holes and a reflink copy shares the template extents.

| Guest MiB | Arm | Saves | Private memory + state bytes p50 / p95 / max | Private scratch bytes p50 / p95 / max | Save ms p50 / p95 / max | Balloon wait ms p50 / p95 / max |
| --- | --- | --- | --- | --- | --- | --- |
| 256 | Raw diff | 20 | 16,201,728 / 16,302,080 / 16,371,712 | — | 96.90 / 107.43 / 148.41 | — |
| 256 | Merged dirty reflink | 20 | 76,484,608 / 76,877,824 / 76,877,824 | — | 255.96 / 277.09 / 331.20 | — |
| 256 | Full-scan reflink | 5 | 71,892,992 / 72,024,064 / 72,024,064 | — | 734.69 / 1,153.73 / 1,153.73 | — |
| 256 | Reporting + punch | 5 | 173,547,520 / 173,576,192 / 173,576,192 | — | 716.72 / 2,065.46 / 2,065.46 | — |
| 256 | Balloon + punch | 5 | 173,252,608 / 173,531,136 / 173,531,136 | — | 787.91 / 840.33 / 840.33 | 101.90 / 129.56 / 129.56 |
| 2048 | Raw diff | 20 | 49,508,352 / 55,541,760 / 55,648,256 | 2,629,632 / 2,662,400 / 2,670,592 | 228.38 / 269.19 / 273.05 | — |
| 2048 | Merged dirty reflink | 20 | 138,803,200 / 215,633,920 / 217,993,216 | 2,629,632 / 2,662,400 / 2,670,592 | 591.61 / 665.18 / 670.59 | — |
| 2048 | Full-scan reflink | 5 | 117,293,056 / 124,641,280 / 124,641,280 | 2,613,248 / 2,723,840 / 2,723,840 | 17,373.26 / 24,378.04 / 24,378.04 | — |
| 2048 | Reporting + punch | 5 | 279,212,032 / 284,860,416 / 284,860,416 | 2,695,168 / 2,703,360 / 2,703,360 | 23,910.60 / 27,647.30 / 27,647.30 | — |
| 2048 | Balloon + punch | 5 | 264,962,048 / 275,648,512 / 275,648,512 | 2,629,632 / 2,650,112 / 2,650,112 | 22,848.69 / 24,987.03 / 24,987.03 | 1,162.54 / 1,552.22 / 1,552.22 |

| Guest MiB | Arm | Parallel | Cache | Attempts | Failed | Wake ms p50 / p95 / max | First memory input resident % |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 256 | Raw diff | 1 | warm | 5 | 0 | 757.50 / 1,913.87 / 1,913.87 | 100.0–100.0 |
| 256 | Raw diff | 1 | cold | 5 | 0 | 1,061.13 / 1,238.98 / 1,238.98 | 0.0–0.0 |
| 256 | Raw diff | 20 | warm | 60 | 0 | 4,247.43 / 4,386.98 / 4,418.88 | 100.0–100.0 |
| 256 | Raw diff | 20 | cold | 60 | 0 | 4,877.44 / 5,489.25 / 5,527.96 | 0.0–0.0 |
| 256 | Merged dirty reflink | 1 | warm | 5 | 0 | 391.47 / 468.95 / 468.95 | 100.0–100.0 |
| 256 | Merged dirty reflink | 1 | cold | 5 | 0 | 965.77 / 993.48 / 993.48 | 0.0–0.0 |
| 256 | Merged dirty reflink | 20 | warm | 60 | 0 | 1,838.03 / 2,556.71 / 2,702.06 | 31.3–100.0 |
| 256 | Merged dirty reflink | 20 | cold | 60 | 0 | 6,226.71 / 6,465.62 / 6,520.57 | 0.0–0.0 |
| 256 | Full-scan reflink | 1 | warm | 5 | 0 | 489.03 / 512.97 / 512.97 | 100.0–100.0 |
| 256 | Full-scan reflink | 1 | cold | 5 | 0 | 1,355.22 / 1,624.43 / 1,624.43 | 0.0–0.0 |
| 256 | Full-scan reflink | 20 | warm | 60 | 0 | 1,944.34 / 1,994.48 / 2,009.16 | 100.0–100.0 |
| 256 | Full-scan reflink | 20 | cold | 60 | 0 | 6,119.17 / 6,825.75 / 6,900.58 | 0.0–0.0 |
| 256 | Reporting + punch | 1 | warm | 5 | 0 | 264.85 / 314.16 / 314.16 | 100.0–100.0 |
| 256 | Reporting + punch | 1 | cold | 5 | 0 | 2,845.73 / 2,994.40 / 2,994.40 | 0.0–0.0 |
| 256 | Reporting + punch | 20 | warm | 60 | 0 | 1,659.21 / 1,825.30 / 1,846.18 | 100.0–100.0 |
| 256 | Reporting + punch | 20 | cold | 60 | 0 | 19,186.46 / 20,580.65 / 20,705.22 | 0.0–0.0 |
| 256 | Balloon + punch | 1 | warm | 5 | 0 | 513.82 / 594.11 / 594.11 | 100.0–100.0 |
| 256 | Balloon + punch | 1 | cold | 5 | 0 | 2,988.88 / 3,124.82 / 3,124.82 | 0.0–0.0 |
| 256 | Balloon + punch | 20 | warm | 60 | 0 | 2,183.43 / 2,324.51 / 2,344.75 | 100.0–100.0 |
| 256 | Balloon + punch | 20 | cold | 60 | 0 | 18,274.96 / 20,120.93 / 20,264.55 | 0.0–0.0 |
| 2048 | Raw diff | 1 | warm | 5 | 0 | 4,541.76 / 6,230.12 / 6,230.12 | 1.9–4.1 |
| 2048 | Raw diff | 1 | cold | 5 | 0 | 5,851.41 / 9,017.74 / 9,017.74 | 0.0–0.0 |
| 2048 | Raw diff | 20 | warm | 40 | 0 | 21,321.76 / 22,417.94 / 22,838.80 | 3.8–4.1 |
| 2048 | Raw diff | 20 | cold | 40 | 0 | 24,830.90 / 26,019.98 / 26,123.49 | 0.0–0.0 |
| 2048 | Merged dirty reflink | 1 | warm | 5 | 0 | 5,271.74 / 5,903.22 / 5,903.22 | 10.7–13.8 |
| 2048 | Merged dirty reflink | 1 | cold | 5 | 0 | 5,154.70 / 6,241.16 / 6,241.16 | 0.0–0.0 |
| 2048 | Merged dirty reflink | 20 | warm | 40 | 0 | 12,680.39 / 13,186.51 / 13,248.01 | 11.1–13.9 |
| 2048 | Merged dirty reflink | 20 | cold | 40 | 0 | 29,361.08 / 30,309.95 / 30,482.83 | 0.0–0.0 |
| 2048 | Full-scan reflink | 1 | warm | 5 | 0 | 6,026.33 / 7,117.24 / 7,117.24 | 31.6–100.0 |
| 2048 | Full-scan reflink | 1 | cold | 5 | 0 | 4,671.82 / 5,755.72 / 5,755.72 | 0.0–0.0 |
| 2048 | Full-scan reflink | 20 | warm | 40 | 0 | 13,468.63 / 13,962.25 / 13,986.91 | 10.0–29.5 |
| 2048 | Full-scan reflink | 20 | cold | 40 | 0 | 24,743.75 / 26,001.27 / 26,023.67 | 0.0–0.0 |
| 2048 | Reporting + punch | 1 | warm | 5 | 0 | 2,208.35 / 2,741.92 / 2,741.92 | 13.6–13.9 |
| 2048 | Reporting + punch | 1 | cold | 5 | 0 | 6,999.27 / 7,940.06 / 7,940.06 | 0.0–0.0 |
| 2048 | Reporting + punch | 20 | warm | 40 | 0 | 11,890.19 / 12,861.09 / 13,123.59 | 10.5–11.9 |
| 2048 | Reporting + punch | 20 | cold | 40 | 0 | 38,251.26 / 42,377.27 / 42,729.09 | 0.0–0.0 |
| 2048 | Balloon + punch | 1 | warm | 5 | 0 | 8,359.35 / 9,400.86 / 9,400.86 | 14.1–15.4 |
| 2048 | Balloon + punch | 1 | cold | 5 | 0 | 7,991.47 / 8,628.97 / 8,628.97 | 0.0–0.0 |
| 2048 | Balloon + punch | 20 | warm | 40 | 0 | 20,125.54 / 20,957.98 / 20,999.77 | 13.3–14.6 |
| 2048 | Balloon + punch | 20 | cold | 40 | 0 | 47,177.49 / 49,767.21 / 50,519.80 | 0.0–0.0 |

The first raw diff comparison was discarded because the control assumed byte-for-byte stability across two save operations. The corrected native XFS control found one changing 64-byte block in each guest size. The merged memory agreed with the full snapshots everywhere the full controls agreed. The first 2 GiB reporting and balloon attempts stopped before saving when the agent state check returned no JSON reply. The retry extended the reply window and verified the template cohort before saving. The wake tables contain 1,100 measured attempts with 0 failures. In 10 of 10 same-snapshot entropy controls, the Node stream repeated before reseeding and all three draws differed after reseeding. The plugin entropy check covered its long-lived broker process. The agent check covered each of the three fixture Node processes. Reseed long-lived RNGs before either a template clone or a sleeping guest serves requests.

### What this means for snapshot design

Use a materialized reflink for the sleeping format. The byte-exact full-scan implementation stored 68.6 MiB for plugins and 114.3 MiB for agents, including state and the measured private scratch. Those allocations are 26.8% and 5.6% of the original dense memory sizes. Median save was 0.73 s and 17.4 s. The full-scan save needs temporary space for a dense snapshot. The agent scratch numbers cover the idle fixture. A job that writes files adds its own disk allocation.

The measured dirty-merge path published the same restore format in 0.26 s for plugins and 0.59 s for agents at the median. It occupied 72.9 MiB and 134.9 MiB at the median. Treat it as a faster save path to qualify on a pinned Firecracker release because diff snapshots are still in developer preview. Persisting a raw delta moves work into wake. Its plugin cached single p95 was 1.91 s and cached 20-way p95 was 4.39 s. Merging at save gave 0.47 s cached and 0.99 s evicted for one plugin wake. Start with the full-scan writer as the correctness reference and fallback. Keep the 500 ms gate scoped to a separately qualified warm wake. The native XFS evicted and 20-way measurements exceed it.

Free-page reporting plus punching cost 165.5 MiB per plugin sleeper. Adding the balloon changed that to 165.2 MiB and raised the plugin cached single p95 from 0.31 s to 0.59 s. Zero punching requires a full write before reclaiming disk blocks. The XFS allocation figures include additional inode blocks reported after sync. The changed-page extents alone undercounted space, so the report uses allocated inode blocks minus the shared mappings. In the agent guest, reporting plus punching kept 268.8 MiB, and adding the balloon kept 255.2 MiB. Both figures include private scratch.

### Restore after a restart or image upgrade

Keep a versioned manifest with the exact state, memory, and scratch-disk generation for each sleeping VM. Persist the snapshot files and their parent directory before publishing the manifest. Restore the pinned Firecracker snapshot format and CPU configuration with the same immutable guest OS and tool images. Reopen the agent scratch disk saved with that state. The restore can use a new vsock socket path.

A raw diff also needs the exact base memory file and any prior layers in order. A materialized reflink holds the referenced blocks through its own inode, so the template memory inode can be removed once no raw diffs depend on it. A hole-punched memory file is also self-contained. Preserve old immutable drive images until their sleepers have resumed or been retired. A new base image starts a new template generation. Resume the old generation or cold-start from authoritative broker state when retiring it. The benchmark used fresh VMM processes and the original images. Host reboot, torn writes and substitution of an upgraded image were outside the test. [Firecracker snapshot versioning](https://github.com/firecracker-microvm/firecracker/blob/v1.16.1/docs/snapshotting/versioning.md)

Raw follow-up logs, source harnesses and filesystem metadata are retained under `~/space-runtime-measure/xfs-followup/` on perf-01. The JSON companion records numeric results and file hashes.

## Whole-file read over the space channel

A disposable four-vCPU, 2 GiB Firecracker guest read the payloads over AF_VSOCK. The host broker joined Firecracker’s Unix socket, checked one immutable authority row keyed by caller, file ID and requested revision, then opened that saved file and streamed it with `sendfile`. Authority timing covers the indexed rusqlite query and row fetch against a static fixture table. Product authorization, revocation and Photos decoding are outside this test. The guest issued at most 4 MiB of credit per stream at a time, measured file latency from request through the final byte, and checked the length and file ID markers at both ends.

Each corpus held 40,000 MiB of fully allocated files. With only 26 GiB free on native XFS, this arm used a 41 GiB reflink-enabled XFS filesystem on a disposable `/dev/loop0` with direct I/O enabled. Its sparse backing file was `~/space-runtime-measure/xfs-followup/bulk.img` on perf-01’s ext4 `/dev/sda1`. The 200 large files held 40,000 MiB of physical payload. Each 4 MiB file reflinked a different, nonoverlapping range from those extents. Within each size layout, every logical payload byte came from a distinct storage offset. The two layouts shared the physical extents. This kept both layouts within the disk reserve. Before each run the driver evicted both layouts’ payload files with `POSIX_FADV_DONTNEED` and sampled residency with `mincore`. Startup, corpus generation and authority setup were outside the timer. CPU columns report CPU seconds per elapsed second. Firecracker CPU includes vCPU execution and overlaps the guest reader CPU.

| Files | Each MiB | Streams | Wall s | MB/s | Per-file ms p50 / p95 / max | Authority µs p50 / p95 / max | Broker cores | Firecracker cores | Guest reader cores |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 10000 | 4 | 1 | 128.9 | 325.5 | 12.36 / 17.66 / 45.79 | 58.00 / 75.39 / 370.35 | 0.10 | 0.99 | 0.06 |
| 10000 | 4 | 4 | 76.4 | 548.7 | 28.13 / 46.18 / 115.49 | 72.23 / 180.88 / 1,856.78 | 0.25 | 1.98 | 0.26 |
| 10000 | 4 | 8 | 71.7 | 585.4 | 54.04 / 81.37 / 244.28 | 70.35 / 284.96 / 4,512.37 | 0.30 | 2.25 | 0.39 |
| 200 | 200 | 1 | 120.3 | 348.6 | 591.84 / 675.47 / 815.14 | 59.97 / 94.04 / 437.07 | 0.10 | 1.09 | 0.06 |
| 200 | 200 | 4 | 75.2 | 557.7 | 1,477.28 / 1,709.15 / 1,855.10 | 79.87 / 388.65 / 1,131.30 | 0.24 | 2.02 | 0.28 |
| 200 | 200 | 8 | 70.5 | 594.9 | 2,795.76 / 3,102.16 / 3,255.86 | 72.26 / 277.06 / 1,030.75 | 0.26 | 2.31 | 0.40 |

All 30,600 reads completed with one successful check each. The sampled payload pages were fully evicted before all six runs. Authority median was 58–80 µs and p95 was 75–389 µs. The table reports one complete corpus pass per cell. Broker and Firecracker columns omit other host kernel workers. The guest column is CPU charged to the reader process.

**What this means for the design.** Use four streams as the starting limit for a heavy-job VM. They delivered 549–558 MB/s, 94% of the eight-stream throughput, with a lower file-latency p95 and lower CPU use. For this fixture, 300 MB/s at one stream and 500 MB/s at four are reasonable qualification targets. Publish a rate for a deployment after qualifying its storage and production broker. The wire contract can guarantee bounded in-flight data and back pressure independently of provisioning.

The earlier 2 MiB range fixture’s 168.45 ms p95 covers staging, commit, head advance and reconstruction with 32 range calls. That is about 12.4 MB/s when expressed as bytes divided by elapsed time for that workflow. The workload and timing boundary differ from transfer-only streaming. [Step 0 range measurements](plugin-api-v2-step0.md#atomic-large-save)

The wire should open a whole-file read with an item and requested revision, pin that exact immutable revision for the stream, and return the length and a stream ID after authorizing the caller identified by the enrolled channel. Carry ordered binary frames and enforce bounded byte credits on each stream and across the VM. Close the pin on completion, cancellation or expiry. Route revocation through stream cancellation so returning credit does not perform a new authority lookup. The benchmark checked immutable revisions and flow control. It did not exercise revocation.

## Simplification opportunities

The guest relay and stdio adapter share a newline-delimited frame format, so one driver can validate the operation and timing boundary for all three walls.

Keep one current snapshot generation per space and VM role. Reuse one broker flow-control mechanism for large files. Combine agent reseeding and the cohort check into a readiness exchange.
