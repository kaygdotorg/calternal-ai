# Overnight implementation handoff — 2026-11-16

The bounded localhost preview is implemented and verified on Linux with disposable data. It is not the full orchestrator or a production migration.

## Delivered

- Rust host with space-checked Notes operations, durable versions and retry receipts, checked Markdown materialization, startup replay, directory locking, bounded trusted plugin workers and strict HTTP/session boundaries.
- Pure Rust wire/validation contracts, generated schemas and TypeScript, shared conformance fixtures, HTTP client and `cl` CLI.
- TypeScript SDK and repository-shipped Notes/Notes Insights plugins. The real worker computes statistics and returns portable GenUI; no model result is simulated.
- Claude-authored Svelte web UI adapted from the existing Calternal mockup: one sidebar, main card, optional AI pane, Notes/history/conflict drafts, deep links, plugin controls and GenUI rendering. Real-browser verification and screenshots are in [ui-preview.md](ui-preview.md).
- Generated Swift UniFFI contract/validation facade; Linux Rust feature compilation passes. Apple compilation, execution, networking/sync and native UI remain unverified.

## Review fixes incorporated

Independent pre-implementation reviews froze the operation/worker/GenUI contracts and persistence rules. Integration review then fixed plugin toggle retries replaying an old mutation, loss of client request IDs after uncertain responses, recovery placement fsync, API/asset fallback, worker overflow and diagnostic leakage, bounded worker admission, disable-during-call result rejection, optional-null parity, browser draft loss and stale-session results. Concrete tests cover these boundaries.

## Verification evidence

- Rust workspace: **39 tests pass**, including four real process-abort/restart points and the deterministic plugin-disable race. See `artifacts/rust-tests.log` locally.
- Browser code: **14 tests pass**, clean Svelte check/build; Claude exercised the built UI against the real Rust server for Notes, conflicts, deep-link reload, guest isolation, plugin preview/enable/statistics/disable and keyboard drawer behavior. Automated accessibility/real-device frame gates are still pending.
- Plugin SDK: **6 tests pass**; bundled worker: **2 tests pass**. See `artifacts/plugin-tests.log` locally.
- Real-server integration: **1 test passes** with disposable port-0 server and restart persistence, in `tests/integration/notes-plugin.test.mjs`.
- Real CLI: repeated live-server runs and isolated restart smoke pass; optional UniFFI compilation/generation, contract drift checks, formatting and lane Clippy checks pass. Details: [core lane report](reviews/tonight-core-verification-sol.md).
- The final worker digest is pinned in `crates/server/trusted-plugins.json`. A rebuild from settled source matches it.
- Development-host debug smoke measurements are recorded with binary hash and conditions in [measurements/preview-smoke.json](measurements/preview-smoke.json). They are small-fixture HTTP/worker measurements, not a production performance gate or 120 Hz UI evidence. Cold worker process overhead remains material; workers must not sit on the UI frame path.

## Test it

The morning preview runs at `http://127.0.0.1:3210` on the development host in BB terminal `term_765cdmdpka`, using fresh `.dev-data`. Previous browser fixtures were preserved under ignored `artifacts/verified-browser-fixtures-*`. Read the owner token privately from `.dev-data/dev-tokens/owner.token`; do not paste it into chat. Run/build/CLI instructions are in [README.md](../README.md).

All four completed implementation threads were archived after handoff. Work is kept on `feat/overnight-preview`; no email, public deployment, real-data import or provider-account access occurred. Product questions and next engineering work are in [morning-decisions.md](morning-decisions.md).
