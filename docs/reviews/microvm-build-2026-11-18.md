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
passed. Two preceding live attempts failed with ENOENT at sandbox start.
A later reproduction traced the same symptom to a garbage-collected ext4
formatter path. The machine-level live suite passed five cases and ignored
the perf-only benchmark. Both leak checks passed.

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
the interrupted state and calls `machines.resume`. It checks foreign resume
is refused and the owner sees the retained workspace with a different boot
ID. It then deletes the account through the public account operation and
checks that the durable machine erasure queue completes and the disk is
removed. The untouched account keeps its original boot ID. The capacity
fixture also cancels a waiting lease and checks that it never gets assigned.

The dev-01 command used `scripts/check-podman-tests.sh cargo test --locked
-j6 --manifest-path tests/conformance/Cargo.toml --test isolation microvm_cases --
--test-threads=1 --show-output` with the absolute stage 3 guest configuration.
The run after merging `45fa2c0` passed all three cases in 17.23 seconds.
Both leak checks passed. This exercises the retained workspace behind the
provider checkpoint flow.
The full provider resume example in ordinary conformance uses its separate
fixture.

## Release artifact checks

The first locked perf attempt stopped at KVM probing. The dev-built static
binary reproduced the failure under `strace -f -e trace=ioctl` on dev-01:
`KVM_GET_API_VERSION(0x1)` returned `EINVAL`. Passing an explicit zero in
both variadic KVM calls returned API version 12 and created a VM.

A separate `strace` reproduction traced the sandbox start `ENOENT` to
`execve` of a garbage-collected Nix ext4 formatter. The deployment command
now roots a dedicated Nix runtime profile. With that profile and the fixed
static test binary, the live isolation and lifecycle case passed on dev-01.
The Podman and Firecracker leak checks passed. A restricted PATH test also
confirmed that the Firecracker check still detects a fixture leak when the
host has no Podman.

## Backend gate commands

Run these in the development VM lane. Each cargo build or test uses six jobs.
The gate runner prints the ordinary conformance skip reason and keeps each
command's output in `apps/docs/.cache/gates/`.

```sh
nix develop -c npm run build:plugins
nix develop -c cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins
nix develop -c ./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features -j6
nix develop -c env CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 \
  --test isolation -- --test-threads=1 --show-output
nix develop -c npm run test:integration
nix develop -c npm run test:plugins
nix develop -c npm test --workspace @calternal/genui
nix develop -c cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
nix develop -c sh scripts/check-docs.sh
nix develop -c node --test scripts/docs-site/publication.test.mjs
nix develop -c cargo fmt --all -- --check
nix develop -c rustfmt --edition 2024 --check tests/conformance/microvm_cases.rs
```

The supplementary serial KVM run exports `CALTERNAL_MICROVM_TEST_CONFIG`
as an absolute operator config path. It runs the same isolation command with
the `microvm_cases` filter and this machine-side case group.

```sh
nix develop -c ./scripts/check-podman-tests.sh cargo test --locked -j6 \
  -p calternal-machine --lib sandbox::microvm -- --nocapture
```

## Review gate

| Item | Verification |
| --- | --- |
| 1. Shared types | Uses existing account, space and thread IDs, machine lease and output types, and readiness contracts. |
| 2. Dependencies | Uses dependencies already available to the existing machine and server crates. New operator tooling uses Python's standard library and nftables. |
| 3. Lock scope | The server uses the existing installation-wide machine registry. One account's guest lock spans disk binding and release to keep a single disk owner. TAP and account maps hold short in-memory locks. |
| 4. Async I/O | Host file and process operations use Tokio. Policy verification, image hashing, directory sync and durable rename run on blocking tasks. |
| 5. Reads and flushes | Warm exec uses one authenticated vsock channel with no explicit host filesystem reads or flushes. The capacity check adds in-memory lookups and no SQL. Disk binding flushes the new disk and affected directories. |
| 6. Constant work | The role verifies and loads the immutable image and network policy at startup. Slots are in memory. The size table is constant. |
| 7. Bounds | The writable disk is capped per account, the config bounds guest count and the warm pool has at most two entries. Image operations retain one active and one rollback root. Account erasure removes its durable disk. |
| 8. Compatibility | The pool uses the current machine protocol. There are no database migrations or alternate image readers. |
| 9. Space identity | Space homes, account disks and thread workspaces use the existing UUID identities. |
| 10. Single declaration | The size profile declares the 512 MiB limit. The machine and server both read it. One policy supplies DNS, allowed destinations and TAP slots. |
| 11. Callers | Machine CLI selects the backend. The server uses its existing admission, readiness and erasure paths. |
| 12. Reuse | Uses the existing queue and lease fence, durable fsio rename and directory sync, provider-home logic, and Podman exec. |
| 13. Public docs | The docs gate checks Cargo and TypeScript references. Public configuration fields document units, bounds and startup handling. |
| 14. Documentation | Deployment covers rollback, image updates, KVM access and operator-managed firewall policy. Live proof is recorded here and on issue 51. |
| 15. Formatting and size | The machine workspace and the changed conformance fixture have explicit formatting gates. Capacity accounting moved out of the server's state module. |

## Simplification opportunities

Keep the immutable root shared across all guests and ship a new image as one
operator update. The writable account disk and existing lease queue are
sufficient for workspace recovery and admission. The firewall receipt lets
the machine role open its assigned TAPs without running a firewall manager.
Installing the provider adapter under the standard executable path could
remove the extra PATH entry from both the OCI image and guest launcher.
