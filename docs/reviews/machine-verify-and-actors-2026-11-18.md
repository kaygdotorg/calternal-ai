# Machine verification and journal attribution

Issue [#43](https://git.kayg.org/kayg/calternal-ai/issues/43), branch
`lane/thread-streaming`. The work uses disposable data on
`claude@calternal-dev-01-compute02`.

- `baea22c` records the admitted Actor on content journal writes.
- `3dd4427` adds enrolled machine verification.
- `9979acc` makes the auth worker's role-order tests deterministic.

## Role-batch ordering

The old test used `Sender::capacity()` to count queued writes. A Tokio sender
can hold a reservation before it publishes a message. The test's read could
therefore see the original `reader` role before the `editor` write arrived.
The worker collects role writes in mailbox order, applies a savepoint per
request, and releases its corresponding result after recovery publication.
Deferred reads may see a later role within that batch. Single jobs remain
ordering boundaries. Production authority code needed no change.

The deterministic regressions queue exact roles for one member, bracket two
batches with a paused ordinary job, and hold a reservation to reproduce the
early read. They check each result, rollback of a rejected savepoint, the final
role and the number of publications. With six concurrent test processes,
300 repetitions of the original test produced 9 failures on dev-02 and 1 on
dev-01. The captured failures have the same intermediate read assertion. The
four replacement role-batch tests passed in 300 repetitions on each VM.

## Machine verification

`machines.verify` uses one exact enrolled machine and one space in which the
caller can use plugins. An account owner can check their own machine. A pool
machine requires the existing server settings capability. The runner uses the
fenced Check job, machine channel and readiness cache. The diagnostic sends no
agent credential or space instructions. The response reports sandbox launch and
the provider check, including the sign-in command if reported. Placement and
execution each get six seconds to fit the existing 15-second client timeout.
The response can be replayed with the same request ID. A restart ends any
in-progress diagnostic so it does not occupy an active slot or repeat on another
machine.

Verification uses the existing per-space admission receipt and job commits. Its
target check uses one indexed authority lookup and one machine registry lookup
at admission and delivery, plus a pool authority recheck before allocating a
pool lease. Admission loads the job once. If it is still active, the outcome
waiter reads one job row on subscription and once after the run settles.
Progress wakes read no database row. The typed Check event updates the existing
memory cache with zero SQLite or filesystem reads and zero flushes.
Verification reads no account retry settings and writes no agent scope.

## Journal attribution

An admitted person, agent or plugin supplies the existing Actor identity to a
content mutation. The journal carries `data.actor` alongside the established
account author. The content transaction keeps any delegated route in the same
outbox row as the result. Replay uses that route with the original account, and
receipts retain their original result envelope. This adds zero SQLite reads,
userdata reads, transactions or flushes to content admission and publication.
Older journal entries retain their original account author field.
The existing `trash.expire` maintenance event continues to attribute retention
to the account that put the item in Trash and identifies the automatic action
through its event kind.

## Account deletion still open

The original app at `origin/dev` explicitly excludes account removal in
`docs/adr/0058-roles-are-capability-sets.md`. It preserves the last person with
`people.manage`. The current app requires a sole Owner to transfer ownership
before leaving an owned space. Issue #43 calls for refusing with the affected
space names when a departing account is the sole Owner of shared content. A
transfer preserves content and requires an eligible member. A future approved
archive requires a custodian and retention rules. A future approved destructive
disposition removes shared data and needs separate consent.

The existing recovery barrier can durably commit revoked authority and key
tombstones, and startup can replay a key unlink. It does not retire cached
content, jobs, account-input handles, and machine leases against concurrent
writes. Removing an account directory alone would let a request already
admitted recreate it. Deleting the key row or directory before publishing the
authority fence could also leave a recoverable account with lost data. The
account deletion HTTP operation and CLI remain unwired, so there are no
observed deletion reads or flushes.

The implementation needs a protected account-and-space erasure manifest
replayed before startup admission. It must fence new writes and drain or retire
the affected handles, revoke sessions and machine and turn authority, then
remove keys and owned trees, with the permanent erasure record retained. An
existing protected commit has a log intent sync, a FULL SQLite commit and a log
completion sync. File cleanup then adds a key directory sync per erased owner
and a directory sync per affected root after durable removal. Those are
primitive costs, pending measurement of the full deletion. Offline enrolled
machines can retain local provider homes after authority revocation. Any
managed wipe must be delivered and acknowledged by the machine, with pending
work visible until reconnect.

The recovery segment can still hold older plaintext directory rows after their
deletion is committed. Forced compaction would erase the superseded active
generation at a cost of two file syncs, three directory syncs and a rewrite of
the current protected inventory. Independent old backups require their own
retention and restore rules. An offline maintenance operation could drain the
installation using its root guard, at the cost of service downtime. Live HTTP
erasure needs owner-local retirement so unrelated accounts remain available.

## Verification gates

Commands ran under `nix develop`. Test commands used Cargo's `-j6` limit.
The workspace used `RUST_TEST_THREADS=1`. The standalone conformance command
set `CARGO_TARGET_DIR=$PWD/target` and
`CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal`.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Passed before testing. |
| `cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins` | Passed. |
| `./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features --no-fail-fast -j6` | 573 tests passed before the test-only role commit. |
| `./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 82 passed. The four focused isolation and replay cases also passed separately. |
| `npm run test:integration` | 1 passed. |
| `npm run test:plugins` | 23 passed, 1 skipped by the existing suite. |
| `CALTERNAL_WORKER_TEST_BACKEND=podman ./scripts/check-podman-tests.sh cargo test --locked -p calternal-server --all-features -j6 --test plugin_host -- --test-threads=1` | 18 passed. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed. |
| `python3 contract/verify-cli.py` | Passed. The workspace also exercises human and JSON `machine verify` output through a real server. |
| `cargo test --locked -j6 -p calternal-auth --lib --no-run --message-format=json` and direct libtest invocation | Passed on dev-01 and dev-02. The before loop invoked the exact original test. The after loop invoked `db::role_batch_tests --test-threads=1`. Six processes ran at a time on each VM. |
| `cargo test --locked --all-features -j6 -p calternal-auth -p calternal-recovery --lib` | 34 auth tests and 38 recovery tests passed after the role test change. |
| `cargo fmt --all -- --check` and `rustfmt --edition 2024 --config skip_children=true --check tests/conformance/agent_cases.rs tests/conformance/threads_cases/streaming.rs` | Passed. |
| `CARGO_TARGET_DIR=$PWD/target scripts/check-docs.sh` | Passed. |
| `node scripts/docs-site/generate.mjs --check` | Checked all 294 pages after regeneration on dev-01. This branch includes the updated Actor page. The two Recovery pages with inherited source drift are outside this lane. |

The Podman leak check passed for each wrapped test run. The dev-01 full gate log
is `~/Developer/lanes/thread-streaming/.lane-gates/43-final.log`. The contention
logs on both VMs are `~/Developer/lanes/thread-streaming-role/role-{before,after}.log`.
The final merge incorporated `origin/dev` at `a4ce199`. The incoming changes
were web, documentation, the JS lockfile and Nix browser dependencies. The
backend, conformance and plugin sources tested above were identical.

## Review gate

1. Shared IDs, capabilities, errors, Actor and ProviderCheck serve both paths.
2. Dependencies remain in existing crates and keep the existing host direction.
3. No new request-path global locks. Verification uses the existing machine
   registry and account authority worker and the existing bounded job subscriber.
4. Registry, SQLite and file operations run on their existing blocking workers.
5. Attribution adds no reads or flushes. The diagnostic costs are above. The
   narrow verification schema is decoded before the shared Check input is
   decoded. The second decode is local and performs no I/O.
6. The operation and its schemas are built once through the existing catalog.
7. Diagnostics use the existing journal receipt, live-buffer, job retention and
   readiness cache limits. Attribution follows the journal's retention.
8. The stored thread format and SQLite schemas are unchanged.
9. Verification and Actor fields use canonical UUID identities.
10. Discovery and policy derive the diagnostic from the existing operation table.
    Pool administration uses the stored server settings capability.
11. The HTTP operation and CLI are wired. The actor is delivered through the live
    journal and checked for notes and stored thread checkpoints.
12. Verification reuses Check, job admission and readiness. Attribution reuses
    the existing content outbox and journal commit.
13. New public types and methods describe their purpose and boundary.
14. CLI, thread and readiness docs and their generated pages accompany the code.
15. Touched files are formatted. Turn input and provider launch concerns have
    their own modules.

## Simplification opportunities

Distinguish one-shot internal jobs from resumable provider turns in the shared
job model. A single owner-retirement seam for content, jobs and inputs would
also give account erasure one place to drain handles before unlinking data.
