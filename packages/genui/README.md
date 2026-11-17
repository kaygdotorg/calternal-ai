# Portable GenUI core

Pure TypeScript parsing, validation and interpretation of generated component descriptions.
The Rust implementation is [calternal-genui](../../crates/genui/README.md). Neither library
renders controls, executes actions, fetches resources or runs JavaScript from a description.
The browser and native clients walk the returned data into their own views. No Wasm is used.

```ts
import { compile, evaluate } from '@calternal/genui';
const compiled = compile('$count=1\nButton("Count " + $count, Set($count,$count+1))');
if (compiled.program) {
  const snapshot = evaluate(compiled.program);
  // snapshot.state, snapshot.roots and snapshot.diagnostics are JSON data.
  // Dispatching a described action remains the authorized client's responsibility.
}
```

`compile` returns the statement AST and positioned diagnostics; any error leaves `program`
null. Only programs returned by `compile` belong at the evaluator boundary: the TypeScript
`Program` interface is not an untrusted AST deserialization API. `evaluate` initializes state
unless an external state snapshot is supplied. A supplied snapshot replaces state rather than
merging with defaults. `initialState` runs initializers only, while validating the full program's
budgets. Resource/initializer failures discard partial state and roots. A local component error
returns an `Error` node and a diagnostic while preserving siblings. No action changes state
until the client explicitly processes it and supplies the next snapshot.

The four action descriptions are `Set`, `Send`, `Copy` and `OpenUrl`. They carry no authority.
`OpenUrl` accepts only HTTP(S); resource/media strings are data for an authorized client adapter,
not filesystem or network access granted by this library.

## Language and catalog

The credited upstream lexer/parser supplies state (`$name`), named values/functions,
components with positional/named arguments, lists/objects, templates, conditional and lazy
logical expressions, pure math/object functions, and whitelisted list/text methods. Named
functions can compose components and list callbacks execute inside the same step budget.
Function values are not exported to renderers. The catalog currently contains **67 components,
4 actions and 24 pure functions**, as data in [catalog-data.json](src/catalog-data.json).
Rust embeds that exact file; there is no separately maintained Rust vocabulary.

Each component has `accessibility.required`: an explicit list of required name/description
arguments. Inputs and interactive widgets require a label (or their existing title); images
require alt text; visual media/data components declare label and description where appropriate.
These arguments are required strings. Compilation rejects omission, null and blank literals;
evaluation checks resolved values again. An inaccessible component produces an Error node,
never an interactive component without its required accessible name. Platform keyboard,
VoiceOver, contrast and reduced-motion behavior still belong to the native implementations.

Positions count one-based UTF-16 code units, matching the upstream TypeScript syntax. Text is
JSON-compatible Unicode; unmatched surrogate code units produced by escapes, indexing or
slicing normalize to U+FFFD in both languages. Object enumeration follows ECMAScript integer-key
ordering. Equality preserves scalar and reference identity inside an evaluation. Nonfinite
numeric literals are rejected; nonfinite arithmetic yields null. Diagnostic wording is neutral
and deterministic; upstream fuzzy suggestions, unused-value warnings and prompt-generation
helpers are omitted. This is a Calternal description format, not a bb plugin compatibility layer.

## Limits and costs

| Ceiling | Default |
| --- | ---: |
| Source, statement AST, state, intermediate value and final snapshot bytes (UTF-8 JSON) | 262144 |
| AST expressions / constructed nodes / emitted node occurrences | 2000 each |
| AST, value and evaluator dependency depth | 64 |
| State declarations / supplied state entries | 256 each |
| Expression evaluation steps per complete pass | 200000 |
| Elements per list | 1000 |

Hosts can lower ceilings per call, never raise them. Limits are engineering bounds, not measured
frame-time budgets. Compiler limits precede activation; evaluator limits are rechecked on every
call, including lowered AST ceilings and externally supplied state. Reusing a cached node does
not bypass the final emitted-node count. External state must be finite, acyclic JSON with plain
objects and no interpreter tags or prototype-related names.

Compilation allocates tokens, syntax and local symbol/reference tables; cost grows with the
current source, AST and dependency graph only. Evaluation is bounded by expression visits and
value bytes; conservative intermediate-value checks can scan an existing value on each visit
(O(steps × bounded value bytes), plus local name lookups). State, a per-call named-value cache
and the output tree are allocated locally. There are no persistent caches, queues, locks,
filesystem writes, fsyncs or network round trips. Avoid calling a whole-tree evaluator per frame
or per keystroke; clients retain native control state and schedule these calls off hot paths.

## Reuse and deliberate exclusions

Source: [ComicBit/bb-plugin-genui](https://github.com/ComicBit/bb-plugin-genui/tree/d8cac8d1781bac336214cc38981aba4a58d9c1c0),
commit `d8cac8d1781bac336214cc38981aba4a58d9c1c0`. Upstream Apache-2.0 [LICENSE](LICENSE)
and [NOTICE](NOTICE) are preserved here, in the Rust crate and with the fixture corpus.

Dropped components: `File`, `Snippet`, `ImageSearch`, `Changes`, `Commits`, `PullRequest`,
`Threads`, `Tests`, `Json`, `Logs`, `StackTrace`, `Findings`, `Coverage`, `Dependencies`,
`Pipeline`, `Benchmark`, `Waterfall`, `Treemap`, `Environments`, `Scene`, `Shader`.

Dropped integrations: bb registration/server tools, React/DOM renderers, file targets and `Load`,
thread activity, project/environment lookups, report/file parsing and provenance loading,
markdown/HTML output, streaming partial-document rendering, interaction probes, form submission
assembly and agent prompt generation. Per-frame Scene/Shader callbacks are excluded pending a
native motion contract. Legacy ignored parameters are removed. Pure rich-data nodes remain
catalog descriptions; their renderer-specific normalizers and UI tests were not copied.

The extraction helper accepts the absolute path of the upstream `core/catalog.ts` and runs on
a build VM with Node 22.13+; it does not fetch the upstream repository. Review regenerated catalog
and accessibility fields before accepting a catalog change.

## Verification

Run only on an authorized dev VM, inside its lane directory:

```sh
nix develop -c npm install --ignore-scripts --no-audit --no-fund
nix develop -c npm test --workspace @calternal/genui
nix develop -c ./scripts/with-toolchain.sh cargo test -j4 -p calternal-genui
nix develop -c ./scripts/with-toolchain.sh cargo build -j4 -p calternal-genui --example conformance
nix develop -c node packages/genui/scripts/conformance.mjs
nix develop -c ./scripts/check-docs.sh
```

[Shared fixtures](../../contract/genui-fixtures/README.md) contain independent JSON expectations.
The direct comparison additionally checks complete ASTs and snapshots for every case.

## Questions for the user — proposals only

| Idea | Existing format | Options and costs |
| --- | --- | --- |
| Bind lists to data | `List(items)` accepts data; `.filter/.map` eagerly creates bounded descriptions. | Keep bounded snapshots: no new protocol, limited large-list support. Add keyed collection/resource bindings with paging/subscription: requires scoped data adapters, row identity and native virtualization contracts. |
| Complex parts as one native node | `Table`, `Calendar`, `Transcript`, etc. are single catalog nodes. | Compose existing nodes: less native work, limited specialist interaction. Add editor/thread/timeline nodes: requires versioned schemas, capability negotiation and implementations on each client. |
| Declared motion | No per-frame callbacks, Scene or Shader runtime. | Fixed motion presets: smaller validation and reduced-motion surface. Parameterized timelines: more expression/interpolation limits, native implementation and reduced-motion testing. |
| Components made of components | Named pure functions already return composed descriptions. | Keep local named functions: no new definition/lifecycle protocol. Add reusable versioned component definitions: needs parameter/schema, name scope, recursion, accessibility propagation and upgrade rules. |

These proposals add no enabled components or product decisions. Renderer fallback/version policy
and native implementation remain separate work.
