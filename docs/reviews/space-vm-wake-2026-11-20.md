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
interruption recovery and invalid-checkpoint handling. The historical perf-02
plain-launcher experiment measured 1,638.89 ms p50 and 2,253.21 ms p95 from
arrival through the first validated reply. Those measurements describe the
experimental arm. Product timing uses the lane's own perf capture.

## Simplification opportunities

The worker seccomp JSON already serves as the single source for the guest
filter. The package launcher and carrier also share one fixed worker command
at the Rust boundary. A future separate small-guest image could omit Podman
and the agent runtime from its filesystem.
