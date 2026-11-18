# Release upgrade fixture

The disposable installation in `release-n.tar.gz` comes from the unchanged
release recorded in `manifest.json`. The generator exercises the live setup
command, WebAuthn registration and login, space and invitation operations,
Notes and Trash, thread creation, secret storage and machine enrollment.
The machine workspace also contains one sample user file.

The archive uses the public local conformance origin. All accounts, passkeys,
keys and values were generated for the fixture. The archive retains the
on-disk key records needed for the roots to open. The generator holds
session cookies and WebAuthn signing keys in memory and omits the setup token,
startup file, sockets, locks and SQLite SHM. SQLite WAL remains with its
database. Never create this fixture from an existing installation.

## Reproduce on dev-01

Build release N at the recorded revision with
`cargo build --locked -j6 -p calternal-server --features test-plugins` in the
Nix development shell. Save that executable as `release-n-calternal`.
Build the candidate from this revision with
`CALTERNAL_RELEASE_TEST_MIGRATION=v6 cargo build --locked -j6 -p calternal-server --features test-plugins --target-dir target/upgrade-next`.
Save a copy as `release-next-calternal`. The second build adds a disposable
machine registry table to exercise migration and downgrade behavior. The
production build leaves the machine schema at version 5.

From the candidate source root, inside the dev-01 Nix shell:

```sh
ulimit -c 0
npm run build:plugins
export CALTERNAL_RELEASE_N_BIN="$PWD/release-n-calternal"
export CALTERNAL_RELEASE_NEXT_BIN="$PWD/release-next-calternal"
export CALTERNAL_RELEASE_N_REVISION=SHA_FROM_MANIFEST
export CALTERNAL_RELEASE_FIXTURE_OUT="$PWD/tests/conformance/fixtures/upgrades"
scripts/check-podman-tests.sh cargo test --locked -j6 \
  --manifest-path tests/conformance/Cargo.toml --features release-upgrade \
  --test isolation release_upgrade_proves_compatible_handoff_and_refuses_an_incompatible_downgrade \
  -- --test-threads=1
scripts/check-podman-tests.sh cargo test --locked -j6 \
  --manifest-path tests/conformance/Cargo.toml --features release-upgrade \
  --test isolation checked_in_release_fixture_opens_after_upgrade -- --test-threads=1
```

Omit `CALTERNAL_RELEASE_FIXTURE_OUT` when checking a candidate without
replacing the checked-in sample. The live test creates new sessions for the
fresh replay proof. The archive smoke test checks startup, saved revisions
and workspace bytes without reusing a session. For the full conformance run
on the migrated build, set
`CALTERNAL_SERVER_BIN="$PWD/target/upgrade-next/debug/calternal"`.
The provider fixtures use the executable name `calternal` on their local PATH.

Real unattended release pairs use
[`scripts/verify-additive-upgrade.sh`](../../../../scripts/verify-additive-upgrade.sh)
with the two built images. That proof binds their exact digests and opens
the migrated roots with the shipped executables and packaged plugin manifests.
