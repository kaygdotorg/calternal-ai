# People over HTTP

Forgejo #37, thread `thr_v5xgiyrgrd`, branch `lane/people`. Started at
`f56c786`. The requests merge brought in `origin/dev` at `af4830f`
through `61f0d6c`, advancing its auth schema from version 7 to version 8.

The port follows the reference auth crate and ADRs 0058, 0059 and 0060 for
capability-based authority, encrypted single-use enrollment links and
group-scoped directory visibility. The HTTP handlers and the CLI use the same
operation declarations. A group grant follows its current membership through
a query, so there is no projection to refresh when that grant changes.
[People and access](../people.md) · [Glossary](../glossary.md)

Invite enrollment reuses the passkey ceremony and account provisioning
transaction. The same transaction creates the personal space, records the
destination-space grant, consumes the invite and stores the passkey and session.
An existing account can redeem through the authenticated operation. A completed
redemption can be replayed with the same request ID while the account has access
to the space.

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | Reuses `AccountId`, `SpaceId`, the common capability set and caller-safe errors. Group and invite UUIDs use the existing shared identifier macro. |
| 2. Dependencies | Auth uses pure core wire metadata and existing types, filesystem and rusqlite dependencies. OpenSSL is the same AES-256-GCM dependency already used for protected secrets. No new crate. |
| 3. Request locks | One owned SQLite connection and bounded auth mailbox order authority reads, writes and recovery publication for an installation. The connection has no mutex. The recovery lock covers the protected commit. The existing per-space content admission lock spans the receipt lookup, the cross-thread wait for the protected auth commit, and the journal receipt commit to keep one request ID exclusive across content and directory writes. Role batches share one protected commit. Access checks deferred by a batch run after its recovery publication attempt. |
| 4. Async safety | HTTP people handlers use blocking tasks with the existing bounded auth worker. The development-token status check awaits an indexed read on that worker. Enrollment verifies passkeys through the existing ceremony and stores results through the shared database worker. |
| 5. Read and flush counts | One cached SQL query expands and deduplicates capability grants during ordinary space admission. Member listing adds up to three queries in one snapshot for authority, the account seek and the group seek. The group query counts current active users through indexed membership joins. A scoped write checks current scope at host admission and again inside the authority transaction. A new role write has one content-journal receipt check, one journal commit, and a savepoint with the authority write and its atomic auth receipt. The whole batch shares one FULL auth commit and the recovery protocol's two file fsyncs per nonempty delta before compaction. |
| 6. Constant work | Discovery caches schemas and the host caches policies derived from the same operation table. Capability reads use a cached statement. The invite lookup subkey is derived once at startup. |
| 7. Bounds and retention | At most 100 live invites per space. Terminal invite rows are kept for seven days by default from expiry, redemption or revocation. The operator sets `CALTERNAL_INVITE_RETENTION_DAYS`. Startup and minute sweeps delete protected batches of at most 256, including idle spaces. People, groups, group members, space grants, and invite lists use 50-row UUID cursor pages. Accounts retain the source ADR's no-account-removal rule. |
| 8. Format | One disposable-preview schema, with no migration chain or old reader. The recovery inventory maps group grants to their owning space. |
| 9. Space UUIDs | Space operations use canonical UUIDs and scopes. Unknown and hidden spaces return the same response. Server-directory actions confer no automatic space membership. |
| 10. Declarations | The people operation table defines the operation, capability, risk, target and schemas together. `roles.list` reads the existing stored role definitions. Group-derived capabilities come from the same role rows as direct grants. |
| 11. Wiring | The host dispatches all operations over the normal HTTP and setup routes. The product CLI uses the same client and receipts. The operation inventory test covers each new binding. |
| 12. Reuse | Reuses `setup::insert_account`, the protected commit and receipt path, session revocation, `fsio::private::publish`, the shared constant-time hash comparison, and the reference auth crate's invite encryption and group visibility. |
| 13. Reference docs | Public contract, authority methods and identifiers carry updated comments. The docs gate covers the affected Rust and TypeScript crates. |
| 14. Matching docs | The repository and generated site describe command shapes, role authority, enrollment, and link handling. The shipped generated diff covers people-related pages. |
| 15. Format and size | Groups, grants, invitations, preview fixtures and protected commit code have separate modules. The server's existing startup file contains only command registration and dispatch wiring for the CLI. Workspace and standalone conformance formatting cover the edits. |

## Verification

Builds and tests ran as `ubuntu@calternal-dev-02-std` under
`~/Developer/lanes/people` with `nix develop -c`. The dev-01 integration
gate owns the Podman run. Logs are in `.people-gates/final/` under the lane directory.

```sh
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/people.tar
ssh ubuntu@calternal-dev-02-std 'mkdir -p ~/Developer/lanes/people && cd ~/Developer/lanes/people && tar xf -' < /tmp/people.tar
```

| Command inside `nix develop -c` | Result |
| --- | --- |
| `npm ci` | Pass after merging the plugin rename. |
| `npm run build:plugins` | Pass before Cargo tests. |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Pass. |
| `CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 60 passed, including the four people and invite cases. |
| `cargo test --locked -j6 -p calternal-recovery people_tests::` | Pass. Compared the protected recovery snapshot with committed authority after each people mutation. |
| `cargo test --locked --workspace --all-features -j6` | The machine-pool test failed because Podman is unavailable on dev-02. The test starts Podman before it can exercise the pool. |
| `cargo test --locked --workspace --all-features --no-fail-fast -j6 -- --skip pool_machines_serve_only_granted_accounts` | The provider resume test reproduced the known checkpoint assertion at `provider_turns.rs:291`. The run also caught two space-alias regression cases, fixed by retaining scope admission's not-found behavior. |
| `cargo test --locked -j6 -p calternal-server --test spaces_matrix` | All seven passed after the scope fix. |
| `cargo test --locked -j6 -p calternal-cli --test people` | Pass, including the checked-in human and JSON goldens. |
| `cargo test --locked --workspace --all-features --no-fail-fast -j6 -- --skip pool_machines_serve_only_granted_accounts --skip fake_official_cli_streams_checkpoints_crashes_stops_and_requires_explicit_resume` | 540 passed and one ignored, with the two named cases excluded. |
| `npm run test:integration` | Pass, one real-server integration case. |
| `npm run test:plugins` | Pass. The signed-in provider exercise is opt-in. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Pass. |
| `scripts/check-docs.sh` | Pass. |
| `cargo fmt --all --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml --check` | Pass. |
| `node --test scripts/docs-site/publication.test.mjs` | Pass. |
| `node scripts/docs-site/generate.mjs` and `node scripts/docs-site/generate.mjs --check` | Generated and checked on the VM. The branch includes the people pages and their navigation. |

The provider resume assertion compares the machine in a saved checkpoint as
soon as the new run reaches its running state. An independent baseline run
reproduced the assertion. [Threads lane report](threads-ui-ops-2026-11-17.md)

A separate disposable CLI smoke ran `calternal --json people roles`,
`groups create`, `groups add-member`, `people add`, `people members`,
`invites create`, `invites list`, and `invites redeem --secret-file -`
through a real local server. It verified the group count and the role granted
on redemption. The CLI renderer goldens cover twelve output shapes at 40, 80,
and 120 columns and in JSON.

## Directory bounds and auth worker follow-up

The follow-up at schema 8 passed 62 standalone HTTP cases, the qualified
workspace run, integration, plugin, contract, documentation, and formatting
gates on dev-02. The worker's four targeted tests passed after removing the old
connection mutex and its accessor. The new conformance cases cover account and
group page transitions, group member continuation, invitation retention at
startup and the live cap.

The reference app defaults an invite's expiry to seven days and supplies no
terminal-row purge. The follow-up uses that seven-day duration as the initial
retention default. Terminal expiry, revocation, and redemption start the clock
at the first terminal event. Pruning removes rows from the live database and
the current recoverable authority image through the same protected commit.

Development-host observations on `calternal-dev-02-std` used the debug server
and disposable data. Each client had its own warm loopback connection and
space. All used the same development Owner account. The role requests
alternated Reader and Editor with unique request IDs. There were 20 samples
per client after one warm-up call. The 32-client run also grows one account's
protected receipt history, so the difference from a single client includes
that workload, queueing, SQL, recovery and journal commits. These observations
are not product performance gates.

```sh
nix develop -c python3 perf/people-development.py .people-gates/calternal-before-bounds
nix develop -c python3 perf/people-development.py target/debug/calternal
```

| Operation | Clients | Before p50 / p95 (ms) | Follow-up p50 / p95 (ms) |
| --- | ---: | ---: | ---: |
| Member list | 1 | 0.542 / 1.088 | 0.836 / 1.047 |
| Member list | 32 | 9.259 / 12.602 | 6.173 / 7.312 |
| Role change | 1 | 18.349 / 22.038 | 14.866 / 16.873 |
| Role change | 32 | 1668.231 / 2766.419 | 1307.991 / 2085.439 |

## Protected role-write batching

Before grouping, each role change independently acquired the shared auth worker
and ran an IMMEDIATE transaction, a recovery proposal file sync, the FULL SQLite
commit, and a resolution file sync. Recovery recaptured the actor's retained
receipts on each commit. Each caller then waited for its space-local journal
receipt commit, which maintains request-ID exclusivity with content writes.

The worker now takes role writes from up to 32 queued jobs without pausing for
arrivals. It retains the order of writes and stops at another kind of write.
Pure session and capability reads between role jobs wait behind that batch.
Each request has its own authorization check, receipt and savepoint. One
proposal, one FULL SQLite commit and one resolution publish the successful
changes together. Every success waits for publication. If publication fails,
the worker returns a failure for each otherwise successful request. The
space-local journal receipt remains part of each successful HTTP write.

The same development-host command after the requests merge and batching ran on
dev-02 with the same fixture and sample count.

| Operation | Clients | Before batching p50 / p95 (ms) | After batching p50 / p95 (ms) |
| --- | ---: | ---: | ---: |
| Member list | 1 | 0.836 / 1.047 | 0.492 / 0.562 |
| Member list | 32 | 6.173 / 7.312 | 4.851 / 5.419 |
| Role change | 1 | 14.866 / 16.873 | 13.501 / 14.585 |
| Role change | 32 | 1307.991 / 2085.439 | 512.211 / 850.647 |

At 32 clients, role-change p95 fell by 59%. Compared with the one-client sample,
the final 32-client run added 4.857 ms p95 to a member list and 836.062 ms p95 to
a role change. That increase includes journal commits, queueing, and the cost of
recapturing the growing receipt history. The earlier and final runs used
different source revisions of the server around the requests merge. The
unoptimized builds had the same optimization level.

### Requests merge and batching verification

The dev-02 rebuild used `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`,
and `CARGO_PROFILE_TEST_DEBUG=0`. The post-merge Cargo runs were scoped to
the affected packages. The broad workspace and docs script runs above are
earlier lane evidence. Dev-01 owns the final workspace gate with Podman.

| Command inside `nix develop -c` | Result |
| --- | --- |
| `npm run build:plugins` | Passed before Cargo tests. |
| `cargo test --locked -j6 -p calternal-auth -p calternal-recovery` | 124 passed. Covered grouping queued writes, savepoint isolation of a refused change, withholding success on a publication failure, and erasure of request authority when the last group grant disappears. |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed. |
| `CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 68 passed. |
| `cargo test --locked -j6 -p calternal-cli --test people --test requests` | Both passed. |
| `cargo test --locked -j6 -p calternal-server --features test-plugins --test spaces_matrix --test requests_review` | 8 passed. |
| `npm run test:integration` and `npm run test:plugins` | Passed. One opt-in provider case skipped. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed. |
| `cargo doc --locked -j6 --no-deps -p calternal-auth -p calternal-core -p calternal-cli -p calternal-server --all-features`, `npm run check --prefix scripts/docs-tools`, and `node packages/genui/scripts/check-docs.mjs` | Passed. |
| `cargo fmt --all --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml --check` | Passed. |
| `node scripts/docs-site/generate.mjs` | Generated 271 pages. The checked-in CLI reference includes the merged command text. |

## Remaining decisions

- **Group-held server authority** needs a grant mechanism. Local operator
  provisioning keeps that authority off HTTP but requires host access. A
  protected HTTP grant can be audited and managed remotely, and needs
  last-administrator and delegation rules.
- **Delegated local group management** could expose the source's group-manager
  workflow with visibility checks, or keep membership edits in server
  operators' hands. Delegation reduces operator work and carries the scope of
  every space grant held by the group.
- **Disabled sole owners** remain stored as Owner and lose access. Requiring
  prior transfer preserves an active Owner but can delay disable. A separate
  protected recovery action would need explicit authority for shared and
  personal spaces.
- **Invitation lifecycle** could extend expiry in place or use revoke and
  reissue. In-place extension keeps the credential already distributed.
  Reissue rotates it. Plan and quota enrollment needs its own server-scoped
  policy. The space link currently grants only its recorded role.
- **Visible people picker** could use a visibility-filtered query outside the
  server administration directory, or rely on links and exact account IDs.
  Directory pages already have a fixed bound.

## Simplification opportunities

- Use the same authority-aware admission primitive for directory and content
  writes so the current scope can be checked once under the operation's
  transaction boundary and journal receipts can still replay safely.
- Move existing CLI HTTP transport out of the account-input module. The people
  commands use the same credentials, retries and output rendering now.
- Recovery could capture directory receipts by scope, keeping a busy account
  from recopying its receipt history across all spaces in each protected batch.
  That would change the recovery record boundary and need fresh crash tests.
