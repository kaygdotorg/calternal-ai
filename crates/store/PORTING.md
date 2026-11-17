# Per-space journal port

Source: `/home/kayg/Developer/calternal`, git object
`ea57b84b9c9e40e8b67b7d1e1f1805915123b040` (`origin/dev` at review).
Read with git only. This is a development port; it never imports real data.

| Source path and lines | Adaptation and reason |
|---|---|
| `crates/store/Cargo.toml:1–27` | Workspace metadata/dependencies adapted; no old core/engine/server/plugin/testkit dependency. Local DTOs/errors replace old core imports. `tempfile` supplies disposable fixtures in place of old testkit. |
| `crates/store/src/lib.rs:1–114,172–286,625–647` | `UserStore` becomes `SpaceStore`. Explicit typed UUIDv7 space selects `<state>/spaces/<space-id>/index.db`; external witness/key path is `<secrets>/spaces/<space-id>/{token.key,head}`. Account and space IDs are distinct local types pending a shared-types crate. Fresh generators allocate independently; identity registration remains the host's job. Host must validate roots and current authorization before use. |
| `crates/store/src/lib.rs:172–286,320–469`; `token.rs:15–67` | Public opens always require the independent secrets root and signed cursors. Derive the signing key from the random per-space seed plus explicit space UUID with BLAKE3 context `calternal space journal cursor v1`. Plain cursor compatibility is retained only in private test fixtures. Public `position` rejects unsigned positions, including `head()` display output; use `token_at` or entry tokens. Public `token_at` rejects negative/missing positions instead of emitting unsigned unusable cursors. No deployed aliases or key compatibility is required. |
| `crates/store/src/lib.rs:34–81,473–517`; `writer.rs:148–240`; `read.rs:15–43` | `StampedEvent.author` is the acting `AccountId`, stored and read explicitly. Both expiring request keys and permanent publication keys use a fixed-width canonical account UUID prefix plus the opaque request ID; callers cannot bypass encoding. Retrieval requires the authenticated acting account supplied by trusted host code. A space store itself does not authenticate callers. |
| `crates/store/src/schema.rs:13–40,67–86` | Preserve all three source migrations and four STRICT application tables; migration 4 adds `journal.author TEXT NOT NULL`. No invented actor is assigned to existing rows: a populated legacy database cannot receive this migration without a separate reviewed import. `meta.space_id` binds the database and refuses another space. Schema is v4. The source's older-build rollback comment no longer applies to writers lacking author attribution. |
| `crates/store/src/{highwater.rs:1–651,proof.rs:1–298,restore.rs:1–279,shared.rs:1–539,token.rs:69–257}` | Retain witness slot checks, inode/path recovery, restore epoch ordering, bounded proof/issued-token caches, flusher deadlines and save guards. Imports/terminology change; entry checksum and 32-bit source tag wire format are retained. Witness detects rollback; it does not reconstruct files, authentication or FO1 snapshots. |
| `crates/store/src/writer.rs:19–132,148–250`; `read.rs:45–123` | Preserve bounded 64-command writer, post-commit ordered broadcasts, indexed capped reads, expiration sweep, atomic duplicate rollback and permanent publication receipts. Ordinary commits remain WAL/NORMAL; permanent publications use FULL then restore NORMAL. NORMAL is not a power-loss-durable content acknowledgement guarantee. |
| `crates/core/src/journal.rs:14–137` and its inline tests | Copy only sync token/proof parser, journal DTO and parser tests into `contracts.rs`; omit action/sink/recorder traits because they depend on old engine contracts. Add author to the DTO. Stable local store errors preserve Internal/IdempotencyConflict/SyncTokenExpired semantics; InvalidInput covers binding errors. |
| `crates/engine/src/runtime.rs:1–195`; `crates/engine/src/tests.rs:626–674` | Adapt lazy OnceCell map/idle sweep into `SpaceStores`, omitting action in-flight response machinery. Host explicitly schedules `sweep` to bound fully idle release (600s target); `get` also sweeps. Initialization happens outside map locks; eviction drains outside them. Held handles/listeners and now active owned save guards prevent eviction. Concurrent release/reopen stress retains 4 spaces × 4 workers × 40 writes and every contiguous sequence assertion, using store commits instead of old engine actions. |
| `crates/store/src/tests.rs:1–266`; `tests/restore.rs:1–874`; `tests/restore_failures.rs:1–361` | Carry every test and assertion. Mechanical fixture changes: private explicit fixed-space test opens, typed account on events/receipts, new witness directory, tempfile wrapper, explicit author on the one raw journal INSERT. Legacy unsigned fixtures remain private to preserve those source tests without exposing unsigned public cursors. |

Public operation documentation states writer/SQLite/allocation costs. Root locks,
real membership checks, installation ownership and one pool per installation
remain host responsibilities; independent database files are not tenant isolation.
No backend endpoint or UI is enabled by this crate. FO1 security recovery,
account-private event streams, real-data import and full tiering are outside this
lane. The existing documentation gate does not name the new crate; a separate
`cargo doc -p calternal-store` checks its denied missing docs and broken links.

## Tests

- All **73** source store tests: main store 9, restore 30, restore failures 12,
  highwater 9, proof cache 6, schema 2, shared flush activity 3, token hashing 2.
- **3** source core token-parser tests and **1** adapted engine release/reopen
  stress test: **77 ported tests total**, none ignored or weakened.
- **3** new tests: member-qualified request/permanent receipts plus author
  persistence; equal-epoch/copied-key cross-space cursors and DB binding;
  live handles/listeners/owned saves preventing release. **80 store tests total**.
- The old engine's other full-engine restore/integration tests require its
  operation/runtime host and are not imported as obsolete dependencies. Store
  restore and restore-failure suites are complete.

## Dependencies

`rusqlite 0.41` with bundled SQLite (same workspace version): journal SQL;
`blake3 1.8.7`: inherited checks/tags and per-space key derivation;
`tokio 1` (resolved by lockfile): async channels/tasks/timers;
`tracing 0.1.44`: inherited diagnostics; `uuid 1` with v4/v7/serde:
inherited epoch/key randomness, new UUIDv7 IDs and serialization;
workspace `serde 1`, `serde_json 1`, `thiserror 2`: DTOs/JSON/local errors;
workspace `calternal-fsio`: inherited durable metadata writes;
dev-only `tempfile 3.27`: disposable fixtures. Versions resolved in Cargo.lock
are recorded in the lane's final verification report. No old product dependency
or sqlx is added.

## Questions for the user

None. This lane's choices follow the supplied decisions and source contracts.

## Verification

Executed in this checkout with the bounded toolchain wrapper:

- `scripts/with-toolchain.sh cargo test --locked -j4 -p calternal-store -p calternal-roots -p calternal-fsio`:
  exit 0; store 80, roots 15, fsio 56 passed; no failures or ignored tests.
- `scripts/with-toolchain.sh cargo doc --locked -j4 --no-deps -p calternal-store -p calternal-roots -p calternal-fsio`:
  exit 0, including denied missing docs/broken intra-doc links.
- `scripts/with-toolchain.sh cargo test --workspace -j4`: exit 101,
  blocked by concurrent auth lane's E0308 at `crates/auth/src/spaces.rs:166`
  (`MutexGuard<Connection>` versus `Connection`). No auth source was changed here.
- `scripts/check-docs.sh`: exit 101 at the same auth compilation error,
  before completion of the repository gate. Store rustdoc was checked separately
  because this script's package list does not yet include calternal-store.
- `git diff --check -- Cargo.toml Cargo.lock crates/store crates/roots crates/fsio`:
  exit 0.

Resolved dependencies: rusqlite 0.41.1, BLAKE3 1.8.7, Tokio 1.54.1,
tracing 0.1.44, UUID 1.29.2, serde 1.0.229, serde_json 1.0.154,
thiserror 2.0.21, tempfile 3.27.0. Cargo generated the new local package's
Cargo.lock entry; no new third-party package entries were needed.
