# Plugin rename lane report — 2026-11-17

Thread: `thr_2f8y5u386e`
Issue: `kayg/calternal-ai#30`
Branch: `lane/plugin-rename`

Renamed the official plugin IDs and folders to `calternal-notes` and
`calternal-notes-insights`. The Notes kind (`notes`), operation names (including
`notes-insights.summarize`), and `/notes/<id>` address remain unchanged. No old-ID
alias or compatibility path was added. Npm package names remain `@calternal/notes`
and `@calternal/notes-insights`, matching the existing `@calternal/claude-code`
package naming pattern.

## Commands and results

All build and test commands below ran on `ubuntu@calternal-dev-02-std` in
`~/Developer/lanes/plugin-rename`, under `nix develop`.

- `nix develop -c npm ci --ignore-scripts --no-audit --no-fund` — passed.
- `nix develop -c npm run build:plugins` — passed; rebuilt all plugin bundles.
- `nix develop -c cargo build --locked -j6 -p calternal-server --features test-plugins` — passed.
- `nix develop -c sh -c 'umask 022; CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" CONFORMANCE_RESULTS="$PWD/tests/conformance/corrected-results" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1'` — passed, 48 tests.
- `nix develop -c npm run test:plugins` — passed: SDK 12, Notes Insights 2, Claude Code 6; its signed-in Claude test was skipped because it is opt-in.
- `nix develop -c npm run test:integration` — passed, 1 test.
- `nix develop -c cargo run --locked -j6 -p calternal-core --bin export-contract` — passed and regenerated the contract artifacts.
- `nix develop -c cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` — passed: contract artifacts match Rust models.
- `UPDATE_GOLDEN=1 cargo test --locked -j6 -p calternal-server --test cli_golden` — passed and regenerated the plugin digest CLI goldens.
- `UPDATE_GOLDEN=1 cargo test --locked -j6 -p calternal-server --bin calternal cli_output_tests::machine_and_plugin_list_goldens_cover_120_80_and_40_columns -- --exact` — passed and regenerated the binary CLI goldens.
- `sha256sum -c tests/conformance/binaries.sha256` — passed after updating the server, worker, and conformance source checksums.
- `nix develop -c sh scripts/check-docs.sh` — passed. Rustdoc emitted existing warnings in `content::pool` and actor documentation; SDK and plugin TypeDoc checks passed.
- `nix develop -c cargo fmt --all -- --check` — passed.
- `nix develop -c npm run docs:site` — passed; generated and checked 220 pages. Only the regenerated plugin digest page was retained; unrelated stale generated pages were discarded.
- `rg -c '@esbuild/' package-lock.json` — `78`; lockfile edits only changed workspace path keys.
- `git grep -n -F '"notes-insights"' -- ':!docs/reviews/**'` — no matches.
- `git grep -n -E 'plugin_id[[:space:]]*:[[:space:]]*"(notes|notes-insights)"|source_app[[:space:]]*:[[:space:]]*"(notes|notes-insights)"' -- ':!docs/reviews/**'` — no matches.
- `git grep -n -E 'plugins/(notes|notes-insights)(/|[[:space:]"`]|$)' -- ':!docs/reviews/**'` — no matches.
- `nix develop -c cargo test --locked -j6 --workspace` — one environment-bound failure: `pool_machines_serve_only_granted_accounts` timed out because rootless Podman cannot create a user namespace on this VM (`newuidmap: write to uid_map failed: Operation not permitted`). The remaining workspace test output passed. Retrying the test serially produced the same environment error; host-level user namespace settings were left untouched.

## Simplification review

1. **Shared types:** No types or shared helpers were added or copied.
2. **Dependency direction:** No crate or dependency was added; only existing npm workspace paths changed.
3. **Request-path globals:** No request-path state or locks changed.
4. **Async blocking:** No async or I/O behavior changed.
5. **Parse/load counts:** No hot path changed; read and flush counts are unchanged.
6. **Cached constants:** No schemas, catalogs, or validators changed.
7. **Bounds and retention:** No stored data, retention, or request receipts changed.
8. **Compatibility:** Old plugin ID aliases were not added. Tests and registry use the new IDs; operation names retain their existing namespace.
9. **Space identity:** Plugin IDs changed only; space UUIDs, the `notes` kind, and note addresses remain unchanged.
10. **Single source of truth:** Plugin IDs and worker paths were updated in the existing manifests and trusted registry; test, generated, and documentation references follow them.
11. **Wiring:** No public items or operations were added.
12. **Reuse:** Existing manifests, trusted registry, worker builder, and docs package list were updated in place; no new persistence, locking, or validation primitive was needed.
13. **Docs gate:** `scripts/check-docs.sh` passed; no public API was introduced.
14. **Documentation:** Source docs, the plugin digest page, generated CLI goldens, and this dated report reflect the rename.
15. **Formatting and file size:** `cargo fmt --all -- --check` passed; no source file grew from this rename.

## Unfinished and questions

The full workspace test command remains blocked by the dev VM's rootless Podman user-namespace restriction described above. No product or design question is open.

## Simplification opportunities

The existing short npm package names already separate package naming from canonical plugin IDs. Keeping that convention avoids adding another package-name migration while the plugin folders and manifest IDs use their official `calternal-*` names.
