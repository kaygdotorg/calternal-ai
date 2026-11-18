# Wallpaper server review

2026-11-18 · [Issue #49](https://git.kayg.org/kayg/calternal-ai/issues/49)

`wallpapers.list` returns the eight fixed credits and default ID to a signed-in
account. The JPEG routes expose six small default copies at sign-in. Every
other rendition requires a current credential. The routes use the server's
strict same-origin policy and the existing operation envelope for failures.
The no-key case gives clients a 404 for fallback. The wire shape, sizes, cache
bounds and provider requirements are documented in the [wallpaper contract](../wallpapers.md).

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | The list has contract types in core. It uses the existing identity, error, scope and SHA-256 primitives. |
| 2. Dependencies | The server promotes its existing workspace reqwest dependency to runtime use. The ported DNS guard is a server module. No additional crate is required. |
| 3. Lock scope | Warm reads acquire one wallpaper concurrency permit and bypass fill locks. A fill serializes only against other fills of the same photo. The photo lock covers atomic publication to prevent duplicate upstream fetches and partial cache delivery. The budget lock covers only timestamps. |
| 4. Async I/O | Cache filesystem I/O and hashing run in blocking tasks. DNS, HTTP and the synchronization primitives are async. |
| 5. Hot path | Listing has zero storage reads and flushes. A warm rendition has one bounded file read and one digest, with zero flushes. A cold miss rechecks its slot once under the photo lock, reads a tracking marker on first use when needed, and flushes each newly published file and its directory once. |
| 6. Constant work | The catalog JSON, route table and operation schemas are reused. The outbound client is built at startup. |
| 7. Bounds | 30 fixed image slots totaling 34.75 MiB, eight tracking markers, up to 16 MiB of atomic staging, startup eviction, 16 concurrent requests and 48 outbound attempts per process rolling hour. Current defaults remain installed until cache reset or catalog removal. |
| 8. Compatibility | The request accepts only the new operation and canonical rendition names. |
| 9. Scope | The catalog is installation-wide. Session admission and the exact public rendition allowlist are its access boundary. |
| 10. Declarations | One core operation ID, one account policy row and one rendition table used by admission and storage. |
| 11. Wiring | The normal operation dispatch, discovery catalog and HTTP router call the new code. Existing generic CLI invocation reaches the list. |
| 12. Reuse | Existing app backdrop API sequence, referral parameters and netguard ranges. Existing `calternal_fsio::write_atomic` for publication and the main API middleware for session delivery. |
| 13. Public docs | Core wire models, startup configuration and main middleware contracts carry reference comments. |
| 14. Product docs | Root and server READMEs, the Settings contract, generated artifacts and conformance notes ship together. |
| 15. Formatting and size | Wallpaper and netguard modules are split by concern. Existing long integration files received only the necessary wiring. |

## Verification on dev-02

Source base `e7eb8f9` from `origin/dev`. Implementation `cd9bdd1`.
Commands ran in `~/Developer/lanes/wallpapers` on
`ubuntu@calternal-dev-02-std` through `nix develop`. Cargo used
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`,
`CARGO_INCREMENTAL=0` and `CARGO_TARGET_DIR="$PWD/target"`.

| Command | Result |
| --- | --- |
| `npm ci --ignore-scripts --no-audit --no-fund` | Passed. |
| `npm run build:plugins` | Passed before workspace testing. |
| `cargo test --locked -j6 --workspace` | One existing machine test failed because Podman was unavailable. All three wallpaper unit tests passed. |
| `cargo test --locked -j6 --workspace -- --skip pool_machines_serve_only_granted_accounts` | 588 passed, one ignored by the repository and one filtered out. |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed. |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --test isolation -- --test-threads=1` | 90 reported passing, including the new wallpaper case. Existing Podman-dependent cases can return early when the runtime is unavailable. |
| `npm run test:integration` | One test passed against the disposable server. |
| `npm run test:plugins` | 24 passed. The real signed-in Claude test was skipped by its opt-in guard. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed. |
| `node scripts/docs-site/generate.mjs` and `./scripts/check-docs.sh` | Regenerated the operation pages. The rerun passed and checked 299 pages. |
| `cargo fmt --all -- --check` | Passed. |

The full workspace gate remains blocked on
`pool_machines_serve_only_granted_accounts`. Podman from a lane-local Nix
shell failed to initialize rootlessly because dev-02 lacks `newuidmap`.
A disposable user-namespace run with isolated Podman storage also failed,
with `crun` unable to mount `devpts`. No VM-wide runtime configuration was
changed. The wallpaper tests used loopback or synthetic cache data.

## Open questions

Unsplash describes a view-beacon option for apps serving images through
their own infrastructure. Confirm a provider-approved arrangement before
production enablement. A separately approved release-asset arrangement adds
asset management. Direct hotlinking would require a CSP change and send
browser requests to Unsplash. A signed-in picker event would cover later
selections served from cache, at the cost of client wiring and a separate
selection-event budget. See [morning decisions](../morning-decisions.md#wallpaper-attribution-and-tracking-issue-49).

## Simplification opportunities

Once the shell is wired to the server catalog, retire the duplicated web
build-time photo list and downloader. A future plugin network broker can
reuse the guarded HTTPS client with its own declared host policy.
