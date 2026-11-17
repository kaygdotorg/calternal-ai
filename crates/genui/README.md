# calternal-genui

Pure Rust parser, statement/expression AST, static validator and bounded evaluator for native
clients. The TypeScript package consumes the same catalog and shared fixtures; there is no Wasm,
platform UI dependency, JavaScript runtime, filesystem/network effect or action dispatch here.

```rust
use calternal_genui::{Budgets, compile, evaluate};
let compiled = compile("Text(\"Hello\")", Budgets::default());
assert!(compiled.diagnostics.is_empty());
let snapshot = evaluate(compiled.program.as_ref().unwrap(), None, Budgets::default());
assert_eq!(snapshot.roots[0]["type"], "Text");
```

`compile` is the only public constructor of a validated `Program`. AST records are serializable
for inspection/fixtures; callers cannot deserialize an untrusted AST into `Program`. `evaluate`
rechecks program/state/output budgets and returns JSON descriptions. `initial_state` uses the
same checks and initializes state without evaluating roots. Actions describe effects for an
authorized caller to process; the interpreter executes none of them.

Module responsibilities:

- `ast`, `lexer`, `parser`: source positions, syntax and bounded parsing.
- `catalog`, `validate`, `budgets`, `error`: shared data vocabulary, static checks, limits and capped diagnostics.
- `value`, `math`, `evaluate`: local identity-preserving values, pure operations and native descriptions.
- `tests/conformance.rs`: every shared JSON fixture; `examples/conformance.rs`: fixture-only JSON-lines adapter for direct TypeScript comparison.

Compilation allocates only source-local tokens/AST/symbol tables. Evaluation allocates a local
state/cache/tree and performs conservative intermediate-value scans: O(steps × bounded value
bytes + local name lookups). There are no queues or cross-call caches. The first `catalog()` call
initializes one immutable JSON tree with OnceLock; later reads are constant-time atomic checks.
No operation performs I/O, fsyncs or network round trips. These are bounds, not perf-VM benchmarks.

See the [format, limits, exclusions and proposals](../../packages/genui/README.md) and
[fixture contract](../../contract/genui-fixtures/README.md). Client-core/UniFFI wiring and native
renderers are outside this lane. The crate embeds the package's catalog directly and is currently
workspace-only; packaging it separately would need the shared data included in its package.

Dependencies use existing workspace serde/serde_json; JSON insertion order is enabled for language
conformance. The exact resolved versions are recorded in the fixture handoff report. The credited
upstream Apache-2.0 [LICENSE](LICENSE) and [NOTICE](NOTICE) accompany the derivative implementation.
