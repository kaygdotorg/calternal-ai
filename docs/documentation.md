# Documentation is part of the change

Every public Rust item (including modules, types, variants, fields, functions, traits and constants) and every public TypeScript export/member must carry reference documentation. Update its comment in the same edit as its code. Explain purpose, invariants, units, failures and cost where relevant; operation wire types identify their operations. Comments must describe current behavior without claiming authorization or isolation that a type does not enforce.

The gates enforce the presence of comments and valid reference links. They cannot prove that prose is accurate or that an existing comment was meaningfully updated alongside a behavior change; review must check that part of the rule. Do not suppress the lints, hide exports, or add empty comments to pass.

## Check

From the repository root, after the usual `npm ci`:

```sh
npm run check:docs
# Equivalent:
./scripts/check-docs.sh
```

The script runs a locked `cargo check --workspace` over every crate with all targets and features (including UniFFI), then checks the standalone conformance suite (`tests/conformance`, its own manifest). Every crate root denies `missing_docs` and `rustdoc::broken_intra_doc_links`, so a new crate joins the gate by joining the workspace. It then runs `cargo doc --no-deps --workspace` because Rust checks documentation links during rustdoc, and TypeDoc validates all TypeScript modules discovered recursively under `src/` in the SDK, Notes and Notes Insights. Notes currently exports a JSON manifest only; Notes Insights is an executable module without named exports. Future TypeScript exports in either package join the same gate automatically. Referenced external contracts remain documented at their Rust source of truth.

TypeDoc requires documentation for exports, public members and signatures and treats validation warnings as errors. Private/protected members and external dependencies are excluded. The checker does not use `excludeNotDocumented`: missing comments must fail instead of disappearing from the site. JSON manifests are data, not TypeScript declarations, and cannot carry TSDoc comments.

Cargo commands use `scripts/with-toolchain.sh`, `--locked` and at most `-j4`. Checks neither contact the preview server nor read `.dev-data`. Reference generation is a build operation with no runtime access to application data.

To verify the gates themselves:

```sh
node --test scripts/test-docs-gate.mjs
```

These four negative tests temporarily introduce undocumented TypeScript exports/members and a Rust public constant or broken link, require rejection, and restore/remove their probes in `finally` blocks. Run them serially with other documentation checks; do not terminate them while a probe is installed.

## Build and read

```sh
npm run build:docs
# Equivalent:
./scripts/build-docs.sh
```

This first runs both gates, builds Rust documentation without dependencies, then generates the TypeDoc site. Outputs are ignored under `reference-docs/`; Cargo's intermediate build cache also lives there. Open:

- `reference-docs/rust/doc/calternal_core/index.html` for the pure operation and worker contracts.
- `reference-docs/rust/doc/calternal_client/index.html` for HTTP effects, retry and error semantics.
- `reference-docs/rust/doc/calternal_bindings/index.html` for synchronous UniFFI validation scaffolding.
- `reference-docs/rust/doc/cl/index.html` for the CLI crate.
- `reference-docs/typescript/plugin-sdk/index.html` for the SDK's exported API.
- `reference-docs/typescript/calternal-notes-insights/index.html` for the worker module (no named public API today).

Rustdoc also emits pages for the contract exporter and optional bindings generator. Delete `reference-docs/` to clean all generated documentation and its Cargo cache. Do not check generated reference HTML into Git.

The contract exporter derives JSON Schema descriptions and TypeScript type/field comments from Rust comments, including the type-level documentation exposed by `ts-rs`. It removes trailing line whitespace without changing type tokens. After contract-comment edits, regenerate and check drift with:

```sh
./scripts/with-toolchain.sh cargo run --locked -j4 -p calternal-core --bin export-contract
./scripts/with-toolchain.sh cargo run --locked -j4 -p calternal-core --bin export-contract -- --check
```

Never hand-edit `contract/` or `packages/contracts/` output. These metadata/comment changes must preserve the schemas' validation constraints and TypeScript wire types.

## Documentation tools

TypeDoc is lighter here than adding ESLint, a TypeScript parser, a JSDoc plugin and a separate reference generator: one tool performs validation and generation. Its tools-only package and lockfile live under `scripts/docs-tools/`, separate from the product lockfile. The first documentation command installs it with `npm ci --ignore-scripts`; subsequent commands reuse it. To restore a changed or absent tools install, run:

```sh
npm ci --prefix scripts/docs-tools --ignore-scripts --no-audit --no-fund
```

The only new direct development dependencies are `typedoc` **0.28.14**, for reference generation and missing-comment/link validation, and `typescript` **5.9.3**, its explicitly pinned compiler peer (matching the current SDK compiler). Transitive dependencies are locked. The first install needs npm registry access; an installed toolset works offline. No runtime dependency is added.

## Server lane integration

Every workspace crate, the server included, is in both Cargo invocations through `--workspace`; there is no per-crate list to maintain.
