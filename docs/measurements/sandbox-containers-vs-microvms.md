
# Rootless containers and microVMs: per-thread sandbox measurements

Measured 2026-11-17 on `calternal-perf-01-backup01`. Measurement and report only. No technology selection is made. Raw results and scripts are retained on that VM under `~/calternal-ai-sandbox-measure`; the machine-readable summary is [sandbox-containers-vs-microvms.json](sandbox-containers-vs-microvms.json). perf-02 was not used.

Markdown is the human-readable report. The requested JSON companion preserves numeric quantiles, per-thread observations, tool metadata, coverage and cleanup records for machine processing.


## Machine and scope

Debian 13.7; Linux `6.12.115+deb13.1-amd64`; four exposed CPU cores, AMD Ryzen 7 3700X model; 7.70 GiB visible RAM, no swap. Host filesystem: ext4 on `/dev/sda1`, itself a virtual disk. KVM hypervisor, AMD `svm` and nested paging `npt` exposed; `/dev/kvm` usable. `/sys/module/kvm_amd/parameters/nested` is recorded in metadata. This is a VM on Proxmox: Firecracker is nested virtualization. No bare-metal comparison was run.

All benchmark and workload builds ran on perf-01 under `/usr/local/bin/perf-lock.sh`, whose two exclusive locks cover `/var/lock/calternal-perf.lock` and `/tmp/calternal-build-slot.1`. Nothing was built or measured on hd01. `uidmap` and its Debian dependency were installed for rootless UID mapping, prebuilt tools were obtained with Nix, and `kayg` was added to the `kvm` group. No host-global firewall, forwarding or swap configuration was changed.


### Tool versions


| Tool | Version / source |
| --- | --- |
| Podman | 6.1.4, Nix |
| crun | 1.30.1, Nix |
| runc | runc version 1.5.2 |
| Firecracker | 1.16.1, Nix; default seccomp, no jailer |
| Guest kernel | 6.1.188+, Firecracker CI prebuilt image |
| CRIU | 4.1.1, Nix |
| slirp4netns | 1.3.7, Nix (microVM network test only) |
| pasta (passt) | 2026_07_16.090d739, Nix (container network test only) |
| Rust / Cargo | 1.99.1, repository pinned rust-overlay toolchain |
| Node | 24.21.0 |
| git / ripgrep | 2.55.0 / 15.2.0 |
| e2fsprogs / BusyBox | 1.47.4 / 1.37.0 |
| Nix | nix (Determinate Nix 3.24.1) 2.35.3 |
| Python | Python 3.13.5 |
| nftables | nftables v1.1.3 (Commodore Bullmoose #4) |
| iproute2 | ip utility, iproute2-6.15.0, libbpf 1.5.0 |

The JSON `metadata` object contains exact command lines and full version output (including compiler, kernel, runtime dependencies and Docker, which was not used for these workloads). `closure.txt` lists the full Nix store closure; `dev-setup.log`, `dev.env`, `tools.paths` and `rg.path` retain setup details.


## Workload and method

One shared Nix-based toolset, 2.17 GiB allocated as a directory, contains git, Node 24, Rust 1.99.1, ripgrep, a C compiler, OpenSSL and prefetched Cargo dependency sources. Cargo runs offline. The source snapshot contains tracked and non-ignored working-tree files, including local edits, with generated build output and ignored caches excluded. Source origin HEAD: `c9aebce00982f5eb6e936f452c5eef029807dc9e`; this was a dirty working tree. `cargo fetch --locked` initially rejected its stale lockfile; `cargo fetch` resolved it in the disposable copy only. Every measured build used that same resolved source and lockfile, `--locked --offline`, a fresh target directory and `CARGO_INCREMENTAL=0`. Local product files and their lockfile were not changed.

Idle means a shell and `node -e 'setInterval(()=>{}, 1000000)'`. Light adds `rg --hidden -n 'pub|async|space' /work` every three seconds. Build means `cargo build --workspace --locked --offline -j2`, with the idle processes still present in the two sandbox cases. Containers use rootless native overlay, crun, a 1 GiB cgroup memory limit and 256 PIDs, no CPU quota. Firecracker uses two vCPUs and 1 GiB configured RAM, a read-only ext4 toolset disk, and an 8 GiB logical sparse ext4 writable disk. Its guest overlayfs uses that disk for upper/work directories. The checkout and target live in `/work` in the writable layer. Block devices were chosen because Firecracker exposes virtio-block directly; virtio-fs was not used. A minimal shell init is used, with no systemd or SSH daemon. The container uses the same userspace tools; guest-only bootstrap scripts differ.

The guest balloon is initially zero with `deflate_on_oom=true`, one-second statistics polling, and `free_page_reporting=true`. These are requests to a cooperating guest, not a host memory limit. One later idle test explicitly inflates a 512 MiB balloon. No host KSM tuning was performed.

Each start creates a fresh sandbox, then runs `true` through an actual shell. Container time includes Podman run, inspect and exec; Firecracker time includes sparse writable-disk creation/formatting, process launch, API configuration, guest boot and serial-shell round trip. `boot_seconds` separately excludes Firecracker disk preparation. Startup excludes source checkout injection in both options, tool/image downloads and base creation. Host page caches are retained; these are fresh-instance starts, not cold-storage-cache starts. Each option has 30 single starts and 30 batches of ten, yielding 300 parallel individual observations. Parallel individual observations within a batch are correlated. Quantiles use median and nearest-rank p95.

Freeze/resume uses Podman CLI pause/unpause and Firecracker Unix HTTP API PATCH `/vm`; these control paths have different client overhead. MicroVM shell commands use serial, container commands use `podman exec`. Times are orchestration-inclusive, not intrinsic runtime limits. Removal uses immediate forced termination outside timed regions. Preflight and interrupted initial runs are excluded and retained separately in `raw/events-preflight.jsonl`.


## 1. Fresh start to usable shell


| Option | Concurrency | Individual n | p50 ms | p95 ms | max ms | Batch p50 / p95 / max ms |
| --- | --- | --- | --- | --- | --- | --- |
| podman | 1 | 30 | 632.53 | 661.72 | 669.32 | 632.75 / 663.84 / 669.55 |
| podman | 10 | 300 | 3013.49 | 3909.32 | 4418.13 | 3855.68 / 4290.03 / 4430.80 |
| firecracker | 1 | 30 | 2675.98 | 3601.80 | 3625.28 | 2676.25 / 3602.10 / 3625.55 |
| firecracker | 10 | 300 | 5238.00 | 5591.86 | 5919.73 | 5580.82 / 5848.93 / 5928.50 |

Ancillary runtime control, thirty single starts each with the same Podman storage/CLI configuration:

| Runtime | n | p50 ms | p95 ms | max ms |
| --- | --- | --- | --- | --- |
| /nix/store/xa8qn1gvlb2sx333479pz7lm4p8rnr1y-crun-1.30.1/bin/crun | 30 | 642.46 | 719.52 | 732.80 |
| /usr/bin/runc | 30 | 698.79 | 794.68 | 806.61 |


## 2. Freeze, resume and disk snapshots


| Option | n | Freeze p50 / p95 / max ms | Resume p50 / p95 / max ms |
| --- | --- | --- | --- |
| podman | 30 | 97.86 / 110.53 / 118.64 | 96.19 / 115.96 / 119.67 |
| firecracker | 30 | 0.18 / 0.24 / 0.38 | 0.19 / 0.23 / 0.23 |

Pause leaves memory resident; it is not a memory-reclamation operation.

Podman rootless checkpoint attempt: exit 125, 62.68 ms. Diagnostic: `Error: checkpointing a container requires root`. No valid checkpoint or restore size/time exists.

Firecracker full snapshot, one idle trial: 3066.68 ms including explicit host fsync of state and memory files. Guest filesystems were synced before pause, outside the snapshot timer. Memory file logical / allocated: 1024.00 / 1024.00 MiB; state file 25911 bytes. The per-thread writable disk must also be retained; it is not included in these snapshot file sizes.

Firecracker restore, one trial: API-ready 5.03 ms; usable shell 107.11 ms. Retained writable disk allocation was 136.69 MiB. The memory file was still in host page cache; restore is demand-paged, so this does not measure a cold-disk full working-set restore.

After requesting a 512 MiB balloon and waiting three seconds: VMM PSS 31.02 MiB, RSS 31.75 MiB. This observation follows snapshot restore and is not a matched comparison with the pre-snapshot anonymous-memory profile.

Balloon-driver statistics were not obtained: the harness sent a JSON body with `GET /balloon/statistics`, which Firecracker rejected with HTTP 400, `GET request cannot have a body.` This is a harness request error, not evidence that balloon statistics are unsupported. The host PSS observation above remains available; the driver's achieved balloon size was not verified.


## 3. Host memory per thread

Idle/light profiles wait five seconds after population, then take five samples two seconds apart. PSS/RSS come from host `/proc`; containers sum their cgroup processes, VMs use the VMM process. `memory.current` includes cgroup kernel/file-cache charges; it is not equivalent to PSS. Shared image pages can be charged to an earlier cgroup, so it is not an independent per-thread allocation. Host `MemAvailable` deltas include all host activity and reclaim estimates; negative values are retained as measurement noise/cache effects. RSS double-counts shared mapped pages, whereas PSS apportions them, including sharing with processes outside the measured cohort. Container conmon/CLI/rootless-helper costs outside the workload cgroup are absent from its PSS and CPU columns; host deltas can include them. VMM PSS excludes host block-cache and some KVM/kernel allocations.


| Option | Activity | n | PSS / thread MiB | RSS / thread MiB | memory.current / thread MiB | Host available delta total MiB | Allocated writable / thread MiB |
| --- | --- | --- | --- | --- | --- | --- | --- |
| podman | idle | 1 | 50.91 | 52.43 | 21.07 | 20.66 | 4.37 |
| podman | idle | 10 | 14.20 | 52.41 | 16.52 | 327.52 | 4.37 |
| podman | light | 1 | 51.28 | 56.09 | 18.32 | -11.09 | 4.37 |
| podman | light | 10 | 14.42 | 56.07 | 16.60 | 176.14 | 4.37 |
| firecracker | idle | 1 | 128.46 | 129.20 | n/a | 134.69 | 72.69 |
| firecracker | idle | 10 | 123.73 | 128.08 | n/a | 1240.22 | 72.69 |
| firecracker | light | 1 | 140.29 | 141.03 | n/a | 348.62 | 72.69 |
| firecracker | light | 10 | 136.93 | 141.27 | n/a | 1134.37 | 72.69 |

The build-memory sampler sleeps 0.5 seconds between host samples; raw elapsed timestamps include the sampling work. Sampler overhead is not subtracted. Peaks can miss shorter transients and are one run per configuration. Ten-build rows are reported even if interrupted or failed; an unsuccessful build is not an overhead result.


| Option | Concurrent builds | Peak aggregate PSS MiB | Peak aggregate cgroup memory MiB | Min host available MiB | Successful builds |
| --- | --- | --- | --- | --- | --- |
| podman | 1 | 864.05 | 1023.98 | 5064.02 | 1/1 |
| podman | 4 | 2220.99 | 3598.73 | 4061.02 | 4/4 |
| firecracker | 1 | 988.55 | n/a | 4665.11 | 1/1 |
| firecracker | 4 | 3945.87 | n/a | 2137.99 | 4/4 |
| podman | 10 | 5187.94 | 6209.74 | 1121.87 | 10/10 |
| firecracker | 10 | 5966.73 | n/a | 496.75 | 0/10 |
| host | 1 | 73.57 | n/a | 5936.36 | 1/1 |
| host | 4 | 249.49 | n/a | 4381.64 | 4/4 |

Raw memory samples retain each thread separately; the summary JSON includes per-thread peak PSS. Build PSS includes build processes, not just a parent shell. Guest-reported RAM is not used as a cost measurement.


## 4. Disk and copy-on-write sharing


| Shared once | Allocated GiB | Logical GiB |
| --- | --- | --- |
| Podman image store including metadata | 2.17 | n/a |
| Firecracker read-only ext4 base | 2.22 | 4.00 |

The common checkout archive is 3.67 MiB; extracted checkout 4.36 MiB. Per-thread sparse files are reported by allocated host blocks, not 8 GiB apparent size. Container figures count allocated upper-directory blocks.

podman: initial writable allocated MiB 4.37, 4.37; after writing 1 MiB to thread 1: 5.37, 4.37. Thread 2 reports `ABSENT`. Shared base hash unchanged: True. Both inspect records have the same lower directory and distinct upper directories.

firecracker: initial writable allocated MiB 72.68, 72.68; after writing 1 MiB to thread 1: 73.69, 72.68. Thread 2 reports `ABSENT`. Shared base hash unchanged: True. Both VMM fd records resolve to the same base file device/inode; sparse writable disks are distinct. Blank writable-disk allocated MiB: 68.23, 68.23


| Option | Concurrent builds | Before build / thread MiB | After build / thread MiB | Growth / thread MiB |
| --- | --- | --- | --- | --- |
| podman | 1 | 4.37 | 1319.46 | 1315.09 |
| podman | 4 | 4.37 | 1319.47 | 1315.10 |
| firecracker | 1 | 72.68 | 1453.84 | 1381.16 |
| firecracker | 4 | 72.68 | 1441.22 | 1368.54 |
| podman | 10 | 4.37 | 1319.47 | 1315.10 |
| firecracker | 10 | 72.68 | unavailable | unavailable |

The measured microVM configuration shares the toolset base once, like container lower layers; full private copies of that base are not required. Each option still allocates checkout modifications and build artifacts privately. Guest overlayfs copy-up occurs inside each sparse disk; no host ext4 reflink, qcow2 or device-mapper snapshot is used. Filesystem journal/metadata consume private disk space even before build output. This experiment does not claim identical disk cost or memory sharing between the two implementations. See the [Linux overlayfs documentation](https://github.com/torvalds/linux/blob/master/Documentation/filesystems/overlayfs.rst) for lower/upper and copy-up semantics.


## 5. CPU, build wall time and I/O


| Option | Activity | n | CPU seconds in interval | Interval seconds | One-core CPU % / thread |
| --- | --- | --- | --- | --- | --- |
| podman | idle | 1 | 0.00 | 10.02 | 0.00 |
| podman | idle | 10 | 0.00 | 10.12 | 0.00 |
| podman | light | 1 | 0.04 | 10.02 | 0.37 |
| podman | light | 10 | 0.32 | 10.13 | 0.31 |
| firecracker | idle | 1 | 0.01 | 10.01 | 0.10 |
| firecracker | idle | 10 | 0.26 | 10.07 | 0.26 |
| firecracker | light | 1 | 0.05 | 10.01 | 0.50 |
| firecracker | light | 10 | 0.59 | 10.09 | 0.58 |

CPU uses container cgroup `cpu.stat` (including exited ripgrep children) or process-wide VMM utime+stime. The short interval and host timer resolution limit very small idle results; a reported zero is below this sampling resolution, not proof of zero cost. No physical-node CPU allocation or steal-time attribution was measured.


| Option | Concurrent builds | Per-build wall seconds | Batch wall seconds | Batch overhead vs same host concurrency | Exit statuses |
| --- | --- | --- | --- | --- | --- |
| podman | 1 | 62.62 | 62.65 | 4.04% | 0 |
| podman | 4 | 145.41, 146.23, 145.36, 146.66 | 146.90 | 1.86% | 0, 0, 0, 0 |
| firecracker | 1 | 170.84 | 171.15 | 184.19% | 0 |
| firecracker | 4 | 233.38, 232.63, 232.67, 234.02 | 234.08 | 62.32% | 0, 0, 0, 0 |
| podman | 10 | 395.11, 397.30, 394.81, 394.98, 395.30, 395.98, 391.03, 390.92, 394.45, 397.65 | 397.75 | not a valid comparison | 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 |
| firecracker | 10 | 39.43, 39.87, 40.56, 41.03, 41.55, 41.90, 42.15, 42.38, 42.70, 42.84 | 42.88 | not a valid comparison | error, error, error, error, error, error, error, error, error, error |
| host | 1 | 59.81 | 60.22 | 0.00% | 0 |
| host | 4 | 143.41, 143.76, 143.77, 142.46 | 144.21 | 0.00% | 0, 0, 0, 0 |

The host control uses the same Nix toolchain, dependency cache, source snapshot, job count and fresh targets, without a guest/container. Host timing includes process launch; sandbox timing includes one command round trip. Four concurrent builds oversubscribe four host cores. MicroVMs have two vCPUs each; containers and the direct-host control can schedule compiler helper threads across all four host cores. There is no CPU-affinity-equivalent control here. Page caches are not evicted between builds. Initial host controls failed before compilation because the login environment referenced absent sccache; these are retained in `raw/events-build-preflight.jsonl` and excluded. Valid host controls were retried later with an explicit environment, after the sandbox runs, so build order was not randomized. These constraints, and one build repetition, limit causal interpretation of overhead percentages. No test-suite timing is included.


| Option | I/O command | Wall ms | MiB/s including command transport |
| --- | --- | --- | --- |
| podman | dd 64 MiB zeroes, conv=fsync; delete | 470.67 | 135.98 |
| firecracker | dd 64 MiB zeroes, conv=fsync; delete | 847.52 | 75.51 |

This is a short sequential-write sanity check, not a storage benchmark: cached reads, random I/O, durability after power loss, repeated fsync distributions and physical storage saturation were not measured.


## 6. Density


| Option | Activity | Observed live threads | Stop reason | Host available MiB at stop | Aggregate PSS MiB |
| --- | --- | --- | --- | --- | --- |
| podman | idle | 194 | launch_or_workload_error | 4019.77 | 2097.43 |
| podman | light | 194 | launch_or_workload_error | 3730.01 | 2136.67 |
| firecracker | idle | 46 | 768MiB_available_guard | 695.50 | 5729.32 |
| firecracker | light | 42 | 768MiB_available_guard | 607.93 | 5827.52 |

Podman launches failed with exit status 126 at 194 live containers while host MemAvailable was about 4 GiB, well above the host-memory guard. The cause was not diagnosed, so this is an observed launch limit rather than a measured RAM capacity. The exact failed commands are retained in the JSON `observations.density_error` records.

Density stops before launching another thread when host `MemAvailable` falls below 768 MiB, with a test cap of 512 containers or 80 microVMs. It does not intentionally trigger the host OOM killer or disturb unrelated services. The results are observed lower bounds under that reserve policy, not the maximum until host OOM and not an extrapolation to a clean 7 GB machine. This VM has 7.70 GiB RAM and baseline services consume part of it. There is no swap, so no swap threshold can be measured. Build runs use a separate 512 MiB host-available guard. A malicious or memory-heavy guest could touch its full configured RAM irrespective of the idle/light density shown.


## 7. Restricted egress and first outbound HTTP request

podman: one first-request trial to `http://1.1.1.1/`; sandbox start 629.14 ms; startup/rule setup through request launch 640.92 ms; request command 312.91 ms; full measured sequence 953.84 ms. Response/Node timing: `{"status":301,"ms":26.188856}`.

firecracker: one first-request trial to `http://1.1.1.1/`; sandbox start 2213.80 ms; startup/rule setup through request launch 2257.13 ms; request command 3143.76 ms; full measured sequence 5400.90 ms. Response/Node timing: `{"status":301,"ms":59.281796}`.

podman second destination (1.0.0.1) request: blocked (no response within the 2 s timeout).

firecracker second destination (1.0.0.1) request: blocked (exit status 3, the 2 s timeout).

Network trials use a separate network namespace for every sandbox. Podman 6 removed slirp4netns, so the container uses `--network=pasta`; the microVM uses slirp4netns (host loopback disabled) on a TAP in the namespace that is bridged to a second TAP, `fc0`, which Firecracker opens (slirp4netns holds `tap0` itself, which Firecracker cannot also open). Earlier failed attempts (slirp4netns rejected by Podman 6, nft syntax error, TAP busy) are retained in `raw/events-network-preflight.jsonl` and excluded. The container trial applies host-admin nftables OUTPUT rules inside its network namespace. The microVM trial applies host-admin nftables netdev ingress rules on `fc0`, before the userspace network stack, permitting ARP and IPv4 destination 1.1.1.1 TCP port 80 only, dropping other IP egress. An otherwise identical request to 1.0.0.1 is the denial control. Guest rules alone are not relied on for VM egress restriction. Rules are scoped to disposable namespaces; no global host firewall was modified. One HTTP destination/IP rule does not demonstrate a production domain allowlist, DNS controls, TLS validation or robust proxy policy. First-request timing excludes DNS by using a literal IP. No per-thread networking RAM/CPU profile is inferred from the network-disabled workload runs.


## 8. Main-thread fan-out and resume to first command output

The request timer starts before dispatching local orchestration tasks and ends after the last child emits `FIRST_COMMAND_OUTPUT`. Every child receives a fresh sandbox, the frozen working tree plus Git metadata, and a separate `.agent-credential` file with mode 0600, then runs a command that verifies the Git working tree and reads the fixture credential through Node before producing output. The credential is a dummy string with no privileges; no live credential service or model API is contacted. The timestamp is when orchestration receives the short command output after command completion, including the exec/serial acknowledgement; separate first-byte streaming latency is not measured. This measures sandbox orchestration and filesystem readiness, not model thinking or external credential-fetch latency. Thirty requests are measured for each N/load case. The ten background idle sandboxes are populated before timing and remain alive across the associated trials. Their setup is excluded. Cleanup follows the measured output and is excluded. Valid completed requests are retained across harness restarts. A partial serial-line parsing race interrupted the original microVM sequence; that unrecorded request is excluded and retried after correcting the parser. These samples are not an estimate of production provisioning reliability.

Fan-out fixture: 9.20 MiB allocated tree including `.git`, archive 6.39 MiB; Git metadata HEAD `66eca0dd6ab524dbc8b747b62ec911deb1f2d7a5`. Earlier items use the source-only working-tree fixture without `.git`; finished measurements were retained.


| Option | New subagents | Existing idle | Requests | Request to last first output p50 s | p95 s | max s |
| --- | --- | --- | --- | --- | --- | --- |
| podman | 3 | 0 | 30 | 2.95 | 3.22 | 3.41 |
| podman | 10 | 0 | 30 | 10.13 | 11.15 | 12.56 |
| podman | 3 | 10 | 30 | 3.11 | 3.97 | 3.98 |
| podman | 10 | 10 | 30 | 10.34 | 12.02 | 12.19 |
| firecracker | 3 | 0 | 30 | 3.74 | 4.53 | 6.45 |
| firecracker | 10 | 0 | 30 | 9.08 | 11.34 | 11.42 |
| firecracker | 3 | 10 | 30 | 3.58 | 4.97 | 4.98 |
| firecracker | 10 | 10 | 30 | 9.08 | 10.73 | 10.78 |

Container requests include creation, shell readiness, archive extraction into the writable layer, credential creation, and the first Git/Node command. MicroVM requests include writable-disk formatting/import of the same checkout plus credential, VMM configuration/guest boot and that same command through serial. The shared base already exists and host cache remains warm; no remote clone or image download is included. Each request waits for all children; the table reports request quantiles, not quantiles of individual child starts. No pass/fail latency target was specified, so no fast-enough judgment is assigned.


| Option | Frozen source | n | Resume to first output p50 ms | p95 ms | max ms |
| --- | --- | --- | --- | --- | --- |
| podman | paused in RAM | 30 | 350.73 | 432.11 | 460.08 |
| firecracker | paused in RAM | 30 | 48.16 | 183.83 | 2890.68 |

Resume timing begins after the pause has completed and includes unpause/resume plus the first Git/Node command output. This is RAM-retained freezing of one populated sandbox, with no background cohort; it is separate from snapshot-to-disk restoration in item 2.


## Isolation properties (documented facts, not penetration testing)


| Property | Rootless Podman | Firecracker |
| --- | --- | --- |
| Kernel boundary | Processes use the host Linux kernel; user namespaces map root to an unprivileged host identity. | Each microVM runs its own guest kernel behind KVM and a userspace VMM. |
| Kernel fault scope | A host-kernel fault/exploit reachable from one container can affect the host and other threads sharing that kernel. Rootless mapping does not create a separate kernel. | A guest-kernel panic ordinarily stops that microVM. Guest-kernel privilege alone does not grant host kernel privilege; a KVM/VMM/host escape or host fault can affect other VMs. |
| Resource and filesystem isolation | Namespaces, capabilities, seccomp and cgroups provide process/filesystem/resource controls; shared user identity, exposed mounts and misconfiguration still matter. | Separate guest address space and block devices; VMM seccomp plus jailer/cgroups/namespaces provide additional host process containment when configured. |
| Network restriction | Requires runtime/host/proxy policy per namespace; rootless networking alone is not an outbound allowlist. | Firecracker does not filter network traffic; host TAP/namespace/proxy policy is required. |
| What this run proves | Successful rootless operation and the measured workloads; no adversarial escape or multi-user identity separation audit. | Successful nested guest operation and the measured workloads; no adversarial escape audit. The VMM ran as kayg without jailer, so production jailer overhead and containment are unmeasured. |

Sources: [Podman run](https://docs.podman.io/en/latest/markdown/podman-run.1.html) describes filesystem/network/process isolation; [Linux user namespaces manual](https://man7.org/linux/man-pages/man7/user_namespaces.7.html) defines privilege mappings. The shared-kernel fault scope is the consequence of that shared kernel, not a tested exploit. [Firecracker design](https://github.com/firecracker-microvm/firecracker/blob/v1.16.1/docs/design.md) describes KVM/VMM barriers, seccomp, jailer and host-side filtering. Fault scope assumes those barriers operate correctly; host-level faults remain a common dependency.


## Limits, missing results and reproducibility

Nested virtualization changes the Firecracker execution environment: guest vCPU scheduling is layered over this VM's scheduling; guest page-table and device/I/O paths also traverse the outer virtualization layer. The host control and containers have one guest-kernel layer, while Firecracker adds another. CPU contention, virtual-disk caching and Proxmox scheduling can affect the results. No bare-metal measurements, physical-node counters or outer-host configuration were available, so no magnitude or correction factor is assigned. These numbers must not be presented as bare-metal microVM performance. The [KVM nested-guest documentation](https://github.com/torvalds/linux/blob/master/Documentation/virt/kvm/x86/running-nested-guests.rst) defines the L0/L1/L2 execution layers.

Cloud Hypervisor was not measured. Rootless checkpoint results are limited to the installed Podman/runtime/CRIU combination. Snapshot/restore and I/O each have one successful trial at most; snapshot cold-storage restore, memory-snapshot cloning across ten threads, diff snapshots, active-build snapshots, physical disk throughput and a host-OOM density maximum are unmeasured. The kernel and VMM versions differ by construction. No claim of paid-service isolation, tenant accounting, production safety, cost in currency or fleet-wide density follows from this run.

MicroVM snapshot memory can itself be shared with private copy-on-write mappings on restore; this is documented by [Firecracker snapshot support](https://github.com/firecracker-microvm/firecracker/blob/v1.16.1/docs/snapshotting/snapshot-support.md). This run tests one restore, not clone sharing. Disk images are caller-managed and must be retained consistently. Ballooning/free-page reporting require guest cooperation and do not cap malicious guest memory consumption; see [Firecracker balloon documentation](https://github.com/firecracker-microvm/firecracker/blob/v1.16.1/docs/ballooning.md). [Podman checkpoint documentation](https://docs.podman.io/en/latest/markdown/podman-container-checkpoint.1.html) describes its runtime/CRIU checkpoint operations; actual rootless failure diagnostics above take precedence for this configuration.


### Exact setup and execution

Scripts on perf-01 are the executable record: `setup.py`, `bench.py`, `metadata.py`, `sharing.py`, `network.py`, `flow.py`, `versions.py`, `summarize.py`, `run.sh`, with setup corrections and logs alongside them. They contain exact Firecracker API JSON, shell workload commands, Podman flags, mkfs options, sampling code and nftables rules. The rootfs bootstrap correction is recorded in `fixboot.sh`; the final base hash is recorded in metadata. Setup retries are not measurement samples.

```sh
# On hd01: read/copy only; tracked + non-ignored working-tree snapshot
cat /home/kayg/Developer/Calternal-AI/AGENTS.md
cd /home/kayg/Developer/Calternal-AI
git ls-files -z --cached --others --exclude-standard | tar --null -T - -cf - | \
  ssh kayg@calternal-perf-01-backup01 \
  'mkdir -p ~/calternal-ai-sandbox-measure/source; tar -xf - -C ~/calternal-ai-sandbox-measure/source'

# On perf-01 only, all runs hold the supplied lock
cd ~/calternal-ai-sandbox-measure
/usr/local/bin/perf-lock.sh nix shell nixpkgs#podman nixpkgs#crun \
  nixpkgs#firecracker nixpkgs#e2fsprogs nixpkgs#busybox nixpkgs#criu \
  nixpkgs#slirp4netns --command bash -c 'command -v podman crun firecracker mkfs.ext4 busybox criu slirp4netns > tools.paths'
/usr/local/bin/perf-lock.sh nix shell nixpkgs#ripgrep \
  --command bash -c 'command -v rg > rg.path'
/usr/local/bin/perf-lock.sh sudo apt-get install -y uidmap
sudo usermod -aG kvm kayg  # then reconnect
# Pinned development shell was resolved once; its compiler environment builds the common root
cd checkout
/usr/local/bin/perf-lock.sh nix develop --no-write-lock-file -c bash -c 'env > ../dev.env'
cd ..
/usr/local/bin/perf-lock.sh python3 setup.py
/usr/local/bin/perf-lock.sh bash fixboot.sh
/usr/local/bin/perf-lock.sh bash run.sh
/usr/local/bin/perf-lock.sh python3 sharing.py
/usr/local/bin/perf-lock.sh python3 flow.py
/usr/local/bin/perf-lock.sh python3 versions.py
/usr/local/bin/perf-lock.sh python3 summarize.py
```

`run.sh` executes metadata, startup, profiles, host/sandbox builds at 1 and 4, ten-build profiles, restricted networking and density in sequence. Build commands source the environment derived from the pinned Nix development shell rather than installing another compiler in the guest. `tools.paths` and the full closure pin the actual measured binaries even though the initial Nix acquisition used the host registry.

Raw events: `raw/events.jsonl`; excluded host setup failures: `raw/events-build-preflight.jsonl`; excluded network attempts: `raw/events-network-preflight.jsonl`; preflight observations: `raw/events-preflight.jsonl`; per-build logs and 0.5-second samples: `raw/*-build-*`; versions/input hashes: `raw/metadata.json`; individual VM serial/API logs: `fc-*/console.log`; phase logs: `starts.log`, `profiles.log`, `builds.log`, `build10.log`, `network.log`, `density.log`, `sharing.log`, `flow.log`, `continue.log`. The JSON summary retains failures, limits, quantiles and raw-file paths. Disposable containers, VMs, image stores, disks, snapshots, toolset copies and Cargo/build caches are removed after final collection; scripts and raw observations remain. Nix store entries are shared package-manager state and were not globally garbage-collected.

The final coverage audit found all 660 startup observations, 240 fan-out requests, eight idle/light profiles, four density cases and both thirty-sample resume cases. `raw/final-audit.json` records the checks; raw memory traces exist for all eight build configurations, including the guard-stopped ten-VM run. `raw/cleanup.json` records zero remaining test containers/images and removal of the large fixtures, leaving about 19 MiB of scripts, logs and results. Both records are copied into the JSON companion. No completed measurement was rerun during this final audit.

## Simplification opportunities

- Keep the common toolset manifest as the single input to both image formats.
- Record client command overhead and runtime/guest work as separate phases; the current end-to-end timings intentionally include both.

