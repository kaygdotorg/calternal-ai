# Independent simplify review — 2026-11-17

**Disposition: do not merge as passing.** Ten findings below have enabled,
deterministic failing regression tests. Product code was not fixed.

Thread: `thr_b7x26n22q8`; Forgejo [issue #23](https://git.kayg.org/kayg/calternal-ai/issues/23).
Reviewed simplify head `cd74077`, independently of its author. Merged `origin/dev`
`89342f1` as `6d30975`; that merge changed no Rust/backend source or conformance
source. Test commits are `f87b7be` and `1f24d31`. The standalone conformance
lockfile's `calternal-machine` entry needed `calternal-types`; no product source
or dependency version was changed.

The glossary, plugin API, addresses, orchestrator decisions (including the grill),
AGENTS.md and both simplify reports were read. Reference ADRs 0058–0060 and 0062
were read through `git show origin/dev:...` in the existing app; it was not modified.
Severity means impact, not attribution. Several defects below predate simplify;
keeping a defect in the port does not make it newly introduced.

## Findings, ranked

### 1. High: a draft can be delivered after its recorded space is revoked

Test: `simplify_review_cases::resaved_draft_cannot_survive_revocation_under_its_old_space_index`
(TCP, real passkey sessions).

`account_input/db.rs:132` changes only `body` on an input conflict. Saving a draft
ID first in A, then in B, changes the JSON space to B but leaves indexed `space`
as A. Delete B: `erase_space` and `clean` filter the stale SQL column, and
`drafts.list` returns B's private text after B no longer exists. The test permits
refusing the reassignment; it does not choose whether draft reassignment should
be supported. If accepted, the persisted ownership facts must agree. Inherited
by the port; this is not a cross-account disclosure.

### 2. High: cancelling a cold worker start permanently consumes global capacity

Test: `cancelled_worker_cold_start_releases_capacity_for_other_spaces`.

`plugins/worker.rs:298–313` increments `starting`, awaits `spawn_blocking`, then
decrements it. Dropping the invocation future during that await skips the
decrement, including when the detached start later fails. Repeat to exhaust all
worker slots; unrelated spaces receive `worker_capacity` until restart. The test
occupies the only blocking-pool thread, polls admission exactly once, cancels,
releases the barrier and drains the queued start before checking another space.
No real Node executable or sleep-driven race is needed. Introduced by the new
cold-start reservation implementation.

### 3. High: machine administration bypasses the storage-root guard

Test: `machine_cli_refuses_missing_roots_before_creating_appdata`.

`machines/admin.rs:19` opens `system/machines.db` directly. Even `calternal
machines list --appdata <missing>` succeeds and creates the missing appdata tree.
The command has neither the other roots nor a pinned installation ID. A missing
mount can therefore gain a new database before validation. Serve's guarded
`Store::open` does not protect this separate entry point. This is a residual
startup-path gap, not a failure of `calternal-roots::prepare` itself.

### 4. High coverage gap: machine authority acknowledges without protected recovery

Test: `machine_pool_grant_requires_a_working_recovery_log`.

With guarded roots but an unusable recovery directory, `admin::pool_grant`
still acknowledges a new execution entitlement. Machine enrollment/revocation
and grants write `machines.db`; none receives a recovery sink. Recovery's closed
inventory contains auth tables only. The test uses a regular file in place of
the recovery directory, avoiding permission/root-user ambiguity.

This is **outside the documented auth-only recovery inventory**, and predates
this wiring; it is an unmet broader security-change requirement in this review,
not evidence that auth transactions bypass their configured barrier. Options:
extend the protected inventory/coordinator to machine authority (cross-database
commit design and fault tests), or keep the uncovered authority paths disabled
until that protection exists. The same audit found no secret metadata in the
closed inventory; secret values must not be added to it accidentally. Ordinary
job state is not itself an auth authority change.

### 5. High: one request ID executes both content and directory writes in one space

Test: `simplify_review_cases::request_id_cannot_execute_content_and_directory_writes_in_one_space`.

A successful `notes.create` followed by `spaces.set_role`, same account, space
and request ID, succeeds twice. The second response is HTTP 200, expected 409.
`api/dispatch.rs` routes directory requests to `space_receipts` without consulting
content's journal receipts, and the reverse path also lacks a unified admission
record. This violates uniqueness per `(account, space)` across writes; it does
not disclose another account's stored result. The split stores predate simplify;
this lane's claim of a unified scoped retry contract is incomplete.

### 6. Medium: a Manager cancelling someone else's job panics on the async worker

Tests: `manager_can_cancel_another_accounts_job_without_blocking_the_async_worker`
and `simplify_review_cases::manager_can_cancel_a_job_started_by_another_account`.

`plugins/job_api.rs:119` evaluates `directory.capabilities` directly inside an
async handler when the caller is not the starter. `Spaces::capabilities` reaches
`Db::run_blocking` and Tokio panics: “Cannot block the current thread from within
a runtime.” The starter path short-circuits this expression, explaining why the
existing cancellation cases pass. The direct test catches the task panic; the
TCP case sees the connection close without an HTTP response. Inherited path.

### 7. Medium: directory request IDs still collide across spaces

Tests: `directory_request_ids_are_independent_between_spaces` and
`simplify_review_cases::directory_request_id_can_be_reused_in_another_space`.

`auth/schema.sql:132` keys receipts by `(actor, request_id)`; the lookup at
`auth/spaces.rs:442` has no space. Two legitimate role changes in distinct spaces
under the same request ID produce a conflict on the second. Canonical server
fingerprints include the space, so this repro does not return foreign content.
The retained directory receipt schema also has no timestamp or retention path;
its lifetime is not controlled by `CALTERNAL_RECEIPT_RETENTION_DAYS`.

### 8. Medium: pending input still treats request IDs as account-wide identities

Test: `simplify_review_cases::pending_request_ids_are_independent_between_spaces`.

The input primary key is `(kind, id)` and pending IDs equal the enclosed request
ID. Queueing independent writes in two owned spaces with one ID rejects the
second with `pending identity reused with different input`. Sent routing also
stores one space per ID. This consumer was not fully ported to the new namespace.
Options: give pending submissions their own account-wide identity, distinct from
operation request IDs (extra identity/mapping), or include space in pending keys
and submit/confirm selectors (contract/client changes). No choice was made here.

### 9. Medium: an expired control receipt remains reserved by its publication key

Test: `expired_control_receipt_allows_a_new_request_after_restart`.

`content/journal.rs:164` publishes control changes using
`content/<request-id>/control`, account-qualified by `commit_once`.
The ordinary receipt expires, but `publication_receipts` never does. After normal
startup pruning, a new operation/input under the expired ID writes its NORMAL
receipt/event and fails its FULL barrier with a conflicting publication digest.
The test ages only the stored receipt timestamp, restarts through the real root
guard, proves the receipt is absent, then submits the new control receipt. It
injects persistent time metadata, not a complete host clock. The permanent table
was disclosed in the lane report, but this functional consequence was not.

### 10. Medium / gate 3: plugin transactions still serialize unrelated spaces

Test: `held_plugin_toggle_does_not_block_another_space`.

`Store::set_plugin_enabled` takes the installation-wide writer through the FULL
transaction. A held result encoder in A prevents B from even entering its result
encoder. Barriers make the held transaction explicit; the timeout is a bounded
progress assertion, and both threads are released/joined before the test fails.
This models a stalled transaction; it is not a disk-performance measurement.
The exception is disclosed in Store's comment as “rare manager actions,” but
still fails the stated review gate. The lane report's zero-global-lock headline
and AppState's “never waits for another” comment are too broad.

## Concurrency audit and checks that held

| Structure | Checked and result |
| --- | --- |
| Content space slots | Cache lookup/insert is atomic; first-open and a complete content read/check/write share the space lock. Sixteen concurrent first-open writes across two spaces preserve eight items/events each, including reused IDs across spaces. |
| Same-space updates | A held write and a concurrent update with the same base produce one new version and one stale-revision conflict. No lost update in this controlled schedule. |
| Different-space content | B commits while A is held inside its write callback. This exercises actual space locks, not a mock lock implementation. |
| Catalog writer | Reads use independent WAL connections. First route insertion still uses one global writer/FULL database; new-item creation is a global serialization exception. No fsync-stall instrumentation was added there. |
| Plugin state | Reads use ReadPool; row/generation update is inside the write transaction. Cross-space writer blocking is finding 10. Receipt admission/publication are separate intervals; simultaneous identical plugin calls were not exhaustively verified. |
| Journal handles / eviction | A live handle and bounded follower survive pressure from 70 other spaces; Arc identity of the pinned journal head remains the same, and listener admission is retained. `is_unused` checks handles/listeners/save activity. |
| Eviction caveat | Capacity eviction calls `map.remove` while holding the cache map; last-handle destruction can drain a writer/flush a witness there. Idle sweep removes then destroys outside the map lock. Reopen during that destruction window and forced fsync stalls were not deterministically instrumented; do not treat this as a proof of safe eviction in all schedules. |
| Worker pool | Pool lock is released before hashing/spawn; counters reserve global capacity. Cancellation breaks reservation release (finding 2). Existing bounded-capacity and pinned-bundle tests pass. |
| Session cache | Bounded insertion and revocation fence lock order inspected. Worker/session revocation and expiry conformance cases pass. Ten-minute revoked-fence expiry with an arbitrarily delayed cached lookup was not exhaustively modeled. |
| Auth directory | Accounts/spaces share one bounded dedicated worker/connection. Membership reads on request paths therefore still queue behind other auth work, including recovery fsyncs. No global-free claim can include this service. |
| Account inputs | Owner-local connections and IMMEDIATE state transitions retained. No account transaction is held over async normal operation dispatch. Stale ownership indexing fails revocation cleanup (finding 1). |
| Jobs | Per-space connection maps and durable admission retained. Non-starter cancellation violates the blocking boundary (finding 6). Duplicate cold-open/eviction schedules in Jobs were inspected but not stress-qualified. |

No measured VM timing is presented as a performance pass. Timeouts bound deadlock
and progress probes; barriers control the relevant held intervals.

## Recovery and startup audit

Serve opens `Store` under the root guard before creating auth/machine/job stores,
opens the recovery coordinator with the externally pinned installation UUID, and
attaches it to the shared Auth/Spaces handle before fixture seeding and admission.
Account creation, setup/recovery tokens, passkey registration/removal/rename,
login counters/session creation, explicit session revocation, spaces,
memberships and ownership use `spaces::commit` / `commit_with`. The revoke fence
runs after SQLite commit even if recovery completion fails. Telemetry touches
remain explicitly excluded. The account-input normal write path dispatches back
through normal authority checks; its space revocations use the directory barrier.
Its drafts/pending metadata is not itself an auth delta.

Groups, invites, role tables and memberships remain in the closed recovery
inventory. This is not a claim of coverage for future/unimplemented mutation APIs.
Machine authority and secrets metadata are outside that inventory (finding 4).
No bypass of the configured auth transaction barrier was found in the paths above.

Existing root tests cover swapped/missing/foreign/occupied roots and refusal to
adopt/repair partial roots through `--create`. Existing recovery tests exercise
truncation, corruption, foreign installation, witnessed suffix deletion and replay
refusal; they passed in this run. I did not add a subprocess-level serve test for
every corrupt-log variant or test physical power failure. Low-level Auth/Spaces
fixture constructors still deliberately permit unguarded synthetic roots; they
must not be counted as proof that every entry point is guarded.

## Live and account feed port

Compared `git diff origin/dev..cd74077` for live, store-live and account-input
modules. Subscribe-before-replay, indexed replay on broadcast hints/lag, monotonic
per-space sequence advancement, restore reset, bounded pages/subscriptions,
session/membership checks before entry delivery and identifier-only item checks
remain. Canonical SpaceId parsing replaces string normalization. No dropped
live session/member/item check or backlog/live gap was identified. Existing live
restart/replay, foreign bookmark and idle logout cases pass with real sessions.
The account feed still carries invalidation bookmarks, not private text; per-page
cleanup and per-frame session checks remain. Finding 1 is in the draft data path.

## Single-schema comparison

Compared the retained definitions to the old v1–v16 chain in `origin/dev` and
`spaces_schema.sql`, not to a database upgraded through compatibility code.

| Retained tables | Preserved invariants / deliberate differences |
| --- | --- |
| users/accounts | Unique NOCASE username, quota nonnegative check, role insert/update validation, role-in-use and first-person-admin triggers retained. |
| passkeys | Credential primary key, account FK, user index, label default and usage fields retained. |
| installations/sessions | Account FK, unique token hash, account index, expiry/revoke/assertion/registration fields retained. DAV kind/scopes and DAV-only triggers removed with the deliberately removed DAV/sharing subsystem. |
| roles | Role/capability composite key, references, admin seed and role-use triggers retained. |
| invites / invite_groups | Hash uniqueness, creator FK, quota check, expiry/use/revoke, lookup_version, group/join cascade rules retained. |
| groups / group_members | Local/OIDC source check, manager boolean check, membership composite key, group cascade and account/group index retained. |
| spaces / memberships | Separate account/space identity triggers, deferred owner-membership FK, account/space FKs, membership role FK and forbidden stored owner role retained. |
| directory receipts | Actor FK retained; unscoped/unbounded key remains, as finding 7 explains. |

No additional dropped constraint/index/trigger was identified for these retained
tables. This was a source comparison, not exhaustive SQL mutation/property testing.
Migration compatibility tests were intentionally removed; other schema versions
are refused. Removed sharing/DAV tables are not silently retained.

## Fifteen-item review gate

| # | Assessment |
| --- | --- |
| 1 | Shared account/space/installation IDs and capability/error/time primitives are imported from types. No new ID copy in this review. Domain-specific auth refusal translation remains. |
| 2 | Types/fsio sit below consumers; no helper-only dependency was added by this review. |
| 3 | **Fails:** plugin global writer (finding 10); catalog first-route writer and shared auth worker also remain. Eviction destruction needs further proof. |
| 4 | **Fails:** non-starter job cancellation calls blocking directory code on an async worker (finding 6). Normal content/journal paths use blocking host tasks. |
| 5 | **Partial:** two current capability lookups for notes.update, each two indexed SQL reads. Input still decodes in core validation, item dispatch validation and the typed handler; Store::update's initial get is followed by Content::write's current-head get. Counts below; no parse/load-once pass claimed. |
| 6 | Plugin schemas/catalog/policy state are cached in Plugin/Registry; cold workers reuse compiled schemas. No exhaustive allocation profile performed. |
| 7 | **Fails:** permanent publication receipts; directory receipts have no age/retention; leaked worker reservations. Account event/counter and live/worker caps otherwise remain explicit. |
| 8 | One current auth/store schema; old migration chain and lossy v0 projection removed. Store's strict/optional-guard test mode is explicitly unfinished in the lane report. |
| 9 | Typed space UUIDs on reviewed production paths; no names used as space authority. Notes adapter remains intentionally app-specific; it is not a generic plugin host completion. |
| 10 | **Partial:** shared Capability enum is reused, but SQL role seed strings and TypeScript manifest vocabulary remain separate declarations. Receipt ownership facts diverge in findings 1, 5, 7 and 8. |
| 11 | Runtime callers for the roots/recovery integrations now exist. Exhaustive public-symbol-to-production-caller inventory was not repeated. Test helpers remain test-only. |
| 12 | Reuses fsio private/atomic placement, existing SpaceStore writer/journal and recovery coordinator. Checked-copy placement still does not use fsio's guarded exchange; external-editor races remain excluded. |
| 13 | Docs gate includes types, roots/recovery and workspace rustdoc. It passes. Comments explain per-space fsync locking; broad global-free claims need narrowing. |
| 14 | **Fails as a complete claim:** this report documents residual scope/retention/startup issues that the zero-lock and bounded-receipt statements miss. |
| 15 | Formatting passes; file split is incomplete: worker.rs 790 lines, jobs.rs 644, store.rs 621, spaces.rs 603. New review test file stays below 600 lines. |

Write-side cost audit, rather than a fabricated physical-fsync count: a fresh
content mutation has one FULL content transaction, one NORMAL event/receipt
transaction, one FULL journal barrier, one FULL outbox delete and one FULL
materialized-state update. A new route adds a FULL catalog insert. Placement
uses file/directory durability through fsio; renames/cold setup add work. Reads
include repeated current-head joins in the adapter/write/publication/placement
steps, kind registration reads, receipt/outbox probes and file hashes on update.
Exact whole-request SQL/file syscall counts were not instrumented, so gate 5's
complete count requirement remains open. Recovery adds two file fsyncs for a
nonempty protected auth delta per its existing counted tests; this review did not
remeasure that independently.

## Commands and results

All builds/tests ran only on `ubuntu@calternal-dev-02-std` in
`~/Developer/lanes/simplify-review`, through `nix develop -c`. Cargo commands
below used `-j6`; the required docs script internally uses `-j4`.
Sync used the requested `git ls-files -co --exclude-standard -z | tar --null
--ignore-failed-read -T - -cf /tmp/simplify-review.tar`, followed by SSH extraction.
Additional test-only syncs used small tar archives of changed files.

| Command inside the dev VM lane | Result |
| --- | --- |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Pass; rebuilt immediately before final conformance. |
| `npm ci --ignore-scripts --no-audit --no-fund` | Pass. |
| `npm run build:plugins` | Pass. |
| `cargo test --locked -j6 --workspace --no-fail-fast` | Final exit 101: 480 passed, 7 new regression failures, 1 inherited ignored; only the review regression targets failed. |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 -p calternal-client --test real_server -- --ignored --test-threads=1` | Pass: the inherited opt-in real-server client test was run explicitly, 1 passed. |
| `cargo test --locked -j6 -p calternal-auth --test simplify_review -- --test-threads=1` | Exit 101: 1 new regression failed (directory scope). |
| `cargo test --locked -j6 -p calternal-server --test simplify_review -- --test-threads=1` | Exit 101: 2 positive concurrency probes passed, 6 defect probes failed. |
| `umask 022; CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | Final: 38 inherited passed; 5 new regressions failed. |
| `npm run test:plugins` | Pass: SDK 11, shipped worker 2. |
| `npm run test:integration` | Pass: 1 real-server integration test. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Pass, committed artifacts match models. |
| `scripts/check-docs.sh` | Pass: workspace/all-feature checks, standalone conformance check, rustdoc and TypeScript docs. |
| `cargo fmt --all -- --check` | Pass. New standalone conformance module formatted with `rustfmt --edition 2024`; insertion in isolation.rs is one module declaration. |
| `cargo generate-lockfile --offline --manifest-path tests/conformance/Cargo.toml` | One missing calternal-types dependency entry added; no version changes. |

Initial conformance/docs attempts stopped on the stale standalone lockfile.
An intermediate conformance attempt also used a default-feature binary rebuilt by
workspace tests and could not start fixtures. Neither is counted as isolation
failure evidence; the final run uses an explicitly rebuilt test-plugins binary.
Logs remained in `/tmp/simplify-review-*.log` on the dev VM. Lane target trees
were removed after checks; no other lane's processes/files were touched.

## Unfinished and questions

Product fixes are deliberately unfinished. The branch carries failing tests as
review evidence, not a green candidate. No production host/data, UI implementation
or subagents were used. No all-schedules concurrency proof, physical power-cut
proof, cross-process session-cache proof, exact receipt-boundary injected-clock
model, every CLI/fixture guard variant, or every recovery startup corruption
variant is claimed. In particular, a receipt expiring while the same ID is in
flight needs a shared admission/publication model and further controlled tests.

Questions/options needing a recorded engineering decision: how machine authority
joins protected recovery (finding 4), and how pending identity represents the new
per-space request namespace (finding 8). Existing decisions already settle that
request IDs are per account/space, expired IDs are new requests and revoked input
must not be delivered; those requirements are not questions.

## Simplification opportunities

- Give request admission one account/space identity and one bounded lifecycle,
  instead of independently checking directory, content, control and pending keys.
- Keep input ownership in one authoritative representation; derive or update its
  lookup columns atomically so cleanup cannot disagree with delivered records.
- Make worker-start reservations RAII-owned and release them in cancellation paths.
- Keep per-space plugin state beside other per-space state; avoid a global writer
  justified only by expected low frequency.
- Remove misleading global-free/parse-once comments; carry decoded input and loaded
  heads through the operation instead of decoding/loading again.
