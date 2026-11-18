# Per-person microVM build proof

## Stage 1

The dev-01 build produced a digest-pinned read-only root with a normalized
OCI filesystem. Podman uses the filesystem as its overlay lower layer.
Its private uppers and the journaled home share the bounded account disk.
The machine opens KVM as its service user, verifies API 12, and creates a
VM before selecting Firecracker. A nested Proxmox guest can exercise the
same check. Podman fallback advertises `sandbox_fallback: "kvm_unavailable"`
through machine readiness.

- `nix develop .#microvm -c deploy/microvm/build-image.sh ...` built the guest.
  The first build exposed a Nix builder restriction on upstream set-ID modes.
  Rootless OCI unpack outside the derivation and a normalized archive resolved it.
- With `CALTERNAL_MICROVM_TEST_CONFIG` pointing to the 512 MiB guest,
  `scripts/check-podman-tests.sh cargo test --locked -j6 -p calternal-machine --lib sandbox::microvm -- --nocapture`
  reported five passes and the perf-only benchmark ignored. The live test
  covered separate boot IDs, thread root overlays, retained files after crash,
  and erasure. A readable non-KVM device was rejected.
- `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal` and the same guest config
  were used for the serial conformance filter `microvm_cases`. Both cases
  passed, including the entitlement check for container fallback readiness.
  Both commands passed the Podman and Firecracker leak checks.

## Stage 2

The 512 MiB profile declares two active threads in the machine protocol.
The server accounts for them across spaces and releases a place when a
lease closes. The machine checks the same profile before allocating local
work. The third thread sees the existing waiting state and queue position.
The test grants the pool to two accounts and checks that the other account
can still start while the first is waiting. The 1,024 MiB profile has no
measured per-account cap yet.

On dev-01 the plugin build, test-plugin server build, all-features
workspace tests, integration, plugins, GenUI, contract, docs, publication,
and formatting gates passed. The serial conformance rerun passed 117 cases
with the live KVM case visibly skipped when no image was configured. An
explicit 512 MiB image run under the leak checker passed all three
microVM conformance cases in 12.23 seconds. An earlier traced run also
passed. Two preceding live attempts failed with ENOENT at sandbox start
and the cause was not isolated. The machine-level live suite passed
five cases and ignored the perf-only benchmark. Both leak checks passed.

## Stage 3

The operator installs one IPv4 policy in a dedicated firewall namespace.
The machine accepts its root-owned installation receipt only for the
current host boot and network namespace. It parses the policy once at
initialization and attaches one leased TAP to each networked VMM. The
microVM's rootless thread container uses its guest-local user-mode network.
Worker containers and the KVM fallback have no network. The role needs
access to KVM and its assigned TAPs. Firewall administration and namespace
setup stay with the operator.

On dev-01, `python3 deploy/microvm/test-egress.py` passed both policy
compiler tests. `tests/support/microvm-egress.sh VM_CONFIG MACHINE_TEST_BINARY`
passed with the real KVM guest under the leak checker. The thread resolved
an approved DNS record and opened its declared TCP service. Unlisted TCP,
the host gateway, the peer TAP route, the operator-denied control plane
address, and metadata were denied. nftables counters confirmed the peer
route and unsolicited ingress drops. The native suite tests explicit
missing policy and missing receipt. A separate mount namespace covering
`/dev/kvm` with `/dev/null` verified that an unusable KVM device produces the
visible isolation skip.

The negative tests used synthetic upstream services. A separate dev-01
run used the same guest image and firewall installer with real IPv4
egress. The installed policy allowed the resolved addresses for Anthropic
and the npm registry only, plus its configured DNS resolver. It returned
HTTP 200 from the npm registry. Official Claude Code 2.1.326 completed a
one-answer turn (3.04 seconds) and a coding turn (7.59 seconds) in rootless
thread containers. The coding turn read the fixture modules, corrected a
bug and passed the Node test suite, which the harness reran independently.
Both completed with zero cgroup OOM kills and no guest swap. These are
development-host observations. The fixture provisioned the Firecracker TAP
directly and exercised the shipped guest agent and container launcher.

The ordinary dev-01 backend gate passed all ten checks, with 117 serial
conformance cases and a visible skip for the hardware fixture when no
guest config was supplied. With the absolute guest config, the machine
microVM suite passed eight cases, with the perf benchmark ignored, and
all three filtered HTTP conformance cases passed. Both leak checks passed.

## Stage 4

The HTTP fixture now verifies that two accounts have different boot IDs
while their sibling threads share only their account's kernel. It kills the
first account's exact Firecracker process during a leased command, waits for
the interrupted state and reopens the same workspace with a different boot
ID. It then deletes the account through the public account operation and
checks that the durable machine erasure queue completes and the disk is
removed. The untouched account keeps its original boot ID. The capacity
fixture also cancels a waiting lease and checks that it never gets assigned.

The dev-01 command used `scripts/check-podman-tests.sh cargo test --locked
-j6 --manifest-path tests/conformance/Cargo.toml --test isolation microvm_cases --
--test-threads=1 --show-output` with the absolute stage 3 guest configuration.
All three cases passed in 11.39 seconds. Both leak checks passed. This
exercises the retained workspace behind the provider checkpoint flow.
The full provider resume example in ordinary conformance uses its separate
fixture.
