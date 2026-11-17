# Calternal-AI simplification review

Branch `dev` at `1c84595`, main checkout, read-only. Nothing was built or run: every claim below comes from reading the code, and anything about runtime cost is reasoning from the code, not a measurement.

Confidence: **High** = read every line cited; **Medium** = read the cited lines, but the conclusion depends on behaviour I could not run (SQLite flush counts, rustfmt, tests).

Size: S = under half a day, M = one to three days, L = a week or needs an owner decision first.

"Conflicts" refers to the lanes in progress: `lane/plugin-jobs` (touches `crates/server/src/api.rs`, `plugins/mod.rs`, `plugins/broker.rs`, `trusted-plugins.json`, `crates/plugin-host`, `packages/plugin-sdk/src/manifest-v1.ts`), `lane/secrets` (new `crates/secrets`, edits `scripts/check-docs.sh`), `lane/machine` (no commits beyond `dev` yet).

## 1. Ranked table (value for effort, best first)

| # | Finding | Why it matters | Size | Conf. | Conflicts with work in progress |
|---|---|---|---|---|---|
| 1 | `GET /describe` rebuilds 38 JSON Schemas on every call | performance | S | High | plugin-jobs edits `api.rs` (trivial merge) |
| 2 | Resolver runs the same capability query twice; an update makes 5 capability round trips | performance | S | High | plugin-jobs edits `api.rs` |
| 3 | Docs gate omits `calternal-server` and `calternal-recovery`; `docs/documentation.md` describes an older gate | docs | S | High | secrets edits `check-docs.sh` (one-line merge) |
| 4 | `private_fs` lives in `auth`, so `content` (and now `secrets`) depend on the whole auth crate for two file helpers | modularity, reuse | S | High | secrets imports `calternal_auth::private_fs` |
| 5 | README quick start cannot work as written; three docs still describe `/s/<space>` routes and the old storage | docs | S | High | none |
| 6 | `auth.db` is opened twice (two connections, two worker threads) and coordinated through a process-global map | performance, modularity | S-M | High | none |
| 7 | Three stacked global serialisation points make the per-space design moot | performance | M | High | plugin-jobs (`api.rs`, `broker.rs` take the same lock) |
| 8 | `SpaceId`/`AccountId` defined three times with different rules; four same-shaped error structs; no shared types crate | modularity, reuse | M | High | secrets and machine will both need the IDs |
| 9 | `personal`/`shared` alias shim runs through directory, store, resolver and every response | performance, simplicity | M | High | plugin-jobs edits `api.rs`, `tests/support` |
| 10 | Auth schema is 15 verbatim legacy migrations plus one; about 14 tables have no writer | simplicity | M | High | none (recovery lists the tables, but is unwired) |
| 11 | One note update reads the body 3 times, hashes the file twice, parses or clones the input about 8 times, and keeps the full body forever in a receipt | performance | M | High (counts), Medium (flushes) | plugin-jobs (`api.rs`) |
| 12 | About 6,000 lines are ported or built but have no caller (recovery, roots guard, most of fsio, v1 worker types, dead server exports) | simplicity, docs | S per item | High | none |
| 13 | Worker cold start hashes the bundle and spawns a process on an async thread while holding the pool lock | performance | S-M | High | plugin-jobs directly |
| 14 | Capability and role vocabulary is written in five places; custom roles vanish from `describe` | versatility, reuse | M | High | plugin-jobs edits `manifest-v1.ts` |
| 15 | Store keeps a 4-step migration chain, a legacy witness reader and a test-only non-strict mode | simplicity | S-M | High | none |
| 16 | Two per-space caches with two idle sweeps; the handle one of them holds is never used | performance, simplicity | S-M | High | none |
| 17 | `content` hand-rolls check-then-replace placement while `fsio::swap` already provides the race-free version | reuse, correctness | M | Medium | none |
| 18 | Notes is hard-coded in core, server, client and CLI; one operation is declared in six places | versatility, modularity | L | High | plugin-jobs (`api.rs`, `plugins/mod.rs`) |
| 19 | Four receipt stores, cross-checked by hand in the server | simplicity, performance | L, needs decision | High | plugin-jobs (`broker.rs` writes receipts) |
| 20 | v0 contract remnants: second manifest type with a lossy "compatibility projection", second GenUI format | simplicity | M | High | plugin-jobs; web UI is Claude-only |
| 21 | Files over 800 lines mixing concerns (`api.rs`, `auth/db.rs`, `core/models.rs`) | modularity | M | High | plugin-jobs (`api.rs`) |
| 22 | Naming drift (`tokens`, `TokenTable`, principal/actor/account/user, `Scope::Note`) and unformatted dense code | docs, readability | S-M | High (names), Medium (fmt) | plugin-jobs (`api.rs`) |
| 23 | Dependency drift: two `getrandom` majors, `sha2` and `blake3`, `rusqlite` declared five times | simplicity | S | High | secrets adds a sixth `rusqlite` line |
| 24 | Minor: session cache never evicts idle entries; `pool.get` sweeps on every call | performance | S | High | none |

## 2. Per-finding detail

### 1. `describe` rebuilds the schema catalog per request (S, High)

- **What:** `describe` calls `operation_catalog()` on every request (`crates/server/src/api.rs:336`). That function runs `schemars::schema_for!` twice for each of 19 operations and round-trips each through `serde_json::Value` (`crates/core/src/validation.rs:546-560`). Its own doc says "Allocates fresh schema trees on each call; callers may cache" and no caller does.
- **Why:** per-request work that is constant for the process lifetime; the web client calls `describe` on every connect.
- **Fix:** build the operations array once in a `OnceLock` (or pre-serialise it) and splice in the principal.

### 2. Duplicate capability lookups (S, High)

- **What:** `resolve_for` calls `directory.capabilities(...)` twice in a row with identical arguments and discards the first result (`crates/server/src/resolve.rs:47-54`). Each call is a cross-thread round trip to the auth-db worker plus two SQL statements (`crates/auth/src/spaces.rs:397-403`), returns `Vec<String>`, and is converted back to a bitset by linear name scans (`crates/server/src/auth.rs:266-275`).
- **Count for one `notes.update`:** resolver 2 (`api.rs:491`), admission 1 (`api.rs:523`), inside the handler 1 (`api.rs:990`), delivery 1 (`api.rs:563`), plus a receipt lookup on the same worker (`api.rs:997`). Two of these happen while the global store lock is held.
- **Fix:** delete the first lookup now. Then have `capabilities` return the bitset directly (one SQL with the role join) and pass the admitted set into the handler, keeping only the delivery recheck.

### 3. Documentation gate gaps (S, High)

- `scripts/check-docs.sh:10-18` lists 11 crates; `calternal-server` and `calternal-recovery` are missing although both carry `#![deny(missing_docs)]` (`crates/server/src/lib.rs:5`, `crates/recovery/src/lib.rs:2`). `tests/conformance` has its own `Cargo.toml` outside the workspace, so neither the gate nor `cargo test --workspace` (`scripts/verify-preview.sh:12`) sees it.
- `docs/documentation.md` still says the script covers "core, client, CLI and bindings" and that the server is "intentionally absent".
- Cost statements, which the owner's rules ask for: `auth` has 82 public functions and about 12 doc lines mentioning cost, blocking or allocation; `fsio` 57 and 6; `roots` 3 and 0. By contrast `server`, `store` and `content` state costs almost everywhere.
- Comments written to satisfy the gate, not the reader: ten identical "Worker v2: X; bounded transport and host-authorized effects" lines (`packages/plugin-sdk/src/index.ts:540-567`) and nine "Schema representative for ..." fields (`crates/core/src/models.rs:797` onward).
- **Fix:** add the two packages to both cargo invocations, add the conformance manifest, rewrite the "Check" section of `docs/documentation.md`, and add a cost line to the auth public API as part of finding 10.

### 4. `private_fs` belongs in `fsio` (S, High)

- **What:** `crates/auth/src/private_fs.rs` (80 lines: owner-only directory and file creation) uses only `std` and `calternal_fsio::sync_dir`. It has 13 callers outside auth: `content` 8 (`db.rs:19,21`, `files.rs:148,259`, `journal.rs:70-72`, `lib.rs:98`), `server` 5. `content` has no other use of `calternal_auth`, so the whole of auth (webauthn-rs, openssl, unicode tables) is a dependency of content for this. `lane/secrets` repeats the pattern: its `Cargo.toml` says "private 0600/0700 file helpers, typed owner IDs and the entropy helper come from the ported auth crate".
- `fsio` already has the near-duplicate `create_dir_all_durable` (`crates/fsio/src/lib.rs:267`).
- Side effect on the hot path: `private_fs::directory` does a stat and a chmod on every call, and is called twice per item write (`files.rs:148`, `files.rs:259`).
- **Fix:** move the module to `calternal_fsio::private`, re-export from auth for one commit, drop `calternal-auth` from `content/Cargo.toml`, and stop calling it per write (create the kind directory at registration).

### 5. Stale documentation (S, High)

- `README.md:20-22` tells the reader to open `.dev-data/dev-tokens/owner.token`, but tokens exist only with `--development` (`crates/server/src/main.rs:37-39`, `crates/server/src/auth.rs:203-218`) and `scripts/dev-preview.sh` does not pass it.
- `README.md:24` and `docs/ui-preview.md:13,39,41` describe `/s/<space>/notes/<id>`; `docs/addresses.md` says an address never contains its space. (The web router still builds `/s/` paths, `apps/web/src/lib/route.ts:27-29`; that is a Claude-lane UI change.)
- `crates/server/README.md:97` says `state.sqlite3` is authoritative for revisions and files are `Notes/<uuid>.md`; `crates/server/src/store.rs:3-4` says that database "has no item, content or history tables", and filenames now come from titles.
- `README.md:40` "SQLite is the preview's authoritative revision/receipt store" and `docs/tonight-progress.md` ("39 tests", terminal ID, branch `feat/overnight-preview`) are point-in-time handoffs presented as current.
- Seven lane reports sit beside code as if they were reference docs: `crates/auth/VALIDATION.md`, `crates/auth/SPACES-VALIDATION.md`, `crates/server/HTTP-VALIDATION.md`, `crates/client/VERIFICATION.md`, `crates/plugin-host/VERIFICATION.md`, `crates/recovery/REVIEW.md`, plus each `PORTING.md`. They record what one agent ran on one day.
- **Fix:** correct the README quick start, move dated reports to `docs/reviews/` (which already exists), and mark `tonight-*.md` and `morning-decisions.md` as historical or delete them.

### 6. `auth.db` opened twice (S-M, High)

- **What:** `Auth::open` opens a `Db` (`crates/auth/src/account.rs:36`) and `Spaces::open` opens another on the same file (`crates/auth/src/spaces.rs:330`); the server creates both (`crates/server/src/auth.rs:207,220`). Each `Db` is a connection inside an `Arc<Mutex>` plus its own `auth-db` worker thread (`crates/auth/src/db.rs:339-342,440-458`).
- Because there are two, the recovery hook is a process-global `HashMap` keyed by canonical path (`spaces.rs:586-592`), every commit calls `std::fs::canonicalize` and takes that global mutex (`spaces.rs:612-621`), and a `static COMMIT_GATE` serialises commits across both (`spaces.rs:640`).
- A third connection is opened read-only on every alias lookup (`crates/server/src/store.rs:182-198`), see finding 9.
- **Fix:** give `Spaces` a constructor that takes the existing `Db` clone; store the sink in `Db`; delete the global map, the per-commit `canonicalize` and the static gate. The `Mutex` around the connection can go too once `open_conn` is the only non-worker user.

### 7. Three global serialisation points (M, High)

- **What:**
  1. `AppState.store: Mutex<Store>` (`crates/server/src/api.rs:49`). Every read and write for every space takes it (`api.rs:420-434`), and it is held across the whole write including file and directory fsync, and across auth-db round trips (`api.rs:989-1006`). Plugin host calls take it again on each boundary (`crates/server/src/plugins/broker.rs:89,127,155`).
  2. All journal work for all spaces is funnelled through one `content-journals` thread that runs one job at a time with `block_on` on a second, private tokio runtime (`crates/content/src/journal.rs:25-56,82-95`).
  3. The item routing catalog is one `Mutex<Connection>` (`crates/content/src/lib.rs:57,245,260`).
- **Why:** the store crate's stated design is "Independent writers avoid a global application write queue" (`crates/store/src/lib.rs:3-5`), and `content` has per-space mutexes and 64 write stripes (`content/src/lib.rs:62,116,455`). None of that can take effect behind point 1. A slow disk in one space stalls every space, including reads.
- **Fix:** `Content` methods already take `&self`; `Store::create/update/materialize` take `&mut self` without needing it (`crates/server/src/store.rs:393,439,475`). Put only the plugin-state connection behind its own mutex and share `Store` by `Arc`. Replace the bridge thread with direct calls on the `SpaceStore` handle each cached space already holds (finding 16), run from the blocking task. Open the catalog read connection per blocking thread, or keep a small pool.
- **Conflicts:** plugin-jobs changes the same lock sites in `api.rs` and `broker.rs`; do this right after that lane merges, or hand it to that lane.

### 8. No shared types crate (M, High)

- **IDs, three definitions with different rules:**
  - `calternal_auth::AccountId` / `SpaceId`: public tuple field, `parse` accepts any UUID version or case (`crates/auth/src/lib.rs:44-76`, `crates/auth/src/spaces.rs:13-26`).
  - `calternal_store::AccountId` / `SpaceId`: private field, UUIDv7 only, serde as UUID (`crates/store/src/types.rs:6-60`). Its own doc says "Temporarily owned by this crate until shared types are available" (`types.rs:62-64`).
  - `calternal_plugin_host::AccountId` / `SpaceId`: lowercase UUIDv7 only, serde as string (`crates/plugin-host/src/identity.rs:6-52`).
  - The server converts between them through strings (`crates/server/src/store.rs:486-490`, `crates/server/src/api.rs:861-867,930-936`) and through `Identity { id: Arc<str> }` (`crates/server/src/auth.rs:36-39`).
- **Errors, same shape four times:** `core::ApiError`, `core::ValidationError`, `content::Error` and `server::HostError` are all code + message (+ details) (`crates/core/src/models.rs:88`, `crates/core/src/validation.rs:9`, `crates/content/src/types.rs:10`, `crates/server/src/error.rs:11`), converted field by field (`crates/server/src/store.rs:69-75`). `FaultPoint` exists twice with a mapping function (`store.rs:76-82`).
- **Fix:** one small crate (`calternal-types`, no I/O dependencies) with the strict v7 IDs and one caller-safe error; `auth`, `store`, `plugin-host`, `content`, `server` re-export it. `core` stays the wire contract.
- **Conflicts:** secrets takes its IDs from auth today; machine will need them. Landing this first saves two more conversions.

### 9. `personal`/`shared` alias shim (M, High)

- **What:** real spaces are UUIDs, but a name layer remains for old fixtures:
  - `Directory::content_space` and `wire_space` (`crates/server/src/auth.rs:63-69,293-308`), `SPACES` (`auth.rs:32`, no users), `DevPrincipal` with claimed grants that are documented as never conferring authority (`auth.rs:92-145`).
  - `fixture_bindings` table and `Store::binding`, which for a non-UUID name opens a new read-only SQLite connection to `auth.db` on every call (`crates/server/src/store.rs:150,178-220`). `space()` is called two or three times per operation.
  - `wire_spaces` walks every successful response tree to rewrite `space_id` (`crates/server/src/api.rs:567-592,907`).
  - `development_directory` table in the auth schema (`crates/auth/src/spaces_schema.sql:39`), CLI default `--space personal` (`crates/cli/src/main.rs:61`), core docs "development inputs temporarily accept seeded aliases" (`crates/core/src/spaces.rs:23`).
- **Why:** the owner's rule is that everything is a space with a UUID and no compatibility is needed. The shim adds a per-response tree walk and per-call database opens, and two extra trait methods every future directory must implement.
- **Fix:** tests and CLI read the space UUIDs from `describe`; delete the two trait methods, `wire_spaces`, `fixture_bindings`, `binding`, `SPACES`, `DevPrincipal`; `seed_development` returns the IDs it created.

### 10. Legacy auth schema (M, High)

- **What:** `crates/auth/src/db.rs:16-325` is `SCHEMA_V1` to `SCHEMA_V15` copied from the old app, then `spaces_schema.sql` as step 16 (`db.rs:489-506`). The chain adds and then drops a column (`host_cookie`, `db.rs:130-135`), rewrites the same triggers three times (`db.rs:141-185`), and creates tables nothing in this repository writes: `groups`, `group_members`, `invites`, `invite_groups`, `installation_notes_scopes`, `note_shares`, `share_grants`, `note_journal`, `share_fanout`, `recipient_control`, `recipient_control_acks`, `recipient_share_state`, `share_fanout_acks`, `recipient_share_fences`, `sharing_installation_resets`, `sharing_reset_cursors`, `share_fanout_deliveries`. The only non-test statement touching them is one insert guarded by `EXISTS (SELECT 1 FROM share_grants ...)`, which is always empty (`crates/auth/src/session.rs:454-460`).
- There are two role systems: server-wide `roles`/`role_capabilities` with `admin`/`member` (`db.rs:62-88`, read at `http_facing.rs:97`, `setup.rs:378`) and `space_roles`/`space_role_capabilities`.
- Kept for old behaviour: three ignored tests (`crates/server/tests/host_cookies.rs:9,114`, `crates/server/tests/login.rs:87`), source test text files and a v2 SQL fixture (`crates/auth/tests/fixtures/`), and about 540 lines of migration tests in `db.rs`.
- `crates/recovery/src/state.rs:11-43` hard-codes all of these table names in its snapshot inventory.
- **Fix:** one `schema.sql` at version 1 containing only tables with a writer; delete the migration tests, fixtures and ignored tests. `db.rs` drops to roughly 250 lines.
- **Owner decision needed:** AGENTS.md lists invites (ADR 0059) and groups (ADR 0060) as accepted decisions. I recommend dropping their tables now and adding them back with the code that uses them, but that is the owner's call. The sharing fan-out, DAV scope and `host_cookie` parts are dead either way.

### 11. Repeated work in one note update (M; counts High, flush count Medium)

- **Input parsed or cloned about eight times:** `parse_request` then `validate_operation_input`, whose `decode` clones the whole input map (`crates/core/src/validation.rs:81-83,165-177`); validated again for item-scoped operations (`crates/server/src/api.rs:460`); the whole input cloned only to read `space_id` (`api.rs:521`); `fingerprint` converts to `Value`, builds a sorted copy, serialises it and formats it again (`api.rs:416-418`, `validation.rs:475-490`); decoded again into the typed input (`api.rs:977`); then `bytes.to_vec()` (`crates/content/src/lib.rs:476`). For a 1 MiB note that is several MiB of copying before any I/O.
- **Body read from SQLite three times:** `Store::update` calls `get` (`crates/server/src/store.rs:451`), `Content::write` calls `get` (`content/src/lib.rs:468`), `place` calls `get` (`crates/content/src/files.rs:253`).
- **File read and hashed twice:** `check_file` (`files.rs:198`) and `place` (`files.rs:287`).
- **Small repeats:** the kind registration JSON is selected and parsed three times (`files.rs:120,145,254`); metadata is serialised three times (`lib.rs:457,479,480`); most statements use `query_row`, which prepares each time (only `list` and `history` use `prepare_cached`, `db.rs:95,143`).
- **Flushes:** by my reading at least six synchronous flushes: content transaction (`lib.rs:482`), catalog `requests` insert (`lib.rs:394`), a second journal transaction whose only purpose is to force the first one to disk (`journal.rs:177-187`, with a permanent `publication_receipts` row per write), outbox delete (`lib.rs:419-425`), file and directory (`files.rs:316`), placement update (`files.rs:330`). Not measured.
- **Unbounded growth:** the receipt stores the full response, which for notes is the whole note including Markdown (`crates/server/src/store.rs:429,468`, `journal.rs:171`), and retention is set to effectively forever (`journal.rs:42-45`). Every revision is therefore stored twice permanently: `versions.bytes` and `idempotency.result`.
- **Fix:** parse once into the typed input and fingerprint the raw `input` bytes; pass the already-loaded head row down; let `SpaceStore::commit` take a durability flag instead of the barrier transaction; store `{item, revision}` in the receipt and rebuild the response on replay; restore a finite retention.

### 12. Unwired code (S per item, High)

No non-test caller exists for:

- `crates/recovery` (about 1,250 non-test lines): no crate depends on it and the server never calls `configure_recovery`.
- `crates/roots/src/guard.rs` (481 lines): the server takes its own ad-hoc lock instead (`crates/server/src/store.rs:118-128`); only the `Roots` struct is used.
- `fsio` modules `beneath`, `bulk`, `directory`, `paced`, `swap`, `usage` (about 2,500 lines): other crates use only `write_atomic(_mode)`, `rename_safely`, `sync_dir`, `TEMP_PREFIX`, `DataLock` and `names`. The `test-hooks` feature (`crates/fsio/Cargo.toml:9`) is enabled by nothing. `usage` is documented as "legacy per-person accounting" (`crates/fsio/PORTING.md`).
- `store` public API with no consumer: `subscribe`, `token_at`, `position`, `head`, `epoch_head`, `restored`, save-activity guards.
- v1 worker protocol in `core`: `WorkerRequest`, `WorkerResponse`, `WorkerResource`, `WorkerOperation`, `WorkerError*`, `validate_worker_request`, `parse_worker_response`, `MAX_WORKER_OUTPUT_BYTES` (`crates/core/src/models.rs:690-782`); used only by core tests, the contract exporter and the SDK's v1 validators (`packages/plugin-sdk/src/index.ts:471-530`), while the shipped plugin uses `WorkerRuntimeV2`.
- Server exports: `resolve::resolve` (`resolve.rs:19`), `store::blocked_error`, `Store::mat_state`, `MatState`, `Store::open_roots` (only called by `open`), `authz::development_role` (`authz/capabilities.rs:175`), `BUILTIN_ROLES` (tests only).
- `crates/genui` and `packages/genui`: no consumer yet (expected; listed so it is a decision, not an accident).

**Fix:** for each, either wire it or move it out of the build until its consumer exists. Recovery and the roots guard are the ones worth an explicit decision, because the server currently runs without the protections they were ported for.

### 13. Worker cold start blocks the async runtime (S-M, High)

- **What:** `WorkerPool::invoke` is `async` but takes a `std::sync::Mutex` and, inside it, calls `spawn` (`crates/server/src/plugins/worker.rs:240-252`), which reads and SHA-256-hashes the bundle, compiles the manifest's JSON Schemas, starts a process and three threads (`worker.rs:317-320`, `plugins/mod.rs:440-447`). The runtime has two worker threads (`crates/server/src/main.rs:83`).
- Schemas are compiled per (plugin, space) worker (`WorkerSession::new(plugin.manifest.clone(), space)`), not once per plugin.
- **Fix:** do the spawn in `spawn_blocking` outside the map lock (insert a placeholder), and compile validators once in `Plugin::new`.
- **Conflicts:** squarely inside plugin-jobs; hand it to that lane.

### 14. Capability and role vocabulary in five places (M, High)

- `ManifestCapability` in core (`crates/core/src/plugin_manifest.rs`), `Capability` in the server (`crates/server/src/authz/capabilities.rs:19-87`), seeded SQL rows (`crates/auth/src/spaces_schema.sql:7-14`), string literals in `SpaceChange::capability` and the privilege checks (`crates/auth/src/spaces.rs:190-198,255,305`), and a TypeScript set (`packages/plugin-sdk/src/manifest-v1.ts:7`). They are bridged by serialising an enum to JSON to get its name (`crates/server/src/plugins/mod.rs:208-219`).
- Roles are data in auth (custom roles are possible), but the wire type is a closed four-variant enum (`crates/core/src/models.rs:627-637`), and `describe` silently drops any space whose role is not one of the four (`crates/server/src/auth.rs:287-290`, `filter_map(...ok())`).
- **Fix:** one `Capability` enum in the shared types crate with `name()`; generate the SQL seed and the TS list from it; make the wire role a string plus the capability list.

### 15. Store legacy (S-M, High)

- Four migrations for a schema with no deployed data, the last being `ALTER TABLE journal ADD COLUMN author TEXT NOT NULL` (`crates/store/src/schema.rs:12-41`); a branch for "unbound legacy journal" (`schema.rs:71-84`).
- `read_legacy` upgrades an old one-line witness file (`crates/store/src/highwater.rs:78-90,339-357`).
- `strict: bool` and `secrets: Option<&Path>` exist so `#[cfg(test)]` constructors can open unsigned stores (`crates/store/src/lib.rs:132,222-253`); production always passes `true` and `Some`. Every `Option<Guard>` branch is test-only.
- **Fix:** one `CREATE` schema, delete `read_legacy`, make the guard non-optional and port the affected tests to the signed path.

### 16. Two per-space caches (S-M, High)

- `Content.spaces` (cap 64, idle sweep, `crates/content/src/lib.rs:52-70,162-195`) and `SpaceStores` (map + `OnceCell` + idle sweep, `crates/store/src/pool.rs`) track the same lifecycle. Content sets the pool's idle time to zero and pins each store by holding a handle in `Space._journal` (`crates/content/src/journal.rs:47`, `crates/content/src/db.rs:13`).
- That handle is never used: every journal call asks the pool again through the bridge thread (`journal.rs:79,105,143,195`), and `pool.get` runs a full sweep each time (`pool.rs:47`).
- **Fix:** drop `SpaceStores` from this path; `Space` owns its `SpaceStore` and calls it directly. This is also the enabling step for finding 7.

### 17. Hand-rolled checked placement (M, Medium)

- `Space::place` hashes the destination, compares, then calls `write_atomic_mode`; the comment concedes "external writers racing the final hash/replace interval remain out of scope" (`crates/content/src/files.rs:287-317`).
- `fsio::swap::write_unless_changed` and `Prepared` exist for exactly this, using an atomic exchange (`crates/fsio/src/swap.rs:21-37`), and have no caller.
- **Fix:** call `write_unless_changed` with a hash predicate. Medium confidence because I did not verify the exchange path's platform requirements against the preview's filesystems.

### 18. Notes hard-coded in the host (L, High)

- Adding or changing one operation touches: `is_known_operation` (`crates/core/src/validation.rs:126-149`), `validate_operation_input` (`validation.rs:181-275`), `operation_catalog` (`validation.rs:546-630`), `POLICIES` (`crates/server/src/authz/mod.rs:64-174`), `dispatch_checked` (`crates/server/src/api.rs:602-730`), per-operation client methods (`crates/client/src/lib.rs:358-455`) and the CLI.
- Core knows a specific plugin: `"notes-insights.summarize"` is in the fixed catalog (`validation.rs:147,625`) and in `Stats`/`SummarizeResult`.
- The server `Store` is a Notes adapter over the generic content service (`crates/server/src/store.rs:261-472`), and generic code carries Notes names: `Scope::Note`, `Receipt.note_id`, "note file was changed" (`store.rs:667`).
- **Why:** `docs/plugin-api.md` says apps are plugins and Notes is the second one. Today a second kind needs host changes in six places.
- **Fix, in two steps:** (S) one declarative operation table in core from which known-check, validation, catalog and policy rows are derived. (L) replace `notes.*` with generic item operations by kind, with Notes declaring its kind in its manifest.

### 19. Four receipt stores (L, needs a decision; High)

- `space_receipts` in auth (`crates/auth/src/spaces_schema.sql:29-36`); `idempotency` and `publication_receipts` in each space's `index.db` (`crates/store/src/schema.rs:26-38`); a per-space `outbox` plus a global `requests` routing table in content (`crates/content/src/db.rs:58`, `crates/content/src/lib.rs:103`); `plugin_outbox` in the server (`crates/server/src/store.rs:148`).
- The server checks the namespaces against each other by hand in three places (`api.rs:537-543,656-665,997-1006`); the "request_id was already used" message is constructed in nine places across four crates.
- The global table exists because of one rule: "Same-account request IDs are unique across this service's spaces" (`content/src/lib.rs:431`).
- **Decision for the owner:** if a request ID only has to be unique per (account, space), the global `requests` table, the cross-checks and one flush per write disappear, and receipts live only in the space journal (plus auth for `spaces.*`). I did not find this settled in either repository.

### 20. v0 contract remnants (M, High)

- Two manifest types: `ManifestV1` and the v0 `PluginManifest`; `wire_manifest` is described as a "Compatibility projection ... Lossy by design" that returns empty descriptions, no views and omits mutating operations (`crates/server/src/plugins/mod.rs:334-374`). `plugins.list` serves the lossy one.
- Two GenUI formats: the v0 JSON tree (`GenUiNode`, `validate_genui` in core; `validateGenUiView` in the SDK; the web renderer) and the new language (`crates/genui`, `packages/genui`), with no bridge.
- Interim `item_operations` binding in the trusted file (`plugins/mod.rs:68-91`); `lane/plugin-jobs` already removes it.
- **Fix:** serve `ManifestV1` from `plugins.list` and delete the v0 manifest type and SDK validators once the web client reads v1 (UI change, Claude lane).

### 21. Large mixed files (M, High)

| File | Lines | Mixes |
|---|---|---|
| `crates/server/src/api.rs` | 1,037 | router, Host/Origin/session middleware, envelope, dispatch and policy, Notes handlers, plugin enable/disable, worker invocation, preview |
| `crates/auth/src/db.rs` | 1,069 | 325 lines of legacy SQL, the worker, about 540 lines of migration tests |
| `crates/core/src/models.rs` | 851 | envelope, Notes DTOs, v0 plugin manifest, v0 GenUI, principal, v1 worker protocol, schema root |
| `crates/genui/src/evaluate.rs` | 922 | one evaluator (acceptable) |
| `packages/contracts/src/index.ts` | 1,208 | generated (acceptable) |
| `tests/conformance/isolation.rs` | 2,305 | every conformance scenario in one test file |
| `crates/recovery/src/tests.rs`, `crates/server/tests/http.rs`, `crates/auth/tests/account.rs`, `crates/store/src/tests/restore.rs` | 880-1,320 | tests |

**Fix:** split `api.rs` into `http/guard.rs`, `dispatch.rs`, `ops/notes.rs`, `ops/plugins.rs`. `db.rs` and `models.rs` shrink on their own through findings 10 and 12.

### 22. Naming and formatting (S-M; names High, formatting Medium)

- `AppState.tokens` holds the directory ("legacy field name preserves startup callers", `api.rs:50-51`); the production directory type is called `TokenTable` (`auth.rs:149`).
- The same thing is `principal`, `actor`, `account`, `user` (table `users`, `AccountId`, `principal: &str`, `actor: AccountId`).
- Per-file `#![deny(missing_docs)]` repeated in every server module although the crate root sets it.
- Several files contain long unformatted lines that rustfmt would not produce (`crates/content/src/lib.rs:175-187,463-487`, `crates/content/src/db.rs:83`, `crates/auth/src/spaces.rs:397-461`), so `cargo fmt --check` in `scripts/verify-preview.sh:6` probably fails on `dev`. Not run.

### 23. Dependency drift (S, High)

`getrandom` 0.3 in server and 0.4.3 in auth; `sha2`+`hex` in server and content, `blake3` in auth, store, recovery; `rustix` "1" and "1.1.5"; `rusqlite` declared separately in five crates (six with secrets) and not in `[workspace.dependencies]`; `uuid` redeclared in store.

### 24. Minor

- Session cache entries are removed only when that token is presented again after expiry, or on revoke (`crates/auth/src/session.rs:86,173-180`); the `revoked` set only grows. Bounded by issued sessions.
- `SpaceStores::get` sweeps the whole map on every call (`crates/store/src/pool.rs:47`).
- `describe` builds each space grant by a JSON round trip (`crates/server/src/auth.rs:289`).

## 3. Review checklist for every lane before merge

1. **No new copy of a shared type.** IDs, the caller-safe error, capabilities and time/hash helpers are imported, never redefined. A comment saying "until shared types exist" is a blocker.
2. **Dependencies point downward.** A crate is not added as a dependency for one helper; move the helper to the lower crate instead (`fsio` for files, the types crate for IDs).
3. **Nothing global on the request path.** No process-wide or installation-wide mutex, single thread or single connection is taken for per-space work; any lock held across an fsync or a cross-thread call is named and justified in the doc comment.
4. **No blocking call on an async thread.** File reads, hashing, process spawn, `std::sync::Mutex` that can be held across I/O, and SQLite all run on a blocking thread.
5. **Parse once, load once.** Request input is decoded a single time; a row or file already read is passed down, not fetched again. Count the reads and flushes of the hot path and state them in the PR.
6. **Constant work is cached.** Anything that depends only on the build or startup state (schemas, catalogs, compiled validators) is built once.
7. **Everything stored has a bound.** Every cache, queue, receipt and history table states its cap or retention; "forever" needs the owner's sign-off.
8. **No compatibility code.** No migration chains, legacy readers, aliases, "interim" projections or ignored tests kept for old behaviour; a schema is one current definition.
9. **Spaces only, by UUID.** No names standing in for IDs, no "Home", no Notes-specific names in generic code.
10. **One place per fact.** An operation, capability or role is declared once and everything else is derived; if the change touches more than two files to add one operation, stop and fix that first.
11. **No unwired code.** Every public item has a non-test caller in this merge, or the PR says which lane will call it and when. Unused feature flags and exports are removed.
12. **Reuse before writing.** The PR names the existing primitive it considered (`fsio`, `store`, `plugin-host`) for anything touching files, durability, locking or validation.
13. **Docs gate really covers it.** The crate or package is in `scripts/check-docs.sh`; public comments state purpose, failure and cost (I/O, locks, allocation), and do not restate the item name.
14. **Docs match the code in the same merge.** README, crate README and `docs/` statements about routes, storage and startup flags are updated; dated lane reports go under `docs/reviews/`, not beside the code.
15. **Small files, formatted.** `cargo fmt --check` is clean; a source file past about 600 lines is split by concern before it merges.
