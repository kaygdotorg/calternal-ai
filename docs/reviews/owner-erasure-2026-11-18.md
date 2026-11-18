# Owner erasure qualification

This follow-up covers the live owner fence, space deletion, and the trusted
account deletion route. Existing app ADR 0058 supplies the last-administrator
invariant and leaves account removal unimplemented. The preview reuses its
current roles and the existing confidential-storage recovery barrier.
[Issue #43](https://git.kayg.org/kayg/calternal-ai/issues/43).

## Read and flush accounting

| Step | Reads | Flushes |
| --- | --- | --- |
| Ordinary durable admission | Zero new file or SQLite reads. The owner lifecycle takes short sharded in-memory locks to admit and release a step. | Zero additional flushes. |
| Account authority cutover | An active-principal check, one installation lookup, target existence and people-manager checks, and one owned-space query with indexed direct and group membership probes. An admin deleting another account adds the administrator capability lookup. An active administrator target adds an existence check for another manager. Installations are returned by the deletion statement. | One auth FULL commit and the existing two recovery log file syncs. |
| Explicit space cutover | Existing member enumeration, one space receipt lookup and the protected authorization and receipt queries. Pending content publication runs only if the space has an unpublished intent. | One auth FULL commit and two recovery log file syncs. The removed journal receives no commit. |
| Physical cleanup per owner | One metadata check per root parent, owner-local key and fingerprint reads when it has secrets, and indexed cleanup of item or account routes. Removal visits only the selected owner trees. Placement cleanup visits current leases and the matching persisted rows. | One key directory sync when a key exists, one parent sync per present root parent, and conditional FULL commits for key bookkeeping, catalog removal and placement cleanup. Existing lease-stop commits apply to active leases. |
| Retire work in retained spaces | One indexed job update per distinct space in the existing job route map, with cancellation IDs returned by that update. | One local FULL transaction per visited store. Only stores with changes append WAL frames. |
| Complete one batch | One protected erasure inventory read at startup. The normal HTTP path carries the UUIDs returned by cutover. | One auth FULL completion commit and two recovery log file syncs. Forced compaction adds two file and three directory syncs when the segment contains frames. SQLite truncates the auth WAL before completion, with its own checkpoint syncs. |

These are code-path counts and conditional costs. The existing recovery
coordinator serializes the two protected publishes and compaction. Cleanup
and drain run outside that worker.

## Shared owner policy

A person or administrator must transfer a shared space before deleting its
sole Owner. Transfer keeps the space and needs a current member. Archiving
would require a custodian and retention rules. Deleting the space would
remove other members' content. The refusal names affected spaces, including
shared membership through a group.

## Crash and isolation coverage

| Crash point | Required outcome on restart |
| --- | --- |
| Auth SQLite commit before the recovery resolution marker | Existing witness resolves the committed proposal. Account and personal space remain revoked. |
| After the protected publish, before drain and key unlink | Startup closes both UUIDs and resumes cleanup. |
| After key unlink, before removing the trees | Neither the erased key nor the account is revived. The remaining trees are removed. |
| After file and tree removal, before marking complete | Repeating cleanup succeeds and retains the permanent erasure record. |

At each cutover, the old cookie and passkey fail to sign in. Both the
account and its personal space are absent from all three roots. The recovery
suite also reads the protected erasure record after forced compaction and a
fresh coordinator open.

The HTTP isolation suite checks unauthorized cross-account deletion, stale
passkey proof, successful reverification, an administrator and the account
owner both receiving the transfer refusal, the successful transfer, the
remaining member's content, account-local key and file erasure, and the CLI's
human and JSON results. An enrolled machine and an issued thread token lose
access, and the retained space loses the deleted person's protected requests
and thread authority. Its space case verifies removal from all roots and
absence after restart. The cached-writer regression on baseline
`246cdb7` committed a new note after HTTP deletion. The identical test on
the candidate expects absence and verifies that the tree stays removed.
The old test ran from an isolated `git archive 246cdb7` with only the
regression appended to `crates/server/tests/secrets.rs`. Its command was
`cargo test --locked -j6 -p calternal-server --features test-plugins --test secrets stale_space_writer_cannot_reopen_after_http_delete -- --exact --test-threads=1`.
It exited 101 on the note returned by the stale write.
The content-level test also demonstrates that erasure waits for an admitted
durable step while rejecting the next step on a retained handle.

## Review gate

1. Reused shared UUIDs, capabilities, errors, and the existing storage owner
   type, now defined in `calternal-types`.
2. Owner identity and admission use existing dependencies. No crate was added.
3. The sharded registry is held only for in-memory admission. Drain runs
   outside the auth worker. Recovery serialization covers protected commits
   and compaction with the existing barrier.
4. HTTP cleanup uses a blocking worker. Store opens, directory syncs, and
   SQLite work stay on blocking workers.
5. Normal admission adds zero reads or flushes. Cutover and cleanup counts
   are in the table above.
6. The lifecycle is initialized once per installation. Recovery and database
   metadata use the existing coordinator and prepared statement paths.
7. Only erased UUIDs and existing key fingerprints are retained permanently,
   per the owner decision of 2026-11-18. Existing receipt, job, and subscription
   bounds apply.
8. Auth schema version 9 is the single preview schema. A version 8 preview
   database requires fresh development data.
9. Storage and operations use typed UUIDs. Names only identify refused spaces
   in the message.
10. The fresh-proof rule is shared with existing auth. The core route
    inventory supplies the endpoint and the operation uses existing roles.
11. The host wires startup reconciliation and live cleanup. The CLI calls
    the protected HTTP route.
12. Reused the per-space lock, witnessed auth recovery, key unlink and
    fingerprint protocol, SQLite job cancellation and machine lease stop.
13. Public Rust additions carry doc comments. The docs gate includes crate
    and site verification.
14. The operator, CLI, recovery and documentation-site sources describe the
    shipped behavior.
15. The space cutover, auth cutover, server cleanup, machine cleanup and
    owner lifecycle live in separate modules. Qualification includes the
    workspace and standalone conformance formatting checks. The pre-existing
    larger store and test harness files keep their existing organization.

## Verification

Gates ran from the lane directory on dev-01 using `nix develop -c`.
The Rust gates used `CARGO_PROFILE_TEST_DEBUG=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, and `RUST_TEST_THREADS=1`.

```sh
npm run build:plugins
cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins
./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features --no-fail-fast -j6
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_TARGET_DIR="$PWD/target" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml \
  -j6 --test isolation -- --test-threads=1 erasure_cases
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_TARGET_DIR="$PWD/target" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml \
  -j6 --test isolation -- --test-threads=1
npm run test:integration
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 \
  -p calternal-client --test real_server real_server_operations_receipts_and_authority \
  -- --ignored --exact --test-threads=1
npm run test:plugins
CALTERNAL_WORKER_TEST_BACKEND=podman ./scripts/check-podman-tests.sh \
  cargo test --locked -p calternal-server --all-features -j6 --test plugin_host -- --test-threads=1
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
python3 contract/verify-cli.py
node scripts/docs-site/generate.mjs
CARGO_TARGET_DIR="$PWD/target" scripts/check-docs.sh
cargo fmt --all -- --check
rustfmt --edition 2024 --config skip_children=true --check \
  tests/conformance/isolation.rs tests/conformance/erasure_cases.rs \
  tests/conformance/requests_cases.rs tests/conformance/requests_review_cases.rs
node scripts/docs-site/generate.mjs --check
```

The workspace passed 577 tests. Its one opt-in Rust client integration test
passed when run separately against the built server. Conformance passed all
85 cases, including the three erasure cases run separately. The Node
integration test passed. The TypeScript plugin suites passed 23 tests and
skipped the existing opt-in real Claude case. The Podman plugin host passed
18 tests, and all four scoped container leak checks passed. The operation
contract and CLI smoke checks exited successfully. The documentation gate,
both formatting checks, and the drift check for 296 documentation pages
passed. The upstream merge changed UI and documentation paths. The backend
and conformance inputs matched the verified candidate. The lane's dev-01
target directories were removed after qualification.

## Simplification opportunities

Account erasure visits the job stores indexed in the in-memory route map.
An existing account-to-job reverse route, if introduced for user-facing job
lookup, could also limit that cleanup to affected stores. Account feed
readers could release idle SQLite connections sooner on lifecycle close
using their existing session invalidation.
