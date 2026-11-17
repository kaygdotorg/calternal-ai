# GenUI lane verification — 2026-11-17

Implemented in the shared `feat/overnight-preview` checkout; builds/tests ran only as
`ubuntu@calternal-dev-02-std` in `~/Developer/lanes/genui`. No other lane directory or process,
preview data, reserved listener, production machine, commit or push was touched. No other
lane's crate was removed from the VM workspace. No subagents or new threads were created.

## Changes

- `packages/genui`: credited upstream lexer/parser/AST adapted to a JSON-only native description
  boundary, pure evaluation, static validation, a shared catalog, accessibility contracts,
  deployment ceilings, API/field comments, package tests and extraction/documentation/parity tools.
- `crates/genui`: `calternal-genui`, pure Rust AST/parser/validator/evaluator, identity-preserving
  local values, bounded diagnostics, same embedded catalog, fixture suite and parity adapter.
- `contract/genui-fixtures`: 394 explicit language-neutral golden cases, generator, provenance,
  licenses/notices, format/count documentation and this handoff.
- `scripts/check-docs.sh`: parent-authorized additions of `-p calternal-genui` to both Rust gates
  and the package TypeScript reference gate. The adjacent content-crate addition belongs to its lane.
- Root Cargo/npm workspace wildcards already include the new crate/package; no entry edit needed.
  Root lockfiles on hd01 were never modified or copied back from the VM.

## Commands and results

Commands below run after `cd ~/Developer/lanes/genui` on the dev VM, with Cargo at `-j4`.
All listed final checks exited 0.

| Exact command | Result |
| --- | --- |
| `nix develop -c npm install --ignore-scripts --no-audit --no-fund` | Installed workspace dependencies; VM-only npm lockfile regeneration. |
| `nix develop -c npm test --workspace @calternal/genui` | **396 passed**, 0 failed: 394 JSON fixtures + 2 JS-only defensive/catalog checks. |
| `nix develop -c ./scripts/with-toolchain.sh cargo test -j4 -p calternal-genui -- --nocapture` | **394 fixtures passed** in 1 integration test; unit/doc harnesses also pass; 0 ignores in this crate. |
| `nix develop -c ./scripts/with-toolchain.sh cargo build -j4 -p calternal-genui --example conformance` | Fixture JSON-lines adapter built. |
| `nix develop -c node packages/genui/scripts/conformance.mjs` | **394 complete AST/compile-diagnostic/evaluation snapshots agree exactly**, including all source positions and native keys. |
| `nix develop -c ./scripts/with-toolchain.sh cargo test -j4 --workspace` | **385 passed, 0 failed, 4 ignored** in this VM snapshot. No ignored test was added by this lane. This snapshot includes concurrent lanes' source. |
| `nix develop -c npm run build --workspace @calternal/plugin-sdk` | Built SDK declarations required by the existing Notes worker documentation gate. |
| `nix develop -c ./scripts/check-docs.sh` | Rust check/rustdoc and existing + GenUI TypeScript documentation gates passed. Two existing core rustdoc HTML-tag warnings remain outside this lane. |
| `nix develop -c rustfmt --edition 2024 crates/genui/src/*.rs crates/genui/tests/*.rs crates/genui/examples/*.rs` | Owned Rust sources formatted. |
| `git diff --check -- packages/genui crates/genui contract/genui-fixtures scripts/check-docs.sh` (hd01, static only) | Passed; no build/test ran on the editing host. |

Earlier failures were not waived: Rust syntax errors and the initial TypeScript Unicode-library
setting were corrected; three hand-authored fixture expectations had incorrect positions/error
counts and were corrected against the upstream parser behavior. The first documentation attempt
found inherited inline fields/barrel aliases missing reference comments; those were documented.
The first full documentation script then failed because SDK declarations were unbuilt; building
the SDK resolved the worker import errors. Final checks above pass. No expected snapshot was
replaced wholesale by output from either implementation.

Corpus: parsing 8, language/initialization 115, catalog/accessibility 256, budgets 15 = **394**.
Extra budget regressions cover nested interpreted loops and a cached component expanded to more
than 2000 emitted occurrences. State cannot inject interpreter-tagged native nodes/actions.

## Dependencies and lockfile integration

Added direct dependency declarations in owned manifests:

| Manifest | Declaration | Resolved on VM |
| --- | --- | --- |
| `crates/genui/Cargo.toml` | `serde.workspace = true` | `serde 1.0.229`, existing workspace dependency |
| `crates/genui/Cargo.toml` | `serde_json = { workspace = true, features = ["preserve_order"] }` | `serde_json 1.0.154`, existing workspace dependency; insertion order needed for keys/values/entries conformance |
| `packages/genui/package.json` | dev dependency `typescript: ^5.9.3` | `typescript 5.9.3`, no production npm dependency |

The ordered-map feature resolves to `indexmap 2.14.2`, `equivalent 1.0.3`, `hashbrown 0.17.1`
and `foldhash 0.2.0`. These registry versions already exist in the editing checkout's lockfile;
the parent still needs the new crate and serde_json feature dependency edges and npm workspace entry.
No sqlx, SQLite, runtime JS engine, Wasm, renderer or platform dependency was added.

Full resolved Rust dependency closure:
`serde 1.0.229`, `serde_core 1.0.229`, `serde_derive 1.0.229`, `proc-macro2 1.0.107`,
`unicode-ident 1.0.27`, `quote 1.0.47`, `syn 3.0.10`, `serde_json 1.0.154`, `foldhash 0.2.0`,
`indexmap 2.14.2`, `equivalent 1.0.3`, `hashbrown 0.17.1`, `itoa 1.0.19`, `memchr 2.8.3`,
`zmij 1.0.23`.

Per parent instruction, Cargo ran without `--locked` to generate locks only inside this VM lane.
The documentation script's internal `--locked` checks passed against that generated lockfile.
Parent integration must regenerate/merge both root lockfiles before claiming locked builds from hd01's source snapshot.

## Quality and remaining work

Small modules separate syntax, catalog/validation/budgets and value/math/evaluation. Public
records are data; public operations document invariants/errors and cost. Compile work is bounded
to this description and dependency graph. Evaluation uses a per-call cache, bounded steps and
conservative value scans (O(steps × bounded value bytes), plus local name lookups), allocating
only local state/cache/output. Except one-time immutable catalog initialization in Rust, there
are no locks, queues or cross-call caches. No operation does I/O, fsyncs, round trips or effects.

Deliberately excluded: bb wiring, file loading/targets, project/thread/environment integrations,
React/DOM, HTML/markdown generation, partial streaming rendering, renderer-specific normalization,
per-frame callbacks, Scene/Shader, UniFFI wiring and native views. All 21 excluded catalog names
are listed in the package README. Required format fields are checked; actual native keyboard,
VoiceOver, contrast, reduced-motion and device performance remain renderer work. Fixtures are
finite conformance evidence, not a proof of arbitrary-input equivalence or native performance.

## Questions for the user

The [catalog proposals](../../packages/genui/README.md#questions-for-the-user--proposals-only)
remain proposals, with no new component enabled:

1. Data-bound lists: bounded snapshot/map compositions (smaller current contract) or keyed,
   paged/subscribed native list bindings (new scoped adapters, identity and virtualization work).
2. Complex native parts: compose current catalog nodes (less client work, fewer specialist
   interactions) or add editor/thread/timeline nodes (new versioned schemas and client implementations).
3. Motion: fixed declarative presets (smaller validation/reduced-motion surface) or parameterized
   timelines (more interpolation limits, native behavior and reduced-motion tests).
4. Reusable composition: local named pure functions (no lifecycle contract) or versioned reusable
   component definitions (scope, parameter schemas, recursion, accessibility propagation and upgrades).

Issue #3 has been updated. No other external message or publication was sent.
