# Three-root startup guard port

Source: `/home/kayg/Developer/calternal` git object
`ea57b84b9c9e40e8b67b7d1e1f1805915123b040` (`origin/dev` at review).
Read using git only. This is a library; no CLI, server integration or real-data
migration is enabled. The root workspace receives only this lane's two dependency
entries; wildcard membership already includes both crates.

| Source path and lines | Adaptation and reason |
| --- | --- |
| `crates/calternal/src/storage.rs:13–27,50–81` | `prepare_with_wait` retains retry-only-on-WouldBlock, 100ms interval, one-shot callback and bounded deadline. The library caller supplies shutdown grace instead of importing the old binary's grace module; no tracing policy or Notes shutdown coupling. |
| `crates/calternal/src/storage.rs:29–48` | Omit Notes shutdown helpers: library guard docs require lifetime retention through completion of all writers. No Notes/server dependency. |
| `crates/calternal/src/storage.rs:83–131,133–176` | Extend D/S to three separate roles, per ADR 0062 amendment :159–166 and :433–442. Explicit setup is allowed only when all markers are absent and every existing root contains only empty directories/regular lock sentinels. Nonempty unpaired roots are refused before creating missing peers. No legacy single-root adoption: three-role migration is a separate step. |
| `crates/calternal/src/storage.rs:167–176` (`require_root`) | Replace path `is_dir` checks with component-by-component `openat(DIRECTORY, NOFOLLOW, CLOEXEC)` from `/`, retaining descriptors. Refuse parent components, lexical overlaps, symlink ancestors, duplicate device/inode roots. Read every marker/auth/legacy requirement and validate lock entry types before any CREATE operation. Setup creates missing components descriptor-relatively and fsyncs each parent. |
| `crates/calternal/src/storage.rs:178–219` | Preserve nonempty regular `system/auth.db` requirement, strengthened with no-follow opens including `system`. UUID comes explicitly from caller configuration outside lost state rather than old core's ID type. Marker encoding is exactly `UUID\nrole\n`, roles `ssd-home`, `hdd-home`, `app-state`; malformed or legacy untyped markers are not silently adopted. New markers use private adjacent exclusive temp images, fsync, exclusive rename, directory fsync relative to pinned descriptors. |
| `crates/calternal/src/storage.rs:222–235` | Port explicit offline `re_pair`; confirmation is a required library boolean instead of CLI flag parsing. It never creates directories/databases, requires auth/history presence and all locks, replaces markers durably, refuses known swapped roles. This is presence checking, not verified inventory/provenance or a migration tool. Interrupted partial marker publication remains refused, as in source. |
| `crates/calternal/src/storage.rs:237–261` | Retain legacy auth/history presence fence for both Home trees. Descriptor-relative no-follow traversal replaces path existence checks; missing state copy refuses before lock creation and checks repeat under locks. No space registry or changed Home identity mapping is invented. |
| `crates/calternal/src/storage.rs:263–end` | Omit rotational-disk warning task, a binary diagnostic unrelated to opening roots. |
| `crates/calternal/tests/storage_startup.rs:201,221,248,293,321,346,522,536,579` | Port all nine named startup scenarios; map below. Replace binary launch/CLI parsing with library calls, old auth setup with a disposable nonempty auth sentinel, two roots with three roles, and implicit ID creation with a caller-pinned UUID. The fixture proves the source presence fence, not SQLite/auth integrity. Source /sys read-only-path assertion uses a chmod-0555 disposable state directory instead; run as the ordinary unprivileged development user. Product shutdown/grace assertions are replaced by caller-supplied bound and paused-clock checks. |

## Validation boundary

Every existing root path component, marker, lock, system directory and auth file
is opened without following symlinks; every mutating operation uses a retained
directory descriptor. Refuse absent/mismatched markers, role swaps, wrong pinned
UUID, missing roots and missing/empty auth **before creating lock sentinels**.
Initial explicit setup creates only directories, locks and markers, no auth DB.
All three locks must succeed or partial guards are released. A contender that
already holds a later lock can leave earlier regular lock sentinels, as in source;
this is contention after validation, not invalid-root adoption.

This does **not** check physical device separation, mount IDs/table continuity,
rotational hardware, cryptographic marker provenance, SQLite integrity, recovery
snapshot completeness, migration inventory equivalence, or bind-mount ancestry
under different path names. SSD Home and state may share a filesystem. A missing
mount is detected by absent roots/markers/identity, not a device probe. Copied
valid markers are not a mount identity proof. The caller must pin the UUID outside
app state. Cooperative locks do not prevent a hostile process from mutating real
directories/marker bytes; symlinks cannot redirect this guard's descriptor I/O.
Separate path-based consumers do not inherit descriptor containment automatically.

FO1 empty-state reconstruction stays disabled. Existing nonempty auth is checked
for presence/length only, preserved without opening/migrating SQLite. No claim of
full FO1, multi-tenant sandboxing or three-root content tiering is made.

## Source fixture mapping

**9 source scenarios ported; 0 ignored; 6 additional root tests.**

| Source test line | Ported test |
| --- | --- |
| 201 | `missing_home_refuses_without_creating_directory_or_lock` |
| 221 | `missing_state_never_recreates_databases_even_with_create` |
| 248 | `roots_must_keep_their_persistent_partner` (includes confirmation, busy repair, missing/corrupt marker repair) |
| 293 | `all_missing_markers_with_existing_data_refuse_setup` (includes explicit offline repair) |
| 321 | `create_refuses_unpaired_notes_or_databases_before_creating_other_root` |
| 346 | `legacy_history_and_invalid_state_are_refused` |
| 522 | `all_roots_remain_exclusively_locked` |
| 536 | `startup_waits_for_a_previous_server_to_release_all_roots` |
| 579 | `a_lock_held_past_the_grace_still_fails_startup` |

Additional tests cover swapped roles/pinned identity, each missing root/marker,
root/ancestor/marker/system/auth/lock symlinks, distinct nonnested roles/no implicit
database, offline repair refusal, and validation of every lock before creation.
Full migration/rollback test :386 and 100x load test :566 belong to later port-map
steps, not the nine requested scenarios; neither is represented as ported/ignored.

## Dependencies

- `calternal-fsio` workspace: inherited lock semantics and temp prefix.
- `uuid` workspace (`1`, resolved **1.29.2**): caller-pinned UUID and exclusive temp names, replaces old core ID dependency.
- `tokio` workspace (`1`, resolved **1.54.1**): bounded asynchronous lock retry; test-only `sync` and `test-util` for deterministic callbacks/paused time.
- `rustix` `1.1.5`, `fs` (resolved **1.1.6**): pinned no-follow roots, descriptor-relative validation/setup/marker durability.
- Dev-only `tempfile` `3.27` (resolved **3.27.0**): disposable, automatically cleaned test roots.

No old engine/server/plugin/core or SQL dependency.

## Verification

See the verification results appended after final checks below. Shared-checkout
`Cargo.lock` belongs outside this lane and was not edited by this lane. Another
lane subsequently refreshed it, including these crates; all final commands below
also passed directly in the shared checkout. Dependency-resolved
verification uses disposable copy `/tmp/calternal-foundation-port-5xyfxkms`, built
from tracked and unignored workspace files, excluding real data, ignored secrets,
node_modules, build outputs and git internals. Only owned crates are refreshed
there after edits. This qualifies that snapshot, not concurrent later lane edits.

## Unfinished / Questions for the user

No new product questions. Foundation-port-map questions remain outside this lane.
Coordinator integration should include these two crates in the centrally owned
docs gate's explicit package list. The shared lockfile has already been updated
by another lane. The current gate
checks only core/client/CLI/bindings, so a separate strict rustdoc command covers
both new crates. Server wiring, migration, device/mount identity policy and FO1
proof/reconstruction are not implemented by this bounded port.


## Final verification results

Executed from `/home/kayg/Developer/Calternal-AI`, using the existing toolchain:

```sh
./scripts/with-toolchain.sh cargo test --locked -j4 -p calternal-fsio -p calternal-roots --all-features
./scripts/with-toolchain.sh cargo test --locked -j4 --workspace --all-features
scripts/check-docs.sh
./scripts/with-toolchain.sh cargo doc --locked -j4 --no-deps -p calternal-fsio -p calternal-roots --all-features
git diff --check -- Cargo.toml crates/fsio crates/roots
```

All **exit 0**. Crate tests: **56 fsio + 15 roots passed, 0 failed, 0 ignored**;
no doctest failures. Workspace: **141 passed, 0 failed, 2 ignored**. Those two
ignored tests belong to the concurrent auth lane: legacy real-passkey migration/
login/recovery fixtures `origin_dev_schema_migrates_idempotently_and_rolls_back_partial_failure`
and `schema_two_copy_keeps_real_credentials_sessions_and_recovery`; this lane
introduced no ignored tests. Existing server missing-doc warnings were not changed.

The docs gate passes Rust checks/reference generation and both TypeScript package
documentation validations. Separate rustdoc covers the two new crates under their
`deny(missing_docs, rustdoc::broken_intra_doc_links)` policies.

Logs: `/tmp/calternal-port-shared-{crate-tests,workspace-tests,docs-gate,rustdoc}.log`.
Initial shared-checkout attempts exited 101 because the then-unmodified lockfile
needed new package entries; after the other lane's lock refresh, the final direct
checks above passed. Disposable-copy verification also passed (141 workspace tests,
2 auth ignores, docs gate and strict rustdoc). Its initial TypeScript gate needed
workspace package links, SDK declarations and Node types; running `npm ci
--ignore-scripts --no-audit --no-fund` in that disposable copy provided the normal
workspace dependencies. No shared product TypeScript files were edited.
