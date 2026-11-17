# Per-owner secret keys — 2026-11-17

Thread: `thr_sesad9aeyd`. [Forgejo issue #26](https://git.kayg.org/kayg/calternal-ai/issues/26).
Branch: `lane/secret-keys`. Baseline: `origin/dev` at `9f64cc9`.
Build/test host: `ubuntu@calternal-dev-02-std`, `~/Developer/lanes/secret-keys`.

## Change

Each account or space has its own 0600 `secret.key` beside `secrets.db`. The old
server-wide key and owner registry code are removed. Owner identity, name and version
remain authenticated encryption inputs; independently generated owner keys provide
isolation even when the encryption's owner binding is deliberately held constant.

Key publication uses `fsio::private::publish`, built on the existing exclusive rename
primitive. Unlike hard-link staging, it cannot leave another link to published key
bytes after a crash. Unpublished staging bytes have never encrypted a committed row.
The owner transaction records a fingerprint through the auth recovery barrier before
committing any ciphertext. No key bytes enter the log.

Erasure records an irreversible protected tombstone before unlink and directory fsync.
An owner-local IMMEDIATE transaction serializes release against erase. Deleting a space
marks erasing in the same protected transaction as deletion, then unlinks before the
operation returns. Startup finishes interrupted erasures and refuses missing/wrong live
keys. The erase API also handles a lost database or interrupted initial key creation.

`secrets.manage` controls space list/provide/revoke/cancel and defaults to Manager and
Owner. Requesting use still requires `plugins.use`; the plugin host still checks declared
secrets. Personal secrets remain account-private. Plugin-owned secrets remain under their
enabling space. Pending requests expire after 15 minutes, refuse late answers and can
be requested again. Rotation continues to invalidate old handles immediately.

## Verification

All gates passed on the final implementation. All commands run from the lane
directory on the named VM under `nix develop -c`.

| Exact command | Result |
| --- | --- |
| `cargo test --locked -j6 --workspace` | PASS: 496 tests across 65 result groups, including doctests |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | PASS |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --test isolation -- --test-threads=1` | PASS: 44 tests, including real-session capability subtraction and HTTP owner deletion/key erasure |
| `npm run test:plugins` | PASS: 11 SDK and 2 plugin tests |
| `npm run test:integration` | PASS: 1 real-server integration test |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | PASS |
| `cargo build --locked -j6 -p calternal-cli` then `python3 contract/verify-cli.py` | PASS: real-server CLI smoke, including restart persistence |
| `sh scripts/check-docs.sh` | PASS: Rust checks/rustdoc, standalone conformance checks, TypeScript and GenUI docs |
| `cargo fmt --all -- --check` | PASS |
| `rustfmt --check --edition 2024 --config skip_children=true tests/conformance/isolation.rs` | PASS on the touched standalone harness; unrelated child modules retain their original formatting |
| `node scripts/docs-site/generate.mjs` | PASS: 201 pages generated; only changed secrets/contract pages retained |

Preparation: `cargo fmt --all`, `cargo run --locked -j6 -p calternal-core --bin
export-contract`, `npm ci --ignore-scripts --no-audit --no-fund`, `npm run
build:plugins`, and `npm ci --prefix apps/docs --ignore-scripts --no-audit
--no-fund`. The docs gate installs its isolated pinned documentation tools itself.

The new end-to-end deletion assertion caught an actual failure: an outer SQLite
UPSERT overrode the recovery trigger's `INSERT OR IGNORE`, producing a duplicate
dirty-marker constraint error during repeated lifecycle publication. The auth API now
uses explicit insert/update and avoids rewriting unchanged state. A protected recovery
regression verifies repeated record/erase calls, refusal to revive a tombstone, and
post-deletion idempotence. The HTTP deletion assertion and final full gates pass.


The source transfer is the user-prescribed `git ls-files -co --exclude-standard -z |
tar --null --ignore-failed-read -T - -cf /tmp/secret-keys.tar`, followed by SSH extraction
into that lane directory. No builds or tests run on hd01.

## Review gate

1. Shared `AccountId`, `SpaceId`, `Capability`, `Actor` and `now_ms` are reused. No
   duplicate shared ID/error/capability type is introduced.
2. Existing dependency direction is unchanged. No dependency or crate is added;
   exclusive publication belongs in fsio, using its existing rename/temp helpers.
3. Ordinary secret calls open only the owner's database/key. Only cold key lifecycle
   metadata uses the existing protected auth worker. The owner transaction spans fsync
   and recovery publication to prevent release across erasure; its comment explains why.
4. Operations, trusted provide, deletion cleanup and startup key reconciliation run on
   blocking workers. SQLite, filesystem work and hashing do not run on request async workers.
5. Known-owner `use_value`: one database open/schema-version read, one ciphertext read,
   one fingerprint/erasing-state read, one 32-byte key read, one AEAD open and one audit
   transaction commit (one WAL fsync). A key is loaded once per call and never cached.
   Reads have no flush; first creation additionally publishes/syncs a key and crosses
   the protected auth barrier. README states the cold-path costs.
6. Schema SQL and request TTL are constants; schemas are installed on initial database
   creation. The operation catalogue/validators retain their existing initialization.
7. Existing bounds remain: 1,000 secrets, 64 pending requests, 10,000 audit rows per
   owner; values 64 KiB. Requests expire after 15 minutes and are pruned before new
   admission. Recovery keeps one fixed fingerprint/tombstone row per owner, not key
   history. A production prune horizon depends on backup retention; see limitations.
8. No migration, legacy key reader, unlock flow or compatibility fallback is added.
   This is disposable preview data; no deployed-store rekeying is attempted.
9. Account and space paths use typed UUID identities. The two namespaces remain
   distinct even when UUID bytes match. No names or app-specific owner aliases are used.
10. `secrets.manage` is declared in the shared vocabulary; defaults are auth policy rows;
    server policy consumes it. Key lifecycle metadata lives in the auth recovery inventory;
    owner-local fingerprint state provides the separate decryption/erase check.
11. New public APIs have production callers: fsio publication from key creation;
    lifecycle attachment, fingerprint validation and auth registry from server startup;
    owner erase from space deletion. Account deletion can call that same explicit API
    when an account-delete operation exists; no unused account-delete route is added.
12. Reuse considered: original app invite key publication/AEAD/owner-binding pattern,
    existing fsio private preparation/exclusive rename, SQLite IMMEDIATE/FULL, auth
    `spaces::commit` barrier and recovery dirty-record inventory.
13. All new public items document purpose, failure and cost; secrets/fsio/auth are
    covered by the workspace docs gate. Changed secret API comments move with the code.
14. README, plugin decision text, generated operation descriptions and docs-site sources
    change together. This dated report is the lane record.
15. Formatting is checked on the VM. New modules separate key lifecycle and its tests;
    `secrets/local.rs` remains below 600 lines. Existing larger server entrypoints and
    recovery/conformance test harnesses receive only integration additions; splitting
    unrelated shared harnesses is outside this lane.

## Limits and unfinished work

No account-delete route exists in this preview; the explicit owner erase API and auth
account-deletion tombstone trigger are ready for its future caller. No production
backup/restore runbook, rekeying of deployed stores or performance qualification is
claimed. A separately retained copy of a key can still decrypt its matching database;
erasing a live key cannot erase copies held elsewhere.

Backup retention must settle when tombstones may be pruned. Keeping one per historical
owner preserves restore refusal at a small per-owner metadata cost; pruning requires
proof that every older ciphertext backup has expired. This lane preserves the markers.

## Simplification opportunities

Use the same erasure path when account deletion is introduced. Keep key bytes out of
recovery and avoid a derived-key cache, which would complicate immediate erasure. The
old installation-wide secrets registry has been removed rather than retained alongside
the protected owner-key inventory.
