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
explicit 512 MiB image run under the leak checker and strace passed all
three microVM conformance cases. Before that run, two live attempts failed
with ENOENT at sandbox start. The machine-level live suite passed five
cases and ignored the perf-only benchmark. Both leak checks passed.
