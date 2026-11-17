**Thread:** thr_nrmwx3nuxu · GPT-6.1-Sol medium · shared checkout

**Goal:** GenUI core as portable component descriptions, mandatory accessibility and resource limits, Rust evaluation for native clients, shared JSON conformance.

**Status**
- [x] Rust parser, AST, evaluator and validator
- [x] TypeScript client core; bb host integrations removed
- [x] Required accessibility fields and compile/evaluation budgets
- [x] Three initial fixture-position expectations corrected
- [x] 394 fixtures pass in both languages; full AST/diagnostic/evaluation snapshots agree
- [x] TypeScript reference documentation gate
- [x] Workspace: 385 passed, 0 failed, 4 ignores; documentation gate and handoff
- [ ] Parent integration: merge/regenerate VM-only lockfile entries

Handoff: `contract/genui-fixtures/verification.md` (source awaits parent integration).

**Open**
- Data-bound lists: interpreted map/filter now; propose keyed/virtualized List data bindings (new native adapter contract).
- Complex parts: reuse catalog nodes or add editor/thread native nodes (more client implementation/release work).
- Motion: fixed declarative presets or parameterized timelines (different validation/reduced-motion costs).
- Composition: named pure functions now; propose versioned reusable component definitions (scope/version/recursion contract).
