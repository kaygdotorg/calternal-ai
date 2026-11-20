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

   These arithmetic ceilings use the initial 45.55 GiB available disk and 6.51 GiB MemAvailable on this 7.70 GiB host. Disk subtracts the 8 GiB guard and shared fixture allocation (about 272 MiB for plugins or 4.56 GiB for agents). RAM subtracts the 1 GiB guard and divides by configured guest RAM, before VMM overhead. The 512 MiB and 1 GiB fresh PSS values are medians from the pre-snapshot samples. The 8 GiB guest booted and restored because its resident pages occupied a fraction of configured RAM. Fully backed admission would exclude it on this host. That snapshot benchmark exercised one agent space at a time. The [always-on sweep](#always-on-agent-density) measured mixed populations of agent and plugin VMs.
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

## Always-on agent density

Measured on `calternal-perf-01-backup01` with four visible AMD Ryzen 7 3700X CPUs, 7.70 GiB RAM and no swap. The host itself is a KVM VM. Firecracker was v1.16.1 and the agent disks were on native XFS. Agent guests had two vCPUs and 2 or 4 GiB configured RAM. Each booted one or three rootless Podman containers with an idle Node process in each. Each active VM ran the workload in one container while the other containers stayed idle. Plugin code ran in a separate 256 MiB VM. Files and build output lived in disposable guest scratch. No userdata was mounted.

The editor-and-shell loop reread a source slice, edited scratch files and streamed output over vsock every two seconds. At one-minute intervals it compiled and ran a small test against the SQLite header. Each light population had to show fresh output and a successful test from every active VM during its measured window. The clean build compiled the upstream SQLite amalgamation shipped with `libsqlite3-sys 0.39.1` at `-O2` and checked the output object. It represents one CPU-heavy compiler per active VM. The toolset and fixtures were built on dev-01. The guest compiles were the measured workloads. These short fixtures cover co-tenancy and memory residency over the measured windows. The toolset kept the existing agent runtime and added the compiler and source fixture.

Each density arm first measured the plugin with zero neighbouring agent VMs, then increased the number of prestarted neighbours. The always-on plugin served brokered settings calls while the driver independently woke a snapshot of the same five-worker fixture. Wake includes reseeding and cohort validation. The baseline primed the plugin snapshot before the sweep. The JSON reports cache residency at each population. Memory pressure can affect the wake. Idle and light broker measurements use 12 and 70 second target windows. The timed wake sequence runs alongside the broker sampling and issues up to 12 sequential wakes early in each window. Build latency samples were kept during collective compile overlap and the memory sampler continued through completion. The agent density sweeps used free-page reporting off with the balloon at zero.

A population passes if its plugin broker p95 is at most the larger of 5 ms and twice its own plugin-only baseline, and wake p95 is at most the larger of 500 ms and twice its own baseline. A stage also needs successful workload checks, at least 100 timed broker calls and five timed wakes, and zero probe errors. The sweep stops at the first tested miss or the memory guard below 1.5 GiB host MemAvailable. A passing tested population is a lower bound on density when the next sample skipped counts. The criterion measures co-tenancy on this machine. The observed wake times also show which populations stay within an absolute 500 ms.

The sampler read each VMM’s host RSS and PSS once a second. PSS apportions file-backed sharing. Host MemAvailable captures more than VMM process memory, including kernel activity and competing caches. Host and guest steal come from their respective CPU counters. Host vCPU wait is Linux scheduler wait divided by runtime plus scheduler wait for the vCPU threads. That percentage is conditional on runnable time. Idle vCPUs can show a large ratio on little runnable time. Scheduler statistics were enabled during the locked arms and restored when each arm released the lock. Each arm held its own perf lock with a 17-minute watchdog. [Linux scheduler counters](https://docs.kernel.org/scheduler/sched-stats.html#proc-pid-schedstat)

The initial 2 GiB one-agent idle and light sweeps each accumulated 4,095 broker and warm-up calls plus one initialization call on its settings worker. The worker reached its invocation cap and disconnected. The raw logs retain both attempts. The corrected arms use a new warm settings worker for each population with the same snapshot fixture and preserve the agent VMs between steps. The later light arms use that procedure as well. [Worker rotation contract](../../crates/plugin-host/README.md)

### Resident footprint with one neighbouring agent VM

| Guest MiB | Agents | Activity | Samples | VMM PSS MiB p50 / p95 / max | VMM RSS MiB p50 / p95 / max | Host min available MiB |
| --- | --- | --- | --- | --- | --- | --- |
| 2048 | 1 | idle | 12 | 234.2 / 234.2 / 234.2 | 236.5 / 236.5 / 236.5 | 6,535.3 |
| 2048 | 1 | light | 70 | 253.8 / 261.3 / 261.3 | 256.0 / 263.5 / 263.5 | 6,481.4 |
| 2048 | 1 | build | 53 | 582.2 / 616.3 / 616.4 | 584.5 / 618.6 / 618.7 | 6,146.1 |
| 2048 | 3 | idle | 13 | 244.0 / 244.0 / 244.0 | 246.3 / 246.3 / 246.3 | 6,492.8 |
| 2048 | 3 | light | 70 | 292.8 / 294.8 / 294.8 | 295.1 / 297.1 / 297.1 | 6,426.9 |
| 2048 | 3 | build | 53 | 609.5 / 621.2 / 621.3 | 611.8 / 623.5 / 623.6 | 6,115.1 |
| 4096 | 1 | idle | 13 | 323.7 / 323.7 / 323.7 | 326.0 / 326.0 / 326.0 | 6,437.2 |
| 4096 | 1 | light | 70 | 365.3 / 371.4 / 371.4 | 367.6 / 373.7 / 373.7 | 6,368.3 |
| 4096 | 1 | build | 53 | 678.4 / 699.3 / 699.3 | 680.7 / 701.6 / 701.7 | 6,029.9 |
| 4096 | 3 | idle | 13 | 362.7 / 362.7 / 362.7 | 365.0 / 365.0 / 365.0 | 6,397.0 |
| 4096 | 3 | light | 70 | 394.3 / 399.5 / 399.5 | 396.5 / 401.7 / 401.7 | 6,349.1 |
| 4096 | 3 | build | 55 | 720.3 / 729.9 / 730.0 | 722.5 / 732.1 / 732.3 | 5,988.8 |

### Plugin latency as the agent population grows

| Guest MiB | Agents | Activity | Measured populations | Last pass | First miss | Baseline RTT p95 ms | Baseline wake p95 ms | Last pass RTT p95 ms | Last pass wake p95 ms | Miss RTT p95 ms | Miss wake p95 ms | Stop |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2048 | 1 | idle | 1 / 2 / 3 / 4 / 6 / 8 / 10 / 12 / 16 / 20 / 24 | 24 | 25 started (target 28) | 5.26 | 377.1 | 8.23 | 335.9 | — | — | 1.5 GiB host MemAvailable guard |
| 2048 | 1 | light | 1 / 4 / 8 / 12 / 16 | 16 | 20 started | 4.43 | 255.8 | 2.43 | 237.3 | — | — | 1.5 GiB host MemAvailable guard |
| 2048 | 1 | build | 1 / 2 / 3 / 4 / 6 / 8 | 8 | 10 started | 7.22 | 481.3 | 12.66 | 743.6 | — | — | guest connection refused |
| 2048 | 3 | idle | 1 / 2 / 3 / 4 / 6 / 8 / 10 / 12 / 16 / 20 | 20 | 22 started (target 24) | 5.77 | 447.7 | 3.06 | 454.6 | — | — | 1.5 GiB host MemAvailable guard |
| 2048 | 3 | light | 1 / 4 / 8 / 12 / 16 | 16 | 19 started (target 20) | 5.73 | 417.4 | 2.39 | 313.8 | — | — | 1.5 GiB host MemAvailable guard |
| 2048 | 3 | build | 1 | 0 | 1 | 2.75 | 291.6 | — | — | 6.69 | 216.7 | broker p95 threshold |
| 4096 | 1 | idle | 1 / 2 / 3 / 4 / 6 / 8 / 10 / 12 / 16 | 16 | 17 started (target 20) | 4.46 | 704.9 | 5.43 | 524.9 | — | — | 1.5 GiB host MemAvailable guard |
| 4096 | 1 | light | 1 / 4 / 8 / 12 | 12 | 15 started (target 16) | 5.74 | 350.0 | 2.46 | 350.2 | — | — | 1.5 GiB host MemAvailable guard |
| 4096 | 1 | build | 1 / 3 / 4 / 6 | 4 | 6 | 5.99 | 562.2 | 11.38 | 679.9 | 12.37 | 682.8 | broker p95 threshold |
| 4096 | 3 | idle | 1 / 2 / 3 / 4 / 6 / 8 / 10 / 12 | 12 | 15 started (target 16) | 7.24 | 476.6 | 6.73 | 506.4 | — | — | 1.5 GiB host MemAvailable guard |
| 4096 | 3 | light | 1 / 4 / 8 / 12 | 12 | 14 started (target 16) | 5.39 | 493.0 | 2.55 | 438.6 | — | — | 1.5 GiB host MemAvailable guard |
| 4096 | 3 | build | 1 / 3 | 1 | 3 | 6.26 | 510.9 | 7.29 | 272.8 | 13.05 | 439.2 | broker p95 threshold |

The corrected one-agent 2 GiB idle arm passed at 24 VMs and reached the memory guard after starting a 25th. In the clean-build arm for that shape, ten VMs started but a guest connection was refused before the stage produced valid probe data. The last eight-VM stage had 1,896 MiB of host MemAvailable. The ten-VM error did not identify whether memory or another failure caused the connection loss. The 2 GiB three-agent arm missed the broker limit at one build on a 2.75 ms baseline. Its 6.69 ms broker p95 and 217 ms wake p95 describe that arm's relative result. Build gates across these arms depended on the observed baselines and available CPU.

### CPU steal and vCPU scheduler wait

| Guest MiB | Agents | Activity | Neighbours | Result | Host steal % | Guest median steal % | Guest max steal % | Agent vCPU wait % | Plugin vCPU wait % |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2048 | 1 | idle | 24 | pass | 0.16 | 0.21 | 1.02 | 73.66 | 10.99 |
| 2048 | 1 | light | 16 | pass | 0.11 | 0.04 | 0.16 | 7.13 | 4.86 |
| 2048 | 1 | build | 8 | pass | 0.08 | 32.25 | 32.53 | 61.05 | 69.53 |
| 2048 | 3 | idle | 20 | pass | 0.10 | 0.12 | 0.50 | 66.99 | 13.98 |
| 2048 | 3 | light | 16 | pass | 0.08 | 0.05 | 0.41 | 8.14 | 7.20 |
| 2048 | 3 | build | 1 | miss | 0.09 | 0.15 | 0.15 | 0.27 | 21.42 |
| 4096 | 1 | idle | 16 | pass | 0.06 | 0.12 | 0.29 | 59.85 | 21.68 |
| 4096 | 1 | light | 12 | pass | 0.08 | 0.05 | 0.34 | 7.39 | 6.49 |
| 4096 | 1 | build | 4 | pass | 0.08 | 6.27 | 6.47 | 14.18 | 68.05 |
| 4096 | 1 | build | 6 | miss | 0.12 | 21.42 | 22.55 | 43.87 | 70.27 |
| 4096 | 3 | idle | 12 | pass | 0.06 | 0.12 | 0.21 | 54.59 | 24.99 |
| 4096 | 3 | light | 12 | pass | 0.09 | 0.06 | 0.13 | 10.07 | 6.52 |
| 4096 | 3 | build | 1 | pass | 0.13 | 0.12 | 0.12 | 0.22 | 21.87 |
| 4096 | 3 | build | 3 | miss | 0.10 | 2.46 | 2.46 | 5.04 | 56.90 |

Idle and light agent guests recorded little steal even near the memory guard. Steal grew with sustained compiler overlap while host-level steal stayed low. At eight builds in the 2 GiB one-agent arm, median agent guest steal was 32.25%, median agent vCPU run-queue wait was 61.05% of run plus wait, and the live plugin vCPUs spent 69.53% of that active scheduling time waiting.

### Reclaiming resident memory from awake guests

Reclamation was measured in a separate, otherwise idle agent VM. The fixture touched one byte per page in a 512 MiB buffer in the 2 GiB guest or a 1,024 MiB buffer in the 4 GiB guest, held it for 12 seconds and then exited. The idle column is median VMM PSS 60–75 seconds after the touch started. The peak-to-idle drop is measured host PSS. It can include other guest memory changing during the window.

For the balloon pass, the driver requested half the configured RAM and recorded the actual inflated amount and settle time. It then requested deflation and repeated the touch. The return time is the first 0.5-second sample at or above 80% of the observed PSS rise from the ballooned idle level. The deflation column reports how long it took to observe an actual balloon of zero. Balloon size is guest capacity released, while PSS measures its host residency effect.

| Guest MiB | Agents | FPR | Fresh PSS MiB | Touch peak MiB | Idle at 60–75 s MiB | Peak to idle drop MiB | Balloon idle PSS MiB | Actual balloon MiB | Inflation s | Deflation to zero s | 80% rebound s | Return peak PSS MiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2048 | 1 | off | 227.3 | 748.5 | 748.5 | 0.0 | — | — | — | — | — | — |
| 2048 | 1 | on | 219.4 | 747.6 | 259.6 | 488.0 | 226.5 | 1024 | 0.20 | 0.26 | 6.28 | 759.0 |
| 2048 | 3 | off | 261.4 | 779.7 | 779.7 | 0.0 | — | — | — | — | — | — |
| 2048 | 3 | on | 261.3 | 786.7 | 306.7 | 480.0 | 271.7 | 1024 | 0.20 | 0.25 | 5.77 | 784.4 |
| 4096 | 1 | off | 327.4 | 1,368.5 | 1,368.5 | 0.0 | — | — | — | — | — | — |
| 4096 | 1 | on | 327.5 | 1,368.2 | 385.0 | 983.2 | 358.5 | 2048 | 0.30 | 0.81 | 20.49 | 1,386.3 |
| 4096 | 3 | off | 363.6 | 1,395.5 | 1,395.5 | 0.0 | — | — | — | — | — | — |
| 4096 | 3 | on | 361.9 | 1,396.5 | 420.5 | 976.0 | 385.4 | 2048 | 0.31 | 0.81 | 9.92 | 1,422.0 |

### What this means for the design

Use 2 GiB as the default agent VM tier for the tested editor, shell and SQLite build. Assign 4 GiB when the job requires more guest memory. An always-on agent avoids an agent wake on its request path but carries resident memory from recent work. The build measurements support separate admission for runnable compiles and for resident VMs. Keep capacity for the plugin broker and plugin wakes when admitting builds. On perf-01 the relative gate allowed up to eight simultaneous builds in one sweep, but the wake p95 at eight was 744 ms and median guest CPU steal was 32%. In two other sweeps the broker gate tripped with several GiB still available. A small concurrency limit, measured again with explicit plugin CPU headroom, gives a stronger latency commitment than pricing by guest RAM alone.

The sizing tables reserve 25% of physical RAM for the OS, plugins and burst headroom, add 128 MiB per agent VM for costs outside VMM PSS, and round the observed memory budget up to the next 128 MiB. The all-activity budget also covers the highest observed PSS in the allocation and rebound tests. The counts are memory-only divisions of the remainder. The fully backed column reserves the complete configured guest memory and the same overhead. A production density price needs a bound on per-agent memory and locally qualified CPU, I/O and plugin load. A self-hoster should subtract any services that need more than the 25% pool before using these counts.

Enable free-page reporting for always-on guests if the production kernel and VMM qualification preserve the reclaim result. Across both cohorts it released 480–488 MiB after the 512 MiB touch in 2 GiB guests and 976–983 MiB after the 1 GiB touch in 4 GiB guests. The extra half-guest balloon reclaimed 26–35 MiB once free-page reporting had settled. The balloon dropped to zero in 0.25–0.81 seconds. The return to 80% of the tested PSS rise took 5.8–20.5 seconds. Budget the peak memory for work that can resume concurrently, even with reporting enabled.

For capacity pricing with a bounded profile and the stated reserve, a 64 GiB host has memory for 48–54 of the measured 2 GiB VMs or 29–32 of the 4 GiB VMs. A 256 GiB host has memory for 192–219 or 118–128 respectively. Use the fully backed counts for an unrestricted guest memory policy. A 16 GiB self-hoster can budget up to five fully backed 2 GiB guests or two fully backed 4 GiB guests after the 4 GiB host reserve, then size concurrent builds to the local plugin latency target. A workload capped to the measured profiles gives higher memory-only ceilings of 12–13 or 7–8.

### Memory-only capacity by activity

| Guest MiB | Agents | Activity | Peak sampled PSS MiB | Budget MiB/VM | 16 GiB | 64 GiB | 256 GiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2048 | 1 | idle | 236.6 | 384 | 32 | 128 | 512 |
| 2048 | 1 | light | 272.6 | 512 | 24 | 96 | 384 |
| 2048 | 1 | build | 619.8 | 768 | 16 | 64 | 256 |
| 2048 | 3 | idle | 250.9 | 384 | 32 | 128 | 512 |
| 2048 | 3 | light | 307.5 | 512 | 24 | 96 | 384 |
| 2048 | 3 | build | 621.3 | 768 | 16 | 64 | 256 |
| 4096 | 1 | idle | 323.7 | 512 | 24 | 96 | 384 |
| 4096 | 1 | light | 371.7 | 512 | 24 | 96 | 384 |
| 4096 | 1 | build | 708.1 | 896 | 13 | 54 | 219 |
| 4096 | 3 | idle | 362.7 | 512 | 24 | 96 | 384 |
| 4096 | 3 | light | 401.0 | 640 | 19 | 76 | 307 |
| 4096 | 3 | build | 743.1 | 896 | 13 | 54 | 219 |

### Memory-only capacity including the allocation stress test

| Guest MiB | Agents | PSS retained after touch with FPR off MiB | Allocation test peak PSS MiB | Worst measured budget MiB/VM | 16 GiB | 64 GiB | 256 GiB | Fully backed 16 / 64 / 256 GiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2048 | 1 | 748.5 | 759.0 | 896 | 13 | 54 | 219 | 5 / 22 / 90 |
| 2048 | 3 | 779.7 | 786.7 | 1024 | 12 | 48 | 192 | 5 / 22 / 90 |
| 4096 | 1 | 1,368.5 | 1,386.3 | 1536 | 8 | 32 | 128 | 2 / 11 / 46 |
| 4096 | 3 | 1,395.5 | 1,422.0 | 1664 | 7 | 29 | 118 | 2 / 11 / 46 |

Raw per-stage data, workload output and the fixture harness are retained under `~/space-runtime-measure/density-followup/` on perf-01. The JSON companion records sample counts, failures, p50/p95/max, cache residency, CPU counters and raw checksums. The density probes kept agent cohorts online for the duration of each arm. Multi-day memory retention, all agents in a cohort compiling simultaneously and additional host sizes were outside the observed workloads.

## Simplification opportunities

The guest relay and stdio adapter share a newline-delimited frame format, so one driver can validate the operation and timing boundary for all three walls.

Keep one current snapshot generation per space and VM role. Reuse one broker flow-control mechanism for large files. Combine agent reseeding and the cohort check into a readiness exchange. Use one build admission queue per host with separate plugin CPU headroom.

## Product VM follow-up on perf-01

The November 20, 2026 UTC follow-up used Calternal-AI `bb34fcde39404d0ba5263e34c114751cbd583a1e`, Firecracker 1.16.1 and the pinned Linux 6.1.188 guest kernel. The host was `calternal-perf-01-backup01`, with 4 AMD Ryzen vCPUs, 7.70 GiB of RAM and no swap. All measured jobs held `/usr/local/bin/perf-lock.sh`. The host binaries, guest agent and images were prepared on dev-01 and verified by SHA-256 after transfer. The test builds include measurement fixtures from [perf/vm-measure](../../perf/vm-measure). Raw logs and artifact manifests are under `/srv/homes/perf/vm88/` on perf-01. The agent seed build log is under `~/Developer/lanes/vm-measure/warm-dev/` on dev-01. [Forgejo #88](https://git.kayg.org/kayg/calternal-ai/issues/88)

### Product snapshot publication and wake on native XFS

The test placed the machine runtime and snapshots on the native `/srv/homes` XFS volume (`/dev/sdb1`, `reflink=1`). It used a private subdirectory on the existing filesystem. The partition counters include XFS metadata and journal traffic and any unrelated writes that completed during a sample. Each sector in the kernel counter represents 512 bytes. These counts describe writes seen by the perf VM's block device.

The machine fixture used the production small-VM placement, checkpoint and restore functions with 256 MiB RAM, two vCPUs, 1 GiB of guest scratch and no virtual network adapter. Save time includes guest quiescing, durable manifest publication, old-generation cleanup and VMM teardown. Wake starts before the restore request and ends after a fresh Node process returns validated entropy output. The fixture requires the expected generation UUID and verifies the SHA-256 of a 64 KiB marker written by guest init, both in the saved scratch image and in the restored guest's scratch drive. Checks of the scratch image run outside the latency timers. Filesystem caches follow the preceding trial and were left intact. This wake includes machine and guest work. The enrolled server path is measured separately below.

A separate live KVM interruption fixture created candidate Firecracker state and memory files, then stopped the VMM before manifest publication. It reopened the previous generation and ran Node. The raw test response is in `small_snapshot_reseeds_recovers_after_interruption_and_rejects_mixed_files.log`. The first smoke series is in `measure_xfs_small_vm.log` and the repeated series is in `measure_xfs_small_vm-repeat.log`.

| Checkpoint format | Published | Sleep through publication and reap p50 / p95 (ms) | Block bytes written p50 / p95 | Successful restores | Successful restore p50 / p95 (ms) | First-attempt failures | Cold fallbacks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Full on XFS | 2 | 508.12 / 528.60 | 279,148,544 / 288,661,504 | 2 | 2,645.25 / 3,391.37 | 0 | 0 |
| Dirty-merge reflink | 30 | 1,243.39 / 1,388.46 | 82,286,592 / 84,594,688 | 29 | 3,019.48 / 4,090.14 | 8 | 1 |

The dirty-merge median wrote 71% fewer block bytes than the two full saves. Its successful wake p95 exceeded the 500 ms gate by a factor of 8.2. Seven first attempts timed out at restored plugin Start and recovered on retry. One timed out during reseed and fell back to a cold boot after the next failed restore. Across 30 dirty-merge cycles, the sum of timed attempts through the first validated answer was 3,213.83 ms p50 and 7,394.21 ms p95. Scratch matched on every successful restore. The cold fallback used a fresh scratch drive and is excluded from that restore check. The initial smoke run also hit a restored Start timeout and is excluded from the table.

The earlier native-XFS fixture measured 469 ms p95 for a cached single wake with a long-lived Node host. The product starts a fresh rootless package during wake. The observed gap combines the different runtime path and test conditions. [Forgejo #77](https://git.kayg.org/kayg/calternal-ai/issues/77)

### Product small-VM cold boot to first item head

Each trial creates a new disposable space and local enrolled runner with no usable snapshot. Timing starts at server dispatch of one approved item-head call and ends when the result arrives. The fixture verifies the result and confirms that Firecracker launched with a fresh boot definition. Trials reuse the immutable root and kernel on the same host, with normal filesystem caching.

The VMM interval includes dispatch, assignment, scratch setup and Firecracker startup through its startup-complete log immediately after vCPU resume. Kernel boot ends at guest PID 1's first serial marker. The agent interval ends when a guest-init observer sees the agent's control socket. The last interval includes the host readiness handshake, new package startup, the callback and the first reply. Serial observations are polled on the host every 2 ms and the guest observer checks its control socket every 5 ms. Raw samples and per-trial serial output are in `cold-qualified/`.

| Stage | Samples | p50 (ms) | p95 (ms) |
| --- | ---: | ---: | ---: |
| Request arrival to VMM start | 30 | 95.99 | 102.45 |
| Kernel boot | 30 | 1,313.27 | 1,642.92 |
| Guest init to agent socket ready | 30 | 1,617.58 | 3,905.00 |
| Agent socket ready to package start and first reply | 30 | 994.72 | 3,825.41 |
| Request arrival to validated item-head reply | 30 | 4,107.33 | 7,094.26 |

All 30 qualified calls succeeded. The longest took 7,798.55 ms. Percentiles use the sample median and nearest-rank p95. Stage percentiles are computed independently. The harness discarded the initial timing probes after identifying the Firecracker logger's local time offset and split serial writes. The qualified series timestamps complete serial lines with the same host clock used for request arrival.

### Release builds in separate agent guests

The workload is an offline `cargo build --release --locked -p calternal-server` under the repository's Nix toolchain in a networkless KVM guest. Cargo has two jobs and release incremental compilation enabled. Warm trials start on private XFS reflinks of the same data image, with a completed release target and the locked crates already cached. Each guest touches `crates/server/src/lib.rs` inside its own source tree. The cold full build starts with the same dependency cache and an empty target directory. Every agent guest has two vCPUs.

The separate 256 MiB plugin VM answers through the enrolled runner and production item-head broker. Its role state and checkpoints use the ext4 full-copy fallback on the same host, so the load test can sample the build interference separately from XFS dirty-restore failures. The agent disks use XFS. The fixture uses the production listener's TCP_NODELAY setting on its local TLS relay and accepted sockets. The lib-test fixture shortens the idle timeout to 180 ms for the traffic run, so the probe can ask for repeated wakes. It drives the existing idle drain and checkpoint sequence. Each tick targets a restored item-head reply followed by a second item-head reply on the awake VM. When a cycle overruns the next scheduled tick, the fixture records the missed tick. The load-row probe distribution includes cycles starting while all the builds in that row overlap. Filesystem caches follow the preceding row. Host CPU steal comes from `/proc/stat` and minimum available memory comes from `/proc/meminfo`. Raw stage directories are `build-incremental-<MiB>-<count>/`, `build-cold-4096-1/` and `build-no-load.vhESwXkO/`. The last directory contains the qualified unloaded baseline. The guard stops a stage below 1 GiB available host RAM or 6 GiB free on the scratch volume.

| Workload | Guest RAM (GiB) | Builds finished / attempted | Build wall p50 / max (s) | Wake p95 (ms) | Broker p95 (ms) | Probe pairs / errors / cold fallbacks | Host steal (%) | Minimum host available (MiB) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Unloaded baseline | — | 0 / 0 | — | 3,844.33 | 5.89 | 10 / 2 / 1 | — | — |
| Cold full build | 4 | 1 / 1 | 726 / 726 | 3,718.43 | 15.97 | 146 / 3 / 1 | 4.47 | 2,667 |
| Incremental | 2 | 1 / 1 | 91 / 91 | 3,756.10 | 6.96 | 23 / 0 / 0 | 5.38 | 4,681 |
| Incremental | 2 | 2 / 2 | 103 / 103 | 3,044.55 | 14.38 | 23 / 0 / 0 | 5.73 | 2,856 |
| Incremental, memory guard | 2 | 0 / 4 | — | 5,330.71 | 28.12 | 12 / 1 / 0 | 5.24 | 1,018 |
| Incremental, memory guard | 2 | 0 / 6 | — | 3,105.74 | 24.91 | 9 / 2 / 1 | 4.92 | 1,019 |
| Incremental, memory guard | 2 | 0 / 8 | — | 4,380.68 | 32.38 | 1 / 2 / 2 | 1.60 | 1,006 |
| Incremental | 4 | 1 / 1 | 92 / 92 | 3,267.25 | 13.54 | 24 / 0 / 0 | 5.47 | 4,273 |
| Incremental | 4 | 2 / 2 | 100 / 100 | 3,480.72 | 18.10 | 23 / 0 / 0 | 5.72 | 1,678 |
| Incremental, memory guard | 4 | 0 / 4 | — | 5,290.78 | 19.84 | 12 / 1 / 0 | 5.28 | 1,009 |
| Incremental, memory guard | 4 | 0 / 6 | — | 3,698.60 | 29.03 | 5 / 2 / 1 | 3.47 | 1,005 |
| Incremental, memory guard | 4 | 0 / 8 | — | — | — | 0 / 2 / 2 | 0.79 | 1,011 |

The unloaded probe missed 45 one-second ticks. The wake p95 already exceeded 500 ms with zero builds. Using the prior broker criterion of the larger of 5 ms and twice the unloaded p95 gives a broker gate of 11.79 ms. One 2 GiB build stayed within that gate. Two 2 GiB builds and one 4 GiB build crossed it. On this 4-vCPU, 7.70 GiB host, the 500 ms wake gate rules out every tested build count, including zero. For background work where the measured wake latency is acceptable, start with a single 2 GiB build slot in a per-host queue. Use a separate window for 4 GiB builds when broker latency matters.

All four-, six-, and eight-build cohorts started compiling `calternal-server`. Each hit the 1 GiB memory guard before a build finished. The table reports the successful probe pairs observed during the truncated overlap windows. The 2 GiB eight-build row has one pair and the 4 GiB eight-build row has zero. The original guarded stops left idle test TLS relays. The table uses the clean rerun after targeted relay teardown and a harness fix. Its output is in `build-guard-recheck.log`. The initial high-concurrency trials are retained under `guard-shakedown/`. A final process check found zero lane Firecracker or TLS relay processes.

## Simplification opportunities

Use one queue per host for both guest sizes. Qualify the unloaded wake first and reserve plugin headroom before raising build concurrency. Feed the latency checks and the admission decision from the same per-cycle records so retry and fallback costs remain visible.

## Where the product wake spends its time

### Summary for the owner (2026-11-20)

**What was measured.** A sleeping space wakes from a Firecracker snapshot on disk. On perf-02 the measured product wake is 1.61 s p50 / 2.12 s p95 against a 500 ms gate. About 0.9 s of it is starting a fresh Node process inside the restored guest. We asked why that is so slow and whether a Node process that was started before the snapshot could be reused safely.

**Why Node takes 0.9 s.** The same Node binary starts in 20 ms on the perf VM host and 20 ms in a freshly booted guest that has been up for a minute. So running Firecracker inside a virtual machine (nested virtualisation) is not what makes Node slow. In a restored guest the same start takes 877 ms p50 (30 runs), with the same number of page faults, no disk reads and the binary already in the guest's page cache. The guest simply stalls on first use of each page of restored memory: tracing the host's KVM showed about 12,400 nested-page-fault exits per Node launch in a restored guest against about 100 in a warm one, costing roughly 120 to 195 µs each. That first-touch tax is also most of the 317 ms reseed. Whether it would be much cheaper on bare metal is not measured; perf-02 is itself a virtual machine.

**Can a pre-started worker be made safe?** Yes, on the evidence gathered, under the launcher rules listed in the last section. A trusted launcher starts Node before the snapshot and parks it. After a restore and the kernel reseed acknowledgement, a small native module in the launcher asks Node's own OpenSSL to reseed its random generators from the kernel (`EVP_RAND_reseed` with prediction resistance on the primary, public and private generators). The launcher then drains Node's cached UUID bytes, gives V8 a fresh seed for contexts created later, and replaces `Math.random` with a generator backed by the secure source. Only then is the plugin's code loaded. Two restores of the same snapshot gave different `randomBytes`, `randomUUID`, `getRandomValues`, Ed25519 key pair, TLS ClientHello (including its random field) and `Math.random` sequences, and the same held for the plugin's first `vm` context. A counter showed three 32-byte kernel reads inside the refresh, after the acknowledgement. Native modules, worker threads, child processes and file writes are all provably refused after the launcher's own addon is loaded. What cannot be freshened: V8's hash seed (not observable by a plugin) and the identity of the original native `Math.random` function. The first round had a hole: the permission scope is spelled `addon`, and the `addons` I dropped did nothing, so plugin code could still have loaded a native module. That is fixed and now tested on every proof restore.

**The choice.**

| Option | Wake from disk | Memory | Status |
| --- | ---: | --- | --- |
| A. Disk wake with the pre-started, refreshed worker | about 0.65 s p50, an estimate: 30 harness wakes reached the worker ready at 985 ms, of which about 400 ms is test-shell overhead that the product path does not pay | none extra | Randomness gaps closed and proven. Needs integration into the product path and an end-to-end measurement. Does not reach 500 ms. |
| B. Disk wake without it | 1.61 s p50 / 2.12 s p95, measured | none extra | Qualified, simplest. |
| C. Keep recently used spaces running | 5 to 23 ms per call (4.73 / 22.69 ms p50 / p95) | about 130 MiB per space; 10 residents about 1.3 GiB, 100 residents about 12.7 GiB, more than all of perf-02's RAM | Qualified. Cost grows with the number of distinct spaces active in the window. |

**Is 500 ms reachable from disk on this hardware?** Not by anything measured. The best disk wake is about 0.65 s (option A), and about 320 to 370 ms of that is the reseed exchange, which is bound by the same first-touch cost. Reaching 500 ms would need that cost cut, for example by backing guest memory with huge pages or populating it before the restore. Neither was tried. Only option C (resident) is far inside the gate. A reasonable plan is C for spaces used in the last N minutes and A or B for the rest; A is worth building only if its 0.6 s matters.

Details: [Node startup after restore, and the refreshed worker](#node-startup-after-restore-and-the-refreshed-worker) at the end of this file.

| Wake boundary | Product XFS restored (ms, p50 / p95) | Prototype cached full (ms, p50 / p95) | Prototype historical dirty replay (ms, p50 / p95) |
| --- | ---: | ---: | ---: |
| Request through placement decision | 21.64 / 39.44 | — | — |
| Placement decision through machine gate entry | 4.11 / 7.88 | — | — |
| Machine placement gate | 0.02 / 0.03 | — | — |
| Manifest and local setup | 0.21 / 0.27 | — | — |
| Prepare private scratch and control drives | 3.10 / 8.75 | — | — |
| VMM startup through API availability | 23.29 / 30.23 | 1.36 / 4.85 | 3.37 / 3.38 |
| Snapshot load and file-backed memory mapping | 1.94 / 6.34 | 1.86 / 9.85 | 2.15 / 2.91 |
| Resume | 0.38 / 1.20 | 0.32 / 3.70 | 1.90 / 4.55 |
| Reseed exchange | 589.82 / 1,503.87 | — | 218.44 / 285.40 |
| Ready check or vsock attach | 19.85 / 30.09 | 27.42 / 36.49 | 34.72 / 66.75 |
| Start rootless package container | 1,139.39 / 1,792.72 | — | — |
| Admit first call before starting Node | 34.28 / 41.63 | — | — |
| Launch package process through worker module readiness | 1,028.52 / 2,953.04 | — | — |
| Send invocation through first callback | 47.76 / 105.00 | — | — |
| Finish invocation after callback | 3.26 / 9.26 | — | — |
| Check the restored worker cohort | — | — | 160.10 / 248.64 |
| Invoke and validate reply on the resumed worker | — | 165.73 / 233.46 | 34.97 / 105.52 |
| **Request through first validated reply** | **3,156.54 / 5,496.60** | **194.47 / 271.34** | **524.01 / 538.96** |

The product added three long intervals to the earlier prototype. It exchanges fresh entropy with the restored guest, creates a new rootless container, then starts Node and loads the worker bundle. The product's separate guest serial samples put the Podman `run` command at 1,105 ms p50 and 1,731 ms p95 inside its 1,139 / 1,793 ms Start exchange. Stages measured inside each product wake account for the timed interval with a 0.05 ms p95 residual. Stage percentiles are computed independently.

The product column uses 10 successful control wakes through the enrolled runner and a validated `item_head` reply. The prototype uses the original five-worker package sources and rebuilt guest image on the same perf VM. Its first validated reply is a setting callback. The cached full replay reproduced the earlier 269 ms p95 within 3 ms. The prototype has live package processes in memory and attaches vsock before its optional reseed exchange. The product reseeds the kernel first, checks guest readiness, and admits the fresh worker after restore. Both map the 256 MiB memory file and resume separately in this trace. Prototype files were explicitly primed. The product control used consecutive sleep and wake cycles with caches left intact. The replay uses the fixture's combined load and resume path for snapshot creation and splits it during timing.

The historical dirty replay uses the five-process checkpoint fixture and per-image warmup. All five materialized memory files had 100% measured residency before wake. Its 539 ms p95 compares with the earlier 469 ms result. The first, simpler dirty replay used one setting invocation before creating the diff and one after reseed and reached 673 / 1,204 ms. That result stays in the raw prototype log as a separate workload.

A follow-up with two package markers measured seven valid restores. From the Node launch request through the first JavaScript statement took 1,166 / 1,571 ms p50 / p95. The first statement through module readiness took 76 / 119 ms. The pre-entry interval includes Podman exec and guest scheduling, Node startup, loading and parsing the script, and delivery of the first marker to the host. The observable top-level initialization of this small package accounts for much less time. The follow-up stopped at seven samples when perf-01 was handed back for the notes gate.

The 49 successful wakes across the XFS control and remedy arms all recorded a reseed acknowledgement before Node launch and the first reply.

The product XFS control cloned a 1 GiB logical scratch drive with 3,633,152–20,004,864 bytes allocated and a 512-byte control drive with one 4 KiB block allocated. The measured preparations used `FICLONE`. They write metadata and share the allocated extents. The product and prototype mapped memory without a bulk copy during restore. In the ext4 control, preparation copied sparse scratch extents and the control file. The private scratch had 3,645,440–14,774,272 bytes allocated. Ext4 preparation was 68.62 ms p50 and 124.49 ms p95. The native XFS result shows that copy time does not explain the seconds in the product wake.

In the earlier 30-cycle XFS series, seven first attempts expired in the two-second restored Start exchange, before Node exec. Those failed attempts took 3,455 ms p50 including earlier wake stages and cleanup. Retrying the same checkpoint took another 3,316 ms p50 and produced a validated reply after 6,879 ms p50 across those seven cycles. The eighth failed after 2,058 ms in the combined reseed and ready deadline. Its retry exhausted the restore budget and validated a cold reply in 11,916 ms, or 13,973 ms across both attempts. The new product trace reproduced the Start timeout on XFS and ext4. The original reseed error names the combined deadline, so its logs cannot distinguish the entropy RPC from the following ready check.

### Cold kernel and guest init

| Cold boundary | Samples | p50 (ms) | p95 (ms) |
| --- | ---: | ---: | ---: |
| Arrival through placement and scratch allocation entry | 8 | 64.76 | 66.11 |
| Format new scratch ext4 | 8 | 20.43 | 21.16 |
| Launch VMM through vCPU start | 8 | 9.62 | 15.92 |
| Kernel entry through guest PID 1 | 8 | 1,229.32 | 1,528.43 |
| Guest PID 1 through control socket | 8 | 1,189.54 | 1,628.20 |
| Agent initialization inside the preceding interval | 8 | 911.89 | 980.35 |
| Three Podman scans inside agent initialization | 8 | 808.83 | 967.39 |
| Host ready poll after socket appears | 8 | 39.37 | 85.43 |
| Start package container | 8 | 162.69 | 171.23 |
| Start Node through first worker callback | 8 | 712.11 | 935.99 |
| **Arrival through first validated reply** | **8** | **3,597.44** | **3,904.53** |

The shipped Linux 6.1.188 kernel enables ext4, virtio-blk, virtio-vsock, virtio-mmio, KVM guest support and SELinux. Firecracker supplied no initrd and the kernel's built-in initramfs source is empty. The normal command line is `root=/dev/vda ro rootfstype=ext4 init=/init reboot=k panic=1 pci=off console=ttyS0 quiet calternal_small_guest=1`. Two diagnostic boots printed their kernel initcall timings with `initcall_debug loglevel=1`. Guest timestamps put the root mount at 978–1,018 ms and PID 1 at 1,023–1,065 ms. The largest logged individual calls included crypto registration (96–98 ms), loop initialization (64–102 ms) and conntrack setup (8–32 ms). The loop driver is a candidate for a smaller offline profile after checking all Podman filesystem paths. Cryptography, namespaces, cgroups, vsock, virtio-blk and ext4 are used by this path. The guest clock and host serial capture have separate origins, so the guest initcall times are explanatory substeps.

The guest-init trace measured `/proc` and `/sys` setup at 4 ms p50, runtime mounts at 13 ms, cgroup mounting at 3 ms, scratch mounting at 10 ms, control-device ownership and loopback together at 4 ms, cold reseed at 6 ms, and cgroup delegation and directory setup at 13 ms. The guest has no network device, TAP, slirp setup, or remote entropy wait. From delegated directories to the agent entering backend initialization took 314 ms p50, including process launch and reading the control drive. Podman ownership sweeps then spent 771 ms p50 in the first `ps`, and 20 ms and 19 ms in the next two. Skipping those scans on a proven fresh scratch disk reduced the guest-init interval to 331 ms p50. It raised the first container Start from 163 to 883 ms p50 by deferring Podman's first initialization. The end-to-end cold trial improved from 3,597 to 3,284 ms p50.

### Timing and evidence

Host markers used `CLOCK_REALTIME` on perf-01 for product and prototype. A host reader also timestamped complete serial lines as they arrived, polling at 2 ms. The guest kernel's own timing output is used only within its diagnostic clock. The product package emitted an explicit measurement marker after loading its module and installing the callback handler. The test keeps the production route, authority checks, and reseed-before-serve rule. The measurement overlay lives in disposable dev-01 source and guest copies. The tools and binaries were built on dev-01 and checksummed before the perf lock was used on perf-01.

Raw product traces and experimental controls are in `/srv/homes/perf/vm88/t/` and `/srv/homes/perf/vm88/te/`. The prototype replays are in `/srv/homes/perf/vm88/proto/` and `/srv/homes/perf/vm88/pd/`. The Node split is in `/srv/homes/perf/vm88/tn/node/`. The prior retry costs come from `/srv/homes/perf/vm88/measure_xfs_small_vm-repeat.log`. The checksummed drivers and summary tooling are in [perf/vm-trace](../../perf/vm-trace).

### Measured remedies

| Wake experiment | Successful restores / errors / cold fallbacks | Successful wake or resident call p50 / p95 (ms) | Effect on the changed stage |
| --- | ---: | ---: | --- |
| Traced product control | 10 / 0 / 0 | 3156.54 / 5496.60 | Current fork-per-reseed helper |
| 1 ms VMM API poll | 7 / 1 / 0 | 3357.24 / 4360.19 | VMM startup 8.73 / 18.09 ms |
| Use reseed acknowledgement and the existing Start guard | 7 / 1 / 0 | 3158.56 / 4185.00 | Ready handoff 0.03 / 0.05 ms |
| Read all 256 MiB before VMM startup | 8 / 0 / 0 | 3740.77 / 4976.02 | New synchronous read 1877.44 / 2599.70 ms |
| Keep the privileged reseed helper resident | 7 / 2 / 1 | 2920.72 / 4168.65 | Reseed 336.75 / 1079.42 ms |
| Faster poll, resident helper and inline readiness | 10 / 1 / 0 | 3006.61 / 4184.27 | VMM startup 3.93 / 4.43 ms and reseed 344.90 / 918.89 ms |
| Keep the same-space VM and its live worker resident | 10 / 0 / 0 | 5.13 / 17.00 | 140.52 / 161.03 MiB Firecracker PSS after the second validated call |

The poll change cut the API-ready observation delay by about 15 ms at the median. Faster polling trades additional socket checks for that time. The inline readiness experiment used the successful entropy acknowledgement and the existing independent `Start` check of the guest readiness marker. It removed a separate authenticated exchange and retained the reseed-before-serve ordering. The resident helper kept the same restricted Unix socket, accepted a fresh seed on each restore, and called the same kernel reseed operation before acknowledging. Its root process stays resident and needs an independent lifecycle review before production use. The combined path still had an unavailable attempt at restored Start.

Synchronous prefetch used up to 256 MiB of file cache per image and moved fault work earlier. Its reseed exchange was shorter in this arm, but the added read raised median total wake from 3.16 to 3.74 s. Both snapshot loaders already map memory. No tested product sleeping path reached 500 ms.

| Cold boot experiment | Valid boots | Kernel p50 / p95 (ms) | Guest init p50 / p95 (ms) | First reply p50 / p95 (ms) |
| --- | ---: | ---: | ---: | ---: |
| Traced default | 8 | 1,229 / 1,528 | 1,190 / 1,628 | 3,597 / 3,905 |
| Skip Podman ownership scans on proven fresh scratch | 8 | 1,204 / 1,391 | 331 / 492 | 3,284 / 3,512 |
| Suppress ordinary kernel serial logging with `loglevel=1` | 8 | 1,286 / 1,581 | 1,249 / 1,462 | 3,667 / 3,993 |
| Strip ELF debug sections from the same kernel | 8 | 1,371 / 1,454 | 1,163 / 1,547 | 3,587 / 3,995 |

Stripping debug sections removed 77,728 bytes and left the compiled configuration intact. Neither the logging change nor the stripped binary materially shortened cold boot in these samples. The experiment left a rebuilt driver-pruned kernel unqualified. Raw stripped-kernel samples are in `/srv/homes/perf/vm88/ts/stripped/`.

The resident arm used a second independently validated item-head call to a live worker in the same space. It reached 5.13 ms p50 and 17.00 ms p95. A prestart policy could acquire that placement before the request and carry the observed 141 MiB median PSS while it waits. This comparison measures one pinned warm placement, with 256 MiB configured guest RAM. It leaves cross-space pool admission and eviction unmeasured. The worker process stayed live between calls. Keeping a package process inside a restorable snapshot would also preserve its userspace RNG and private state. The historical prototype reseeded its long-lived broker, which does not establish a reseed contract for every package process. This experimental branch retains the product rule that package processes are absent from saved checkpoints.

Among the XFS remedies, the lowest observed successful wake p95 was the resident reseed helper at 4.17 s, with two errors and one cold fallback among ten requests. The separate ext4 series reached 2.22 / 3.53 s among eight restored replies, with three unavailable calls and one cold fallback in twelve attempts. The combined case produced 3.01 s p50 and 4.18 s p95, with one failed request. These are small, serial samples under normal filesystem caching. To reach the gate from disk, the remaining work is principally fresh Podman container creation and Node startup, with guest reseed still on the critical path. The restore mapping itself took about 2 ms at the median.

Perf-01 had 71% aggregate CPU idle and 1.08% CPU steal during the XFS traced control. The serial unloaded series had substantial aggregate CPU headroom. That leaves per-vCPU scheduling and nested KVM latency inside the measured stages and limits extrapolation to larger hosts. The concrete RAM limit appears under simultaneous compilation, where every four-, six-, and eight-build run hit the 1 GiB host headroom floor. A resident policy trades those milliseconds for pinned RAM and has to share that same budget.

## Plain plugin processes and resident density

The follow-up ran on perf-02 under the perf lock with Firecracker 1.16.1 and KVM. Perf-02 has four vCPUs and 7,879 MiB RAM with no swap. Its native `/srv/homes` XFS volume has reflink enabled. The small guests still had two vCPUs and 256 MiB configured RAM. The wake fixture checked the answer to `item_head` and a second call while the worker stayed live. Each sleeping trial saved an empty package process set. Each restored guest acknowledged a fresh authenticated reseed before package Start, and Start still checked the guest ready marker. A fresh guest reseeded at boot.

### Wake stages

| Stage, p50 / p95 (ms) | Podman (8) | Plain (30) | Fast, resident helper, selected prefill (30) | Fast, forked helper, natural cache (30) | Fast, resident helper, natural cache (30) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Arrival to placement | 23.33 / 34.44 | 22.83 / 25.59 | 22.08 / 29.46 | 22.54 / 30.18 | 22.46 / 30.39 |
| Machine placement gate | 0.02 / 0.02 | 0.02 / 0.03 | 0.02 / 0.03 | 0.02 / 0.02 | 0.02 / 0.03 |
| Prepare scratch and control drive | 1.73 / 6.83 | 0.89 / 1.46 | 0.87 / 1.15 | 0.87 / 1.28 | 0.94 / 1.21 |
| Snapshot cache preparation | — | — | 425.7 / 736.81 | 4.56 / 8.28 | 4.69 / 5.97 |
| Start VMM through API ready | 23.35 / 26.01 | 23.43 / 36.62 | 3.87 / 4.45 | 4.12 / 24.69 | 4.05 / 4.76 |
| Load snapshot | 2.15 / 2.96 | 2.07 / 4.25 | 1.76 / 2.44 | 1.99 / 2.7 | 1.85 / 4.89 |
| Resume vCPUs | 0.57 / 0.65 | 0.45 / 0.77 | 0.56 / 3.06 | 0.6 / 0.83 | 0.56 / 2 |
| Reseed through acknowledgement | 526.43 / 1,029.63 | 418.94 / 670.88 | 224.66 / 375.62 | 366.59 / 782.12 | 347.94 / 572.9 |
| Post-reseed readiness | 26.03 / 79.53 | 11.53 / 54.82 | 0.19 / 0.29 | 0.23 / 0.26 | 0.2 / 0.25 |
| Package Start | 1,235.55 / 1,796.18 | 18.25 / 43.53 | 27.31 / 66.02 | 39.37 / 99.35 | 30.55 / 84.76 |
| Start reply to Node launch request | 32.58 / 46.31 | 30.29 / 44.99 | 26.18 / 40.91 | 35.66 / 43.75 | 34.29 / 46.85 |
| Node launch to first statement | 916.38 / 1,117.82 | 873.28 / 1,315.26 | 884.14 / 1,253.15 | 966.31 / 1,351.58 | 935.12 / 1,545.91 |
| First statement through module ready | 68.37 / 101.28 | 65.45 / 100.6 | 66.47 / 116.29 | 85.96 / 362.69 | 85.77 / 134.39 |
| Node launch through module ready | 986.48 / 1,172.31 | 940.13 / 1,398.38 | 945.07 / 1,313.41 | 1,084.63 / 1,419.38 | 1,025.63 / 1,614.56 |
| Module ready to first callback | 58.17 / 98.97 | 71.42 / 198.27 | 93.46 / 166.24 | 60.07 / 148.04 | 90.54 / 233.45 |
| Callback to first reply | 2.89 / 3.37 | 2.7 / 3.81 | 2.8 / 3.49 | 2.79 / 4.79 | 2.79 / 6.72 |
| **Arrival to first validated reply** | 2,861.95 / 4,111.27 | 1,638.89 / 2,253.21 | 1,835.3 / 2,397 | 1,636.61 / 2,746.02 | 1,606.82 / 2,123.75 |

Every request in these arms returned the expected item after a restore. The plain Start round trip reduced the median from 1,236 ms with Podman to 18 ms. An earlier 12-wake Podman control put Start at 1,085 / 1,748 ms and first reply at 2,750 / 4,366 ms p50 / p95. Plain Node launch still took 873 ms at the median before the first statement, including exec dispatch and startup of the fresh process. Module initialization itself took 65 ms. The combined arms used a 1 ms VMM API poll and omitted the redundant readiness exchange after successful reseed. The independent Start guard remained in force.

The fastest 30-attempt arm used the plain launcher, resident helper, natural caching, faster poll and inline readiness. It reached 1,607 / 2,124 ms p50 / p95. It had zero unavailable requests and zero cold fallbacks. Its filesystem preparation cloned the sparse 1 GiB scratch image and prepared a 512-byte control drive. Firecracker mapped the existing snapshot memory file. The first statement still took 935 / 1,546 ms after the Node launch request. A second validated call to the live worker took 4.73 / 22.69 ms. The one-time wake still exceeds the 500 ms gate.

### What the plain launcher preserves

| Worker control | Plain-process experiment |
| --- | --- |
| Identity | Guest UID and GID 1100, separate from the trusted guest agent at UID 1000. Empty supplementary groups and capability set. |
| Resources | Dedicated cgroup v2 with 256 MiB memory, 64 PIDs and one CPU of quota. The guest has no swap. |
| Namespaces | Private mount, PID, network, IPC and UTS namespaces with a chroot into the prepared runtime. Private read-only proc with its own PID view and a small private device set. The worker root gives no access to guest data drives, agent sockets or provider credentials. |
| Files and syscalls | Read-only runtime and a read-only, noexec, nosuid and nodev package bind mount. `no_new_privs` and the native amd64 capability-empty policy exported from the existing worker seccomp profile. The Node permissions and 128 MiB heap option were kept. |
| Launch checks | Launcher calls require agent UID 1000 and a normalized placement UUID. Before executing Node the launcher checks UID, GID, namespace PID 1, `no_new_privs`, syscall denial for socket creation, and refusal to open package code for writing. |

The space microVM is the trust boundary. The experiment removes nested user-namespace mapping, OCI masked paths and any engine-applied labeling. It also removes the container monitor, separate PID 1 reaper and ownership inventory. The same-space guest control plane loses those layers of defense. The launcher adds a guest-root setuid entry point callable by the agent, with a fixed command, cgroup path and read-only root. The benchmark covered startup and sleep teardown of one worker process shape. It leaves arbitrary child-process behavior and the build guest backend unqualified. The amd64 filter excludes x86 and x32 compatibility syscalls. The image retained the installed Podman files from the common runtime fixture. The plain path invoked no Podman process.

### Reseed measurement

The host timed vsock connect, authentication and fresh seed request through acknowledgement. The guest separately timed the local helper socket and kernel reseed operation. VMM process major/minor faults and both vCPU threads' scheduler counters were sampled at snapshot load, resume and seed acknowledgement. Scheduler wait here is the sum of runnable wait observed by the perf VM. It excludes time spent blocked and cannot isolate individual pauses by the outer hypervisor. CPU steal is measured across the perf VM over each series.

Each cache condition used the current published memory generation. The cache probe sampled residency with `mincore`. It left the normal cache alone, advised dropping that image's cache pages, or requested `MAP_POPULATE` for all or selected pages after dropping cache. For the selected condition, the input was the present-page footprint of Firecracker's memory mapping at the end of a previous reseed. It includes agent and helper pages along with earlier guest kernel and vsock work. Readahead can make the observed warmed set larger than the selected list. The prefetch process pays the file cache work. Firecracker still maps its own memory and faults its own page tables. Timed preparation is included in first reply.

| Helper and cache state | Cached MiB, p50 | Prep p50 / p95 (ms) | Vsock connect p50 / p95 (ms) | Seed request through ack p50 / p95 (ms) | Whole reseed p50 / p95 (ms) | VMM major faults, p50 | First reply p50 / p95 (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Fork per reseed, natural | 10.2 | 5 / 10 | 114 / 223 | 274 / 333 | 396 / 557 | 74 | 1,635 / 1,984 |
| Resident helper, natural | 12.3 | 5 / 7 | 125 / 412 | 222 / 344 | 348 / 756 | 71 | 1,771 / 2,374 |
| Resident helper, evicted | 0 | 5 / 7 | 129 / 515 | 185 / 429 | 319 / 944 | 110 | 1,622 / 3,041 |
| Resident helper, full prefill | 256 | 1,138 / 1,657 | 82 / 135 | 235 / 341 | 312 / 418 | 0 | 2,439 / 2,713 |
| Resident helper, selected prefill | 49.9 | 445 / 656 | 65 / 108 | 179 / 266 | 261 / 331 | 5 | 1,863 / 2,315 |

Each cache arm returned eight valid restored replies without errors or fallbacks. The selected list contained 5,634 pages, covering 22.0 MiB. Readahead warmed 12,783 pages. Both prefills removed much of the reseed tail. Full prefill eliminated observed major faults between resume and acknowledgement and still left a 312 ms median exchange. Selected prefill lowered reseed p95 to 331 ms, but its preparation cost 445 ms at the median and 656 ms at p95. Keeping ordinary caching did less total work on the arrival path.

The cache conditions affected later work too. Node launch to its first statement was 1,017 ms p50 after eviction, 835 ms with the selected footprint, and 519 ms with all memory cached. The whole memory-file prefill cost more than it saved even with that improvement. The probe counts VMM faults over the stated intervals, including faults in files other than the saved guest RAM where they occur.

| Reseed interval at natural cache, p50 / p95 | Fast forked helper (30) | Fast resident helper (30) |
| --- | ---: | ---: |
| Resume to vsock connected (ms) | 119 / 179 | 140 / 297 |
| Host write of authentication proof (ms) | 0.064 / 0.105 | 0.065 / 0.087 |
| Seed request through acknowledgement (ms) | 263 / 624 | 203 / 339 |
| Guest agent's local helper exchange within that request (ms) | 68 / 254 | 9 / 22 |
| Open, write and issue kernel reseed with the random device, within the helper exchange (ms) | 1.34 / 2.38 | 2.37 / 9.63 |
| Seed request time outside the local helper exchange (ms) | 193 / 295 | 193 / 332 |
| Two vCPUs' runnable wait, resume to acknowledgement (ms) | 2.98 / 9.50 | 2.29 / 6.37 |
| Two vCPUs' summed runtime, resume to acknowledgement (ms) | 266 / 591 | 261 / 442 |
| **Whole reseed (ms)** | **367 / 782** | **348 / 573** |

The fork arm starts a new root helper through socat. The resident arm keeps that helper listening on the agent-only socket and performs the same kernel reseed before acknowledging. That saves about 59 ms p50 within the local exchange. Most remaining wall time is spent establishing the vsock connection and in the seed request outside the local helper exchange. Page faults contribute to the tail, as the eviction and prefill controls show. Runnable queue delay on perf-02 accounted for only a few milliseconds at the median. Blocked time and pauses at the outer hypervisor remain unresolved in these counters. The fastest arm saw 73% aggregate host CPU idle and 0.93% steal. The inner VM's inclusive vCPU runtime includes guest kernel work and time in KVM and can overlap across two vCPUs. The first cache-control guest's serial messages sometimes interleaved. The two natural-cache fast arms used single-write records and supplied all 60 local helper and kernel timings above.

### Cold first reply

| Fresh plain-process boot stage | p50 (ms) | p95 (ms) |
| --- | ---: | ---: |
| Arrival to scratch format entry | 67 | 74 |
| Format scratch | 20 | 22 |
| Start VMM through vCPU start | 29 | 32 |
| Kernel boot through guest PID 1 | 1,204 | 1,463 |
| Guest PID 1 through agent socket | 320 | 493 |
| Observe agent readiness | 54 | 110 |
| Start plain package | 8 | 13 |
| Node launch to first statement | 507 | 583 |
| Package module initialization | 72 | 83 |
| **Arrival to first validated reply** | **2,361** | **2,740** |

All ten fresh boots returned the expected item. This arm skipped the ownership scans on proven fresh scratch and took the plain path through Start. The kernel is the same as the previous cold experiment. The agent took 242 ms p50 to enter its backend from delegated directories. Cold reseed took 6 ms p50. The plain backend skipped the engine startup as well as the scan, so Start was 8 ms p50 instead of paying the deferred engine initialization.

### Resident memory measurement

The density probe increased the same population from one to four to sixteen independently booted VMs in each arm. Each guest held one plain Node worker idle on its input after reporting readiness. Both arms attached a virtio balloon at zero inflation. Only free-page reporting changed. The guest kernel has `CONFIG_VIRTIO_BALLOON=y` and `CONFIG_PAGE_REPORTING=y`. The probe waited seven seconds at each count then read each VMM's PSS and host MemAvailable three times, 500 ms apart. Reporting results are aggregate PSS differences at the same count. PSS prorates shared image pages and omits kernel allocations outside a VMM's address space. Spaces per GiB divides 1,024 MiB by mean PSS per guest at that count.

| Resident guests | PSS off (MiB) | PSS on (MiB) | PSS returned (MiB) | PSS/guest off (MiB) | MemAvailable used off / on (MiB) | Spaces/GiB off |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 132.59 | 132.54 | 0.05 | 132.59 | 134 / 143 | 7.72 |
| 4 | 517.02 | 516.83 | 0.19 | 129.26 | 536 / 530 | 7.92 |
| 16 | 1,994.53 | 1,993.99 | 0.54 | 124.66 | 2,044 / 2,019 | 8.21 |

All 16 guests started in both arms. The MemAvailable headroom at 16 was 1,635 MiB with reporting off and 1,659 MiB with reporting on. Reporting returned less than one aggregate MiB of PSS over the idle interval. The 25 MiB difference in host MemAvailable use also includes other host memory and page-cache changes between arms. This host began the density tests with about 3.6 GiB available, including the effect of about 3.2 GiB of unrelated shared memory. Reserve about 125–133 MiB of VMM PSS per live idle worker, or about eight of these spaces per GiB of available PSS budget. Reserve headroom for the host and package activity separately.

### Evidence

The probes are in [perf/vm-plain](../../perf/vm-plain). The disposable source copies used the same product revision, `bb34fcde`, and [perf/vm-trace](../../perf/vm-trace) instrumentation. The host fixture and small guest agent were built on dev-01. Image assembly replaced `/init` and the agent in a private ext4 copy and installed the launcher and binary filter under `/opt`. The launcher was installed root-owned with mode `04755`. Bundles were checksummed before transfer and verified under the perf lock. The wake and cold runs exercised the server, broker and small-VM placement path. The density test started idle package processes through the small-VM runtime and kept their streams open for the samples.

Raw evidence on perf-02 is rooted at `/srv/homes/perf/vm89/`. The same-host Podman baseline is in `podman/`, the plain wake and fresh cold runs are in `p/`, the cache controls are in `r/`, the natural-cache combined arms are in `s/`, and resident density is in `d/`. Each series contains its host and guest serial logs. Stage figures use same-host realtime markers with percentiles calculated per interval. Guest durations use the guest monotonic clock. Individual stage percentiles cannot be added to recover request percentiles.

## Simplification opportunities

Keep package process setup separate from the build guest's Podman lifecycle. One authenticated reseed acknowledgement plus the Start readiness guard is sufficient for the measured path. Warming snapshot pages synchronously adds request work on this host. The fresh Node process remains the largest wake stage.

## Exact snapshot prefill and the Node startup entropy check

| Wake stage, p50 / p95 (ms) | Evicted, traced (10) | Natural cache, traced (10) | Exact prefill, traced (10) | Exact prefill, no kernel trace (30) |
| --- | ---: | ---: | ---: | ---: |
| Request arrival to placement | 23.51 / 27.25 | 22.5 / 49.52 | 22.21 / 27.95 | 26.37 / 34.26 |
| Machine placement gate | 0.02 / 0.03 | 0.02 / 0.03 | 0.02 / 0.03 | 0.02 / 0.04 |
| Prepare scratch and control drive | 1.02 / 1.4 | 1 / 1.3 | 0.93 / 1.49 | 1.03 / 1.35 |
| Inspect or populate snapshot cache | 54.91 / 83.15 | 52.65 / 83 | 839.22 / 1,069.73 | 885.92 / 1,429.17 |
| Start VMM to API ready | 4.27 / 97.4 | 4.11 / 17.1 | 4.23 / 19.97 | 4.17 / 4.96 |
| Load snapshot | 4.32 / 75.42 | 1.91 / 11.1 | 3.56 / 38.6 | 3.62 / 17.28 |
| Resume, including filter attachment when traced | 115.53 / 125.81 | 101.86 / 168.64 | 101.93 / 122.97 | 0.58 / 1.03 |
| Reseed through acknowledgement | 355.75 / 1,014.03 | 348.71 / 1,467.82 | 302.69 / 453.15 | 272.38 / 448.21 |
| Inline readiness | 0.24 / 0.27 | 0.21 / 0.26 | 0.22 / 0.25 | 0.21 / 0.26 |
| Package Start | 39.19 / 75.62 | 28.99 / 55.98 | 35.31 / 68.87 | 31.3 / 110.46 |
| Node launch to first statement | 845.47 / 1,430.02 | 924.8 / 1,371.94 | 795.94 / 1,163.52 | 862.12 / 1,191.88 |
| First statement through module ready | 62.05 / 83.64 | 71.78 / 113.68 | 56.12 / 71.12 | 64.72 / 93.44 |
| First callback to first reply | 2.9 / 3.8 | 3.14 / 3.85 | 3.42 / 3.95 | 3.1 / 4.25 |
| **Arrival to first validated reply** | 1,582.27 / 3,064.87 | 1,665.82 / 3,285.57 | 2,371.99 / 2,891.77 | 2,351.87 / 3,138.37 |

All arms used fresh workers and the fastest configuration from the previous round, including the resident reseed helper, short API poll and inline readiness. All 30 traced wakes, the training wake and all 30 extra exact-prefill wakes returned the expected item with zero unavailable replies or cold fallbacks. Trace filter attachment added about 100–116 ms at the median in the traced resume row. The cache row includes the Python inspection and optional prefill. These timings use a 256 MiB, two-vCPU guest on perf-02. Package admission still follows authenticated reseed acknowledgement.

### Fault attribution and targeted prefill

| Host fault measures, p50 / p95 | Evicted | Natural cache | Exact prefill |
| --- | ---: | ---: | ---: |
| Reseed major faults, count | 384 / 405 | 261 / 307 | 273.5 / 302 |
| Reseed major fault time, ms | 15.57 / 26.03 | 12.63 / 15.41 | 14.64 / 15.86 |
| Reseed all fault time, ms | 163.74 / 415.52 | 129.83 / 1,048.66 | 119.11 / 230.53 |
| Node launch major faults, count | 501.5 / 514 | 421.5 / 440 | 485.5 / 521 |
| Node launch major fault time, ms | 16.93 / 18.29 | 17.54 / 20.5 | 21.72 / 24.51 |
| Node launch all fault time, ms | 42.4 / 56.8 | 42.98 / 54.49 | 46.96 / 71.88 |

The trace pairs entry and return from Linux `handle_mm_fault` in both Firecracker vCPU threads. It includes only addresses within the saved RAM file mapping. Major counts use the kernel's `VM_FAULT_MAJOR` return bit. Fault wall time is the union of measured intervals across the two threads, clipped to the stage bounds. It includes time descheduled while in the handler, and it cannot attribute delay outside the handler or identify which guest files occupy those guest-physical pages. The trace buffers had zero overruns and dropped records. The stage table uses host realtime markers aligned with monotonic trace timestamps. Each 10-wake p95 is the maximum of that arm. [Linux fault return flags](https://github.com/torvalds/linux/blob/v6.12/include/linux/mm_types.h#L1163-L1203)

The training wake started with zero memory-file cache pages. Its direct fault addresses yielded 6,636 unique 4 KiB offsets from snapshot load through first reply, including reseed and Node launch. The exact prefill used `MADV_RANDOM` and `MADV_POPULATE_READ` on only contiguous ranges in that list. `mincore` confirmed that all 6,636 offsets were cached before load in each exact-prefill trial, with zero extra pages. Direct fault addresses avoid treating speculative readahead as demand. The set was transferred from one wake to subsequent generations of the same guest. It covered a median 45% of the later reseed fault pages and 51% of the Node launch fault pages, so it cannot serve as a fixed list for all wakes.

Node launch spent about 43–47 ms p50 in the host snapshot fault handler across these arms. Major faults contributed about 17–22 ms. This leaves most of the roughly 0.8–0.9 s launch latency outside that handler. Reseed spent 119–164 ms p50 in the handler, including 13–16 ms returning a major fault. The broad fault path is a material part of reseed wall time. Its time includes page-table work and waiting in the kernel alongside backing-file reads. Exact prefill brought reseed p95 down to 453 ms in this run and added 839 ms p50 of synchronous preparation. The ordinary active-cache path kept the shorter median first reply.

Without the kernel trace attached, exact prefill took 2,352 / 3,138 ms to first reply, including 886 / 1,429 ms of cache preparation. All 30 trials again had exactly 6,636 cached pages before load and zero extra pages. The fastest single wake in this arm took 1,405 ms. The earlier 30-wake natural-cache arm remains the best measured on perf-02 at 1,607 / 2,124 ms p50 / p95. The 500 ms disk-wake gate remains unmet.

### Why a prestarted worker fails the entropy requirement

The exact guest Node binary and musl runtime were extracted from the checksummed guest root, then traced on perf-02 under the lock. Node was v24.22.3 with OpenSSL 3.5.9. The worker loaded `node:crypto` and `node:tls`, wrote a checkpoint marker, then blocked on input. No package cryptographic request ran until that input was released. In each of four startup traces, `getrandom(..., 48, 0)` succeeded before the worker's checkpoint marker. A separate forwarding `LD_PRELOAD` counter identified the caller as OpenSSL's `ossl_pool_acquire_entropy` calling musl `getentropy`. The counter records only lengths and symbol names. The unmodified strace runs independently confirm ordering.

| Startup option | OpenSSL bytes requested from the kernel before checkpoint | Startup satisfies the prestart requirement |
| --- | ---: | --- |
| Ordinary worker flags | 48 | No |
| `--predictable --random-seed=1` | 48 | No |
| `--no-node-snapshot` | 48 | No |
| `--openssl-config=/dev/null` | 48 | No |

Stock Node initializes OpenSSL and calls `CSPRNG(nullptr, 0)` in `InitializeOncePerProcessInternal`, before initializing V8 and before loading any JavaScript module. That routine calls `RAND_status` and `RAND_bytes_ex`. A later flag-controlled V8 seed choice leaves this earlier startup step in place. The internal embedding option `kNoInitOpenSSL` changes initialization and is outside the worker CLI. A checkpoint before Node starts avoids copying its initialized userspace cryptographic state, and pays for fresh Node startup on every restore. [Node startup source](https://github.com/nodejs/node/blob/v24.22.3/src/node.cc#L1117-L1184) · [Node CSPRNG implementation](https://github.com/nodejs/node/blob/v24.22.3/deps/ncrypto/ncrypto.cc#L755-L789)

The candidate failed before snapshot creation. There is therefore no qualifying prestarted-worker wake series and no paired restore evidence for `crypto.randomBytes`, `crypto.randomUUID` or a TLS ClientHello. The stock kernel reseed cannot be taken as proof that already-initialized userspace DRBG state has refreshed. OpenSSL's documented implementation checks its process fork marker, reseed counters and time bounds. A restored process retains the saved in-process state. [OpenSSL DRBG generation path](https://github.com/nodejs/node/blob/v24.22.3/deps/openssl/openssl/providers/implementations/rands/drbg.c#L578-L647)

| State a process snapshot would duplicate | Effect for a worker that has served nothing |
| --- | --- |
| OpenSSL DRBG state and any prefilled entropy-derived buffer | The worker can carry cryptographic state into its first request. This disqualifies the tested checkpoint. |
| V8 hash seed and existing hash tables | An identical internal layout is mostly invisible to a normal handler. It removes per-instance hash diversity and supplies no fresh identity. |
| `Math.random` state and cached values, if used | Following the same execution can repeat values. Even a context which has never called it draws its first seed from V8's isolate RNG unless a fixed seed is supplied. It matters for application sampling or identifiers that expect independent instances. |
| Guest process and thread IDs | These are namespace-local and hold no global identity. A snapshot does not introduce a process fork event. |
| Timer objects, deadlines and runtime uptime state | A blocked reader can remain idle. Package-created timers can expire relative to the restored guest clock and trigger side effects after release. |
| Loaded modules and package heap state | Immutable definitions are useful to preserve. Initialization may already have filled caches, allocated identifiers, or captured credentials or time before the first request. The fixture exercised only the narrow blocked-worker shape. |

[V8's lazy Math.random initialization](https://github.com/nodejs/node/blob/v24.22.3/deps/v8/src/numbers/math-random.cc#L14-L62)

### Resident-window cost

Keep spaces with activity in the last N minutes resident. The tested live-worker call took 4.73 / 22.69 ms p50 / p95. Idle spaces use the qualified fresh-worker disk path, measured at 1,607 / 2,124 ms on perf-02. At about 130 MiB VMM PSS per idle resident space, if 10% of spaces saw an operation in the last N minutes, 100 spaces need 10 residents or about 1,300 MiB (1.27 GiB). A fleet of 1,000 needs 100 residents or about 13,000 MiB (12.70 GiB). The second case exceeds perf-02's entire 7.7 GiB of RAM before workload or OS headroom. For example, with a ten-minute window and a single simultaneous cohort of activity, those ten and one hundred residents consume 13,000 and 130,000 MiB-minutes respectively until they age out. The distinct space count inside N determines concurrent memory use.

### Raw evidence

Perf-02 retained `/srv/homes/perf/vm90/f/` with the fault events, paired JSON, memory footprint, clock alignment samples and host/serial logs. The independent `strace` logs are under `/srv/homes/perf/vm90/e/` and the caller counter logs are under `/srv/homes/perf/vm90/e/counts/`. The fixtures and counter were built on dev-01 and checksummed before the locked measurements. The source overlays and tools are in [perf/vm-replay](../../perf/vm-replay).

## Simplification opportunities

Keep fresh-worker disk wake as the qualified path and use residency for spaces with recent traffic. The targeted read has measurable synchronous cost and poor coverage of later faulting pages. Trace the fresh Node launch outside the host fault handler before attributing its remaining latency to the backing file.

## Node startup after restore, and the refreshed worker

All numbers here come from perf-02 under the perf lock, Firecracker 1.16.1, the same 256 MiB two-vCPU guest and the exact guest Node (v24.22.3, OpenSSL 3.5.9; the host copy is byte-identical, sha256 `7e027679...`). The host is itself a KVM guest (AMD Ryzen 7 3700X under another hypervisor), so Firecracker runs nested. The harness is [perf/vm-final](../../perf/vm-final) (`bench.py`, `kvm-trace.py`, `summarize.py`). It exercises the guest and VMM, not the full server: placement (about 22 ms in the earlier tables) and the package Start exchange (about 30 ms) are not in it. Every restored arm drops the snapshot memory file from the host page cache first and checks with `mincore` that zero pages remain.

### Where Node's time goes

| Node launch to first statement | p50 / p95 (ms) | Minor / major faults, p50 | Guest CPU time (user + system), p50 |
| --- | ---: | ---: | ---: |
| (a) perf-02 host, direct (30) | 20.1 / 22.9 | 4,470 / 0 | 31 ms |
| (b) freshly cold-booted guest, up 60 s, nothing restored (30) | 20.0 / 24.5 | 4,134 / 0 | 30 ms |
| (c) restored guest (30) | 877.0 / 1,191.2 | 4,127 / 0 | 731 ms |

The first launch in each of (a) and (b) was cold-cache and slow (617 and 872 ms); the figures above are over all 30 including that one. No arm read from a disk inside the guest (`fsRead` 0). In (c) the binary was already in the guest page cache (11,670 of 32,350 pages of `node`, as in (b) after its runs).

Nested virtualisation is therefore not the cost for a running guest: (a) and (b) agree. In (c) the same work takes about 24 times the CPU time, in user mode as well as in the kernel, which points to the vCPU stalling on memory. The KVM exit trace (`kvm_exit` and `kvm_entry`, five launches per arm; handling time is the gap to the next entry, summed over both vCPUs, so it can exceed wall time) shows where:

| KVM exits during one launch call, median of 5 | Settled cold guest | Restored guest |
| --- | ---: | ---: |
| Launch to first statement (ms) | 20 | 960 |
| All exits | 1,122 | 20,665 |
| Nested page faults (`npf`) | 100 | 12,432 |
| Time handling `npf` (ms, both vCPUs) | 0.7 | 1,935 |
| `npf` time per exit (µs) | 7 | 156 |

Cause: after a restore the guest's RAM is a private mapping of the snapshot file. Each first touch of a 4 KiB guest page is an exit, a host page fault and a mapping update, about 156 µs each here. Node touches about 12,000 distinct pages, so that is most of the 0.9 s. The earlier fault-handler trace saw only 43 to 47 ms inside the host's fault handler because most of the exit cost falls outside it. This also explains why filling the host page cache did not help in the earlier prefill runs. The per-fault price is probably inflated by nesting, but a bare-metal figure was not measured. Source of the exit table: `/srv/homes/perf/vm-final/trace/kvm-trace.jsonl`.

### The refreshed worker

The launcher parks `node --permission` running `standby.cjs` before the snapshot. At wake the host restores, sends the authenticated kernel reseed and waits for the acknowledgement. Only after that it releases the worker. The release runs [reseed-addon.c](../../perf/vm-final/reseed-addon.c), loaded by the launcher, which finds three symbols in the Node executable with `dlsym(RTLD_DEFAULT)` and calls, with NULL library context, no entropy and no additional input:

- `RAND_get0_primary`, `RAND_get0_public`, `RAND_get0_private`, then
- `EVP_RAND_reseed(rng, 1, NULL, 0, NULL, 0)` on each (prediction resistance 1, so the generator draws fresh entropy from the operating system).

Node's statically linked OpenSSL does export these to an addon: `dladdr` places `EVP_RAND_reseed` in `/usr/local/bin/node`, and the addon links no libcrypto. The launcher then calls `crypto.randomUUID()` 128 times to flush Node's UUID cache (a batch of 128) and only then loads `/plugin/worker.js`, so everything the plugin does at load time is fresh. Release is refused with `BLOCKED` until the guest's ready marker exists, and the independent Start guard stays in force. (Second round: the launcher also replaces `Math.random` and seeds V8; see the next section.)

Proof, from two restores of the same standby snapshot (memory file sha256 identical before and after, `b0ade8e0...`). Every value below is a SHA-256 of the value, truncated:

| Value | Restore 1 | Restore 2 | Differ |
| --- | --- | --- | --- |
| `crypto.randomBytes(32)` | `c20b2628bf7c` | `e09efe5f5cf2` | yes |
| `crypto.randomUUID()` (after the flush) | `21ce8d365688` | `bbb15c450199` | yes |
| `crypto.getRandomValues` | `55bff7706984` | `d626a7759d37` | yes |
| Ed25519 public key | `0390129c64ad` | `9cdc43a9e5f0` | yes |
| TLS ClientHello record | `6996a0f3d111` | `4a4c1c19119b` | yes |
| ClientHello `random` field (32 bytes) | `d7762d64800c` | `4242d90793c1` | yes |
| Plugin's load-time call id (UUID) | `e44ccd10-16b9-43...` | `420a9e93-2e25-44...` | yes |

A forwarding `LD_PRELOAD` counter (lengths and caller symbol only) logged `getentropy` calls from `ossl_pool_acquire_entropy`. The 48-byte read happens before the snapshot, at Node startup, as found earlier. In both restores exactly three 32-byte reads occurred between `NATIVE_RESEED_BEGIN` and `NATIVE_RESEED_END`, which are written only after the release, and the host checks that the kernel acknowledgement came before the release acknowledgement in every one of the 30 wakes (acknowledgement to refresh: 491.5 / 563.3 ms p50 / p95, of which 66.3 / 88.5 ms in the reseed call and 26.5 / 79.8 ms flushing the UUID cache; the rest is the test shell channel).

What stays identical across restores, as the same two restores showed: the UUID drawn before the snapshot (`savedUUID` equal), `Math.random` (the next value after restore was `0.9612162602812918` in both) and the process id (1 in its namespace). By construction V8's hash seed, timers and anything a module captured before the snapshot are the same. For an idle worker that served nothing, and with plugin code loaded after release, the realistic hazards are:

| Identical state | Could it hurt a plugin author? |
| --- | --- |
| `Math.random` sequence | Yes, if a plugin uses it for ids, sampling or jitter: two wakes of one snapshot produce the same values. Closed in the second round: the launcher replaces `Math.random` before loading the plugin. |
| V8 hash seed | Mostly invisible; only affects object and hash table layout and hash-flooding resistance per instance. |
| Pid and thread ids | No: namespace-local, not identities. |
| Timers and clocks | Not for a worker that only waits. Timers created by the launcher before the snapshot are the launcher's own. |
| Module state captured at load | Not for plugin code, which loads after release. It matters for the launcher's own dependencies (`node:crypto`, `node:tls`), which cached nothing found by this test except the UUID batch. |

The two checks listed as open here, worker-thread generators and the addon permission drop, were run in the second round (next section). The drop check was wrong in the first round: see there.

### Wake stages, 30 wakes

Same snapshot, each wake from a restored VM with the cache dropped. Compared arm: a fresh Node process started in the restored guest (30 wakes).

| Stage, p50 / p95 (ms) | Fresh Node after restore | Refreshed worker, released after ack |
| --- | ---: | ---: |
| Inspect and drop snapshot cache | 76.1 / 78.5 | 13.8 / 25.8 |
| VMM start through API ready | 1.4 / 1.5 | 1.4 / 2.5 |
| Load snapshot | 4.2 / 26.6 | 4.0 / 4.7 |
| Resume | 0.2 / 0.3 | 0.2 / 0.7 |
| Reseed through acknowledgement | 360.6 / 613.3 | 316.9 / 367.2 |
| Arrival through kernel acknowledgement | 435.7 / 661.6 | 339.3 / 398.4 |
| Launch Node to first statement | 877.0 / 1,191.2 | not needed |
| Generator refresh and UUID flush, in process | not needed | 92.8 / 168.3 (sum of the two medians and p95s above) |
| Package module load, in process | 65 (earlier table) | 47.5 / 66.9 |
| Acknowledgement through worker ready, as measured | not measured | 546.3 / 643.7 (incl. about 400 ms shell channel) |
| **Arrival through worker ready (harness)** | not measured | **889.7 / 994.9** |

Errors: 0 of 30 refreshed-worker wakes, 0 of 30 restored fresh-Node wakes, no cold fallback (the harness has none). The harness's own first-callback and reply steps went through a shell channel that cost about 500 ms per call even for an idle guest, so the arrival-to-reply figure is not reported. Treating the reseed exchange and the in-process refresh as they are, and adding the earlier tables' placement (22 ms), Start (30 ms), first callback (about 60 to 90 ms) and reply (3 ms), the refreshed-worker wake would be about 0.6 s; the fresh-Node wake measured 1.61 s in the product path. These two sums are estimates, not a measurement of either.

### Raw evidence

On perf-02 under `/srv/homes/perf/vm-final/`: `full/samples.jsonl` (all arms, 125 records), `full/run.log`, `trace/kvm-trace.jsonl`, `smoke-ok/`, and `full-attempt1/`, a first full run that stopped in the restored arm when the harness's own cache-eviction check found resident pages; the check now retries, never skips. The prepared image (`guest/image.sha256`, root.ext4 `87962b52...`, vmlinux `f5735774...`) was patched twice after the first smoke runs: the launcher's addon-drop assertion was replaced by a logged probe, and in-process timings were added. The image and the reseed addon were built on dev-01 and checksummed before transfer. The addon in the image was not rebuilt from the committed source in this lane, so its identity rests on the previous agent's build log and `SHA256SUMS`.

## Closing the randomness gaps in the pre-started worker (second round, 2026-11-20)

Same host, guest, kernel and Node as above. Harness: [perf/vm-final](../../perf/vm-final). Two launcher variants were parked and snapshotted: **ship**, the designed launcher, and **wide**, a probe that also allows worker threads and child processes and warms the libuv pool before the snapshot (the worst case for per-thread generators). Each variant was restored twice from one snapshot, with the memory file unchanged before and after (ship `b360c31a...`).

### Math.random and other V8 seeds

After the OpenSSL reseed the launcher (1) discards the 128-UUID batch, (2) calls `v8.setFlagsFromString('--random-seed=<31 fresh bits>')`, which V8 reads when a context first draws, so contexts created later start from a fresh seed, and (3) replaces `Math.random` with a function that fills a buffer from `crypto.getRandomValues` and builds 53-bit doubles. All of it runs before any plugin code is loaded; in-process it costs 16.3 / 34.5 ms p50 / p95 (30 wakes). The main context's original generator was already seeded before the snapshot, so the flag alone could not fix it, which is why it is replaced.

| Observed across two restores of one snapshot | Restore 1 | Restore 2 |
| --- | --- | --- |
| `Math.random`, four draws, hashed | `be7eaf13e980` | `f22dc15d0f64` |
| First `Math.random` in a new `vm` context, hashed | `e3c1de3b7cb6` | `d94eff8c6226` |
| Two `vm` contexts created in one wake give the same value | yes | yes |
| `Math.random` drawn before the snapshot (saved state) | `0.8455718003335847` | `0.8455718003335847` |

What cannot be made fresh, and what a plugin author could notice:

| Item | Fresh? | Visible to a plugin? |
| --- | --- | --- |
| V8 hash seed and identity hashes | No; fixed when the isolate is created, and `--hash-seed` is read only at startup | No: object and `Map` key order is insertion order. Only a timing side channel could tell. |
| The original native `Math.random` function | Gone from the main realm; a function that prints as non-native replaces it | Only by `Math.random.toString()` or identity checks. |
| Two `vm` contexts in one wake | Each wake's seed is fresh, but all new contexts of that wake share it, so they produce the same sequence | A plugin that makes several `vm` contexts and compares their `Math.random` values. The seed has 31 bits, so about 46,000 wakes of one snapshot are needed before two share it. |
| Node internals that captured the original `Math.random` | No | Not by plugins. A search of Node's current source found only the virtual filesystem's temporary names and the test runner's seed. This was not checked on 24.22.3. |
| Worker isolates, if workers were ever allowed | Fresh per wake (measured, one worker per restore). They should take the same flag seed, so workers of one wake would share a sequence (inferred, not measured) | Not reachable: workers are refused (below). |
| Timers, process ids, anything a module captured before the snapshot | No | Not for plugin code, which loads after release. The launcher's own state is the only thing captured. |

### Worker threads, pool threads and child processes

Node's idle process has the threads `MainThread`, `DelayedTaskSche`, `SignalInspector` and four `V8Worker`; none runs crypto. libuv pool threads (`libuv-worker`) appear on the first asynchronous crypto, file system, DNS or zlib call. In the ship launcher none existed before the snapshot, checked from outside the worker through its cgroup (the harness fails the arm otherwise). In the wide variant the pool had four threads and had served 16 asynchronous `randomBytes` and a key generation before the snapshot. After restore, reseed and refresh, in both restores:

| Generator | Restore 1 vs restore 2 |
| --- | --- |
| 16 concurrent asynchronous `randomBytes(32)` on the warmed pool | all 16 distinct within each restore; no value shared between restores |
| 4 concurrent asynchronous Ed25519 key generations | all distinct; none shared |
| `randomBytes`, `randomUUID` and two `Math.random` draws in a new worker thread | all differ |
| Child process | not startable: `spawnSync` returned `EPERM` even with `--allow-child-process`, because the launcher's seccomp filter refuses process creation. A child would also be a fresh `exec`, but this was not measured. |

So OpenSSL's per-thread generators did pick up the primary's reseed, as its reseed counter propagation predicts, even for pool threads that had already generated. The launcher rule below still forbids pool threads before the snapshot as defence in depth.

### The native-module restriction

The first-round check was wrong. Node's permission scope is `addon`. `process.permission.has('addons')` is always false and `drop('addons')` is accepted and does nothing, so the launcher's own `--allow-addons` stayed in force after the drop, and plugin code could still have loaded any readable `.node` file (the package directory is readable). The earlier `ERR_DLOPEN_FAILED` came from a shared object that was not an addon, loaded because the permission was still granted. With `process.permission.drop('addon')` called right after the launcher loads its module, every restore of both variants showed:

| Attempt after the drop, in the ship launcher | Result |
| --- | --- |
| `require()` of a second copy of the addon | `ERR_ACCESS_DENIED` |
| `process.dlopen` of that copy | `ERR_ACCESS_DENIED` |
| `new Worker` | `ERR_ACCESS_DENIED` |
| `child_process.spawnSync` | `ERR_ACCESS_DENIED` |
| `fs.writeFileSync` | `ERR_ACCESS_DENIED` |
| `process.binding('natives')` | `ERR_ACCESS_DENIED` |

The launcher asserts all six in the ship variant, so a build that loses a restriction fails its proof. The drop is process state saved in the snapshot. The product's normal worker flags never include `--allow-addons`; only the parked launcher needs it.

### Rebuilding the reseed module

`nix develop .#deploy -c perf/vm-final/build-addon.sh <dir>` builds `reseed.node` from `reseed-addon.c` with the dev shell's musl compiler and Node's headers. `perf/vm-final/update-image.sh <image-dir> <work-dir>` rebuilds the module and the launcher and installs them into a copy of the guest image. Two builds on dev-01 were identical (`0f63170a...`), and equal to the module that the first round shipped, so that module did come from this source. The image tested here (root.ext4 `51ad1fa7...`) holds the rebuilt module.

### Wake stages with the closed launcher, 30 wakes

| Stage, p50 / p95 (ms) | Refreshed worker, second round |
| --- | ---: |
| Snapshot cache inspection | 14.8 / 26.5 |
| VMM start through API ready | 1.5 / 2.6 |
| Load snapshot | 4.1 / 5.2 |
| Resume | 0.2 / 0.7 |
| Reseed through acknowledgement | 369.0 / 409.2 |
| Arrival through kernel acknowledgement | 392.9 / 431.3 |
| In-process OpenSSL reseed | 87.5 / 98.4 |
| Flush UUID cache | 13.9 / 31.4 |
| V8 seed and `Math.random` replacement | 16.3 / 34.5 |
| Package module load, in process | 65.2 / 105.0 |
| **Arrival through worker ready (harness)** | **984.7 / 1,096.7** |

0 errors in 30 wakes, no fallbacks. The in-process work is 124 / 142 ms p50 / p95 before the package loads, against 93 / 168 ms in the first round; the median reseed exchange this run was slower (369 against 317 ms), within the run-to-run spread of this shared host. The harness still reaches the guest through a shell channel that costs about 400 ms of the 985, so the product-path estimate moves from about 0.6 s to about 0.65 s.

### Launcher rules that make it safe

1. The worker is started only through the trusted launcher module, with `--permission`, `--allow-addons` (dropped immediately), `--allow-fs-read` for the launcher directory and the package directory only, and `--disallow-code-generation-from-strings`. Never `--allow-worker`, `--allow-child-process`, `--allow-fs-write`, `--allow-net`, `--allow-wasi` or the inspector.
2. The launcher's first statements load the reseed module and call `process.permission.drop('addon')`, spelled that way, then assert `process.permission.has('addon') === false`, all before the snapshot. In this fixture the six denials above are asserted after each restore; a product launcher should also run them once before the snapshot.
3. No threads beyond Node's fixed set before the snapshot: the launcher does no asynchronous crypto, file system, DNS or zlib work, and the host side lists the worker's thread names from outside and refuses to snapshot if any `libuv-worker` exists.
4. The launcher's seccomp filter, which refuses process creation and sockets, stays in place as the second layer.
5. The release line reaches only the launcher, and only after the authenticated kernel reseed acknowledgement; release is refused until the guest ready marker exists. The independent Start guard stays in force.
6. After release and before any plugin code: reseed all three OpenSSL generators with prediction resistance, flush the 128-UUID batch, set a fresh V8 random seed from the secure source, replace `Math.random`. Then load the plugin.
7. The reseed module in the image is built from the committed source with `build-addon.sh`, and its checksum is recorded with the image.

### Raw evidence, second round

Under `/srv/homes/perf/vm-final/r3/` on perf-02: `full/samples.jsonl` (the proofs, thread lists and 30 wakes), `full/*.serial`, `smoke-failed/` (a first attempt that stopped because a child process cannot be started), and `../r2/` (an earlier attempt that stopped on a 10 s harness timeout in the wide probe). The probe that found the `addon` scope spelling is `/srv/homes/perf/vm-final/probe/`.


## Simplification opportunities

The test shell channel in the guest costs about 500 ms per call after a restore; the product agent socket should be used for any further wake measurement. The refresh and the plugin-load order are one launcher concern and could be one function. The cache-eviction check and the retry could move into a shared helper used by all `perf/vm-*` harnesses.
