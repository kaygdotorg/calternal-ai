# HTTP account and address lane validation

Implemented trusted account HTTP sign-in/management, current-session API
identification, default-off development tokens, an open-kind item resolver and
space-free note get/update/history/summarize inputs. Source/test deltas and
omissions are in [PORTING.md](PORTING.md). The later address/no-compatibility
instruction is incorporated: files/folders use `/files/<id>`, collections use
`/collections/<id>`, and no space/kind list route or collection storage is added.

No agents, commits, pushes or external human messages. All test data used
private disposable temporary directories and port 0 or in-process routers; no
real data directory, reserved preview port or dedicated performance host was used.
The old repository was only read through git objects at ea57b84b. The shared
checkout remains on feat/overnight-preview; concurrent recovery edits were left
in place. No UI, fsio, roots, store crate, server store/plugin module or recovery
crate implementation was edited by this lane.

## Paths changed by this lane

- `crates/server/Cargo.toml`: four reused software-passkey HTTP fixture dependencies,
  all dev-only. No production dependency added.
- `crates/server/src/{account_http.rs,resolve.rs}`: trusted endpoints and cookie/
  peer-source adapter; current-grant item resolution. Both deny missing docs and
  broken rustdoc links.
- `crates/server/src/{auth.rs,api.rs,authz/mod.rs,main.rs,lib.rs}`: stored directory
  with optional session service; complete-router cookie/host/origin/auth guards;
  open-kind `resolve` dispatch and item-ID scope derivation; note denial hiding at
  admission/commit/replay/delivery; explicit development flag, public origin,
  trusted proxies, ConnectInfo and private first-setup file. Crate docs deny missing
  public docs/links; only the two unowned pre-existing modules retain their prior
  missing-doc warning level.
- `crates/server/src/{error.rs,fault.rs}`: enforce/document all owned public error
  and crash-point surfaces. No fault-injection semantics changed.
- `crates/server/tests/{host_cookies.rs,login.rs,isolation.rs,abuse.rs,account_common/mod.rs}`:
  source tests/fixtures adapted to the current server.
- `crates/server/tests/{account_transport.rs,resolver.rs}`: two inline source
  transport cases and new session/freshness/passkey/resolver coverage.
- `crates/server/tests/{http.rs,authz_matrix.rs}`: fixtures explicitly opt into
  development mode; address existence hiding expects 404; required-capability,
  retry, revocation and worker-delivery assertions retained. Add default-off and
  restart-with-existing-token-files proof.
- `crates/server/{README.md,PORTING.md,HTTP-VALIDATION.md,INTEGRATION-FOLLOWUP.patch}`:
  usage, port ledger, this report and concrete unowned integration fixture edits.
- `crates/auth/src/http_facing.rs`, one module declaration in `lib.rs`, and internal
  additions in `ceremony.rs`/`passkey.rs`: six missing HTTP-facing host APIs plus
  shared discoverable verifier/prompt ownership adapter, detailed in auth PORTING.md.
  No space authority, account/space identifier allocation, migrations or schema
  were changed by this lane. `crates/auth/PORTING.md` has an appended lane section.
- `crates/core/src/{models.rs,validation.rs,bin/export-contract.rs}`: open string
  kind, ResolveInput/ResolveResult, resolver discovery and optional non-null item
  space hints. Export regeneration updates `contract/{operations.json,v0.schema.json}`
  and `packages/contracts/src/index.ts`.
- `crates/cli/src/main.rs`: get/update/history omit the space hint by default;
  collection list/create defaults retain their current fixture aliases. Generic
  `invoke resolve --input ...` already works through the common client.
- `Cargo.lock`: only the server's four dev-dependency references are this lane's
  change. The recovery package entry and root manifest changes belong to the
  concurrent recovery lane. Client source needed no change.

## Source tests

**28 cases ported: 25 runnable, 3 ignored**, with full per-source mapping in
PORTING.md. Counts are 3 host_cookies, 8 applicable login, 3 isolation, 12 abuse,
plus 2 inline HTTP cookie/proxy cases.

Ignored, with source assertions retained:

1. `old_name_is_ignored_and_cleared_once_without_touching_host_session`: superseded
   by the later instruction to add no old-app compatibility. No obsolete-cookie
   cleanup adapter ships.
2. `upgrading_schema_six_removes_cookie_state_and_keeps_passkeys`: superseded by
   the later no-old-schema-compatibility instruction. Its fixture is retained as
   source reference; no migration/compatibility work was added to enable it.
3. `revoking_another_device_signs_it_out`: the basic revocation works, separately
   proved by runnable tests, but source account mutation receipt replay is not
   available. The source's exact retry assertions remain ignored rather than
   removed or weakened. Durable protected account receipts are unfinished.

Three source journal/websocket login cases are unported: this server has no
private account feed/websocket. They are not counted as passing or ignored
executable ports. No empty test stand-ins were introduced.

Nine additional tests cover passkey management/last-key protection, account/
purpose-bound reverify, stale proof and renewed proof, real session-to-Directory
isolation, default-off tokens including restart, hidden existence/wrong/open kind,
one account in two spaces, revocation on the next request, and hint-free note
update/history plus wrong-hint rejection. No benchmark qualification is claimed.

## Exact commands and results

All Cargo commands use the supplied toolchain wrapper and `-j4`.

| Command | Result |
|---|---|
| `./scripts/with-toolchain.sh cargo test -p calternal-auth -j4` | Exit 0: 92 passed, 0 failed, 0 ignored. |
| `./scripts/with-toolchain.sh cargo test -p calternal-server --no-fail-fast -j4` | Exit 0: 67 passed, 0 failed, 3 ignored for the reasons above. |
| `./scripts/with-toolchain.sh cargo test --workspace --no-fail-fast -j4` | **Latest exit 101: 347 passed, 3 failed, 3 ignored.** All three failures are newly added concurrent recovery tests, named below. An earlier recovery snapshot passed 347/0/3; it is superseded by this latest result. |
| `./scripts/check-docs.sh` | Exit 0: Rust checks/reference docs/links and TypeScript documentation gates pass. |
| `./scripts/with-toolchain.sh cargo doc -p calternal-server --no-deps -j4` | Exit 0: owned public documentation/links pass; pre-existing missing-doc warnings in unowned store/plugins remain. |
| `./scripts/with-toolchain.sh cargo run -p calternal-core --bin export-contract -j4` | Exit 0: regenerated the three contract artifacts. |
| `./scripts/with-toolchain.sh cargo run -p calternal-core --bin export-contract -j4 -- --check` | Exit 0: no contract drift. |
| `npm run check --workspace @calternal/web` | Exit 0: 0 errors, 0 warnings; no UI edits. |
| `npm test --workspace @calternal/web` | Exit 0: 14 tests in 3 files pass; no UI edits. |
| `CALTERNA_SERVER_BIN=/tmp/calternal-http-harness-584i41j8/calternal-development node --test tests/integration/*.test.mjs` | Exit 1: original unowned fixture expects 403/forbidden at line 168, receives the required 404/not_found. The wrapper explicitly starts the binary with --development. |
| `node --test /tmp/calternal-http-harness-584i41j8/notes-plugin.test.mjs` | Exit 0: 1 passed with the exact three-line INTEGRATION-FOLLOWUP.patch applied only to a disposable harness copy, plus its absolute repoRoot. This is not a claim that the unedited integration fixture passes. |
| `git diff --check` | Exit 0. |

Latest workspace failures (unowned `crates/recovery/src/tests.rs`):

- `acknowledged_passkey_label_must_reach_recovery` at line 679: restored label is
  Before rather than the acknowledged After.
- `deleting_newest_generation_must_not_resurrect_revoked_membership` at line 629:
  deleting newest acknowledged revocation files does not refuse recovery.
- `repeated_prepare_must_not_publish_a_rolled_back_grant` at line 650: an aborted
  transaction's proposed grant appears in recovery state.

The latest recovery suite has 17 cases (14 pass, 3 fail); an earlier concurrent
snapshot had 14 passing cases. These tests and their implementation are unowned
and were not modified, weakened or ignored by this lane. This is not a passing
workspace qualification, even though all server cases and the docs gate pass.

Earlier diagnostic runs failed on an IP-based WebAuthn RP configuration (fixed by
using localhost as the default passkey origin) and old 403 expectations (updated
in owned tests to the decided address rule). No source security assertion was
removed to make a failing implementation pass.

## Dependencies

No new production packages and no auth dependencies. The server reuses packages
already in the lockfile as **dev-dependencies** for source HTTP tests:

| Package | Manifest / resolved | Reason |
|---|---|---|
| webauthn-authenticator-rs | 0.5.5 / 0.5.5, softpasskey | Real signed software-passkey enrollment/login fixtures. |
| webauthn-rs | 0.5.5 / 0.5.5, conditional-ui | Source challenge response types in the fixture. |
| tempfile | 3.27 / 3.27.0 | Automatically removed disposable test roots. |
| url | 2.5.8 / 2.5.8 | Authenticator origin parsing. |

## Remaining alias users and unowned follow-up

`personal`/`shared` remain current preview fixture aliases, as explicitly requested.
They are not old-app URLs or imported old data. Current users:

- `crates/server/src/auth.rs`: dev fixtures and UUID/root bindings; store cleanup
  uses SPACES, with its implementation unowned. Server HTTP/race/authz/space and
  resolver test fixtures also use these aliases.
- `crates/server/src/plugins.rs`: worker fixture; untouched.
- `crates/cli/src/main.rs`: list/create and plugin command defaults and a fixture;
  item get/update/history now default to no hint.
- `crates/client/tests/http.rs`: existing request fixtures; unchanged.
- `scripts/measure-preview.mjs` and `tests/integration/notes-plugin.test.mjs`:
  development measurement/integration inputs; untouched.
- `apps/web/tests/{mock-server.mjs,store.test.ts,unit.test.ts}`: mock principals,
  draft keys and existing route fixtures; untouched.

The integration harness owner can apply the provided patch: explicitly add
`--development`, and change the personal note denial expectation to 404/not_found.
These are necessary fixture updates to the requested behavior, not a new product
choice. The original integration test currently fails even when run in development
mode because its denied-note expectation predates the address decision.

Claude UI follow-up (no UI implementation done here):

- `apps/web/src/lib/route.ts` still parses/emits `/s/<space>/notes/<id>` and the old
  list form. It must emit `/notes/<id>` for an item, call `resolve` to obtain its
  current owning space/access, and adopt `/collections/<id>` for lists when their
  storage/provider arrives. Do not replace it with a space/kind list route.
- `store.svelte.ts:205–227` requires a space to open a note; resolve first and use
  the resulting owner. Reads, updates, history, conflicts and summarize currently
  send space_id (`:213,250,280,312,369`); they may omit it now. Creation and list
  requests still need explicit target spaces until collection operations exist.
- `store.svelte.ts:263` derives save correlation from `input.space_id`; preserve
  draft/epoch isolation using the resolved note owner when dropping hints.
- `store.svelte.ts:95–96` omits Owner from edit/manage predicates, as already
  reported by the stored-spaces lane. Backend grants are correct; UI behavior
  still needs the Claude lane even though its type check/tests pass.
- `api.ts` and the current app are bearer-oriented; passkey account UI should use
  cookie credentials at the configured passkey origin. The new account endpoints
  are trusted backend services, not plugin-managed account state.

Unfinished backend integration: protected account receipts/events/live delivery;
indexed global item lookup instead of per-membership probes; FO1/root validation
wiring and qualification. Those remain explicit limits. Files/collections providers,
collection storage, deleted-item state and new UI were not added to this lane.

## Questions for the user

None. The repositories and explicit decisions settle this lane's product choices.
The unowned harness/UI/storage work above is a concrete implementation follow-up.
