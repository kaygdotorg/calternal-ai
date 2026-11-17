# Stored spaces lane validation

This lane implements migration 16, stored memberships/ownership, the synchronous
adapter to the inherited bounded auth worker, development-token directory wiring,
eight space operations, Owner on the wire, and regenerated contracts. All test data
was disposable. No agents, commits, pushes, external messages, real-data access,
`.dev-data` access or listener on port 3210 were used.

**Integration is pending two owned-path exceptions.** The shared checkout does
not currently compile the server because its manifest lacks the new auth
library dependency. The user-supplied owned-path list excludes both that manifest
and `crates/server/tests/http.rs`; neither was edited. The concrete proposed
changes are in [SERVER-INTEGRATION.patch](SERVER-INTEGRATION.patch). Both scope
questions were submitted during work and remain unanswered. No approval is
inferred from elapsed time.

## Paths changed by this lane

- `crates/auth/Cargo.toml`: enable UUIDv7 on the existing uuid dependency.
- `crates/auth/src/{spaces.rs,spaces_schema.sql}`: new stored directory API,
  transactional checks/receipts, migration tables/sets, ownership FK and development
  fixture bindings. Create/list/members/set_role/remove/leave/transfer/delete are
  public documented APIs; deletion removes directory records only.
- `crates/auth/src/{db.rs,lib.rs,account.rs}`: migration 16, synchronous admission
  to the same bounded database worker, distinct SpaceId, UUIDv7 allocations,
  Conflict refusal code and updated documentation. Migrations 1–15 SQL unchanged.
- `crates/auth/tests/{spaces.rs,migration.rs}`: 15 new tests; existing migration
  fixtures remove v16 objects when constructing v2, and expect version 16/29 tables
  after upgrade. All previous credential/session/schema assertions remain.
- `crates/auth/{PORTING.md,SPACES-VALIDATION.md,SERVER-INTEGRATION.patch}`: port
  ledger, this report, and reviewable edits for the two excluded paths.
- `crates/core/src/{spaces.rs,lib.rs,models.rs,validation.rs,bin/export-contract.rs}`:
  wire inputs/results, Owner, strict input validation and discovery for eight ops.
- `crates/server/src/{auth.rs,api.rs,authz/mod.rs,authz/capabilities.rs}`: stored
  directory, off-runtime database calls, target-aware operations, policy coverage,
  current checks before content commit/replay/delivery, shared request-ID namespace,
  and temporary development root/input bindings. Changed public surfaces deny
  missing docs and broken rustdoc links. Unowned server modules still have their
  pre-existing missing-doc warnings.
- `crates/server/tests/{authz_matrix.rs,spaces_matrix.rs}`: retain all ten existing
  matrix cases and add seven stored-directory integration cases, including the
  eight-operation role/space/token matrix.
- Generated: `contract/{v0.schema.json,operations.json}` and
  `packages/contracts/src/index.ts`.

Other concurrent changes in root manifests/lockfile, fsio, roots, store and the
docs script were left alone. No `apps/web`, server store/plugin implementation,
server manifest, or existing HTTP test file was edited.

## Source tests and coverage

Source revision: `ea57b84b9c9e40e8b67b7d1e1f1805915123b040`, git-only reads.

**Four source guard cases adapted; zero executable tests ignored:**

1. `crates/auth/tests/people.rs:7`: capabilities plus protected authority. Per the
   user's decision, the sole space owner replaces the last-people-manager guard.
   Explicitly test demotion/removal/leave refusals and all owner-only grants.
2. `people.rs:37`: a member cannot change roles; an inserted capability set works;
   revoked authority also denies receipt replay. The new test uses membership
   capability rows, with independent server roles unchanged.
3. `people.rs:123` and the related `security_regressions.rs:276` race: two writers
   cannot eliminate protected authority. The space adaptation uses concurrent
   ownership transfers across independent database workers; exactly one wins.
   These two source variants are represented by one new race case, not counted
   as two executable tests. Also test transfer racing removal of its target.
4. `security_regressions.rs:325`: directory/group management cannot expose people
   beyond the current authority boundary. The space adaptation gives a shared-group
   Manager `groups.manage` and verifies that this supplies no membership or member
   directory in the other space. This does not claim a new global groups API.

**One partial source projection:** `crates/engine/src/tests.rs:381` supplies the
same-key/different-account privacy regression in `spaces_matrix.rs`. Both accounts
create distinct content with the same request ID, and cross-space reads match
absent-resource refusals. Its signed journal/cursor assertions are **not ported**;
those require port-map step 6. This is not claimed as a complete engine test port.
Server people-role name-promotion and global group-directory action tests remain
with their unimplemented server administration adapters, outside the requested
space operations. No empty ignored stand-ins are used to imply those tests exist.

The lane adds **15 auth tests + 7 server tests**, including UUIDv7 and namespace
separation, owner membership/FKs, sole-owner rules, Manager target restrictions,
last ordinary Manager demotion, two-transfer and transfer/removal races, custom
capability sets, independent space authority, next-request/replay revocation,
restart persistence, no fixture resurrection, admin isolation, deletion preserving
bytes, account-private receipts, cross-domain request-ID conflicts, and each new
operation's role/token/unknown-space policy. Source mapping is further documented
in PORTING.md. Earlier auth cases remain 77 runnable tests.

## Exact commands and results

All Cargo work used `scripts/with-toolchain.sh` with `-j4`. No benchmarks or
performance claims were made. The disposable validation snapshot is
`/tmp/calternal-spaces-validation-js2fvzbw`; it contains a copy of the workspace,
the local auth dependency in the server manifest, and the proposed UUID HTTP-test
assertion. The checkout's shared target cache was reused. Its package set includes
the concurrent store lane snapshot; this does not certify later edits by that lane.

| Command | Result |
|---|---|
| `./scripts/with-toolchain.sh cargo test -p calternal-auth -j4` | Exit 0: **92 passed, 0 failed, 0 ignored** (37 unit, 38 account, 2 migration, 15 space). |
| `./scripts/with-toolchain.sh cargo test -p calternal-server -j4` | Exit 101: unresolved `calternal_auth`; missing dependency entry in excluded manifest. |
| `./scripts/with-toolchain.sh cargo test --workspace --no-fail-fast -j4` | Exit 101: same server dependency blocker. This is **not** a passing checkout-wide run. |
| `./scripts/with-toolchain.sh cargo test --manifest-path /tmp/calternal-spaces-validation-js2fvzbw/Cargo.toml -p calternal-server -j4 --target-dir target` | Exit 0 with proposed integration edits: **33 passed, 0 failed, 0 ignored** (5 unit, 10 existing authz, 10 HTTP, 1 race, 7 stored directory). |
| `./scripts/with-toolchain.sh cargo test --manifest-path /tmp/calternal-spaces-validation-js2fvzbw/Cargo.toml --workspace --no-fail-fast -j4 --target-dir target` | Exit 0 with proposed integration edits: **299 passed, 0 failed, 0 ignored**. |
| `./scripts/check-docs.sh` | Exit 0: selected crate checks, Rust reference docs/links and TypeScript documentation gate pass in the shared checkout. |
| `./scripts/with-toolchain.sh cargo doc --manifest-path /tmp/calternal-spaces-validation-js2fvzbw/Cargo.toml -p calternal-server --no-deps -j4 --target-dir /tmp/calternal-spaces-validation-js2fvzbw/server-docs` | Exit 0; existing missing-doc warnings remain in unowned server modules. |
| `./scripts/with-toolchain.sh cargo run -p calternal-core --bin export-contract -j4` | Exit 0: regenerated all three contract artifacts. |
| `./scripts/with-toolchain.sh cargo run -p calternal-core --bin export-contract -j4 -- --check` | Exit 0: artifacts match Rust models. |
| `npm run check --workspace @calternal/web` | Exit 0: **0 errors, 0 warnings**; no UI files edited. |
| `node --test tests/integration/*.test.mjs` | Exit 0: **1 passed**, using the server binary built from the disposable validated snapshot; temp data and port 0 only. |

The earlier disposable run without the HTTP assertion adaptation had exactly one
failure: `host_origin_and_bearer_checks`, `crates/server/tests/http.rs:524`, expected
literal `"owner"` but received the required UUID. The proposed change asserts
UUIDv7 and stable identity on repeated describe calls; no auth/security assertion
is removed. No failing test was skipped to obtain the successful snapshot result.
A git-object/string comparison also confirmed every v1–15 migration SQL value
matches the pinned source exactly.

## Dependencies

No new external package or version. `uuid` remains workspace requirement `1`,
resolved **1.29.2**; this lane enables its **v7** feature for fresh account/space
identities. The proposed server manifest adds the local **calternal-auth 0.1.0**
workspace dependency, reusing rusqlite/worker code rather than duplicating it.
That dependency will also need Cargo's normal lockfile regeneration when applied.
The auth library adds no dependency on old core/engine/server/plugin crates.

## Temporary alias inventory and routing follow-up

Stored account/space identities and wire references are UUIDs. Only the two
seeded development roots retain an explicit `personal`/`shared` binding; input
aliases resolve to those stored UUIDs before authorization. Discovery returns
UUID grants and effective Owner roles. All new spaces use UUID roots.

Literal assumptions found in this checkout, grouped by owning file:

| Path | Assumption / follow-up |
|---|---|
| `crates/server/src/auth.rs` | `SPACES`, legacy test principals, seeded name/UUID lookup and content-root bindings. Remove aliases only with the routing/storage change. |
| `crates/server/src/store.rs:232` | Cleanup iterates `SPACES`; currently covers seeded alias roots only. Extend cleanup to UUID roots in the storage lane. Generic notes paths otherwise accept UUIDs. This file was not edited. |
| `crates/server/src/api.rs` | The SPA example still shows `/s/personal/...`; this lane adapts API space inputs/results, not UI routes. |
| `crates/cli/src/main.rs:61,366` | CLI default `personal` and its operation test. |
| `crates/client/tests/http.rs:57,98,127,162,226` | Test request fixtures use `personal`. |
| `scripts/measure-preview.mjs:44–50` | Measurement requests use `personal`; no measurements run here. |
| `tests/integration/notes-plugin.test.mjs:120–231` | Real-server HTTP and CLI fixture requests use both aliases; they pass with bindings. |
| `apps/web/tests/{mock-server.mjs,store.test.ts,unit.test.ts}` | Mock principals, notes, draft keys and `/s/personal/...` route fixtures use aliases/Manager. Runtime UI otherwise consumes discovered IDs. |
| `crates/server/tests/{http.rs,race.rs,authz_matrix.rs,spaces_matrix.rs}` | Requests, fixture memberships and some literal filesystem paths use aliases. Existing behavior is preserved; the new matrix also tests UUID inputs and UUID responses. |
| `crates/server/src/plugins.rs:480` | Worker test fixture uses `personal`. |
| `crates/server/README.md:17–18` | Development membership examples use alias names and the previous Manager description. Documentation outside owned paths was not edited. |

Generic prose containing the word shared was excluded from this literal inventory.

## Unfinished / Questions for the user

No unresolved product decisions for these stored space operations.

1. **Apply the prepared dependency entry in `crates/server/Cargo.toml`?**
   Option A: authorize that one manifest entry (plus Cargo's generated lockfile
   update); the validated integration becomes buildable in the shared checkout.
   Option B: have the parent lane apply it; checkout server/workspace tests remain
   blocked until then. These paths are outside the lane's explicit ownership list.
2. **Apply the prepared assertion update in `crates/server/tests/http.rs`?**
   Option A: authorize the UUIDv7/stability assertion; the disposable workspace run
   demonstrates the passing result. Option B: parent owns that test adaptation;
   after adding the dependency the old literal-name assertion will still fail.
3. **Claude UI follow-up (required by the settled Owner decision):**
   `apps/web/src/lib/store.svelte.ts:95–96` treats only `editor`/`manager` as editable
   and only `manager` as manageable. Owners now have correct backend authority but
   lose those controls in the actual UI. The type check does not detect this
   behavioral mismatch. A Claude UI lane must update the predicates; this lane
   cannot edit UI or spawn agents. Old manually entered alias UI routes also need
   routing-lane review because discovery now supplies UUID grants. This is an
   implementation follow-up, not a proposal to change the agreed Owner role.
4. FO1 security-before-ack publication, real HTTP passkey sign-in, full journal/
   signed-cursor integration, legacy import/root rekeying, UUID-root orphan-temp
   cleanup and deletion of content/history remain later port-map work. Development
   directory APIs are not production recovery or isolation qualification.
