# Account authentication port

Source: `/home/kayg/Developer/calternal` at `ea57b84b9c9e40e8b67b7d1e1f1805915123b040`,
read with git only. This library completes port-map §7 step 4's non-HTTP slice.
It exposes no routes, cookies, space authority, content paths or FO1 coordinator.
Build and test through `scripts/with-toolchain.sh`; it supplies existing
Nix-store OpenSSL and the runtime library path. No system package was installed.

## Step 4 source adaptations

All paths/line numbers in this ledger refer to that pinned source revision.

- `crates/auth/src/config.rs:1–165`: retain origin/RP validation, all defaults,
  trusted CIDR validation and the origin test. Replace ConfigError with local
  Error/InvalidInput, and export `trusts_proxy` for later transport adapters.
  Reserved DAV configuration fields retain source names/defaults; no DAV
  authentication or mint API is enabled.
- `passkey.rs:1–235`: retain WebAuthn verifier, resident-key request, user
  verification, signature/counter verification, excluded credentials, account
  back-off and transactional credential/session writes. Remove invite payloads
  (`:47–48,81,94–96`): invites are explicitly outside this slice. All setup and
  login results use AccountId/InstallationId and omit the invite boolean.
- `ceremony.rs:16–44,50–125`: retain expiring take-once challenges, separate
  Setup/Add/Login pools and same-source-only oldest-login eviction. Setup and
  Add retain account binding. Login now stores AccountId and LoginPurpose
  (SignIn/Reverify); all three take paths check account/purpose before removal.
  Callers supply an account for login before starting the challenge, replacing
  the source's account-unbound anonymous start. No account discovery UI or
  transport policy is selected here. Wrong-account/wrong-purpose calls leave
  the rightful challenge available; expiry and successful consumption remain.
- `session.rs:24–398`: retain token hashing, idle expiry/hourly background
  touch, account-owned list/revoke, process-local revoke fence and interactive
  freshness. Omit the DAV cache and revoke-app-password helper (`:91–94,107–108,
  323–331`); no app-password implementation is imported. Installation list omits
  scope columns/fields (`:74–77,267–305`), so returned sessions carry no role,
  capability or space. Legacy columns/tables and kind parsing remain unchanged.
  `require_fresh` additionally verifies account ownership, expiry and the local
  revoke fence because the source's trusted Caller/HTTP checks no longer wrap
  it. Clones share a fence; independent processes/services retain the source's
  cached-session limitation and must not claim immediate cross-process revoke.
- `sharing.rs:943–966`: carry only the SQL dormant-installation reset marker
  into session touch, preserving the legacy side effect without sharing APIs.
- `passkeys.rs:1–233,346–367`: retain list/add/rename/remove, labels and base64url
  IDs; export Row as documented PasskeyInfo. Removal uses IMMEDIATE rather than
  deferred transactions so two independently opened workers cannot both pass
  the last-key check. Omit ActionSpec/Outcome/EventDraft/schema adapters
  (`:235–344`); documented Auth methods perform their freshness checks. No
  account event is published into a shareable space journal.
- `setup.rs:38–60,92–115,132–145,186–241,244–399,402–655`: reuse SQL helpers,
  exact username/UUID selection, Unicode name protections, one-winner token
  redemption, recovery and all six inline tests. Remove CLI path-opening wrappers
  and CreateError/RecoverError (`:62–90,117–130,147–185`); async Auth methods use
  the existing worker and local Error. Create/recover use configured setup_ttl
  (default 24h), replacing command wrappers' fixed 24h. Created.id is AccountId.
  Remove invite checks/redemption (`:262–270,339–342`) and post-commit Home mkdir
  (`:397–398`): accounts are identity only and never select content. Invalid
  setup tokens return SetupTokenInvalid rather than InviteInvalid, since there
  is no invite store/transport error envelope in this library.
- `http.rs:150–176,204–224`: move only source/verified-token setup admission and
  source login admission into the library facade; source strings must already
  be derived by a trusted transport adapter. No unchecked forwarded-header
  parsing, cookies, routes or HTTP types are imported.
- `db.rs:454–467`: restore now_ms/sql helpers using this crate's Error. No SQL
  migration string, user role or server capability table is changed. No space
  table or dependency on old engine/server/plugin/core crates is introduced.

## Step 4 tests

- **12 inline source tests**: config origins (1), ceremony pools (2), session
  printable names (1), passkey label/ID conversion (2), setup names/usernames,
  Unicode normalization/confusables and exact recovery selectors (6).
- **29 adapted source library cases** in `tests/account.rs`: setup (7), create
  (5), login (6), passkeys (9), isolation (2). Source test names are retained
  except `adding_needs_a_sign_in_and_an_add_prompt_works_once`, whose library
  projection is named `an_add_prompt_works_once_and_login_cannot_be_used_to_add`.
  The old anonymous action check belongs to the deferred HTTP adapter.
- **2 existing ignored migration cases re-enabled**: real software credential and
  session survive v2 copy/upgrade; failed migration rolls back, retries
  idempotently and supports real post-upgrade login. People role/quota assertions
  query the persisted rows directly instead of the excluded people.list action.
  Every original identity/credential/session/schema assertion is retained.
- Source software passkey helper: `tests/common/mod.rs:202–278`. Retain its
  explicit discoverability assertion, SoftPasskey resident-key workaround,
  allowCredentials and userHandle injection; replace router/response wrappers
  with direct browser-options values. All fixtures use disposable tempfile roots.
- **11 additional step-4 cases**: proxy CIDRs, hourly session touch/hash storage,
  no-space session shape, two-connection last-key race, account/purpose binding,
  expired/stale/foreign/revoked freshness, foreign revoke fence isolation,
  ceremony expiry/disabled pools, add/rename/remove metadata, expiry recheck at
  redemption, and source/verified-target admission limits.

Step 4 brings **43 runnable source cases (41 newly adapted + 2 re-enabled)**
and 11 additional regressions. Together with step 3: **77 passing, 0 ignored**.
HTTP cookie/origin routing, action idempotent replay, account event journals,
invites/groups/quota/sharing/DAV and live transport suites are outside this
library port. In particular the original journal-event CRUD test is not claimed
as ported; metadata CRUD has separate library coverage. No ported test is ignored
or weakened to get a pass. No production/recovery-readiness claim follows.

## Added dependencies (manifest requirement / resolved)

- webauthn-rs 0.5.5 / 0.5.5, conditional-ui: same WebAuthn verifier as source;
  transitively OpenSSL 0.10.82 / openssl-sys 0.9.118 via the host wrapper.
- serde workspace 1 / 1.0.229 and serde_json workspace 1 / 1.0.154:
  persisted passkeys, browser challenge/credential values and library value types.
- url 2.5.8 / 2.5.8: source origin/RP validation.
- base64 0.23.1 / 0.23.1: source passkey credential identifiers.
- tracing 0.1.44 / 0.1.44: source background touch diagnostics.
- unicode-normalization 0.1.25 / 0.1.26, unicode-security 0.1.2 / 0.1.3,
  unicode-casefold 0.2.0 / 0.2.0, unicode-general-category 0.6.0 / 0.6.0:
  preserve NFC/NFKC, confusable administrator-name protection and invisible rejection.
- dev webauthn-authenticator-rs 0.5.5 / 0.5.5, softpasskey: real cryptographic
  enrollment/assertion tests using the source helper.

No root Cargo.toml entry needed. Cargo resolves the shared lockfile normally;
pre-existing lockfile edits are not reverted. See VALIDATION.md for commands.

## Questions for the user

None for this bounded library slice. HTTP wiring, current space authority and
FO1 security-before-ack publication remain the planned later steps. Recovery
retains the source's same-account add-key behavior; no administrator verification
or notification procedure is invented here.

## Historical step 3 record

Source: `/home/kayg/Developer/calternal`, `origin/dev` pinned to
`ea57b84b9c9e40e8b67b7d1e1f1805915123b040`. Read with git only.
This is port-map section 7 step 3; it does not enable authentication routes,
space authority, root opening, real-data migration or FO1 recovery.

## Source and adaptations

- `crates/auth/src/db.rs:1–332`: all fifteen SQL string values are byte-for-byte
  unchanged, including obsolete intermediate migrations and all sharing tables.
  Version remains 15. The runner at `:413–452` still takes an IMMEDIATE
  transaction before reading user_version, then commits all pending migrations
  together; connection WAL/FULL/foreign keys and 5 s busy timeout are unchanged.
- `db.rs:334–401`: export `Db` and its methods from a standalone `db` module,
  replacing old core Error/Result with this crate's private-diagnostic Error/Result.
  Keep lazy dedicated OS thread, shared connection, channel bound 64, and source
  `run` waiting backpressure. Factor initialization into private `worker` and add
  `try_run` for immediate `QueueFull` refusal: this meets the requested 65th-job
  refusal without changing source callers' waiting behavior. One executing job
  is separate from the 64 queued slots. `run` documents per-call costs and
  cancellation behavior. Thread-start panic and worker-panic behavior are inherited.
- `db.rs:338–339,358–367,376–377`: omit the test-only run counter; only later HTTP
  tests use this instrumentation, and the migrated inline tests do not need it.
- `db.rs:347–348`: retain ordinary std parent creation behind private
  `create_parent`; do not duplicate fsio helpers. Future fsio integration must
  choose its appropriate directory primitive. This is not a durability or root
  validation guarantee; source also used ordinary mkdir.
- `db.rs:403–411`: export `open_conn` for later CLI integration without old engine,
  server or plugin dependencies. Opening still creates a missing database;
  callers must apply the installation/root refusal policy first.
- `db.rs:454–467`: omit unused `now_ms` and `sql` helpers for excluded higher
  layers; no copied behavior calls them in this slice.
- `crates/auth/src/token.rs:1–160`: algorithms and all four tests unchanged;
  substitute local Error/Result and export the documented helpers. Formatting
  follows this workspace's rustfmt. No session/login logic is included.
- `crates/auth/src/limit.rs:1–331`: algorithms and all seven tests unchanged;
  replace the old core UserId wrapper with `uuid::Uuid` (same stable account UUID
  key, never a space ID). The test constructor becomes `Uuid::new_v4`. Export
  bounded primitives and document their methods. No old core dependency needed.
- Original manifest's product/protocol/HTTP/crypto/UI dependencies are excluded;
  only dependencies needed by these three modules remain. `src/lib.rs` defines
  local errors because the rewrite's core has wire ApiError, not the source's
  Error/Result. Private diagnostics must not be sent over the wire.
- All public items have doc comments, with missing docs and broken rustdoc links
  denied at crate level. The existing docs script does not select new auth crates,
  so validation also runs auth cargo doc explicitly. No scripts are changed.

## Source tests

- `db.rs:469–720`: all **7** inline tests carried with original assertions.
  Replace the old engine-coupled testkit's TempDir with a small test-only
  constructor adapter around tempfile; temporary paths remain disposable.
- `token.rs:103–160`: **4** unchanged inline tests.
- `limit.rs:193–331`: **7** inline tests, with UUID adaptation above.
- `tests/reliability_review.rs:805–846`: **1** runnable adapted test. Saturate both
  Tokio general blocking workers and keep the original 100 ms deadline; query
  the auth worker directly instead of excluded `/api/v1/auth/status` HTTP code.
- `tests/migration.rs:1–123`: **1 ignored** full integration test, pending real
  software-passkey enrollment, login/session, setup/recovery and people APIs.
  The original body is preserved in `tests/fixtures/source/migration.rs.txt`.
- `tests/security_regressions.rs:423–507`: **1 ignored** full integration test,
  pending software-passkey enrollment and post-migration login. Original body
  retained in `tests/fixtures/source/security_regression.rs.txt`; its database
  rollback/idempotence assertions also run now in a separate database-only test
  with opaque credential bytes. This does not claim real credential usability.
- `tests/fixtures/auth-v2.sql`: copied verbatim for the rollback regression.

Totals: **21 source cases carried: 19 runnable (18 inline + 1 blocking-pool
adaptation), 2 explicitly ignored**. No passing test had assertions weakened to
make it pass. Four additional runnable tests check historical/fresh/reopen
sqlite_master equality, transactional rollback/identity preservation and retry,
all four connection pragmas, and queue saturation plus waiting backpressure.
The schema comparison includes type/name/tbl_name/sql for tables, indexes and
triggers; it excludes physical rootpage allocation numbers.

## Dependencies

Direct crate requirements / resolved validation versions:

- rusqlite `0.41` / `0.41.1`, bundled SQLite: matches rewrite server, supports all
  inherited SQL including DROP COLUMN and STRICT tables.
- tokio workspace `1` / `1.54.1`, add sync feature: bounded MPSC and oneshot;
  workspace already supplies runtime/macros/time.
- thiserror workspace `2` / `2.0.21`: local typed errors.
- uuid workspace `1` / `1.29.2`: replace old UserId without old core dependency.
- blake3 `1.8.7` / `1.8.7`: unchanged token hashes and constant-time comparison.
- getrandom `0.4.3` / `0.4.3`: unchanged OS entropy fill API.
- tempfile dev `3.27` / `3.27.0`: disposable source-test directory adapter.

During step 3, the shared root Cargo.lock was outside the lane's ownership
and was untouched (see the step 4 preflight note above for its later update).
Validation resolves a copied lockfile in a disposable workspace; integration
must include the new auth package and BLAKE3 dependency entries in the shared
lockfile before invoking --locked against this checkout.

## Questions for the user

None for this bounded port. Space/legacy ownership mapping, invitation semantics
and FO1 coordination stay in their later port-map steps; no product choice was
made here.
