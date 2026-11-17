# Recovery lane report — 2026-11-17

Historical version-one report. Superseded by [REVIEW-VALIDATION.md](REVIEW-VALIDATION.md)
for the independent review, fixes and incremental version-two implementation.

Implemented the protected security intent/completion coordinator and optional auth
barrier. The implementation and proof sketch are in [PROTOCOL.md](PROTOCOL.md),
and source citations/adaptations are in [PORTING.md](PORTING.md).

## Paths changed by this lane

- `crates/recovery/Cargo.toml`
- `crates/recovery/src/lib.rs`
- `crates/recovery/src/tests.rs`
- `crates/recovery/PROTOCOL.md`
- `crates/recovery/PORTING.md`
- `crates/recovery/VALIDATION.md`
- Root `Cargo.toml`: workspace dependency entry (membership already uses `crates/*`).
- Root `Cargo.lock`: generated entry for the new crate, reusing existing packages.
- `crates/auth/src/spaces.rs`: optional RecoverySink, process-wide per-database
  configuration, serialized prepare/commit/completion, receipt and fixture barriers.
- `crates/auth/src/account.rs`: host configuration entry point.
- `crates/auth/src/setup.rs`: account/token/enrollment commit barriers.
- `crates/auth/src/passkeys.rs`: add/remove commit barriers.
- `crates/auth/src/session.rs`: explicit revoke transaction and a cache fence that
  runs after SQLite commit even if recovery completion later fails.

Other shared-checkout changes were left with their owning lanes. No server, core,
store or web source was edited by this lane. No agents, commits, pushes, real data,
`.dev-data`, port 3210 or external messages were used.

## Source and test inventory

FO1 source tests copied: **0**. FO1 source tests ignored: **0**. The pinned source
has no FO1 coordinator implementation or suite; that absence is established by
port-map §6. Existing ported auth last-key/ownership tests and fsio swap/atomic
write tests remain intact and ran in the workspace suite.

New recovery tests: **14**, none ignored. They include 50 combinations of five
mutations and ten primary failure boundaries, plus 35 combinations of those
mutations and seven interrupted-replay boundaries. Mutations are a new membership
grant, membership revocation, role change, ownership transfer and passkey removal.
Additional tests cover actual account/space APIs, committed-session revoke fencing
on completion failure, uncommitted-witness rejection, partial record sets, missing
versions, missing protected directories, wrong installation UUID, truncated intents/completions and
bit-flipped intents/account records/space records, missing records, changed witness digests,
account/space ID collision, restrictive modes and symlink refusal. Space records
are checked for the absence of credential tables/material.

Fault injection surrounds fsio atomic writes and explicit directory barriers.
Interior file-fsync/rename/directory-fsync interruption behavior relies on the
existing fsio primitive tests; this is not a claim of injecting inside fsio's
private syscalls or performing physical power-cut qualification.

## Exact validation commands and results

All Cargo commands used the workspace toolchain wrapper and `-j4` on disposable
fixtures. Results describe the shared checkout at the time of each command.

| Command | Final result |
| --- | --- |
| `./scripts/with-toolchain.sh cargo test -p calternal-recovery -j4` | Exit 0; 14 passed, 0 failed, 0 ignored; 0 doctests. |
| `./scripts/with-toolchain.sh cargo test --workspace -j4` | Exit 0; 347 passed, 0 failed, 3 ignored across 37 test/doc-test suites. |
| `scripts/check-docs.sh` | Exit 0; Rust checks/rustdoc and TypeScript reference gates passed. |
| `./scripts/with-toolchain.sh cargo doc -p calternal-recovery --no-deps -j4` | Exit 0; reference generated, no missing-doc or broken-link diagnostics. |
| `git diff --check` | Exit 0. |

The documentation script explicitly enumerates pre-existing crates, so the new
crate also receives its own rustdoc run. It denies missing public documentation
and broken intra-doc links in its crate attributes. The script itself was left
outside this lane's ownership.

The three workspace ignores are in other lanes' server fixtures; this lane did
not add or modify them:

- `old_name_is_ignored_and_cleared_once_without_touching_host_session`: the fixture
  records the 2026-11-17 decision to omit obsolete cookie compatibility.
- `upgrading_schema_six_removes_cookie_state_and_keeps_passkeys`: the fixture
  records the 2026-11-17 decision to omit old schema compatibility work.
- `revoking_another_device_signs_it_out`: protected account mutation receipt replay
  is not ported; the source retry assertions are retained.

The workspace run emitted 42 server documentation/unused-mut warnings, outside
this lane's owned source. Recovery and auth compile without new warnings.

## Dependencies

These are direct dependencies of the new crate; their packages already existed
in the workspace lockfile. No dependency was added to the auth manifest.

| Dependency | Resolved version | Purpose |
| --- | --- | --- |
| calternal-auth | 0.1.0, workspace | Optional host recovery trait and auth errors. |
| calternal-fsio | 0.1.0, workspace | All atomic private writes, durable mkdir and folder fsync. |
| rusqlite | 0.41.1, bundled | Transaction-bound witnesses and complete logical table capture. |
| serde | 1.0.229 | Typed snapshot serialization. |
| serde_json | 1.0.154 | Versioned JSON envelopes with checksummed payload bytes. |
| uuid | 1.29.2, v7 feature | Stable bindings and fresh transaction UUIDv7 values. |
| blake3 | 1.8.7 | Intent, manifest and record integrity checksums. |
| thiserror | 2.0.21 | Private recovery diagnostics. |
| rustix | 1.1.6, fs feature | Pinned no-follow directory/file descriptors. |
| tempfile | 3.27.0, tests only | Disposable fixture directories. |
| tokio | 1.54.1, tests only | Exercise the actual async account worker APIs. |

## Limits and unfinished integration

- The sink remains optional and is not enabled by the server in this lane. Hosts
  must provision the private directory, validate the externally pinned installation
  UUID/root roles, hold every root lock and configure the sink before serving.
- Losing app state with an unfinished intent is ambiguous and **refused**. A
  surviving authoritative SQLite database is needed to resolve it. Completed
  snapshots can be validated/read for a later rebuild, but this does not claim
  full FO1 availability across that two-store state-loss window.
- Empty-state rebuilding and changes to startup refusal were intentionally not
  implemented. Namespace intents, external key files and later quota/invite
  mutation integration are outside these auth call sites. This is not a complete
  installation export or a production recovery qualification.
- The protected writer currently requires Linux descriptor paths; other platforms
  fail closed. It assumes the host protects the directory from same-UID plugins;
  file modes do not provide that sandbox.
- Full snapshots and retained versions grow with installation history; no
  compaction/retention or incremental performance qualification is implemented.
- Normal publication adds `4 + 3*(1 + live_spaces)` filesystem fsync calls, plus
  SQLite FULL commit durability; first provisioning and unfinished replay add work.

## Questions for the user

None. The chosen installation-private directory and versioned JSON format are
engineering choices within the authorized scope. Separate protected account and
space directories avoid coupling credentials to a member-visible content root;
no product policy was changed.
