# Thread streaming review, 2026-11-17

[Forgejo #41](https://git.kayg.org/kayg/calternal-ai/issues/41) tracks this
lane. The verification base includes the merged deferred content placement
and the fixture bring-up changes at `c765490`. The implementation commits are
`55f986f` for checkpoint additions and `9d8725f` for provider steps and the
storage-limit failure.

## Changes

- `threads.get` accepts a checkpoint journal entry as `since`. A revision
  pair binds an exact change to one thread. The byte offset is relative to the
  body because checkpoints extend an existing message before its closing
  marker and can also rewrite frontmatter. The server opens an incremental
  SQLite BLOB for the requested bytes. A client can replay older checkpoint
  additions while their versions and live thread remain available.
- The Claude Code adapter previously sent a generic start summary and an empty
  completion detail. The server recorded a last-tool marker in the job
  checkpoint and folded only assistant text into the thread. Bash now identifies
  its command name, file tools give a bounded target, and output and completion
  events carry the same step labels. The adapter emits a byte count for tool
  results and the server saves the scrubbed step notices as Markdown.
- The account-free fixture scripts Bash success, Read failure, at least 96 KiB
  of tool output, and a pause after the Read start for reload and Stop. A
  separate scenario crosses the saved Markdown size limit using assistant text.
- An oversized content write previously rejected the whole oversized checkpoint
  and left a generic outcome-unknown job. The last saved revision now remains
  readable with the job in a permanent failed state and a caller-visible
  `thread_full` reason. The preview's ceiling remains 2 MiB of submitted Markdown,
  before placement inserts the identity line.

The checkpoint's warm path loads the body once and then uses the existing write
header, receipt and current-file hash reads. The content write uses three FULL
commit boundaries, plus the existing deferred placement, one file and directory
sync and NORMAL placement bookkeeping. The job checkpoint adds one FULL commit.
Deferred placement performs its own header, version and checked file reads.
The journal range projection adds zero reads and flushes. The delta read uses
one indexed content lookup and at most one positional BLOB read after the
ordinary host authorization. It uses zero userdata reads, job reads or flushes.

## Dev VM gates

Commands ran inside `nix develop -c` in `~/Developer/lanes/thread-streaming`
on dev-01. The lane used the disposable `thread-streaming` Podman image tag.
The real signed-in Claude test remained opt-in.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Passed before Cargo tests |
| `cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins` | Passed |
| `CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-provider-fixture:thread-streaming RUST_TEST_THREADS=1 ./scripts/check-podman-tests.sh cargo test --locked --workspace --all-features --no-fail-fast -j6` | 572 passed, one ignored, and Podman leak check passed |
| `CARGO_TARGET_DIR="$PWD/target" CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" ./scripts/check-podman-tests.sh cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 77 passed and Podman leak check passed |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" ./scripts/check-podman-tests.sh cargo test --locked -j6 -p calternal-client --test real_server -- --ignored` | The workspace opt-in passed separately and Podman leak check passed |
| `npm run test:integration` | 1 passed |
| `npm run test:plugins` | 23 passed and the real Claude opt-in was skipped |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed |
| `sh scripts/check-docs.sh` | Passed for Rust and TypeScript |
| `cargo fmt --all -- --check` | Passed |
| `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Passed |
| `node --test scripts/docs-site/publication.test.mjs` | 2 passed |
| `node scripts/docs-site/generate.mjs` | Generated 292 pages. Lane-owned pages matched the checked-in artifacts. The optional whole-checkout `--check` found five inherited stale Machines, Recovery, Storage and crate-reference pages. It passed after regeneration in the disposable checkout. |

The fixture image was built with
`./scripts/check-podman-tests.sh plugins/calternal-claude-code/test/build-image.sh localhost/calternal-provider-fixture:thread-streaming`.
Its Podman leak check passed. After the gates, the fixture image and the lane's
workspace, conformance and rustdoc target directories were removed.

## Review gate

| # | Evidence |
| --- | --- |
| 1 | Uses the shared UUID, space ID, caller-safe errors and provider event types. The Markdown ceiling is declared once in content storage. |
| 2 | Reuses rusqlite with its existing incremental BLOB feature. The dependency graph has no new crate or reverse dependency. |
| 3 | Both read forms use the existing space store and lock. Checkpoint publication reuses the space outbox. The existing content-to-job admission lock and fsync contract is documented on `Content::admit_item`. |
| 4 | The operation dispatcher and progress writer keep SQLite, hashing and file placement on the host blocking path. The event reader folds only bounded values on the async path. |
| 5 | The costs above distinguish the checkpoint write, publication and delta read. The range comes from the already encoded write buffer and its body insertion point. |
| 6 | The manifest validator is cached at startup. The range query uses the existing per-space prepared statement cache. |
| 7 | The 2 MiB document cap, 32 KiB pending text cap and 128 pending steps stay bounded. Names are limited to 128 bytes and summaries 512 bytes. Range metadata follows the existing immutable version lifetime. Receipt and job retention use their established bounds. |
| 8 | The API extends the existing get operation. The file envelope, SQL schema and ordinary full response retain their established contracts. |
| 9 | The manifest target resolves threads by UUID and routes them into the authorized space. Conformance uses a reader in that space and a foreign caller. |
| 10 | The orchestrator manifest declares the read policy. Rust generates provider wire schemas and SDK types. |
| 11 | The backend dispatcher and CLI use the new read. Checkpoint publication and saved Markdown use the same production event path as the fixture. |
| 12 | Reuses the content revision guard, receipts, immutable versions, ordered outbox and checked fsio placement. The reference app's ADR 0056 supplies stable block identity and address rules. |
| 13 | New public content and provider surfaces document bounds, cost and failures. The docs command and result are listed above. |
| 14 | The root and server READMEs, thread operations, provider protocol, fixture scenarios and their generated thread and SDK references cover the change. |
| 15 | Provider step folding is in its own module. The gate runs workspace and standalone conformance formatting. |

## Unfinished and questions

The existing web adapter continues making full reads and displays a generic
permanent failure notice. Its wiring for checkpoint deltas and the specific
storage message belongs to the UI lane.

Long results will be stored separately and linked. The Markdown grammar, artifact
address and retention remain open. Space-owned file items can reuse access and
download operations at the cost of more items. Thread-owned artifacts make
purge ownership simpler and need their own download and access contract.
Retaining artifacts with the conversation keeps links durable and can grow
storage separately. Quotas or expiry bound that growth and need an expired
link state. See [morning decisions](../morning-decisions.md#long-tool-output-issue-41).

## Requests and list follow-ups

The follow-up started from `b52c314` and incorporates `2a4ee87`, which
contains the accepted streaming lane and a web-only change. Stale passkey
approval, token-bound access
request mismatches, secret names and missing thread continuations each
reproduced in conformance before the fixes. The five focused conformance cases
and the thread list query-plan test then passed.

The shared wire error code now covers passkey re-verification at the account,
approval, mode and secret-provision surfaces. An agent's access request binds
to its token before any job lookup. Secret names retain their existing
validator with a clearer caller error and retry instruction.

Thread lists reuse the signed cursor and CLI cursor parser. A warm page takes
one indexed metadata query and zero body reads or flushes. Sparse filters can
visit extra keys in that index before finding the page. The query looks up
the latest job state only for the delivered rows and one lookahead.

Dev-01 reran the workspace and conformance commands above with dev and test
debug info disabled. Workspace passed 573 tests with one opt-in ignored.
Conformance passed all 81. Plugin build, integration, plugin tests, contract,
docs, formatting and CLI help checks passed. The CLI help checks ran
`calternal invoke --help`, `cl invoke --help` and `calternal threads list --help`.
The optional whole-site snapshot check still reports three inherited pages
for Machines and Recovery. Follow-up Podman leak checks passed. The fixture
image and lane target directories were removed.

## Simplification opportunities

A structured content outbox projection could carry bounded event-specific
coordinates without the journal recognizing the thread checkpoint event name.
Clients that consume deltas can also use their returned revision to replace
the current timer-driven full reads and rely on full reads after a journal gap.
