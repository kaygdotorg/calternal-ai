# Filesystem primitive port

Source: `/home/kayg/Developer/calternal`, git object
`ea57b84b9c9e40e8b67b7d1e1f1805915123b040` (`origin/dev` at review).
Read using git only. No engine/server/plugin/core dependency was imported.

| Source path and lines | Adaptation and reason |
| --- | --- |
| `crates/fsio/Cargo.toml:1–25` | Standalone `publish = false` replaces unavailable workspace publish/lint inheritance. Dependency requirements and target gates are preserved. |
| `crates/fsio/src/lib.rs:10–12,27` | Replace a claim about the old workspace's Clippy ban with this host's caller obligations; deny missing docs and broken intra-doc links locally. |
| `crates/fsio/src/lib.rs:310–374` | Preserve `DataLock` lifetime/drop behavior and existing APIs. Extract the common lock operation and add `acquire_at`: descriptor-relative `.lock`, `NOFOLLOW`, `NONBLOCK`, regular-file check, mode 0600. The new roots guard must not resolve validated directories again through mutable paths. Existing `acquire_existing` behavior remains source-compatible; it is not itself a symlink guard. |
| `crates/fsio/src/names/directory.rs:12–17` | Add the missing public `DirectoryNames` doc comment; behavior unchanged. |
| `crates/fsio/src/usage.rs:1–2,19–179` | Retain legacy per-person accounting and tests unchanged; document that it is not the new application's owning-space/joined-tier quota policy. No accounting integration is enabled by this port. |
| `crates/fsio/src/tests.rs:1–end` | All source tests retained verbatim; append two tests for descriptor-relative locks and root/lock symlink substitution. |
| `crates/fsio/src/{beneath,bulk,directory,names,paced,swap}.rs`, `src/{usage/files,names/directory}.rs` | Remaining generic primitives and tests are copied unchanged, including platform failure behavior, exchange recovery, outside inode retention, ACL inheritance and kernel queue fallback. |

## Test inventory

**54 source tests ported; 0 ignored; 2 new tests.** All of `src/tests.rs`
(28), inline `beneath` (3), `swap` (8), `names/directory` (3), `usage` (8)
and `usage/files` (4). This includes every section 7 step 1 fixture cited at
source tests.rs lines 22,42,59,73,107,150,223,258,390,582,623,708,731.
No source test or assertion was removed or weakened. Source platform/ACL/queue
capability branches are preserved as written; a green Linux run does not certify
Apple or other filesystem behavior.

New tests: `descriptor_lock_rejects_symlinks_and_keeps_outside_bytes` and
`descriptor_lock_cannot_be_redirected_by_root_path_substitution`.

## Dependencies

Source requirements: arc-swap `1.9.2` (published counter snapshots), tracing
`0.1.44` (diagnostics), unicode-normalization `0.1.25` (NFC names), and rustix
`1.1.5` with `fs` on Linux/Android/Apple (descriptor I/O, exchange, durability).
Disposable workspace resolution: **1.9.5, 0.1.44, 0.1.26, 1.1.6**, respectively.
No SQL dependency is introduced. See `../roots/PORTING.md` for verification and
shared-checkout lockfile limits.

## Space terminology (2026-11-17)

- Source `crates/fsio/src/{lib.rs:23,beneath.rs:1–34,306–397}`: folder boundary
  comments and fixtures now call the protected content root a space.
- Source `crates/fsio/src/usage.rs:19–125,355–end` and `usage/files.rs:45–56,102–end`:
  usage roots and provider folders now use `spaces/<space-id>` in data and
  state. Counter fields, variable names, comments, nesting errors and fixtures
  use space terminology. The counter algorithm and all assertions are retained;
  this remains a single-tree primitive, not joined-tier accounting or quota
  authorization. No account ID is inferred as a journal or content owner.
- No dependency changes. All 56 filesystem tests pass; no ignored tests.
