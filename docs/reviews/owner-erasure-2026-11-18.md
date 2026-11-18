# Owner erasure qualification

The initial qualification at `2971c11` covers the live owner fence, space
deletion, and the trusted account deletion route. Existing app ADR 0058 supplies the last-administrator
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

## Owner store fence inventory

Source audit at `2971c11` for [independent review #44](https://git.kayg.org/kayg/calternal-ai/issues/44).
The protected commit callback closes admission by owner UUID immediately after the SQLite commit. A step that
already holds a permit can finish. Cleanup waits for those permits before
removing keys and trees. Startup restores closed UUIDs before opening owner
secret and job stores, and replays pending cleanup before HTTP admission
and job recovery. The table records where each permit starts and ends.

| Store or path | Opener or writer | Coverage at `2971c11` |
| --- | --- | --- |
| Space `content.db` and its materialized userdata | `Content::with_space` → `Space::open` and `recover_space`. Also used by get/list/history/revision reads, receipt and control lookups, discovery, and acquisition of a journal handle. | A space permit precedes the cache lookup, cold journal and SQLite opens, schema work, and outbox/file recovery. It stays held through the callback. A read through this entry point can open and recover the store. [Content], [Space database], [Content recovery] |
| Notes, threads and other registered content kinds | `write_placed` serves create/update, linked writes, trash and restore. `purge` and `expire_trash` remove content. Thread turns append through the same content write path. | User and agent writes take an account permit and a space permit. Host expiry takes a space permit. The protected callbacks cover outbox work, atomic placement and unlink, and item routing in the shared catalog. [Content mutations], [Trash], [Content files], [Content publication], [Turn progress] |
| Space controls, receipts and collections | `set_control`, `admit_checked`, `admit_item`, `admit_erasure`, `record_receipt`, `set_collection_members`, and standard collection creation. | Request paths admit the acting account and target space. The standard collection lookup uses the space permit and its create goes through `write`. The protected space-delete callback closes the space while the admission is held. Drain follows after that call returns. [Content], [Content mutations], [Collections], [Server erasure] |
| Space standing instructions and background materialization | `standing_instructions`, `replace_standing_instructions`, `place_instructions`, explicit `materialize`, lazy recovery and the content sweeper. | Instruction reads and explicit materialization use `with_space`. Replacement adds the actor permit. Outbox replay and file placement run under the space permit in their caller. The sweeper acquires its own space permit before touching a cached space. [Instructions], [Content], [Content publication], [Content sweeper] |
| Space journal `index.db`, journal key and high-water witness | `Journals::open_store` → `SpaceStore::open_with`. Cold open can create directories, open SQLite, initialize the key/witness and rotate on detected rollback. | Cold open is inside `Content::with_space`. The caller binds the shared lifecycle to the journal before exposing its handle. [Content journal], [Journal API] |
| Writes through retained journal handles | `SpaceStore::commit`, writer-side receipt pruning, and rollback epoch rotation. Thread state events can publish through a retained handle. | The journal writer takes a fresh space permit when dequeuing each commit or rotation. A queued operation reaching the writer after close is refused. The crate-local commit covers both entries and receipt work. [Journal writer], [Turn event publisher] |
| Journal witness updates | Explicit `flush_highwater`, the timer-driven flusher, and `flush_after_writer` during shutdown. | `restore::flush` takes a space permit before accessing the witness or opening SQLite for the fallback lookup. Shutdown skips publication when the space is already closed. [Journal API], [Journal witness] |
| Account collection `content.db` | `account_collection_db` serves account collection create, get and membership replacement. | An account permit precedes every open, including schema setup on a get. The scoped connection holds the permit through the write and catalog routing. Membership replacement takes a fresh permit for the second open after its initial read. [Collections] |
| Account `input.db` and `live/*.lock` | Account-input dispatch and prepare, invalidation, membership/space cleanup, and live feed bootstrap. Bootstrap calls `Inputs::admit` to create or lock a listener slot. | Each caller takes the account permit before `Inputs::open`. Feed bootstrap holds it through maintenance, head read, and listener-slot creation. `erase_space` only probes for an existing file before taking the permit. [Input dispatch], [Input database], [Account feed] |
| Retained account feed connection | `Reader::page` reads bookmarks and can run maintenance deletes. `Reader::drop` closes SQLite on a blocking worker. | Each page takes a fresh account permit. Between pages the connection and slot stay open without an owner permit. Stream delivery rechecks the session. Connection close can checkpoint and is outside the drain. [Account feed], [Feed reader] |
| Account `agent-settings.db` | The retry-settings getter and setter share `Jobs::agent_settings` and can both create/open the file. | The account permit is acquired before directory creation and stays held through the SQLite work. [Retry settings] |
| Space `jobs.db` opens and ordinary use | `Jobs::conn` serves startup inventory and lazy opens, including schema install and retention. `Jobs::tx` and direct callers take `JobDatabase::lock` for reads, writes, and the read-only `content.db` attachment for thread summaries. | The cold open and the use of cached connections each take a space permit. Startup can observe the path before seeking admission but skips the database open for closed UUIDs. [Jobs], [Job connection], [Thread index] |
| Jobs initiated for an account | Job admission, claim, running and placing checkpoints, and placement publication. | These entry points add an account permit to the space permit. Admission holds both through route insertion. Claim holds both through live-run registration. Checkpoint forms with only a job ID get the account from the existing SQL statement and retain its permit through commit. [Jobs], [Job claim], [Job checkpoints] |
| Space job maintenance and terminal state | Recovery, wakeups and due retries, per-turn retry settings, finish, cancel/stop, child reporting, account-retirement cleanup in retained spaces, and turn-event outbox reads and acknowledgement deletes. | The database lock gives these operations the space permit. Terminal updates and outbox publication have no second account permit. Account erasure cancels active rows in retained spaces; already committed shared-space events may finish publishing. [Job SQL], [Retry settings], [Job completion], [Turn event publisher] |
| Account or space `secrets.db` and `secret.key` | `LocalStore::reader` and `writer` serve list/audit/pending, token and handle lookups, put/rotate/delete, value release with audit, and prompt request/provide/cancel. | Both open helpers take the owner permit before checking or opening the path. `reader` also uses the ordinary database setup path once the file exists. The scoped connection covers key read or creation, audit and commit. Mutations and value release also take an acting-account permit. [Secret operations], [Secret key] |
| Protected erasure and startup key reconciliation | `restore_fence`, `configure_keys`, `finish`, `LocalStore::erase_owner`, and journal/content/job cache retirement. | Startup key reconciliation reads live key fingerprints and unlinks marked keys with no admitted clients. Live cleanup deliberately bypasses the closed owner's regular opening path for key tombstoning and tree removal after drain. The shared auth and content catalog files use protected transactions and scoped owner-row cleanup. [Server start], [Server erasure], [Secret startup], [Secret erasure], [Auth erasure] |

The drain boundary also depends on the following read and placement paths.

| Path to check | Current boundary |
| --- | --- |
| Notes Trash eligibility scan | `has_expired` opens each existing `content.db` with `SQLITE_OPEN_READ_ONLY` before `Content::expire_trash` acquires a space permit. The probe has no permit of its own. SQLite can use WAL sidecars. The purge rechecks eligibility under the fence. [Notes retention], [Trash] |
| Reads on a retained journal handle | `since`, `idempotency`, and the SQLite fallback for cursor verification open read-only connections without a fresh owner permit. Calls made inside `Content` inherit its permit. Direct stream reads through a retained handle use the live route's authority checks. A cursor-triggered rollback write still goes through the fenced writer. [Journal API], [Journal schema], [Space live feed] |
| Machine and lease authority | `request_work` admits the account and space. A personal machine's session/authentication and `sync_begin` check its owner. Output checks the lease account and space. The registry and `system/machines.db` are installation-wide, and cleanup retires matching authority and lease rows under the registry lock. [Machine admission], [Machine sessions], [Machine cleanup] |
| Existing placements and long polls | `place_waiting` and `reconcile` can act on an existing lease without a fresh permit for its account and space. They use the registry lock and existing lease state. The post-wait `outbox` checks the machine session and reads queued envelopes without a fresh owner permit. Review the interval from protected close to `retire_owner` for late assignment or delivery. This is a source finding, with no race test run for this inventory. [Machine state], [Machine sessions], [Machine cleanup] |
| Machine-side provider homes and workspaces | The machine process opens account directories through its direct or Podman backend under its own root. The server can revoke sessions and stop leases. Those opens and any offline files are outside the server storage permit and tree cleanup. [Direct sandbox], [Podman sandbox] |

[Content]: ../../crates/content/src/lib.rs
[Space database]: ../../crates/content/src/db.rs
[Content recovery]: ../../crates/content/src/recovery.rs
[Content mutations]: ../../crates/content/src/mutations.rs
[Trash]: ../../crates/content/src/trash.rs
[Content files]: ../../crates/content/src/files.rs
[Content publication]: ../../crates/content/src/publication.rs
[Turn progress]: ../../crates/server/src/turns/progress.rs
[Collections]: ../../crates/content/src/collections.rs
[Instructions]: ../../crates/content/src/instructions.rs
[Content sweeper]: ../../crates/content/src/lifecycle.rs
[Content journal]: ../../crates/content/src/journal.rs
[Journal API]: ../../crates/store/src/lib.rs
[Journal schema]: ../../crates/store/src/schema.rs
[Journal writer]: ../../crates/store/src/writer.rs
[Journal witness]: ../../crates/store/src/restore.rs
[Space live feed]: ../../crates/server/src/live.rs
[Input dispatch]: ../../crates/server/src/account_input/dispatch.rs
[Input database]: ../../crates/server/src/account_input/db.rs
[Account feed]: ../../crates/server/src/account_input/http.rs
[Feed reader]: ../../crates/server/src/account_input/reader.rs
[Retry settings]: ../../crates/server/src/plugins/agent_settings.rs
[Jobs]: ../../crates/server/src/plugins/jobs.rs
[Job connection]: ../../crates/server/src/plugins/jobs/database.rs
[Thread index]: ../../crates/server/src/threads/index.rs
[Job claim]: ../../crates/server/src/plugins/jobs/claim.rs
[Job checkpoints]: ../../crates/server/src/plugins/jobs/checkpoint.rs
[Job SQL]: ../../crates/server/src/plugins/job_sql.rs
[Job completion]: ../../crates/server/src/plugins/jobs/complete.rs
[Turn event publisher]: ../../crates/server/src/threads/turn_events.rs
[Secret operations]: ../../crates/secrets/src/local.rs
[Secret key]: ../../crates/secrets/src/key.rs
[Secret startup]: ../../crates/server/src/secrets.rs
[Secret erasure]: ../../crates/secrets/src/lifecycle.rs
[Auth erasure]: ../../crates/auth/src/erasure.rs
[Server start]: ../../crates/server/src/main.rs
[Server erasure]: ../../crates/server/src/erasure.rs
[Notes retention]: ../../crates/server/src/notes/retention.rs
[Machine admission]: ../../crates/server/src/machines/admission.rs
[Machine sessions]: ../../crates/server/src/machines/sessions.rs
[Machine cleanup]: ../../crates/server/src/machines/erasure.rs
[Machine state]: ../../crates/server/src/machines/state.rs
[Direct sandbox]: ../../crates/machine/src/sandbox/direct.rs
[Podman sandbox]: ../../crates/machine/src/sandbox/podman.rs

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

## Independent review follow-up

The original inventory above is pinned to `2971c11`. The three deterministic
[#44](https://git.kayg.org/kayg/calternal-ai/issues/44) regressions are retained.
The revisions below address the four findings from the
[independent report](owner-erasure-independent-review-2026-11-18.md).

| Finding | Revised boundary | Read and flush cost |
| --- | --- | --- |
| Machine-side copies | An assignment commits workspace and account-home provenance in the same transaction as the lease before emitting `Assign`. Lease pruning leaves this inventory intact. Erasure retirement consumes matching locations and carries forward existing workspace discards into a per-machine queue. Server cleanup returns after the queue commits. A signed-in role receives a page of at most 16 instructions and acknowledges only after teardown and removal. | Placement adds two indexed insert-or-ignore statements in the existing FULL commit. Retirement queues and removes routes in the existing cleanup transaction. Polls for machines without pending erasures do no new SQL. A pending poll reads one bounded page. Acknowledgements use the existing discard-result transaction. Machine-side removal syncs the selected subtree's parent before success. |
| Late placement | Assignment and reconciliation take fresh permits for the lease account and space. Closed leases receive stop instead of assignment. The machine sweeper and operator socket start after replay. | In-memory permits. No machine database write on the auth worker. |
| Late exec | Exec admission, worker stdin and outbox delivery take fresh lease permits. Closed lease entries are removed from the outbox while stop or erasure instructions remain deliverable. | In-memory lookups under the existing registry lock. |
| Retained journal reads | `since`, `idempotency`, and the fallback cursor read take the space permit before opening SQLite. The Notes Trash eligibility probe also takes the space permit. | One in-memory admission per read operation. |

Placement provenance is deduplicated by space, account, thread and
machine. Account-home locations are deduplicated by account and machine.
Their lifetime follows the corresponding owner data instead of the timed
lease history. A machine erasure stays queued until a successful report,
including when its machine loses enrollment. The operator list includes
those machine UUIDs. The machine status query uses a separate challenge
signature domain and one indexed count, without replacing the live session.

Administrator deletion checks its current capability before looking up the
selected account, then uses the same browser/native passkey freshness
check as self-deletion in the auth transaction. Shared-space invitations
lose their creator reference and are revoked in that transaction. The
existing retention removes the terminal invitations later. A surviving
Manager can list the revoked row, and the old link is refused. The
[owner questions](../open-questions.md) still ask for confirmation.

| Retained identity reader | Observed behavior |
| --- | --- |
| Journal relay and CLI watch | The relay preserves the historical author UUID and adds `author_label: "a deleted account"` for known erased authors. The CLI uses that label. State comes from the restored lifecycle inventory with no SQL query per event. |
| Member lists | The protected transaction removes the account's memberships. The current member list joins only current membership rows to accounts. Conformance reads the remaining list and shared note after erasure. |
| Thread provenance and historical IDs | Saved Markdown preserves attribution bytes and revision offsets. The web thread parser ignores the raw actor comment and displays message roles. The CLI thread reader emits raw Markdown, including its canonical comments. Provider session reuse compares the stored account UUID against the admitted account before resuming. A friendly deleted-actor label in thread provenance remains a client follow-up. |

The pool-machine conformance case prunes the ended lease, faults during live
erasure, then faults during startup replay after queue publication. After
restart the same queued UUID is visible to the signed machine and operator.
An envelope ACK alone leaves it pending. A successful erase report clears
it. Subsequent account erasure still reaches the pool home after the space
workspace has been removed.

### Follow-up verification

The reviewed commits `f0055ed` and `750c145` were cherry-picked before merging
`origin/dev`. The commands in Verification were repeated with a separate copy
of the server built with `test-plugins`. This keeps a subsequent Cargo test
from replacing the conformance binary with a different feature set.

```sh
cargo test --locked -j6 -p calternal-content --test lifecycle -- --test-threads=1
cargo test --locked -j6 -p calternal-server --test machines_service -- --test-threads=1
cargo test --locked -j6 -p calternal-machine --lib -- --test-threads=1
mkdir -p target/conformance-bin
cp target/debug/calternal target/conformance-bin/calternal
export CALTERNAL_SERVER_BIN="$PWD/target/conformance-bin/calternal"
CARGO_TARGET_DIR="$PWD/target" ./scripts/check-podman-tests.sh \
  cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 \
  --test isolation -- --test-threads=1
```

The focused lifecycle, machine service, and machine library suites passed
3, 7, and 13 tests respectively. Four focused erasure conformance cases
passed with the pinned test server. The first full run used the name
`target/lane-calternal`, which made the streaming fixture unable to find
`calternal` on its PATH. The failing case passed in isolation with the
identical binary under its normal name. The full rerun passed all 86
conformance cases and the Podman leak check. The workspace passed 581
tests. Its opt-in Rust client integration passed separately against the
real server. The Node integration, 23 plugin tests, and 18 Podman plugin
host tests passed. The plugins retained their opt-in signed-in Claude skip.
Contract export, CLI verification, the documentation gate, both formatting
checks, and the drift check for 296 generated pages passed. All four
required Podman scopes passed leak checks. The dev-01 build targets were
removed after qualification.

## Merge with request admission at `cb8e313`

Space operations use the upstream `api::spaces` dispatcher. Its scheduled
host callback routes explicit deletion through the protected erasure path
before any ordinary directory mutation. Space writes retain content receipt
admission and its owner permits. An account-level space create takes the
account permit around its protected directory callback.

Account input preparation, pending-submission cleanup, and feed bootstrap
all use the bounded request workers. A closed account is refused in the
request scope before scheduling I/O. The durable permit is acquired again
inside the existing callback, since the owner can close while a request
waits for a worker. The same ordering applies to each retained feed page,
and the account transaction remains released during nested dispatch. The
additional preflight takes only a short in-memory lookup. It adds no
blocking job, SQLite read, or flush. Stored session and capability checks
retain upstream scheduling on their authority workers.

The merge regression occupies both request read workers and submits an
account input operation against a closed owner. Refusal must return before
either worker is released. It passed with the workspace suite.

The dev-01 verification commands above ran against the merged tree using
the separately built `target/conformance-bin/calternal`. Workspace tests
passed 586 cases, and the full serial conformance suite passed 86 cases.
The four erasure cases also passed on their own. The opt-in Rust client
integration and the Node integration passed. The plugin suites passed 23
tests and retained one opt-in signed-in Claude skip. The dedicated Podman
plugin host passed 18 tests. The focused conformance, workspace, full
conformance, and Podman plugin host runs all passed their container leak
checks. Contract export, CLI verification, the documentation gate, both
formatting checks, and the drift check for 296 documentation pages passed.
The dev-01 lane targets were removed after qualification.

The subsequent merge of `12331ce` brought in the web lane changes and the
morning decisions. It changed no backend or conformance inputs. The
conflicting open questions follow the recorded decisions on dev. The
documentation drift check passed again on dev-01.

## Simplification opportunities

Account erasure visits the job stores indexed in the in-memory route map.
An existing account-to-job reverse route, if introduced for user-facing job
lookup, could also limit that cleanup to affected stores. Account feed
readers could release idle SQLite connections sooner on lifecycle close
using their existing session invalidation.
