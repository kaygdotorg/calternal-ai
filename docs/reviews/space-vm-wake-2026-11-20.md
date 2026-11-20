# Small plugin VM wake policy

## Package launcher

The offline plugin guest stages the verified bundle with the existing worker
preparation function. It writes one source file and sets its mode. The guest
creates one cgroup and writes four resource controls. This path adds no SQLite
writes or scratch-disk flushes. The agent guest still uses its Podman backend.

| Control | Small guest |
| --- | --- |
| Process identity | UID and GID 1100, separate from the guest agent at UID 1000. No supplementary groups or capabilities. |
| Resource controls | Dedicated cgroup v2 with 256 MiB memory, zero swap, 64 PIDs and one CPU of quota. |
| Isolation | Private mount, PID, network, IPC and UTS namespaces. A chroot contains the read-only runtime, private proc and a small device set. The package mount is read-only, noexec, nosuid and nodev. |
| Process restrictions | `no_new_privs`, the existing worker seccomp allowlist compiled for capability-empty amd64, and the existing Node permission flags and 128 MiB heap setting. |
| Package lifecycle | The launcher accepts the guest agent UID and a placement UUID. It closes inherited descriptors beyond the three exec pipes. Before exec it checks UID, GID, namespace PID, `no_new_privs`, denied socket creation and read-only code. Teardown kills the cgroup before removing the package. |

The small guest now omits nested user-namespace mapping, OCI masked paths and
engine-applied labels, the container monitor and init, and Podman ownership
inventory. Node is PID 1 in its namespace and the launcher waits for it.
Cross-space isolation comes from the Firecracker guest. Package code and the
trusted control agent share one space's guest kernel. The launcher enters guest
root for setup through a fixed, agent-only setuid entry point. The amd64 filter
rejects compatibility-architecture calls. The common guest image retains Podman
for agent VMs.

The existing placement gate and authenticated vsock exec transport still
serialize publication and teardown. Source files are staged only after the
restore reseed acknowledgement. Sleep destroys the package before checkpointing.
Package admission and the separate exec boundary provide the seam for future
pre-start work, once the remaining randomness checks are established.

The real-KVM machine fixture on dev-01 passed cold launch, snapshot
publication, rejection before reseed, independent restored Node randomness,
interruption recovery and invalid-checkpoint handling. All three enrolled
runner KVM cases passed, covering item-head execution, idle snapshot and
restore, and carrier lease-loss teardown.

Step 1, `689948e54729`, completed ten disk wakes on locked perf-02. All ten
returned validated replies without errors or cold fallbacks. The timings
below are milliseconds with p50 and p95 computed independently. Raw evidence
is under `/srv/homes/perf/vmw1/launcher`.

| Stage | p50 | p95 |
| --- | ---: | ---: |
| VMM launch to API ready | 23.04 | 23.83 |
| Reseed | 527.16 | 800.38 |
| Package admission | 25.64 | 53.61 |
| First validated reply | 1,577.93 | 1,937.73 |
| Resident follow-up | 4.50 | 5.53 |

## Residency

The broker tracks the end of authorized calls on the existing carrier. An
active call holds a local admission slot. The idle timer and scheduler use
that same state to fence new calls before sending the existing sleep command.
Memory pressure selects the least recently used idle carrier. The scheduler
waits for its normal teardown proof before using the reclaimed reservation.
The machine semaphore enforces the same ceiling through start and teardown.
The reported memory figure is reserved guest RAM. Perf PSS is measured
separately.

The resident call path adds local memory updates and no storage reads or
flushes. It retains the existing ticket and item-read calls. Wake and sleep
use the existing scheduler, checkpoint proof and scratch durability steps.
There are no new tables.

## Restore handoff

The root reseed helper keeps its agent-only Unix listener across checkpoints.
Each exchange still reads 32 fresh host bytes, updates and reseeds the guest
kernel, and acknowledges only on success. The guest publishes its ready
marker before sending the authenticated response. The restored host path uses
that response directly. Start checks the marker independently. The VMM waits
1 ms between API socket checks. The small guest's package staging uses fresh
tmpfs on cold boot and an emptied package directory on restore. Its path has
no Podman ownership inventory. Agent-guest startup retains its existing
ownership reconciliation. The dev-01 image passed the real-KVM snapshot and
reseed fixture and all four enrolled-runner KVM tests.

Use the matching machine-role binary and pinned guest image. Existing
checkpoints remain bound to their original image digest.

## Checkpoint follow-ups

An authenticated reconnect reporting SleepPending can receive the same
numbered sleep command. Its machine, lease and fence must still match the
durable authorization. Account erasure now matches an acknowledged
checkpoint holder directly against the erasure queue, including after the
lease history has been pruned. All nine machine worker tests passed on
dev-01, including both regressions.

The fresh-Node revision passed `scripts/verify-additive-upgrade.sh` against
`localhost/calternal-ai:31fb41179460` and the candidate image `469217aa84a4`.
The script also checked for leaked Podman containers, Firecracker processes
and provider sign-in processes.

Issue #90 still tracks cutoff erasure that retains an unacknowledged newer
snapshot and the operator end state for repeated or revoked-machine erasure
failures.

## Locked perf-02 measurements

The existing enrolled item-head harness ran against an XFS scratch directory
and the rebuilt guest on perf-02. It used the operator's one-second idle
override to generate independent disk wakes with normal filesystem caching.
Each wake was followed by a validated resident call to the same worker.
The 30 disk wakes and 30 resident calls completed with zero errors and zero
cold fallbacks. Host CPU idle was 78.49% and steal was 0.78%. Stage percentiles
are computed independently. Raw data and the measurement overlay are under
`/srv/homes/perf/vmw2`.

| Stage | p50 (ms) | p95 (ms) |
| --- | ---: | ---: |
| Request through placement decision | 22.13 | 30.79 |
| Placement decision through machine gate entry | 4.20 | 7.78 |
| Machine placement gate | 0.016 | 0.024 |
| Manifest and local setup | 0.21 | 0.53 |
| Prepare private scratch and control drives | 1.11 | 1.56 |
| VMM launch through API availability | 3.96 | 19.22 |
| Snapshot load and file-backed memory mapping | 2.01 | 5.08 |
| Resume | 0.40 | 0.59 |
| Reseed through acknowledgement | 402.26 | 692.73 |
| Inline readiness handoff | 0.025 | 0.053 |
| Stage the package and reserve its cgroup | 26.41 | 68.00 |
| Admit first call before starting Node | 30.02 | 45.53 |
| Node launch through first statement | 848.50 | 1,086.47 |
| First statement through package readiness | 94.58 | 181.78 |
| Package readiness through first callback | 133.01 | 293.06 |
| First callback through validated reply | 2.69 | 15.85 |
| **Disk wake through first validated reply** | **1,610.87** | **2,350.86** |
| **Resident call through validated reply** | **4.76** | **6.07** |

Disk wake remains above the 500 ms gate on this host. Compared with the
ten-wake launcher series, the VMM and reseed stages took less time at the
median. Fresh Node startup remained the largest stage.

### Resident memory

The density fixture kept one idle package worker per guest. At each count it
settled for seven seconds and took three readings. The table reports median
aggregate Firecracker PSS and the drop in host MemAvailable from the same
baseline. PSS apportions shared pages among processes. The machine charges
configured guest RAM for each reservation. Evidence is under
`/srv/homes/perf/vmw2/d`.

| Resident spaces | Aggregate VMM PSS (MiB) | Mean PSS per VM (MiB) | MemAvailable drop (MiB) | Reserved guest RAM (MiB) |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 124.56 | 124.56 | 121.99 | 256 |
| 4 | 477.03 | 119.26 | 463.88 | 1,024 |
| 16 | 1,863.26 | 116.45 | 1,874.21 | 4,096 |

## Pre-started worker

Before sleep, the guest tears down the package and parks a new trusted Node
launcher in an empty package mount. The launcher loads the native reseed
module, drops the singular `addon` permission and verifies all six approved
denials. The guest agent checks the process and task names through the cgroup
and procfs before snapshot, including the absence of libuv pool threads. The
launcher and fresh worker share the same seccomp filter and namespace setup.
The launcher reads its own directory and the package directory.

After authenticated kernel reseed and Start admission, the agent sends a
refresh command. The launcher reseeds the three OpenSSL generators with
prediction resistance, discards the 128-entry UUID cache, seeds V8 for new
contexts, replaces main-realm `Math.random` and checks the denials again.
A separate load command follows the refresh acknowledgement. It loads the
package through the Node module loader, including ES modules with top-level
await. A failed pre-load assertion reaps the parked process and leaves fresh
Node available. A failed package after load follows the normal lease
failure path.

The image builder compiles the native module from committed source and
records its checksum. Before publishing the image, its KVM proof exercises
two restores of one checkpoint through the guest agent. The proof checks
distinct random bytes, UUID, Web Crypto bytes, Ed25519 keys, TLS ClientHello
and random field, main-realm and new-context Math.random, plus all six
permission denials. A fresh Node fallback cannot satisfy the standby proof.
The dev-01 gate passed for image manifest `276055f161c5` and native module
`fc880b6ab431`. The proof package used an ES module with top-level await and
exited naturally. The checkpoint regression and all four enrolled-runner KVM
tests also passed on this image.

The final release image `5fe5337cf2a7` passed the additive upgrade script
against `localhost/calternal-ai:31fb41179460`. The machine worker and CLI
tests and `scripts/check-docs.sh` passed on dev-01.

## Simplification opportunities

The worker seccomp JSON already serves as the single source for the guest
filter. The package launcher and carrier also share one fixed worker command
at the Rust boundary. A future separate small-guest image could omit Podman
and the agent runtime from its filesystem.
