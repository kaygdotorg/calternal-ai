# List cursors and approval lifetimes

Forgejo [#40](https://git.kayg.org/kayg/calternal-ai/issues/40). See the
[glossary](../glossary.md), [Notes operations](../notes-operations.md) and
[person requests](../requests.md).

Notes, Tags and Trash return up to 100 rows and omit the continuation on the
last page. The four existing directory operations and note history share the
same cursor authentication. A cursor binds to the signed-in account, operation
and filter values, including the space or item when present. Another authorized
account or space gets the same invalid-input reply as for a malformed cursor.
The signing key lives for one server process. A restart requires a fresh page.

Lists preserve their index order with UUID tie-breakers for timestamps and
folded titles. Each read observes the current committed index. Insertions
before the current position and edits that move a row across it can affect
which rows the client has seen. Tagged Notes retain update order, including
hierarchical tags. The statement walks the live-note index from the last key
and checks each candidate against the exact term key. Sparse tags can require
more candidate probes. Tag counts group adjacent names in their term index.

Approvals now carry `detail.lifetimes` in the private list and account feed.
The existing web transport adapter consumes that field. The protected decision
check uses the same risk method that produces the field. Read, reversible and
external actions can offer both choices. Irreversible and access-widening
actions offer `once`. App-level read and reversible-write grants offer both
choices. Once receipts and grant retention continue to follow the request
contract.

Implementation commits are `4d68e54` for request lifetimes and `464c9c4`
for pagination, CLI output, schemas and conformance.

## List inventory

The catalogue audit includes operations with list-shaped results and the
sequence-based agent event query.

| Operation | Result bound | Continuation |
| --- | --- | --- |
| `notes.list` | 100 live summaries, with or without a tag | Yes |
| `notes.lookup` | 100 folded-title matches | Yes |
| `trash.list` | 100 trashed summaries | Yes |
| `tags.list`, `tags.lookup` | 100 distinct tag names and live counts | Yes |
| `notes.history` | 20 by default, caller selects 1–100 | Yes, with the existing item/revision seek |
| `spaces.members` | 50 direct grants total across account and group rows | Yes |
| `people.list`, `invites.list` | 50 accounts or visible invitations | Yes |
| `groups.list` | 50 groups, each with up to 50 members. A single group can continue its member list | Yes, outer and per group |
| `spaces.list` | All memberships for the caller, with no result-row ceiling | Complete result |
| `roles.list` | Four fixed space roles | Complete result |
| `plugins.list` | One row per registered plugin in the loaded registry | Complete result |
| `secrets.list` | All metadata and all unexpired prompts for the selected owner. Prompts expire after 15 minutes. No result-row ceiling | Complete result |
| `drafts.list`, `pending.list` | All visible records under operator ceilings. Defaults are 256 records of each kind and 16 MiB combined per account | Complete result |
| `requests.list` | Up to 256 protected person-request and grant rows per account, reduced to pending and authorized requests, plus all eligible unexpired secret prompts | Complete result |
| `agent.readiness` | All eligible personal and pool machine options, or the saved pin where applicable | Complete result |
| `agent.events` | Up to 256 retained chunks and 1 MiB of buffered payload | Existing epoch and sequence continuation |
| `threads.list` | Default 100, caller cap 1,000, with the existing 1,000-row source window before runtime filtering | No. Declared in the orchestrator manifest outside the static catalogue and owned by the parallel threads lane |

`resolve` returns a single object and the finite capability vocabulary for it.
GenUI forms and views can contain arrays as part of a single result.

## Read and flush counts

- A warm Notes, Tags or Trash page uses one indexed statement in the selected
  content database, one indexed authority lookup at admission and one at
  delivery. The cache entry also resets SQLite's synchronous mode without
  flushing. The content statement looks ahead by one row or tag group. The
  warm path has zero note-file reads, application-level flushes or cursor
  writes. Cold space access
  includes the usual database and journal opening and may perform the merged
  first-use recovery work. If the Trash index is absent, SQLite also builds it.
- Note history retains one head lookup and one version range read, plus one
  revision-to-sequence lookup on continuation. The existing resolver and
  authorization checks still apply.
- Directory row reads retain their existing cost. Memberships seek up to two
  sets of grants after the directory authorization query. People seek once.
  Invitations seek once, with authorization and owner checks and at most 50
  link decryptions. Groups seek once plus up to 50 bounded per-group member
  reads, or one selected-group lookup and one member seek. Cursor verification
  and sealing add no storage reads or flushes.
- Lifetimes add no SQLite statements or flushes. The inbox uses the existing
  protected request query and checks delivery access. The decision reads the
  risk alongside the other decision fields in its existing transaction.

## Verification on dev-02

The lane fast-forwarded to `origin/dev` at `c765490`, which includes the
performance lane and the shared write header. All commands ran in
`~/Developer/lanes/list-cursors` on `ubuntu@calternal-dev-02-std`
through `nix develop -c`. Format checks also included the independent
conformance sources touched by the lane.

```sh
npm run build:plugins
cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
cargo fmt --all -- --check
xargs -0 rustfmt --edition 2024 --config skip_children=true --check < .list-cursor-rust-files
CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" \
  ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1
./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features -j6 --no-fail-fast
npm run test:integration
npm run test:plugins
CARGO_TARGET_DIR="$PWD/target" sh scripts/check-docs.sh
node scripts/docs-site/generate.mjs
npm run docs:site -- --check
node scripts/docs-site/generate.mjs --check
```

| Gate | Result |
| --- | --- |
| Plugin build and test-enabled server/CLI build | Pass |
| Workspace and touched-source formatting | Pass |
| Workspace tests | 571 passed, 1 ignored, 1 failed. `pool_machines_serve_only_granted_accounts` requires Podman. Dev-02 reports `podman is not available`. The query-plan regression passed. |
| Serial conformance | 77 passed after the fixture setup fix described below. Container-backed cases require the dev-01 gate. |
| Integration and plugin tests | Pass |
| Contract and Rust/TypeScript docs gates | Pass |
| CLI page flags | All five `calternal` commands and `cl notes list` advertise `--cursor`. Human and JSON golden tests passed in the workspace run. |
| Generated docs site | `npm run docs:site -- --check` passed after regeneration. Against the branch tree, the generator detects three inherited stale pages outside this lane. |

The first merged conformance run passed the new cursor and lifetime cases.
An existing Notes crash fixture armed its move fault before the initial
acknowledged save had placed its file. Lazy recovery could hit the fault
during that initial placement and terminate before the intended move. The
fixture now waits for the initial file before arming the fault. The move and
recovery assertions are unchanged.

The three inherited docs-site outputs are `admin/machines.md`,
`admin/recovery.md` and `dev/crates/calternal-recovery.md`. The
lane's generated pages were included in the check and match their sources.
Those three pages are outside the scope of this branch.

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | One `Page` lookahead type in `calternal-types`, adopted by directory, content and history. Existing UUID, error, risk and lifetime types are reused. |
| 2. Dependencies | The lookahead helper moved down to an existing dependency. The server reuses its existing Ring and base64 dependencies. |
| 3. Request path | Key material is immutable after store creation. The existing cache-map lock covers lookup and bookkeeping only. Content page SQL runs under the selected space lock. The preexisting directory authorization path retains its protected worker. |
| 4. Async | New SQLite page reads run through the existing blocking dispatch adapter. Cursor processing performs bounded CPU work and no I/O. |
| 5. Reads and flushes | Counts are recorded above. Each content row's metadata is decoded once. |
| 6. Constant work | The operation catalogue and title statements are cached, SQLite statements are prepared through the existing cache, and the signing key is created once at store open. |
| 7. Bounds | Five content pages expose 100 rows each. Cursor input is bounded to 512 KiB, which covers an encoded tag key from the 256 KiB metadata budget. Cursor state is ephemeral. Existing request and receipt bounds are unchanged. |
| 8. Compatibility | The server issues and accepts a single cursor format. Existing derived Trash heads enter the same partial index when the space opens. |
| 9. Space identity | The scope uses the operation's authorized space UUID or resolved item identity. The account binding uses the authenticated UUID. |
| 10. One source per fact | The cached operation schema selects which calls use cursor processing. One risk method provides the choices for discovery and resolution. |
| 11. Callers | Normal HTTP and CLI entry points exercise the new paths. The account feed uses the same inbox projection. |
| 12. Reuse | Reused the people lookahead algorithm, current content and label indexes, the bundled SQLite statement cache, and Ring. The reference Notes tag grammar and writable transaction paths are retained. |
| 13. Documentation | Public API comments describe scope, bounds and I/O. The docs gate result is above. |
| 14. Consistency | Rust wire types, JSON Schema, TypeScript declarations, CLI goldens, reference pages and operator docs are included. |
| 15. Formatting and size | New server code lives in the cursor and Notes-page modules. The standalone conformance harness gained one module declaration. The new cases are in their own file. |

## Unfinished and questions

- The coordinating container gate runs on dev-01. This lane's VM has no Podman.
- Should the plugin-declared thread list page in this follow-up? Extending the
  manifest and moving the parent/archive filters into the indexed query would
  allow access to rows beyond the existing source window. That touches the
  thread index and should land with the owning lane. A separate follow-up
  leaves the current thread list cap in place.
- Do the complete-result catalogues need paged forms? Keeping their existing
  result shape preserves current callers. Adding pages would bound response
  size and require a client accumulation contract for each of those surfaces.

## Simplification opportunities

- A tag-first composite order index could avoid walking unrelated candidates
  for sparse tags. It would repeat the update key in the projection and add
  write work, so measure the existing path before changing the schema.
- History already authenticates the cursor on the wire. An authorized sequence
  position could remove its revision-to-sequence read after auditing its other
  direct library callers.
