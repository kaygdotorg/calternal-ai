# Isolation and authorization conformance

Standalone Rust integration suite against **our own disposable server** over TCP.
No production or existing preview instance is a target. Linux `/proc` discovers
an ephemeral listener; `ORIGIN` is the fixed WebAuthn relying-party origin, not a
fixed listening port. Each fixture owns a fresh temporary root and kills/reaps
only its child process. Run serially to keep race evidence interpretable.

The binary must be built with the `test-plugins` feature: fixtures pass
`--test-plugin-root tests/plugins` so the test-only `jobs-demo` plugin (plugin
writes, jobs, streams, cancellation) is trusted; `jobs_cases.rs` holds those cases.

Accounts enroll and sign in through the HTTP passkey routes using the same
software-authenticator approach as `crates/server/tests/account_common/mod.rs`.
The software authenticator cannot store discoverable credentials, so fixtures
check the server requested resident credentials, then emulate discovery with
explicit credential IDs and the saved user handle. This proves signed WebAuthn
verification, not platform authenticator/browser behavior. Account provisioning,
otherwise-unavailable membership admission, capability subtraction and absolute
session expiry injection use offline Auth APIs/rusqlite while the server is down.
No asserted content access uses a privileged API or development credential.
One explicit opt-in test starts `--development` to prove the flag boundary.

On hd01, **edit/sync only**. Build and test on the dedicated dev VM lane:

```sh
# From /home/kayg/Developer/Calternal-AI on hd01:
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf - | ssh claude@calternal-dev-01-compute02 'mkdir -p ~/Developer/lanes/isolation-conformance-sol && cd ~/Developer/lanes/isolation-conformance-sol && tar xf -'

# In ~/Developer/lanes/isolation-conformance-sol on the dev VM:
nix develop -c cargo build --locked -j4 -p calternal-server --features test-plugins
nix develop -c npm ci --ignore-scripts --no-audit --no-fund
nix develop -c npm run build:plugins
nix develop -c sh -c 'umask 022; CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CONFORMANCE_RESULTS="$PWD/tests/conformance/corrected-results" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j4 --test isolation -- --test-threads=1'
```

`CONFORMANCE_RESULTS` enables schema/permission, response and race evidence JSON;
it contains no credential secrets or database contents. Logs and evidence live in
this directory; `previous-run/` preserves the inherited partial lane's artifacts
and is historical. `final-results/` preserves the initial failing qualification;
`corrected-results/` contains the current passing evidence. The initial
qualification exposed four product gaps. Their regressions remain enabled and
pass after the authorized server/auth corrections. See
[the report](../../docs/reviews/isolation-conformance-sol.md) for exact results,
severity, limits, structural weaknesses and every table's separation boundary.

The retained cursor test asserts that no foreign text leaks. The additional
`foreign_history_cursor_is_refused` test asserts the stricter refusal contract;
it reproduced acceptance of the previous unbound numeric cursor and now passes
with revision-bound continuations. Required-capability
tests assert a refusal without a capability and success after restoring it, so a
schema error cannot masquerade as successful authorization enforcement. Inventory
guards detect additions to operation discovery and account routes.

Timing comparisons are three-pair, 20 ms median-difference smoke checks on the
shared development VM; they provide no constant-time or performance qualification.
SIGKILL tests cover scheduled interruption and acknowledged/uncertain outcomes,
not deterministic cuts inside every transaction or physical power loss. The
worker race uses a disposable runtime wrapper and a readiness/release barrier,
then invokes the unchanged, digest-pinned shipped bundle.

`live_cases.rs` exercises the journal SSE route with real passkey sessions:
unknown/foreign spaces match, foreign bookmarks reset without content, missing
credentials refuse, reconnect after server/client restart replays backlog, and
logging out closes an idle subscription.

`wallpaper_cases.rs` checks the built-in catalog and the six anonymous sign-in
renditions with real passkey sessions, an unconfigured server, and synthetic
images in a disposable cache. It uses no Unsplash network access.

`microvm_cases.rs` uses a disposable enrolled pool machine and two passkey
accounts. It checks separate kernel boot IDs, same-account thread isolation,
foreign exec refusal, guest crash interruption, reopening a retained
workspace, and durable account disk erasure. It also checks 512 MiB queueing
and cancellation and fallback visibility. Set
`CALTERNAL_MICROVM_TEST_CONFIG` to an operator config for the pinned guest in
the [deployment guide](../../docs/deploy.md#build-and-configure-a-disposable-guest).
Use a configuration with capacity for two guests and a machine user who can
open `/dev/kvm` read-write. The case prints the skip reason if KVM access or
the config is missing. Pass `--show-output` to see that detail in a serial run.

## Release-to-release conformance

Run `scripts/verify-additive-upgrade.sh PREVIOUS_IMAGE CANDIDATE_IMAGE PROOF_OUTPUT`
inside the dev-01 Nix shell with both exact product images in local Podman. The
gate extracts the shipped executables, writes a sample through release N's
authenticated API and CLI, and starts N+1 on those roots. It restarts N on the
migrated roots and checks that both releases can write and read each other's
acknowledged writes. The resulting proof binds the two image IDs for deployment.

The opt-in `release-upgrade` cases also exercise process loss at the intent,
snapshot, version check, migration, verification, restore, and commit
boundaries using debug binaries. The retained sample and generation commands
are in [the fixture directory](fixtures/upgrades/README.md).
