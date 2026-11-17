# Performance lane review — 2026-11-17

Thread `thr_q9bredmvmy`; branch `lane/perf-budgets`;
[issue #25](https://git.kayg.org/kayg/calternal-ai/issues/25).
Results and instrumentation gaps: [performance qualification](../performance.md).

## Gate evidence

All commands ran in `~/Developer/lanes/perf-budgets` on
`claude@calternal-dev-01-compute02`, prefixed with `nix develop -c`:

| Command | Result |
| --- | --- |
| `cargo test --locked --workspace -j6` | 489 passed, 0 failed; one opt-in fixture skipped |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 -p calternal-client --test real_server -- --ignored --test-threads=1` | The opt-in fixture passed explicitly |
| `cargo build -j6 -p calternal-server --features test-plugins` | Passed |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --manifest-path tests/conformance/Cargo.toml -j6 -- --test-threads=1` | 43 passed |
| `npm run test:integration` | 1 passed |
| `npm run test:plugins` | 13 passed |
| `cargo run -j6 -p calternal-core --bin export-contract -- --check` | Contract artifacts match |
| `./scripts/check-docs.sh` | Passed, including workspace all-feature checks, standalone conformance, Rust and TypeScript reference gates |
| `cargo fmt -p calternal-perf -- --check` | Passed |
| `bash -n scripts/perf-check.sh perf/profile.sh perf/startup-profile.sh` | Passed |

Product crates, routes and operations were not edited, so no new isolation case
was introduced. Existing authenticated HTTP/SSE and worker cases cover the product
surface the harness drives. Formatting and documentation checks were repeated as
tooling changed. No timing from this development host is presented as perf evidence.

## Review gate

| Item | Evidence and limits |
| --- | --- |
| 1. Shared types | No product ID, capability, error or authority type copied. The driver carries opaque JSON response fields; benchmark errors stay local to tooling. |
| 2. Dependency direction | A standalone tool crate depends only on existing workspace libraries; no product crate depends on it. HTTP, fixtures, statistics, process lifecycle and profiling warrant a tooling crate. Terminal rendering reuses the shared CLI formatter. |
| 3. Global request state | No product lock/thread added. The mandatory exclusive perf lock is a measurement boundary, not request handling. |
| 4. Blocking work | The driver is synchronous. Its HTTP, filesystem and child-process calls never run on an async executor. The measured product violates this rule at server startup (`Store::open` in async `serve`); issue #25 records the startup stall and owning-lane fix needed. Subscription reads and the timed update use separate scoped threads. |
| 5. Parse/load counts | Budgets load once per suite. Each response parses once. A warm cycle drives six HTTP requests (session, get, list, resolve, update, replay) plus the matching SSE frame; the separate draft loop drives one HTTP write and one changed frame. Raw vectors and a partial summary flush once per arm; run metadata once at entry and final summary once per suite. Product read/fsync counts are not inferred from driver latency; profiles supply evidence. |
| 6. Constant work | HTTP client and budget catalog are reused; one seeded fixture per size is copied into stopped roots for every arm. |
| 7. Bounds/retention | Fixture limits: 100k notes per space, 1k spaces; samples capped at 10,000, warm pairs at 20, cold pairs at 5; at most ten cases. Raw evidence policy: 30 days; scripts/summaries: issue lifetime. Credential-bearing roots are removed after successful arms; failed roots remain for diagnosis and need explicit cleanup within 24 hours. |
| 8. Compatibility | No product migrations, aliases, legacy readers or ignored tests added. The existing opt-in test was run explicitly. |
| 9. UUID spaces | Every API call uses the server-issued space/item UUID; display names are fixture labels only. |
| 10. One declaration per fact | Operations and permissions remain in the product catalog. Budgets are declared in `perf/budgets.json`; the idle runner reads its limits there. |
| 11. Wired callers | Tool modules are called from the executable. The coordinator gate invokes normal and idle modes; the profiler drives a separate control. Unavailable measurements are listed as gaps rather than dead APIs. |
| 12. Reuse | Considered/ported reference `stats.rs`, `server.rs`, `run-vm.sh`, ADR 0062; reused this project's software passkey fixture and the host's perf/cache-drop helpers. Product durability is exercised through the real server, not reimplemented in the driver. Tool evidence files do not claim userdata durability. |
| 13. Reference docs | Workspace wildcard includes the new crate in `check-docs.sh`. Modules state purpose and costs/failures where relevant; no new public product API. |
| 14. Documentation | Crate README, perf README, fixture specification, this dated review and `docs/performance.md` accompany the code. Raw samples and profiles remain on perf-01. |
| 15. Size/format | Rust formatting checked on dev-01; each source module is below 600 lines. |

Numeric breaches and missing mandatory measurements block performance acceptance;
a clean engineering checklist does not turn incomplete qualification green.

## Questions and unfinished work

See the exact hook/fixture requests and option costs in `docs/performance.md`.
New-path limits remain measured proposals until accepted. No inherited limit is
raised. The cardinality fixture is intentionally narrow: repeated warm access,
short Markdown, no attachment or tier-move workload. This is not full ADR 0062
storage or provider-startup qualification.

## Simplification opportunities

Export one shared conformance login fixture; derive metric labels from the budget
catalog; reuse the existing journal wake/replay path for account-feed delivery
instead of maintaining its own polling loop.
