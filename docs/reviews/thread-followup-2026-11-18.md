# Thread account settings, sign-in and file ceiling

## Browser approval diagnosis

On dev-01, the backend at `origin/dev` commit `806421a` accepted
`requests.approve` after an HTTP re-verification with the conformance passkey.
The probe used the replacement cookie from `/api/v1/auth/reverify/finish` and
the existing request-owner and approval lifetime checks.

A Chromium virtual authenticator registered a browser-kind first-owner
installation using the setup calls from `apps/web/e2e/requests-server.mjs`.
The initial approval returned 403 with `passkey_reverification_required`.
Re-verification start and finish returned 200 and replaced the browser
cookie. Approve once, approve for this thread, and secret provide then
returned 200 on both the dev server and the lane server. The probe compared
cookie values in memory and logged only whether the cookie changed.

Commit `3156e14` introduced the dedicated stale-proof error. The Threads
client's retry still checks only `forbidden`. The UI lane owns that
predicate and the browser device-zone fallback. The auth check reads the
current installation's assertion time from SQLite.

The dev snapshot came from `git archive origin/dev` under
`~/Developer/lanes/turn-meta/approval-dev`. The production sources in that
snapshot were unchanged. The Rust probe replaced the existing approval
test's fresh-login step with the browser's re-verification HTTP sequence.

```sh
npm ci --ignore-scripts
npm run build:plugins
CARGO_PROFILE_DEV_DEBUG=0 cargo build --locked -j6 -p calternal-server --features test-plugins
CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 \
  ./scripts/check-podman-tests.sh cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml \
  --test isolation request_operations_keep_questions_approvals_and_secrets_account_private -- --test-threads=1 --nocapture
npm run web:build
```

The Chromium probe used the existing request fixture provider and the exact
same-origin setup, re-verification and approval endpoints, with a disposable
server and a separate browser context for each binary. Diagnostic logs live
under the dev-01 lane's `.gate-readable/`. The full UI script timed out
before approving a request. The browser result above comes from the direct
Chromium HTTP probe.

## Backend changes and costs

`account.set_time_zone` updates the active account's nullable IANA zone with a
protected receipt. The private `auth/me` response exposes that preference.
The heading path reads the account's display name and zone in the same
indexed query. The CLI sends its device zone and numeric offset. Web can
send `Intl.DateTimeFormat().resolvedOptions().timeZone` as `client_time_zone`.
The server snapshots the selected name in the turn intent and resolves its
offset at each write. The name remains private in shared thread reads.
Create and lifecycle edits add one profile lookup. Send reuses the existing
lookup and adds zero account or content reads and flushes. Provider frames
add zero state reads and flushes. A cold named-zone lookup can read the
system zone database and caches the loaded rule.

An attempted sign-in writes one readable prefix line. Completion updates the
same line. Both revisions publish `threads.provider_sign_in` with
`data.notice` and request a full refresh. A flow makes one indexed author-zone
read and at most two ordinary receipted content writes. These run outside the
provider output path. The thread retains its latest 16 flow results and keeps
message byte ranges unchanged.

A full thread rejects an overflowing checkpoint before writing its revision.
The 2 MiB limit includes the host identity line. Live events, the journal
turn reason and reloads use `thread_full`. The previous revision remains
readable.

Auth schema 11 adds the nullable account field under the existing
single-version policy. The branch's verification uses fresh disposable
auth data. Existing installations on schema 10 need the project's
deployment data-upgrade decision before rollout.

## Dev-01 gate commands

Commands run under `nix develop` in `~/Developer/lanes/turn-meta`.

```sh
export CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal"
export CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-provider-fixture:turn-meta-readable
export RUST_TEST_THREADS=1 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all
cargo fmt --manifest-path tests/conformance/Cargo.toml
npm run build:plugins
cargo run --locked -j6 -p calternal-core --bin export-contract
UPDATE_GOLDEN=1 cargo test --locked -j6 -p calternal-cli --test people
./scripts/check-podman-tests.sh plugins/calternal-claude-code/test/build-image.sh "$CALTERNAL_TEST_PROVIDER_IMAGE"
cargo build --locked -j6 -p calternal-server --features test-plugins
./scripts/check-podman-tests.sh cargo test --locked -j6 --workspace --all-features --no-fail-fast
./scripts/check-podman-tests.sh cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --test isolation -- --test-threads=1
npm run test:integration
npm run test:plugins
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
node scripts/docs-site/generate.mjs
./scripts/check-docs.sh
cargo fmt --all -- --check
cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check
```

The follow-up at `56f2220` passed 594 workspace tests, 99 serial conformance
cases, 25 plugin tests, one integration test, both formatter checks, contract
generation and check, site generation, and the docs gate. Workspace and
conformance both passed the Podman leak checker. The real signed-in Claude
and workspace opt-in cases were skipped.

The merged snapshot `5efa93d` includes `origin/dev` at `6f67c18`. It
passed 594 workspace tests, 102 serial conformance cases, 26 plugin tests,
one integration test, the contract and fixture checks, site generation,
the docs gate (326 pages), and both formatter checks. After the merge, the
Chromium probe again passed approve once, approve for this thread and secret
provide. Workspace and conformance leak checks passed. The lane removed its
workspace, conformance and rustdoc targets, plus the isolated baseline
workspace and conformance targets.

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | Existing account/space IDs, error codes, receipt, UTC clock, and a shared zone resolver. |
| 2. Dependencies | One civil time-zone dependency in the shared core used by auth, server and CLI. Protected writes use rusqlite. |
| 3. Lock scope | Profile reads reuse the protected authority worker. Notices reuse the per-space write fence and journal. |
| 4. Blocking | Device discovery runs before the CLI runtime. Account DB, file and time-zone work use existing blocking paths. |
| 5. Reads and flushes | Costs are above. An intent carries the selected zone through a retry, and a notice prefix preserves the output cursor. |
| 6. Constant work | Jiff caches IANA rules and the operation catalog registers the same generated schema once. |
| 7. Bounds | One nullable 128-byte zone per account, 16 notices per thread, a 2 MiB file and existing receipt retention. |
| 8. One format | Current auth schema and current framed transcript. |
| 9. Space identity | File and journal writes remain UUID-scoped. A zone operation targets the authenticated account. |
| 10. Single authority | The protected account row stores the preference. The provider sign-in relay supplies the outcome. |
| 11. Wiring | CLI and HTTP operations call the profile writer. Heading and sign-in writers consume the profile. The UI lane owns browser payload wiring. |
| 12. Reuse | Existing protected profile mutation, content CAS, outbox, provider sign-in relay and tool fixture. |
| 13. Public docs | Shared resolver and authority methods document purpose, failure and cost. |
| 14. Behavior docs | Account operation, CLI, contract, thread guide, generated docs and title options follow the new behavior. |
| 15. Formatting | Formatter checks cover the workspace and standalone conformance crate. New framing and zone code live in separate modules. |

## Simplification opportunities

Use the dedicated fresh-proof error in the Threads retry handler. Keep the
browser on structured message reads so sign-in notices never affect its
message parser.
