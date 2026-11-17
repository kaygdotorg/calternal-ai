# Isolation and authorization conformance — 2026-11-17

The corrected disposable server passes **27 conformance tests**. The final Rust
workspace run passes **368 tests, 0 failures, 3 pre-existing ignores**. Plugin
checks (8 tests), web checks/build (14 tests), the real-server integration test,
and the documentation gate pass. These results establish the bounded expectations
below, not universal tenant isolation, a proven worker sandbox or production readiness.

The initial qualified run was **21 passed, 4 failed**. Each confirmed gap was
retained as a regression and corrected after the project owner expanded this
lane's scope to server/auth product code. No failures were ignored or converted
into expected failures. Historical evidence is in
[qualification.log](../../tests/conformance/qualification.log) and
[final-results](../../tests/conformance/final-results/); corrected evidence is in
[corrected.log](../../tests/conformance/corrected.log) and
[corrected-results](../../tests/conformance/corrected-results/).

The root lockfile update is prepared in
[root-lock.patch](../../tests/conformance/root-lock.patch), pending explicit scope
approval: the expanded ownership list names `crates/server` and `crates/auth`,
not root `Cargo.lock`. All corrected VM checks use Cargo's generated lockfile,
which adds only the existing local fsio dependency to those two packages. Until
that two-edge patch is applied here, this shared checkout's root `--locked` build
is unfinished. No third-party production package or wire type was added.

## Scope, setup and provenance

Changes:

- Rename the inherited `tests/break/` to `tests/conformance/` and `attacks.rs` to
  [isolation.rs](../../tests/conformance/isolation.rs); use a standalone manifest
  and lockfile, README, captured logs and JSON evidence. `previous-run/` preserves
  the inherited partial run; it is not current qualification evidence.
- `crates/server/src/{api.rs,authz/mod.rs}`: reidentify the originating credential
  before successful API delivery; return not-found for missing membership while
  keeping forbidden for members missing a capability.
- `crates/auth/src/{private_fs.rs,db.rs,lib.rs}`, both crate manifests, and
  `crates/server/src/{main.rs,auth.rs,store.rs}`: private directory/file preparation,
  database precreation before SQLite sidecars, fsio atomic private publication,
  and cleanup recognition for fsio temporaries in known UUID space directories.
- `crates/server/src/store.rs`: revision-bound note-history continuations. No
  `crates/core` change: its existing cursor field is opaque and already accepts
  the new bounded token. Stored revisions supply binding without a new key/schema.
- `crates/server/src/lib.rs`: enforce missing public documentation in the now
  documented store module; preserve the existing plugin module warning level.
- `crates/server/tests/{account_transport.rs,authz_matrix.rs,http.rs,spaces_matrix.rs}`:
  require 404 for absent membership; retain 403 assertions for insufficient
  capability and protected owner/Manager targets.
- This report. No auth migration 1–15 text, UI source, commits or pushes changed.

Read the requested architecture sections, progress/decision/address records,
server auth/API/policies/validation/port records, auth spaces/session/passkey code,
store schema/contracts and recovery protocol. Consulted the reference auth HTTP
origin guards and existing software-authenticator fixture through git objects.
The source app permits an absent Origin header; no new absent-Origin restriction
was inferred. Honcho recall was attempted twice and returned HTTP 503; repository
records and the explicit task decisions were used.

Every server fixture uses a fresh temporary root and port 0 on
`claude@calternal-dev-01-compute02`, under
`~/Developer/lanes/isolation-conformance-sol`. No builds/tests ran on hd01. No
existing preview data, reserved listener or other account's directories/processes
were used. Five accounts enroll and sign in through HTTP software-passkey
ceremonies: an admin with no memberships, two separate space owners, an ordinary
account with no memberships, and a membership-management target. Since member
admission has no HTTP binding, test memberships are provisioned offline with
rusqlite, while the server is stopped. Expiry and capability variants are likewise
offline fixture preparation. All asserted access, role and credential operations
use HTTP. Development credentials appear only in the explicit flag-boundary test.

The software authenticator does not implement resident credential storage: tests
assert that the server requests it, then supply credential IDs and the saved user
handle to emulate discovery, as the existing fixture does. This tests real signed
WebAuthn verification, not platform authenticator/browser behavior.

Snapshot fingerprints are in [source.sha256](../../tests/conformance/source.sha256)
and [binaries.sha256](../../tests/conformance/binaries.sha256). Shared branch:
`feat/overnight-preview`; HEAD at reporting: `f5d14508b6b1e68070f991adddff99c185d2c0e3`.
That HEAD alone does not identify concurrent uncommitted changes; fingerprints
identify the actual tested VM source and executable. The binary SHA-256 is
`b93d7e6020e984fe09bd30d91ed098ebdaf088e48953d841d74affb759b526f9`;
worker SHA-256 is
`eb8dffa8b1e6ff1bbe9debdf746b61a44c71bcd55a1fd1b6f384cfb5b4c4de7a`.

## Expectations and results

“Met” means the named finite checks passed against this server snapshot. Public
status/setup/login/logout routes are intentionally not protected content routes;
valid independent devices remain valid until revoked. There are 19 registered
operations and 16 account HTTP routes. Inventory guards fail on additions.

| Expectation | Result | Evidence and practical limit |
| --- | --- | --- |
| Real passkey enrollment and sign-in; two separate owners; ordinary/admin accounts without memberships | Met | Every fixture; no development tokens for content checks. |
| Foreign and absent item IDs have identical not-found status, body and headers | Met | `all_operations_hide_foreign_ids`: resolve/get/update/history/summarize, three caller classes. |
| Foreign and absent space IDs have identical not-found responses on every space-scoped operation | Met after correction | Same test covers all 17 scoped operations; account-scoped create/list are separately checked. |
| No admin space-content bypass; no-membership account sees no spaces | Met | Cross-space matrix and fixture checks, plus workspace `spaces_matrix`. |
| Foreign plugin state is inaccessible | Met | Plugin list/preview/enable/disable all checked under hidden and absent spaces. Plugin IDs are deployment-wide names, not tenant-owned identifiers. |
| Request/receipt IDs do not expose another account/space's result | Met for account namespaces | `input_validation_and_request_id_scope`, note and directory replay tests. There is no receipt-read route. Literal global reuse rejection is unresolved below. |
| Foreign item hint and history cursor cannot escape scope | Met after correction | `cursors_and_space_hints_do_not_cross_item_boundaries`, `foreign_history_cursor_is_refused`; foreign cursor now 400. |
| No large foreign/absent timing difference in the sampled requests | Met for smoke check only | Three paired samples per operation/caller, median delta <=20 ms in corrected evidence. |
| General timing-side-channel or performance proof | Could not be tested | Shared dev VM smoke sampling is not a quiet/perf-host experiment or constant-time proof. No performance qualification claimed. |
| Required policy capability at every bound space operation | Met | `every_bound_space_operation_requires_its_policy_capability`: remove exact capability from complete set, assert denial, restore, assert success for all 17 bindings. |
| Ten space capability names are independent sets | Met for existing bindings | `each_space_capability_is_independent_over_http` tests all ten one at a time; owner-only constraints still apply. |
| `managers.manage` independently required for Manager targets/promotion | Met | `managers_manage_is_required_even_for_owner_target_changes`, plus positive owner/Manager target tests. |
| Required use of `plugins.use`, `members.invite`, `space.settings` | Could not be tested | No current binding requires these three permissions. Summarize/preview are read-only and require `space.read`. No fictional bindings were introduced. |
| Manager cannot appoint, demote or remove Managers | Met | `role_boundaries_and_demoted_receipts`, `manager_cannot_demote_or_remove_another_manager`. |
| Manager cannot remove owner, transfer or delete | Met | Role matrix, target cases, one-hot capability matrix. |
| Owner cannot leave or be removed; ownership changes through transfer | Met | Role checks and workspace stored-space invariant tests. |
| Two simultaneous transfers leave exactly one owner | Met | `simultaneous_transfers_preserve_exactly_one_owner`: eight pairs, one success per pair, owner inventory checked. Transfer/removal race also exercised. |
| Retried note/directory mutations recheck current role and membership | Met | Note replay after demotion; `directory_receipt_retries_recheck_current_membership` after demotion/removal. |
| Running plugin delivers nothing after membership removal | Met | Ready/release barrier, real worker, completed removal, then release; 404. |
| Every non-worker internal membership-removal cut is covered | Could not be tested deterministically | HTTP mutations serialize through the shared store lock, but this suite has no pause hooks at every non-worker delivery point. Worker barrier coverage is explicit, not a proof for every future asynchronous operation. |
| Running plugin delivers nothing after logout or other-device revocation | Met after correction | Two separate barrier cases; both return 401 with no worker result. |
| Running plugin delivers nothing after session expiry | Met | `running_worker_expired_session_blocks_delivery`: short absolute expiry set offline; worker admission succeeds before expiry, delivery is 401 afterward. |
| Anonymous, signed-out, revoked, forged, duplicate-cookie and expired sessions refused at every protected route | Met | Route inventory, account-route/session matrix, describe/operations, plus each registered operation under invalid sessions. |
| Other account's session cannot mutate a person's installations/passkeys or consume their protected prompt | Met | Foreign resources compare equal to absent resources; failed cross-account ceremony consumption preserves rightful completion. |
| Independent live devices remain usable and revoke only intended installation | Met | `account_routes_reject_foreign_resources_and_bad_sessions`; new sign-in survives logout of earlier device. |
| Cookie attributes match source specification | Met | Host cookie, Path=/, HttpOnly, Secure, SameSite=Lax, no Domain, token rotation; workspace cookie/source fixtures. |
| Other Origin/null Origin refused where source refuses them | Met | Account POST routes and operations origin matrix; workspace source isolation tests and foreign GET identity refusal. |
| No Origin behavior matches source | Met for operations and source adapter | Source guard explicitly permits missing Origin; operations succeeds without it. Account matrix probes all routes; invalid ceremony bodies remain invalid. No source-required missing-Origin refusal was found. |
| Setup token/login challenge/ceremony is usable once, including concurrent completion | Met | Sequential replay plus `parallel_setup_and_login_ceremonies_are_take_once`; two setup prompts for one token yield one registration. |
| Parallel rate limits cannot be bypassed by untrusted forwarded source headers | Met for login-start source limiter | 140 parallel requests, no 5xx, at least 20 rate-limited. Other limiter dimensions covered by workspace auth tests, not an exhaustive real-process saturation test. |
| Concurrent removal never leaves no passkey | Met | Two HTTP removals yield exactly one 200 and one 409; remaining authenticator successfully signs in. |
| Development tokens refused without explicit flag, including restart | Met | Test first proves flag-enabled token works, then starts same root without flag and gets 401. |
| Oversized operation/auth bodies and malformed JSON fail cleanly | Met | 2 MiB operation boundary, 64 KiB auth boundary, malformed/non-object JSON; no 5xx. |
| Unknown/mismatched resolve kinds fail cleanly | Met | Existing note resolved as tasks/files/collections/unknown returns 404. |
| Unusual UUID/path-like identifiers fail cleanly | Met | Nil, uppercase, braces, absolute/parent paths, encoded parent segments and NUL; item/space/plugin inputs receive 4xx. All existing identifier-bearing paths are bounded. |
| Same request ID reused by one account in another space is refused | Met | 409; same input replays original result. |
| Same request ID used by different accounts is globally refused | Not met under literal task wording | Existing source contract intentionally accepts separate account namespaces, yielding distinct new items and no foreign result. No global uniqueness change made; see question below. |
| Inventory every created directory/file and SQLite table, with separation boundary | Met | Inventory below and raw schema/mode JSON, including empty inherited tables. |
| Owner-only permission bits on persisted default-mode server entries | Met after correction | Root/system/space/Notes dirs 0700; all recorded files, DBs, WAL/SHM 0600, under umask 022. Source fsio tests cover private atomic temp publication. |
| Abrupt restart after membership/transfer/sign-in preserves internally consistent state | Met for scheduled SIGKILL cases | `interrupted_authority_and_login_are_atomic_with_receipts`: 12 cases, SQL integrity/FK checks, receipt-state agreement, preserved acknowledged sessions and exactly one owner. |
| Every internal auth transaction/recovery cut interrupted deterministically | Could not be tested through this HTTP suite | No auth transaction pause hooks in server. Scheduled delays explore pre/post-commit outcomes but cannot prove each internal cut. Recovery crate's separate workspace tests are not an HTTP integration proof. |

## Confirmed gaps and corrections

| Severity | Original failing regression | Gap and effect | Smallest implemented correction | Corrected result |
| --- | --- | --- | --- | --- |
| High | `running_worker_session_revocation_blocks_delivery` | Guard retained account identity but discarded the originating installation's liveness; successful worker payload escaped after logout completed. | Reidentify the original credential after handler completion and before successful API delivery; discard payload with 401 if no longer valid. The membership check remains in dispatch. | Pass; also passes separate other-device revoke and expiry variants. |
| Medium, contract conformance | `all_operations_hide_foreign_ids` | Twelve space-scoped operations returned 403 rather than required 404 without membership. Foreign and missing were already identical; no existence distinction was observed. | Change policy's absent-membership result to `not_found("space not found")`; retain forbidden for members missing required capabilities. | Pass across all callers, bodies and headers. |
| Medium, bounded by private root | `filesystem_modes_and_complete_sql_inventory` | Internal dirs 0755; DBs, WAL/SHM, lock and Markdown 0644. Root 0700 limited traversal, but entries did not meet owner-only policy. | Private mode-at-creation directory/file helpers with fsio directory sync; precreate SQLite files 0600; publish Markdown/startup metadata with fsio mode-aware atomic writes. | Pass: all recorded dirs 0700, all files 0600. |
| Low, continuation integrity | `foreign_history_cursor_is_refused` | Bare `h<seq>` from another note/space was accepted as local pagination. Returned bytes stayed in the authorized local note, but refusal/binding contract failed. | Use persisted random revision as opaque continuation; resolve it only within requested note after space-bound note lookup. Refuse foreign/missing/unbound cursors. | Pass; normal pagination and replay/restart workspace tests remain passing. |

The existing nondisclosure cursor test was strengthened from accepting a local
200 with no foreign text to requiring 400 while retaining its no-foreign-text
assertion. This follows the corrected refusal contract; the separate original
failing regression remains in place. Existing space tests were likewise updated
to assert the required 404, without broadening accepted status sets.

## Actual physical layout and tables

The running server creates two shared databases, **not** per-space content DBs.
Portable Markdown copies occupy per-space directories. The ported
`calternal-store` crate's `spaces/<id>/index.db`, signed journal cursors and
separate secrets/witness paths are not wired into this server. The history cursor
correction above does not claim to implement that journal protocol.

Observed from `sqlite_master` and directory walking, after two independent-space
note creates and a plugin enable: 4 preview tables plus 29 auth tables. Counts,
full table SQL and individual directory/file modes are in
[storage.json](../../tests/conformance/corrected-results/storage.json). All table
rows share their respective physical SQLite file and depend on trusted host
queries/constraints for logical separation; SQLite supplies no tenant row policy.

| Directory/file pattern | Physical scope | Corrected mode |
| --- | --- | --- |
| `<root>/` | One disposable server installation | 0700 |
| `spaces/` | Shared directory holding space roots | 0700 |
| `spaces/<space-uuid>/` | Separate directory per space that has materialized content | 0700 |
| `spaces/<space-uuid>/Notes/` | Separate Markdown directory per space | 0700 |
| `spaces/<space-uuid>/Notes/<note-uuid>.md` | One portable current-note copy; history remains shared SQL | 0600 |
| `system/` | Shared protected account/directory state | 0700 |
| `system/auth.db`, `-wal`, `-shm` | Shared account, membership and source-schema tables | 0600 |
| `state.sqlite3`, `-wal`, `-shm` | Shared preview content, history, receipts, plugin state | 0600 |
| `.calternal.lock` | Installation-wide preview store lock | 0600 |
| `startup.json`, `setup.token` | Installation startup metadata and one-use initial setup credential file | 0600 |
| `dev-tokens/`, `<principal>.token` (explicit development only) | Installation fixtures; absent from default-mode inventory | 0700 / 0600 by creation paths and existing workspace token-mode tests |
| fsio `.calternal-tmp-<nonce>-<filename>` (transient) | Atomic publication in destination directory; no steady-state entry observed | 0600 by `write_atomic_mode`; shared fsio regression covers prepublication mode |

| Database | Every actual table | Logical separation or global role |
| --- | --- | --- |
| `state.sqlite3` | `notes` | `space_id` predicate; globally unique note ID. |
| `state.sqlite3` | `versions` | `note_id` FK; authorized note lookup binds space before history query. |
| `state.sqlite3` | `receipts` | Primary key `(principal,request_id)`; fingerprint includes operation/input; note reference retains materialization target. |
| `state.sqlite3` | `plugin_state` | Primary key `(space_id,plugin_id)`. |
| `system/auth.db` | `users` | Global account IDs/profile/server role; protected account predicates. |
| `system/auth.db` | `passkeys` | Credential ID primary key; `user_id` owner FK/predicate. |
| `system/auth.db` | `setup_tokens` | Hashed credential primary key; optional `user_id`, purpose and one-use/expiry state; initial token installation-wide. |
| `system/auth.db` | `installations` | Global installation ID, unique token hash, account `user_id`, revocation/expiry state. |
| `system/auth.db` | `roles` | Global server role vocabulary. |
| `system/auth.db` | `role_capabilities` | Global `(role,capability)` server capability definitions. |
| `system/auth.db` | `groups` | Global protected group IDs; empty in this HTTP fixture. |
| `system/auth.db` | `group_members` | `(group_id,user_id)` FKs; empty inherited schema. |
| `system/auth.db` | `invites` | Global protected invite IDs/secrets, creator FK; empty inherited schema. |
| `system/auth.db` | `invite_groups` | `(invite_id,group_id)` FKs; empty inherited schema. |
| `system/auth.db` | `installation_notes_scopes` | Installation FK plus named scope; empty inherited schema. |
| `system/auth.db` | `note_shares` | `(owner_id,note_id)` account-owner key; empty inherited schema, not a working space-sharing API. |
| `system/auth.db` | `share_grants` | Share FK plus person/account FK; empty inherited schema. |
| `system/auth.db` | `note_journal` | `(owner_id,note_id,note_seq)` and actor/installation FKs; empty inherited schema. |
| `system/auth.db` | `share_fanout` | Owner/note/sequence FK to note journal; empty inherited schema. |
| `system/auth.db` | `share_fanout_acks` | Owner/note/sequence plus installation FK; empty inherited schema. |
| `system/auth.db` | `share_fanout_deliveries` | Owner/note/sequence plus recipient account FK; empty inherited schema. |
| `system/auth.db` | `recipient_control` | Recipient account/share FKs; empty inherited schema. |
| `system/auth.db` | `recipient_control_acks` | Control/installation FKs; empty inherited schema. |
| `system/auth.db` | `recipient_share_state` | `(recipient_id,share_id)` FKs; empty inherited schema. |
| `system/auth.db` | `recipient_share_fences` | `(recipient_id,share_id)` FKs plus note sequence; empty inherited schema. |
| `system/auth.db` | `sharing_installation_resets` | Installation FK; empty inherited schema. |
| `system/auth.db` | `sharing_reset_cursors` | Installation/owner/note key; empty inherited schema. |
| `system/auth.db` | `space_roles` | Global space role vocabulary, separate from server roles. |
| `system/auth.db` | `space_role_capabilities` | Global `(role,capability)` space capability definitions. |
| `system/auth.db` | `spaces` | Space UUID and single `owner_account`; deferred FK requires owner membership. |
| `system/auth.db` | `space_memberships` | `(space_id,account_id)` and role; effective owner derived from `spaces.owner_account`. |
| `system/auth.db` | `space_receipts` | `(actor,request_id)` plus operation/fingerprint/result; actor account FK. |
| `system/auth.db` | `development_directory` | Global fixture alias/UUID bindings; zero rows without explicit development. |

No per-space DB, recovery snapshot/log, independent secrets directory or tiered
content root appeared in this default server inventory. Those library capabilities
must not be reported as server integration merely because their crate tests pass.

## Exact validation commands and results

Only sync/read/edit commands run on hd01. Initial sync:

```sh
cd /home/kayg/Developer/Calternal-AI
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf - | ssh claude@calternal-dev-01-compute02 'mkdir -p ~/Developer/lanes/isolation-conformance-sol && cd ~/Developer/lanes/isolation-conformance-sol && tar xf -'
```

Subsequent owned-file tar syncs kept this dedicated VM copy updated. Every command
below ran after `cd ~/Developer/lanes/isolation-conformance-sol` on that VM. Cargo
was limited to `-j4`. The actual documentation script ran inside Nix as requested;
it internally invokes its repository wrapper. No direct wrapper was used for
build/test commands.

| Exact command | Result / captured evidence |
| --- | --- |
| `nix develop -c cargo build --locked -j4 -p calternal-server` | Initial build exit 0. First correction build used `cargo build -j4 -p calternal-server` once to generate the two lockfile dependency edges in the VM copy; final locked builds exit 0. `correction-build.log`. |
| `nix develop -c npm ci --ignore-scripts --no-audit --no-fund` | Exit 0; clean disposable plugin dependency installation. |
| `nix develop -c npm run build:plugins` | Exit 0; digest-pinned worker built. `npm.log`. |
| `nix develop -c sh -c 'umask 022; CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CONFORMANCE_RESULTS="$PWD/tests/conformance/final-results" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j4 --test isolation -- --test-threads=1'` | Initial qualification exit 101: **21 passed, 4 failed, 0 ignored**. `qualification.log`. |
| `nix develop -c sh -c 'umask 022; CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CONFORMANCE_RESULTS="$PWD/tests/conformance/corrected-results" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j4 --test isolation -- --test-threads=1'` | Final exit 0: **27 passed, 0 failed, 0 ignored**. Eight added cases were already present at initial qualification; later two additions separately cover other-device revocation and expiry. `corrected.log`. |
| `nix develop -c cargo test --locked -j4 --workspace --no-fail-fast` | Final exit 0: **368 passed, 0 failed, 3 ignored**, including doc tests. `workspace-final.log`. |
| `nix develop -c npm test --workspace @calternal/plugin-sdk` | Exit 0: **6 passed**. `plugins.log`. |
| `nix develop -c npm test --workspace @calternal/notes-insights` | Exit 0: **2 passed**, recompiles exact worker bundle. `plugins.log`. |
| `nix develop -c npm run check --workspace @calternal/web` | Exit 0: **0 errors, 0 warnings**. `web.log`. |
| `nix develop -c npm test --workspace @calternal/web` | Exit 0: **14 passed in 3 files**. `web.log`. |
| `nix develop -c npm run build:web` | Exit 0; production asset build. Generated VM artifacts only, no UI source changes. `web.log`. |
| `nix develop -c sh -c 'CALTERNA_SERVER_BIN="$PWD/target/debug/calternal" node --test tests/integration/*.test.mjs'` | Exit 0: **1 passed**, disposable server. Variable spelling follows this existing integration fixture. `integration.log`. |
| `nix develop -c ./scripts/check-docs.sh` | Exit 0: Rust check/docs/link gates and TypeScript documentation gates. `docs-final.log`. |
| `nix develop -c rustfmt --edition 2024 <changed Rust files>` | Exit 0; individually listed owned Rust files were formatted and copied back. No workspace-wide format rewrite. |
| `nix develop -c cargo doc --locked -j4 -p calternal-server --no-deps` | Exit 0; changed server public documentation/links pass, with pre-existing plugin-module warnings retained. `server-docs.log`. |
| `git diff --check` (hd01, static diff check only) | Exit 0. |
| `git apply --check tests/conformance/root-lock.patch` (hd01, static patch check only) | Exit 0; pending root lockfile patch applies cleanly. |

The inherited partial suite had 17 tests. This lane added ten tests, retaining all
17. Before plugin build, a diagnostic run failed **6** cases (11 passed): four
were blocked by the missing bundle, plus the confirmed status/mode gaps. After
building the worker, the 25-test qualification exposed precisely the four product
gaps above. Early workspace runs failed two assertions expecting obsolete 403
responses (`account_transport` and `http`); both now require 404 and the full
workspace rerun passes. No failed result is presented as a pass.

The workspace's three pre-existing ignores were not changed: two superseded
old-cookie/schema compatibility fixtures, and one complete account-revocation
retry fixture awaiting protected account mutation receipts. New conformance
coverage verifies actual device revocation but does not claim to supply those
account mutation receipts. No new ignored tests were introduced.

## Structural weaknesses and unfinished proofs

1. **Shared content database and mutex.** Space separation depends on host
   predicates/policy in every handler, version lookup and receipt replay. There is
   no SQLite tenant policy or separate content writer per space in this server.
   Global serialization and per-membership resolver probes also limit scaling.
2. **Worker confinement remains unproven for untrusted plugins.** Passing HTTP
   and revocation checks does not prove OS isolation, network/secret confinement
   or a safe tenant-code runtime. Only the shipped pinned worker was exercised.
3. **Library/server recovery integration is incomplete.** This HTTP binary does
   not configure the protected recovery coordinator or the separate per-space
   journal/root contracts. SQLite restart consistency is not file-only rebuild,
   tiering or recovery freshness qualification.
4. **Account mutation receipt/live-feed gap remains explicit.** The ignored
   inherited retry fixture is still unfinished. There is no account websocket,
   live feed or account receipt endpoint to validate here.
5. **Finite race and timing coverage.** The ready/release worker races are
   deterministic. SIGKILL scheduling is not deterministic internal-cut coverage;
   no physical power-cut test ran. Three timing pairs on a shared dev VM cannot
   qualify all request distributions or future routes.
6. **Absent bindings are unqualified.** No HTTP invitation/join, settings mutation,
   additional plugin-use binding, file/collection content provider or inherited
   sharing engine was invented to satisfy a checklist. Add conformance cases
   when those contracts acquire bindings; inventory guards make additions visible.

## Questions for the user

1. **Apply the generated root `Cargo.lock` edges?** The patch is concrete and
   validated in the VM. Option A: authorize applying the two existing local fsio
   dependency edges here, making root locked builds reproducible. Option B: have
   the parent lane run `git apply tests/conformance/root-lock.patch`; until then this
   shared checkout's root lockfile does not match the corrected manifests.
2. **Request-ID scope:** the literal task wording asks to refuse the same request
   ID across different accounts. Source contracts and existing tests intentionally
   namespace by account. Option A: preserve current independent namespaces (no
   result disclosure, no global coordination; cross-space reuse by the same
   account is already 409). Option B: require global uniqueness (new shared index
   and contention, and rejection reveals that another account used an ID). This
   is a product/protocol choice, so it was not silently changed.

No other product decision was required for the four corrections. No commit,
push, new agent/thread, production action or outside message was performed.
