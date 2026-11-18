# Thread file review, 2026-11-18

The owner approved one self-contained Markdown file per thread. The accepted
format, five examples and failure handling are in [Saved threads](../threads.md).
This revision follows the provider catalog and phases work recorded in
[the first lane review](turn-meta-2026-11-18.md).

## Wire and cost

A full read includes `thread.messages`. A checkpoint insertion includes
`delta.messages`. Both use the same record shape. Agent records include
`provenance` with provider plugin ID, model ID, requested reasoning level,
thread-scoped machine reference, worked milliseconds and reasoning
milliseconds. Provider labels are unknown when the adapter cannot establish
the serving service. The file's native provider session binding stays private.

Extending a saved record changes its byte count. Finishing it can also change
the heading. Both use a full refresh. A new complete record uses the existing
insertion delta, with one indexed lookup and at most one bounded BLOB read.
The client already has a full-refresh path for journal entries without an
insertion. This avoids a second replacement protocol at the cost of transferring
the full thread for continued output.

Send adds one indexed author-name read. Output uses one existing file-body load
per changed checkpoint and the indexed tail offset. The operation reply and receipt reuse the same
parsed snapshot. Collecting durations adds
no per-frame reads or flushes and no additional job flush. A timing-only
terminal checkpoint can add one content write with three FULL SQLite commits,
the usual file and directory syncs and the usual receipt and head checks.

## Dev-01 gates

Source files were copied from this worktree to
`claude@calternal-dev-01-compute02:~/Developer/lanes/turn-meta`. Commands
ran in `nix develop` from that directory. The workspace tests and
conformance each used the isolated Podman leak checker.

```sh
export RUST_TEST_THREADS=1 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0
export CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal"
export CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-provider-fixture:turn-meta-readable
cargo fmt --all
cargo fmt --manifest-path tests/conformance/Cargo.toml
npm run build:plugins
./scripts/check-podman-tests.sh plugins/calternal-claude-code/test/build-image.sh "$CALTERNAL_TEST_PROVIDER_IMAGE"
./scripts/check-podman-tests.sh cargo test --locked -j6 --workspace --all-features --no-fail-fast
cargo build --locked -j6 -p calternal-server --features test-plugins
./scripts/check-podman-tests.sh cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --test isolation -- --test-threads=1
npm run test:integration
npm run test:plugins
cargo run --locked -j6 -p calternal-core --bin export-contract -- --check
node scripts/docs-site/generate.mjs
./scripts/check-docs.sh
cargo fmt --all -- --check
cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check
```

The feature commit `70ab329` passed 589 workspace tests with one opt-in
test ignored, 89 serial conformance cases, 24 plugin tests with one signed-in
Claude test skipped, and one disposable-server integration test. The fixture
image, workspace and conformance leak checks passed. The server build, contract
check, generated site source, documentation checks and both formatter checks
passed. An earlier conformance run on that source timed out waiting for a
worker restart and lost an expiring invite from the list. A full clean rerun
passed both cases and all other cases without a code change.

After merging `origin/dev` at `e085368` into `b7fd7ff`, dev-01 passed
592 workspace tests with one opt-in test ignored, 90 serial conformance cases,
24 plugin tests with one signed-in Claude test skipped, and one integration
test. Workspace and conformance both passed the Podman leak check. The
contract check, site generation, docs gate (299 pages), and both formatter
checks passed. Afterward the lane confirmed removal of the three build
directories with `cargo clean --target-dir target`,
`cargo clean --target-dir tests/conformance/target`, and
`cargo clean --target-dir reference-docs/rust`.

The final lane merge at `acdd98e` also includes `origin/dev` at
`2a08bb8`. That merge changed the UI and its documentation. A path-scoped
`git diff --exit-code b7fd7ff acdd98e` confirmed identical Rust workspace,
plugins, conformance inputs, build scripts, contract and thread-format docs.
The UI lane still needs to adopt the structured transcript. Its current
reader expects the previous comment pairs.

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | Existing UUID, account, space, content request and caller-safe error types. Provenance belongs to the thread transcript. |
| 2. Dependencies | Existing time, serde, rusqlite and content dependencies. No new crate. |
| 3. Lock scope | Existing per-space content and outbox boundaries. The author lookup happens before entering the content write. |
| 4. Blocking work | The existing thread-operation and checkpoint workers handle filesystem, directory and SQL calls. |
| 5. Read and flush counts | One indexed author read per send, no extra per-frame reads or flushes, and the timing-only content write described above. |
| 6. Constant work | The timestamp pattern compiles once. Provider selection uses the loaded manifest catalog. |
| 7. Bounds | Existing 2 MiB transcript cap and 32 KiB checkpoint cap. One native binding and fixed-size provenance per attempt. No persisted reasoning text. Receipts retain the existing operator policy. |
| 8. One format | The reader accepts the new frame. There is no alternate reader or migration. |
| 9. Space identity | Authorization and storage use the existing space UUID. Headings and filenames are presentation. |
| 10. Single authority | The orchestrator manifest declares operation policy. File records carry message identities and the catalog carries provider choices. Derived counts serve the index. |
| 11. Wiring | The writer runs in send and provider checkpoint paths. The shared parser serves full and delta reads. CLI supplies its local offset. Web consumption is pending in the UI lane. |
| 12. Reuse | Content's revision CAS, receipts, file placement, journal and delta BLOB reads. Existing protected name storage and machine-reference hashing. |
| 13. Public docs | Rust doc comments, the threads guide and the generated docs site explain framing, failures and costs. |
| 14. Behavior docs | One file, private session binding, full refresh, local offsets, model provenance and limitations are documented together. |
| 15. Formatting | Cargo formatting covers the workspace and standalone conformance crate. The new record and heading concerns live in separate modules. The inherited runner remains slightly over 600 lines. |

## Unfinished

- Account time zone preference and web adoption. The writer currently receives
  a resolved client offset, and assumes UTC when the client omits it. An
  in-flight turn keeps its submitted offset even if DST changes during the turn.
- A credential-free saved sign-in result line needs an authoritative sign-in
  completion event. Re-authentication reasons identify the provider to the
  affected account.
- Claude subscription title isolation remains unverified. The documented
  `--bare` mode skips user configuration and disables subscription login.
  A separate clean login profile could preserve the requested billing. It
  may require another sign-in, plus a documented credential boundary and an
  end-to-end isolation test.
  An API-key-only title path would need separately authorized account
  authentication and different billing. That is a product change.
  [Claude Code headless documentation](https://code.claude.com/docs/en/headless#start-faster-with-bare-mode)
- Codex title selection needs a Calternal Codex adapter and a subscription
  readiness signal. Automatic scheduling, title regeneration and the
  link-aware title rename remain pending.
- Edited-message history is an open design question. The framing leaves each
  message independently addressable by its stable ID.

## Simplification opportunities

Use structured message records directly in clients, and remove transcript
parsing from the web client when it adopts the new read shape. Consider
sharing Markdown title and frontmatter escaping with the Notes port when
it provides the canonical encoder. If full refresh volume becomes material,
measure it before adding another delta shape.
