# People over HTTP

Forgejo #37, thread `thr_v5xgiyrgrd`, branch `lane/people`. Started at
`f56c786` and merged `origin/dev` at `a7a5275` in `e42f3a3`.

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
| 3. Request locks | One owned SQLite connection and bounded auth mailbox order authority reads, writes and recovery publication for an installation. The redundant connection mutex was removed. The recovery lock covers the protected commit only. The existing per-space content admission lock spans the receipt lookup, the cross-thread wait for the protected auth commit, and the journal receipt commit to keep one request ID exclusive across content and directory writes. Moving reads off the auth worker would need a publication fence to keep them from seeing a SQLite grant before recovery publication. |
| 4. Async safety | HTTP people handlers use blocking tasks with the existing bounded auth worker. The development-token status check awaits an indexed read on that worker. Enrollment verifies passkeys through the existing ceremony and stores results through the shared database worker. |
| 5. Read and flush counts | One cached SQL query expands and deduplicates capability grants during ordinary space admission. Member listing adds up to three queries in one snapshot for authority, the account seek and the group seek. The group query counts current active users through indexed membership joins. Writes recheck authority in their immediate transaction before using auth receipts. Space-scoped writes check one content-journal receipt and publish one journal commit, in addition to the auth write and its recovery publication. The recovery protocol has two file fsyncs per nonempty delta before compaction. The host admission and transaction boundary both read current scope for scoped writes. The existing adapter checks the scope before replay and verifies the typed UUID again when it selects the receipt store. |
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

The 32-client run added 5.337 ms p50 and 6.265 ms p95 to a member list,
and 1293.125 ms p50 and 2068.566 ms p95 to a role write, compared with the
one-client follow-up sample. The auth worker holds the full protected
authority transition in order. Recovery captures the actor-owned receipt rows
alongside each change. Changing that capture boundary or separating readers
would need a recovery design change with fresh crash verification.

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
