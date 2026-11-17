# Threads operations for the UI

Issue #35, thread `thr_a99wqey3vf`, branch `lane/threads-ui-ops`. The lane started
from `origin/dev` at `18a54a27` and merged `d1922c5` as `918e6ca`.

The sidebar reads a bounded content and jobs index page. The thread file records
message timestamps, count and last activity. Rename and unarchive use the normal
manifest policy and receipted content write. Get includes the current nonfinal
job for following the same stream after reload. Content mutations and durable
turn transitions appear on the owning space's existing live journal.

The requests lane (#32) writes person waits to the existing job state
`waiting`. Both thread reads and turn journal publication consume that state
without accessing protected request contents. The conformance fixture commits
the same state transition directly because #32 was still in its own lane.

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | Reuses `SpaceId`, UUIDs, `AccountId`, caller-safe `Error`, `JobState`, `JobView`, content's clock, and existing manifest capabilities. |
| 2. Dependencies | Uses the existing rusqlite, content, jobs and journal dependencies. The product-specific join and event mapping are in the threads module. |
| 3. Request locks | Uses the existing per-space jobs connection and content locks. Publication scheduling takes a short lock on that same cached jobs connection. The task releases jobs before obtaining the journal and awaiting its writer. |
| 4. Async safety | Thread SQL and filesystem work run on blocking workers. The outbox page reads and deletes run in blocking tasks. Journal writes use its existing async writer API. |
| 5. Read and flush counts | One warm list statement, N indexed job seeks inside it, zero thread-file reads and zero flushes. Get loads one content head and one latest job. Rename and unarchive use the same content write path as archive. A jobs transaction does one cached outbox-existence query. Publication batches up to 64 rows and incurs one journal FULL commit per transition and one FULL jobs delete per page. |
| 6. Constant work | Manifest input validators are cached. The index and outbox queries use connection-local prepared statements. A bundled-SQLite regression asserts indexed seeks against both indexes. |
| 7. Bounds and retention | Keeps the existing 1,000-row candidate ceiling, file and job bounds, and operator-configured receipt retention. The outbox reads at most 64 rows at a time and ages pending rows after seven days. |
| 8. Format | Uses the current Markdown envelope and current output schema. Existing preview files without timestamps are outside this format change. |
| 9. Space UUIDs | Resolve first, then read only indexes under the selected space's UUID. Feed delivery rechecks current membership and thread ownership. |
| 10. Declarations | The orchestrator manifest declares each operation, capability, risk and target once. The same job-state mapping supplies list, get and journal entries. Separate authz-matrix rows are independent expectations. |
| 11. Wiring | The CLI dispatches both operations. Server startup installs the publisher before job recovery and authorized operations call the index. The browser adapter is owned by the UI lane. |
| 12. Reuse | Checked the reference app's accepted storage ADR and reused content's revision, filename, archive, receipt and placement path. Reused `SpaceStore::commit`, wakeup and replay after checking the perf lane's changes. |
| 13. Reference docs | Changed public APIs describe purpose and cost. The docs gate covers affected crates and the SDK. |
| 14. Matching docs | Thread and CLI docs, the generated public Threads page and CLI goldens describe the delivered behavior. |
| 15. Format and size | Workspace and standalone conformance formatting checks cover the changed files. The SQL join, event publication and jobs connection bookkeeping have their own modules. |

## Verification

All builds and tests used `ubuntu@calternal-dev-02-std`, in
`~/Developer/lanes/threads-ui-ops`, through `nix develop -c`. The lane source
was transferred with the requested `git ls-files -co --exclude-standard -z` tar
command. Cargo builds and test runs used `-j6`. Full logs are on that VM under
`/tmp/threads-ui-ops-merged-*.log`.

| Command inside `nix develop -c` | Result |
| --- | --- |
| `npm run build:plugins` before Cargo tests | Pass. The notes worker matches its trusted bundle digest. |
| `cargo test --locked -j6 --workspace --no-fail-fast` | 538 passed, two failed, one ignored. The pool test requires Podman. The provider resume test failed on the machine checkpoint assertion. |
| `cargo test --locked -j6 -p calternal-server --test provider_turns -- --test-threads=1` | Three passed and one failed at the same machine checkpoint assertion. |
| `cargo test --locked -j6 -p calternal-server --test provider_turns fake_official_cli_streams_checkpoints_crashes_stops_and_requires_explicit_resume -- --exact --test-threads=1` | Reproduced the failure after restoring the baseline runtime sources from `d1922c5`. |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Pass. |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --test isolation -- --test-threads=1` | Pass, 55 cases including all six thread cases. |
| `cargo test --locked -j6 -p calternal-server --lib list_seeks_the_jobs_thread_index_for_each_summary` | Pass. Bundled SQLite reports seeks through `item_list` and `jobs_thread`. |
| `cargo test --locked -j6 -p calternal-cli --test threads` | Pass, 20 output goldens. |
| `cargo build --locked -j6 -p calternal-cli`, then `python3 contract/verify-cli.py` | Pass, real CLI/HTTP contract smoke. |
| `npm run test:integration` | Pass, one real-server integration case. |
| `npm run test:plugins` | Pass, 22 cases. The signed-in provider opt-in skipped. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Pass. |
| `scripts/check-docs.sh` | Pass. |
| `npm ci --prefix apps/docs --ignore-scripts --no-audit --no-fund`, `node scripts/docs-site/generate.mjs` and `node scripts/docs-site/generate.mjs --check` | Pass. |
| `node --test scripts/docs-site/publication.test.mjs` | Pass, two cases. |
| `cargo fmt --all -- --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Pass. |

A disposable CLI smoke also exercised `calternal --json threads create`,
`rename`, `archive`, `list`, `unarchive` and `get`. Rename kept the ID, archive
removed the row from the ordinary list, and unarchive restored it.

The source transfer and location were:

```sh
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/threads-ui-ops-merged.tar
ssh ubuntu@calternal-dev-02-std 'mkdir -p ~/Developer/lanes/threads-ui-ops && cd ~/Developer/lanes/threads-ui-ops && tar xf -' < /tmp/threads-ui-ops-merged.tar
```

Dev-02 has no Podman. The workspace pool test starts a Podman machine
unconditionally and the lease stays waiting. The separate provider resume test
fails at `crates/server/tests/provider_turns.rs:291` on both the lane and
baseline. It checks the new machine ID as soon as the job is running, when the
saved checkpoint can still contain the prior machine ID. The baseline log is
`/tmp/threads-ui-ops-baseline-provider.log`. Both failures remain visible in #35.

## Remaining work and questions

- Re-run container coverage on dev-01 as part of integration.
- Restoring from only the file tree still needs the foundation's file
  importer. The static index fields are in the Markdown envelope and current
  turn state comes from the jobs index. Extending the shared importer would
  cover other plugins too, at the cost of a foundation contract change. An
  operator-only thread reindex could be delivered sooner, at the cost of a
  second import path.
- If the sidebar needs to distinguish placement waiting from provider
  execution, the job stream already has placement events. A durable placement
  state would support reconnecting list clients at the cost of a new
  cross-engine state transition and checkpoint contract.
- If status is needed for pre-change documents, historical timestamps could be
  reconstructed from retained content revisions where available. Starting
  from fresh preview data keeps the single file format and preserves the
  distinction between recorded and inferred times.

## Real-server follow-up

The accepted personal-space decision is in `docs/settings.md`. The account
creation helper writes the account, a fresh space UUID and its Owner membership
in the same protected transaction in first-owner setup, privileged account
creation and development seeding. The UNIQUE `spaces.personal_for` account
relation identifies the personal space. `spaces.list` exposes it to that caller
as `personal: true`. A second space can have the same display name. The preview
exposes first-owner and recovery passkey enrollment. Invite redemption is a
separate implementation, and its account insert can use the same helper.
The authority schema is version 6 and the disposable preview requires fresh
roots from the prior schema.

The fresh-space probe returned `200 {"threads":[]}` from the repository root
and `404 space not found` from an unrelated working directory. In the second
case, registry startup skipped the missing relative orchestrator manifest, so
dispatch had no policy for `threads.list`. The official trusted registry
already marks the orchestrator default-enabled. The implicit plugin path
now uses the exact shipped native manifest when its relative copy is missing.
Explicit plugin roots keep their existing loading rules.

Static serving checks the real asset first, then serves the shell for GET or
HEAD at a recognized application address. The fallback does not resolve item
existence. The ordinary API remains responsible for identity and access.
The CSP continues to require same-origin JavaScript and now allows `blob:` in
`img-src`. The web lane can move the early theme/font bootstrap from
`apps/web/index.html` into a same-origin classic script in the head before the
app module, with correct base-relative font URLs. Retaining inline JavaScript
would require putting the SHA-256 of the exact emitted bytes in the HTTP
`script-src` header.

The follow-up adds no dependencies or locks. Signup adds two row inserts and
their index work to the existing authority commit. The listing marker comes
from the existing space membership query. The registry prepares known kinds
once at startup and the web fallback pays an extra index-file lookup only
when serving the shell. The new authority field is included by the existing
space recovery record. The conformance case covers first-owner space identity
across restart, duplicate display names, cold empty lists from an unrelated
working directory, and equal refusals for hidden and missing UUIDs.

The follow-up merged `origin/dev` at `6cf46a4` as `c4228ec`. Commands ran
inside `nix develop -c` on dev-02 in the same staged lane directory. Logs are
under `/tmp/threads-followup-*.log` there.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Pass, before Cargo tests. |
| `cargo test --locked -j6 -p calternal-server --test http static_dir_serves_files_and_only_app_addresses_fall_back` | Pass. |
| `cargo test --locked -j6 -p calternal-recovery --lib account_creation_passkey_removal_and_session_revocation_use_the_barrier` | Pass after moving the fixture's synchronous directory lookup onto a blocking worker. |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Pass. |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --test isolation -- --test-threads=1` | Pass, 56 cases. |
| `cargo test --locked -j6 -p calternal-server --test secrets --test spaces_matrix` | Pass, 13 cases after scoping each fixture edit to its shared space. |
| `cargo build --locked -j6 -p calternal-cli` and `python3 contract/verify-cli.py` | Pass. |
| `npm run test:integration` and `npm run test:plugins` | Pass. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Pass. |
| `scripts/check-docs.sh` and `node scripts/docs-site/generate.mjs --check` | Pass. |
| `cargo test --locked -j6 --workspace --no-fail-fast` | 538 passed, two failed, one ignored. The same Podman pool and baseline provider-resume failures remain. |
| `node --test scripts/docs-site/publication.test.mjs` | Pass, two cases. |
| `cargo fmt --all -- --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Pass. |

## Simplification opportunities

- Put the metadata-only content index query behind a shared read interface when
  a second consumer needs a cross-index read. That would remove the threads
  adapter's direct dependency on the names of the content database and columns.
- Let job transactions signal outbox changes directly if another consumer needs
  the same hook. That could remove the extra existence query on transactions
  which never change a saved turn.
