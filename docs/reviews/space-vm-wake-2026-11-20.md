# Small plugin VM wake policy

## Package launcher

The offline plugin guest stages the verified bundle with the existing worker
preparation function. It writes one source file and sets its mode. The guest
creates one cgroup and writes four resource controls. This path adds no SQLite
writes or scratch-disk flushes. The agent guest still uses its Podman backend.

| Control | Small guest |
| --- | --- |
| Process identity | UID and GID 1100, separate from the guest agent at UID 1000. No supplementary groups or capabilities. |
| Resource controls | Dedicated cgroup v2 with 192 MiB memory, zero swap, 64 PIDs and one CPU of quota. The earlier Podman worker allowed 256 MiB. |
| Isolation | Private mount, PID, network, IPC, UTS and cgroup namespaces. The launcher pivots to the read-only runtime with private proc and a small device set, then detaches the guest root. The package mount is read-only, noexec, nosuid and nodev. |
| Process restrictions | `no_new_privs`, the existing worker seccomp allowlist compiled for capability-empty amd64, and the existing Node permission flags. Node old space is capped at 96 MiB with 4 MiB semi-spaces. |
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

The idle interval defaults to 900 seconds and is capped at 24 hours. Operators can set `idle_seconds`
and `resident_memory_mib` in the small-VM configuration. The default memory
ceiling is `memory_mib * max_guests`.

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

When `prestarted_worker` is enabled, the guest tears down the package before
sleep and parks a new trusted Node launcher in an empty package mount. The
launcher loads the native reseed
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

### Same-image worker comparison before review

The locked perf-02 run used the verified guest image for both arms, with
`prestarted_worker` as the only worker-mode setting changed. It ran the
same enrolled item-head traffic, one-second idle interval and XFS scratch
with normal filesystem caching. Each arm completed 30 disk wakes and 30
resident follow-ups with zero errors and zero cold VM fallbacks. All 30
standby restores used the refreshed worker, with zero fresh-worker fallbacks
or refused parks. Each arm also had one cold prime outside the timings.
Host CPU idle and steal were 78.47% and 0.78% for fresh, and 76.23% and
0.70% for standby. Raw traces, mode counts and the probe patch are under
`/srv/homes/perf/vmw3`.

| Stage | Fresh p50 (ms) | Fresh p95 (ms) | Standby p50 (ms) | Standby p95 (ms) |
| --- | ---: | ---: | ---: | ---: |
| Request through placement decision | 22.50 | 39.59 | 22.45 | 28.98 |
| Placement decision through machine gate entry | 4.04 | 5.06 | 3.99 | 4.89 |
| Machine placement gate | 0.017 | 0.022 | 0.017 | 0.024 |
| Manifest and local setup | 0.20 | 0.38 | 0.20 | 0.43 |
| Prepare private scratch and control drives | 1.03 | 1.41 | 1.22 | 1.80 |
| VMM launch through API availability | 4.10 | 17.62 | 3.97 | 23.66 |
| Snapshot load and file-backed memory mapping | 1.82 | 4.30 | 1.91 | 2.91 |
| Resume | 0.41 | 1.33 | 0.45 | 1.07 |
| Kernel reseed through acknowledgement | 352.75 | 507.25 | 317.35 | 698.26 |
| Inline readiness handoff | 0.021 | 0.029 | 0.021 | 0.029 |
| Stage the package and reserve its cgroup | 38.58 | 68.68 | 32.98 | 88.82 |
| Admit first call | 32.12 | 45.09 | 36.35 | 46.98 |
| Node launch or refresh through first package statement | 873.34 | 1,211.35 | 439.95 | 1,056.84 |
| First statement through package readiness | 94.19 | 148.38 | 18.93 | 73.81 |
| Package readiness through first callback | 113.03 | 213.95 | 24.98 | 110.21 |
| First callback through validated reply | 2.61 | 5.53 | 5.03 | 42.10 |
| **Disk wake through first validated reply** | **1,609.43** | **2,102.21** | **927.85** | **1,475.73** |
| **Resident call through validated reply** | **4.79** | **22.80** | **17.16** | **308.31** |

Standby reduced disk-wake p50 by 42% and p95 by 30%. Disk wake still exceeds
the 500 ms gate. The two slowest standby resident calls took 308 and 420 ms.
Their callback markers arrived about 14 and 3 ms after server gate exit.
The trace places most of the delay after those markers and cannot attribute
it to a specific component in the reply path.

## Independent review follow-up

1. The trusted launcher unlinks the addon module from `module.children` and
   clears `require.cache` and `process.mainModule`. The image proof inspects
   those references from an ES module and repeats the six permission denials.
2. The launcher pivots into the private root, detaches the old mount tree,
   restricts proc to `subset=pid,hidepid=invisible`, and unshares the cgroup
   namespace. The native fixture checks the visible mounts and denies guest
   paths, AF_VSOCK, outer signals, user-namespace creation, ptrace attachment,
   UID escalation and out-of-package writes. A bounded fork test reaches
   `pids.max` and reaps all its children.
3. `memory.max` is 192 MiB. The agent and reseed helper have OOM score
   adjustment −1000. The launcher resets the inherited adjustment for the
   worker. Quiesce checks the live helper socket before parking. The memory
   stress test confirms that the worker is killed, the guest checkpoints, and
   its agent and helper can restore it.
4. The launcher parses the two commands from raw bytes and pipes package stdin
   as bytes thereafter. The image proof sends a split binary frame through
   the real guest transport in both modes, with invalid UTF-8, CR, CRLF and
   an unterminated final field.
5. Standby checks the guest ready marker before refresh. A restored guest
   still refuses startup before authenticated reseed. The image proof also
   freezes a parked worker to force refresh timeout and checks that fresh
   Node processes the binary frame in the same restored guest.
6. The machine rejects idle windows outside one second through 24 hours. The
   server clamps values reported by enrolled machines to that same range.
7. Protocol capacity, VM capacity and checkpoint-order tests now have their
   own modules. Each of the three files flagged in review is under 600 lines.
8. Account erasure invalidates checkpoint authorization when either the
   acknowledged holder or the current lease machine is erased. The fixture
   covers different machines in the same row. The separate cutoff and retry
   questions remain in [issue #90](https://git.kayg.org/kayg/calternal-ai/issues/90).

The native fixture uses `workerProof = true` in the guest image derivation.
It runs the probes after the launcher has set its UID, namespaces, mounts,
cgroup and seccomp. The native proof reads `/dev/urandom` directly and checks
that two restores of one snapshot yield different byte sequences.

On dev-01, image manifest `c7e8b4aec3b1` passed the regular checkpoint,
image and process-restart KVM cases. Native probe image `cffb35933192` passed
the isolation and memory-pressure case. The broker suite passed all 10 tests,
including five enrolled KVM cases and the in-flight pressure, revocation and
erasure regressions. All 11 machine worker tests passed. The docs gate and
format check passed. Release image `c90b0ef401be` passed the additive upgrade
from `localhost/calternal-ai:31fb41179460`, including the scoped leak checks.

### Resident tail after review

The corrected perf-02 run used the earlier harness's TCP_NODELAY settings.
Both uninstrumented arms used the reviewed `c7e8b4aec3b1` guest, a one-second
idle override and normal filesystem caching. Each completed 30 disk wakes
and 30 resident calls with zero errors or fallbacks. All 30 standby wakes
used the parked worker. The cold primes were outside the timing samples.

| Worker | Disk wake p50 / p95 (ms) | Resident p50 / p95 (ms) | Worst resident (ms) |
| --- | ---: | ---: | ---: |
| Fresh Node | 1,610.64 / 2,304.99 | 4.69 / 6.46 | 7.03 |
| Standby with raw stdin | 1,118.64 / 1,791.61 | 6.59 / 18.33 | 24.58 |

Three diagnostic guest images used `--trace-gc` and sampled the worker's
guest minor and major fault counters at invocation and host reply. Each arm
completed 15 wake and resident pairs with zero errors or fallbacks. With 15
samples, nearest-rank p95 is the worst observation.

| Standby diagnostic arm | Median disk wake (ms) | Resident p50 / p95 (ms) | Resident calls overlapping a scavenge |
| --- | ---: | ---: | ---: |
| Old readline stdin | 970.54 | 11.19 / 521.33 | 6 |
| Raw byte stdin | 1,099.37 | 8.80 / 494.07 | 1 |
| Raw byte stdin with forced GC after refresh | 2,082.70 | 8.04 / 10.65 | 0 |

Task-triggered V8 scavenges overlapped the long resident calls. In the raw
stdin arm, a 421.69 ms scavenge overlapped 415.59 ms of a 494.07 ms request.
The worker added 122 minor faults and one major fault between invocation
and host reply. The other 14 raw-stdin calls added zero major faults.
Scavenges also overlapped six calls in the old-stdin arm, all with zero
guest major faults. The guest process counters do not identify host-side
snapshot page-in. The binary stdin fix reduced the observed frequency in
this sample and still allowed a long scavenge to interrupt a resident call.
Forced GC shifted collection into the disk wake and took a median 919 ms.
Raw logs, checksums and the summary are under `/srv/homes/perf/vmw4`.

Standby stays off by default. Operators can set `prestarted_worker: true`
to enable it. The plain launcher, idle timeout and memory ceiling apply to
both modes. Disk wake still exceeds the 500 ms target.

## Second independent review follow-up

`deploy/microvm/build-image.sh` now derives the release and native-proof
images from the same normalized root, guest agent, CLI and pinned kernel.
It requires both exact KVM tests before it publishes the release link. The
native test fails if invoked without its image configuration. Both tests
passed for the build that published release manifest `b5159e123bae`.

The release test verifies that retained JavaScript objects exhaust the V8
heap in both worker modes, with the fatal heap-limit diagnostic. The separate
native test verifies the 192 MiB cgroup limit with external Buffer memory.
The plugin guide documents `os.availableParallelism()` and the supported
`process.stdin` interface. The release test confirms `os.cpus()` is empty.

On locked perf-02, a guest agent with measurement probes sampled
`memory.current` after the standby task check and after each refresh, before
package load. The parked cgroup charged 14.70 MiB. All four refreshed
samples charged 14.89 MiB, leaving 177.11 MiB under the 192 MiB cap before
package load. Node RSS was 49.20 MiB fresh and up to 53.75 MiB after standby
release in the small proof package. RSS includes shared resident pages.
Evidence is under `/srv/homes/perf/vmw5`.

## Review gate

The wake path reuses the typed placement and snapshot authorization, durable
checkpoint logic, package staging, worker bounds and transport. Server idle
state is scoped to each carrier. Memory admission uses an atomic semaphore
with a fence around per-space state. File and process work uses existing async
or blocking-task boundaries. Production reads and flushes are unchanged for
resident calls. The small profile adds no retained SQLite state, dependencies
or compatibility readers. Public capacity fields are documented, wired into
machine status, and covered by `scripts/check-docs.sh`. Formatting passed.
The review exception for existing long source files is recorded on issue #97.

## Simplification opportunities

The worker seccomp JSON already serves as the single source for the guest
filter. The package launcher and carrier also share one fixed worker command
at the Rust boundary. A future separate small-guest image could omit Podman
and the agent runtime from its filesystem.
