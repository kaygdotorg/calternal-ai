# GenUI final review — 2026-11-17

The Rust evaluator is split by concern with unchanged method bodies, public APIs and behavior.
All 397 shared fixtures still agree between Rust and TypeScript. Branch:
`lane/genui-final-review`, based on `dev` at `d22ded73900b353a5d6c6f7016c2e52ef0656611`.
This follow-up preserves the finish lane's located-budget corrections and adds no fixture changes.

## Changes

- [evaluate.rs](../../crates/genui/src/evaluate.rs): public snapshots, state initialization,
  whole-pass checks and the single interpreter state owner (168 lines, previously 922).
- [expressions.rs](../../crates/genui/src/evaluate/expressions.rs): expression dispatch,
  lazy operators and data access (274 lines).
- [components.rs](../../crates/genui/src/evaluate/components.rs): native descriptions,
  argument/binding normalization, action data and component-local recovery (282 lines).
- [calls.rs](../../crates/genui/src/evaluate/calls.rs): pure built-ins, receiver methods
  and list callbacks sharing the parent interpreter's counters (242 lines).
- [Crate README](../../crates/genui/README.md): module layout, unchanged costs and consumer tracking.
- This dated review. No TypeScript API/source, manifest, dependency, lockfile, UI or protocol changed.

## Verification

Every build/test ran as `ubuntu@calternal-dev-02-std`, in `~/Developer/lanes/genui`,
on its `lane/genui-final-review` branch. Source editing and static Git checks ran in the
isolated bb worktree on hd01. No subagents, other lane directories, production, preview data,
reserved listener, migration or UI work was involved.

| Exact command on the dev VM | Final result |
| --- | --- |
| `nix develop -c npm ci --ignore-scripts --no-audit --no-fund` | Exit 0; installed the committed workspace lockfile. |
| `nix develop -c ./scripts/with-toolchain.sh cargo test --locked -j4 -p calternal-genui -- --nocapture` | Exit 0; 397 fixtures pass in the integration test; unit/doc harnesses pass; no ignores. |
| `nix develop -c npm test --workspace @calternal/genui` | Exit 0; 399 tests pass: 397 fixtures and two existing JS defensive/catalog tests. |
| `nix develop -c ./scripts/with-toolchain.sh cargo build --locked -j4 -p calternal-genui --example conformance` | Exit 0. |
| `nix develop -c node packages/genui/scripts/conformance.mjs` | Exit 0; all 397 complete AST, compile-diagnostic and evaluation snapshots agree exactly. |
| `nix develop -c ./scripts/with-toolchain.sh cargo fmt --check -p calternal-genui` | Exit 0. |
| `nix develop -c ./scripts/check-docs.sh` | Exit 0; Rust check/rustdoc, backend plugin build and all TypeScript reference gates pass. An existing server unused-import warning remains; GenUI adds no warnings. |
| `git diff --check` (hd01, static only) | Exit 0. |

The first formatting check found two extra blank lines introduced by extraction; formatting
removed them. Initial docs attempts exposed two build-copy setup problems: an obsolete untracked
`crates/server/src/plugins.rs` conflicted with the tracked `plugins/mod.rs`, and the VM Git
repository lacked remote metadata. Only that stale untracked build-copy file was removed, and
VM remotes were set to match the source checkout. No tracked server source or docs-gate logic
was changed. The final commands above pass without weakening the gate.

## Quality and checklist

| Gate | Finding for this follow-up |
| --- | --- |
| 1. Shared types | No IDs, transport errors, capabilities, time or hash helpers added/copied. Existing language-specific syntax/diagnostic records are unchanged. |
| 2. Downward dependencies | No dependency or crate added; these are private modules extending the existing interpreter. |
| 3. Global request work | No lock, thread, connection or mutable global added. The immutable catalog's existing one-time initialization is unchanged. |
| 4. Blocking work | No file, network, process or SQLite call exists in these evaluator modules. |
| 5. Parse/load once | Source is compiled once and the program reused. Evaluating never reparses source. Reads/writes/flushes/fsyncs/round trips: zero. |
| 6. Constant work | The existing immutable catalog stays initialized once and cached; this split creates no new constant work. |
| 7. Bounds | The same per-call node/depth/state/step/list/byte ceilings, local cache and capped diagnostics apply across all modules. |
| 8. Compatibility | No compatibility reader, migration, alias, projection or ignored test added. |
| 9. Space identity | No account/space IDs or names are introduced; local expression identifiers retain their existing semantics. |
| 10. One declaration | No operation, capability, role or component declaration changes or duplication. |
| 11. Consumers | Parent-assigned consumer lanes are recorded on [issue #3](https://git.kayg.org/kayg/calternal-ai/issues/3): web UI #15 uses TypeScript; native work after #17 uses Rust through bindings. Their wiring is outside this follow-up. |
| 12. Reuse | Existing upstream-derived expression/operator, property normalization and math implementations move intact. No file/durability/locking primitive is needed because the library performs no I/O. |
| 13. Documentation | Existing public comments retain purpose, failure and cost. New private modules explain their boundaries and shared-state invariants. The full documentation gate passes. |
| 14. Matching docs | The crate README describes the new layout; this dated report is under `docs/reviews/`. |
| 15. Size/format | Every evaluator file is below 300 lines; GenUI Cargo formatting passes. |

One private interpreter still owns state, named-value cache, node/step/depth counters, identity
suffixes and diagnostics. Restricted parent visibility only connects its module implementations;
no additional runtime abstraction, serialization boundary or exported API is introduced.
Evaluation cost remains O(expression steps × bounded value bytes + local name lookups), with
local state/cache/tree allocations and zero I/O. These bounds are not a device performance result.

## Remaining work and questions for the user

No new product or architecture choice was made. Real renderer/bindings integration belongs to
the consumer lanes recorded on issue #3. The existing list binding, specialized native node,
motion and reusable component proposals remain open there; this change enables none of them.
No dependency was added and no lockfile integration is required for this follow-up.

## Simplification opportunities

The split removes the large mixed evaluator file while keeping one state owner. A later measured
optimization could cache intermediate value sizes instead of rescanning/serializing values on
each expression visit; that would require separate budget-conformance review and is deliberately
outside this behavior-preserving refactor.
