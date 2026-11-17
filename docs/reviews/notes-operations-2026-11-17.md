# Notes operations lane

Worktree `lane/notes-ops`, Forgejo issue [#38](https://git.kayg.org/kayg/calternal-ai/issues/38).
The public contracts are in [Notes operations](../notes-operations.md) and
the [CLI reference](../cli.md). Terms follow the [glossary](../glossary.md).

The port reused the reference Notes tag scanners, first-H1 naming, relative
inline Markdown link rules in ADR 0024, and the 30-day Trash setting in
`crates/notes/src/trash.rs` at the reference repository's `origin/dev`.
The Calternal-AI engine builds on `calternal-content` and `calternal-fsio`
for per-space locks, conditional placement, exclusive moves, guarded unlink,
history and request receipt publication.

## Costs on this branch

These are warm-path code counts for an existing note, with no prior pending
work, no concurrent outside change, and an already-open space. Counts include
the current branch's synchronous placement and exclude SQLite checkpoints and
the journal's background witness work.

| Path | Body reads and parsing | Explicit userdata flushes | SQLite durability |
| --- | --- | --- | --- |
| Delete | One stored head body load. Three checked file reads for the source guard, the move source, and the destination. No Markdown tag parse. | Two directory syncs for the exclusive move. The already durable file is reused. | Four FULL transactions for content revision and indexes, journal/receipt, outbox retirement, and placement bookkeeping. |
| Restore | Same body reads and tag behavior as delete. One indexed read of the prior filename and one indexed probe per attempted filename. | Two directory syncs for the move. | Same four FULL transactions. |
| Content-only save with changed tags | Zero prior body loads. Two checked reads of the old userdata file. One tag parse of the submitted Markdown. | One temporary file sync and one directory sync for atomic replacement. | Same four FULL transactions. The term projection is updated inside the content transaction. |
| Title or tag picker | One prepared range query over the selected space's title or term index. Zero note file or version body reads. | Zero. | Read only. |

First creation of `.trash` adds one parent-directory sync. Placement
recovery loads the committed body and rechecks the affected directories.
Linked rename has one indexed candidate query and bounded additional reads
and placements per changed source, with a single content transaction for
the entire edit. Manual purge makes a durable removal intent before guarded
unlink and deletes the versions before its journal receipt is published.
The performance lane was read before changes to the shared content write
path at `8a7acfa`. A trial integration found shared-path conflicts in content
publication/mutation wiring and server startup. When both lanes enter
`dev`, the placement dependencies must travel with the outbox publisher
and Notes must preserve synchronous Trash moves while ordinary saves use
the performance lane's deferred placement.

## Review gate

| Item | Evidence |
| --- | --- |
| 1 | Shared space and account UUIDs, caller-safe errors, capabilities, content revisions, and UTC formatting are reused. |
| 2 | Notes calls the generic content and fsio APIs. No new workspace crate. The Markdown link scanner uses the reference app's pulldown-cmark dependency. |
| 3 | Each write holds only its selected space's content lock across placement and its own journal commits to fence an admitted revision. New indexes and pending work are per space. The inherited directory worker and identifier catalog still serve admission. |
| 4 | HTTP handlers use the blocking pool for storage and Markdown writes. Retention scans and purges also use the blocking pool. |
| 5 | The costs above show one tag parse per final saved document, one index query per picker, and the file checks needed to preserve outside edits. |
| 6 | Native operation schemas are cached at discovery, and SQLite prepares and caches picker queries per connection. |
| 7 | Current-head projections are replaced at each write. Trash expires in 30 days by default, receipts use the inherited operator-controlled expiry, and identity tombstones retain only the identifier and placement bookkeeping under the stable-address rule. |
| 8 | New operations and fields have one spelling. Index creation and rebuild use current head projections. No storage migration chain or operation aliases were added. |
| 9 | Space UUIDs select the physical database. Note UUIDs resolve to their owning space before content reads, and picker inputs select a space UUID directly. |
| 10 | The shared Notes declaration supplies capability, item or space target, effect risk, validation, and schemas for the new operations and the extended list. |
| 11 | HTTP handlers, address resolution, journal delivery and server retention call the new storage APIs. The new CLI commands use the same operations. |
| 12 | Shared revisions and receipts, fsio name allocation, guarded removal, and exclusive rename already supply the durability primitives. The Notes engine ports the tag and link rules from the reference app. |
| 13 | The complete documentation gate covers the workspace and standalone conformance suite. |
| 14 | Repository, plugin and content readmes, generated operation schemas, CLI goldens, and docs-site sources describe the same behavior. |
| 15 | Both Cargo formatting checks run. New implementation modules are split by storage, admission, Markdown projections and CLI. |

## Verification

All builds and tests ran via `nix develop -c` on
`ubuntu@calternal-dev-02-std` under `~/Developer/lanes/notes-ops`. The merge
base was `origin/dev` at `a7a5275`.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Passed before Cargo tests. |
| `cargo test --locked --workspace -j6` | The first run stopped at `pool_machines_serve_only_granted_accounts`, which starts Podman unconditionally on this VM. |
| `cargo test --locked --workspace -j6 --no-fail-fast -- --skip pool_machines_serve_only_granted_accounts` | 550 passed, one failed, one ignored, one filtered. The failure was the provider-resume assertion at `crates/server/tests/provider_turns.rs:291`. |
| `cargo test --locked -j6 -p calternal-server --test provider_turns fake_official_cli_streams_checkpoints_crashes_stops_and_requires_explicit_resume -- --exact` | The same assertion failed on a clean `origin/dev` build. It reads the old machine checkpoint after observing a running resumed job. |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed. |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CARGO_TARGET_DIR="$PWD/target" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | Passed 59 tests including same-space and foreign Notes access, the journal event, delete/restore and linked-rename recovery, and startup expiry. |
| `npm run test:plugins` and `npm run test:integration` | Passed. |
| Disposable server with `calternal notes list/lookup/rename/delete/restore`, `calternal tags list/lookup`, `calternal trash list/purge`, and `calternal invoke resolve` in `--json` mode | Nine CLI commands and deleted-address resolution passed. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed. |
| `cargo build --locked -j6 -p calternal-cli` and `python3 contract/verify-cli.py` | Passed. |
| `./scripts/check-docs.sh` | Passed. |
| `cargo fmt --all -- --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml --all -- --check` | Passed. |
| `node scripts/docs-site/generate.mjs` and `node scripts/docs-site/generate.mjs --check` | Passed for 241 regenerated pages. Clean `origin/dev` already reports 102 stale generated files. The lane includes only the differences against regenerated `origin/dev`. |

The inherited failing test is
`fake_official_cli_streams_checkpoints_crashes_stops_and_requires_explicit_resume`
at `crates/server/tests/provider_turns.rs:291:5`. The exact assertion message
was **assertion `left != right` failed**. Both sides held the same machine
UUID, `8b64745e-4d47-46b3-be78-f32de9c25f34` on the lane and
`850f95fa-6317-453a-8684-6cb214582b40` on clean `origin/dev`.

All lane target directories were removed from dev-02 after verification.

## Decisions that remain open

- **List continuation.** Trash and tags return a bounded first page, as the
  existing Notes list does. An opaque cursor would expose later rows and
  requires a stable cursor contract for concurrent changes. A separate bulk
  empty-Trash action would add irreversible multi-item admission and
  recovery. The current purge targets one UUID.
- **Filename reuse while a note is in Trash.** The deleted note's UUID stays
  resolvable, and restore prefers its old filename. Reserving the old
  filename while trashed would preserve relative link meaning at the cost of
  unavailable filenames. Tracking inbound identity independently of paths
  would allow reuse but requires a durable binding model. Existing relative
  links are unchanged by Trash moves.
- **Reprojecting older preview data.** New saves project tags and links.
  An explicit offline reindex could populate projections from pre-lane
  Markdown at the cost of one body read per note. Rebuilding the derived SQL
  index from current head projections is already supported.
- **Large rename fan-out.** The current transaction refuses work exceeding
  1,000 sources or 32 MiB of rewritten content. A resumable job would cover
  larger renames at the cost of recovery state and visible progress.

## Remaining integration boundaries

Application-data loss recovery and pre-lane projection backfill remain outside
the conformance run. A linked rename is atomic in the content database and
recoverable across the tested file boundaries. If an outside writer changes
an affected file after the final preflight, the guarded placement can leave
the rename pending for storage reconciliation. Live clients can still use the
stable UUIDs to identify affected notes.

## Simplification opportunities

- Consolidate current-head metadata, file hash, and placement state reads into
  a single shared header read when the performance lane lands.
- Use a common declared item kind for operation policy and journal filtering
  when more native plugins need item events whose names have another prefix.
- A common Markdown frontmatter split could remove independent heading, tag,
  and file encoding boundary checks.
