# GenUI conformance fixtures

Language-neutral JSON, version 1. Every `.json` file contains `{version, origin, cases}`. Both
implementations run every case against its exact `expected` JSON; no runtime-generated snapshots
are accepted as expected results. `build-fixtures.py` authors the cases from explicit expectations,
credited upstream language/components tests and catalog argument/accessibility declarations.
Review generated expectations whenever it changes.

Each case has `name`, `kind`, `source`, `expected`, and optional `budgets`, `evaluationBudgets`,
`state`. `kind` is `parse` (statement AST + diagnostics), `validate` (accepted + diagnostics),
`evaluate` (state + roots + diagnostics), or `initialize` (state + diagnostics). Budget objects
lower the documented default ceilings. Object member ordering is immaterial in JSON equality;
arrays, scalar values, source positions, messages, state, keys and properties agree exactly.

| File | Cases | Coverage |
| --- | ---: | --- |
| `parsing.json` | 8 | Explicit AST goldens, precedence, state, malformed syntax, UTF-16 positions |
| `language.json` | 115 | Operators, functions, methods, state, composition, initialization, bindings, actions, validation and runtime diagnostics |
| `catalog.json` | 256 | Every retained component plus missing/blank/null accessibility fields |
| `budgets.json` | 15 | Source/AST/state/step/depth/list/node/byte limits, nested iteration and cached-node expansion |
| **Total** | **394** | Same corpus in both languages |

The TypeScript suite adds two JS-only checks for catalog metadata and non-JSON/cyclic state,
which JSON fixtures cannot encode. Rust runs the 394 cases in one integration test. The direct
comparison helper additionally compares **complete statement ASTs, compile diagnostics and
initialization/evaluation snapshots for all 394 cases**, including cases whose golden expectation
only checks activation. This catches parser representation drift beyond the three explicit AST
goldens. JSON numbers are compared as numbers, rather than serde_json's integer/float storage kinds.

Upstream provenance: `core/language.test.ts` and `core/components.test.ts` at
`github.com/ComicBit/bb-plugin-genui`, commit `d8cac8d1781bac336214cc38981aba4a58d9c1c0`.
Ported examples cover arithmetic/precedence, aliases, state, functions, nested maps, keying,
object keys, component arguments, binding conversions, actions, error isolation and resource
limits. Diagnostics are updated to neutral Calternal wording and mandatory accessibility.
Upstream renderer/domain normalizer, file-loading, streaming, markdown, Scene/Shader, project,
thread and environment suites are outside this reduced core; see the package's exact exclusions.

Fixtures establish finite language conformance and fail-closed limits. They do not prove native
accessibility, complete arbitrary-input equivalence, sandbox isolation or device frame time.

## Commands

Run in `~/Developer/lanes/genui` on `ubuntu@calternal-dev-02-std`:

```sh
nix develop -c npm test --workspace @calternal/genui
nix develop -c ./scripts/with-toolchain.sh cargo test -j4 -p calternal-genui -- --nocapture
nix develop -c ./scripts/with-toolchain.sh cargo build -j4 -p calternal-genui --example conformance
nix develop -c node packages/genui/scripts/conformance.mjs
nix develop -c ./scripts/with-toolchain.sh cargo test -j4 --workspace
nix develop -c ./scripts/check-docs.sh
```

Lockfiles are regenerated only in the VM lane directory; the parent integrates them. The root
workspace wildcards already discover both new packages, so no workspace entry change is needed.
