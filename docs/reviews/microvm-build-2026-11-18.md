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
