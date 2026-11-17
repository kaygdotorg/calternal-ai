# Plugin manifest and host library verification — 2026-11-17

Library work is complete within this lane's ownership. OS confinement is not
implemented or certified. Server wiring was done afterwards; see the next section.

## Server wiring — 2026-11-17 (thread thr_shzpwcep67, branch `lane/plugin-host`)

The server now loads manifests through this registry, derives its plugin policy
rows from them, and runs long-lived protocol-v2 workers per (plugin, space) with
brokered host calls (`crates/server/src/plugins/`, summarized in
`crates/server/README.md`). notes-insights returns its portable view again and
reads `text` from `read_item`; the Notes manifest is v1. The "Unfinished
integration" items below about the server loader, launcher and pin are done;
OS confinement, scheduler, webhooks, themes, jobs and secrets are still not.

All runs on `claude@calternal-dev-01-compute02` in `~/Developer/lanes/plugin-wiring`
(the aarch64 rebuild on `ubuntu@calternal-dev-02-std`):

| Command | Result |
| --- | --- |
| `nix develop -c cargo test -j6 --workspace --no-fail-fast` | all pass (server: 8 new `plugin_host` tests; ported `race`, `authz_matrix`, `http`) |
| conformance command from `tests/conformance/README.md` (`--test-threads=1`) | **27 passed** |
| `nix develop -c npm run test:integration` | 1 passed |
| `nix develop -c npm run test:plugins` | SDK 14 + notes-insights 2 passed |
| `nix develop -c ./scripts/check-docs.sh` | pass |
| clean rebuild (`rm -rf node_modules */dist`; `npm ci`; `npm run build:plugins`) ×2 on x86_64, ×1 on aarch64 | identical `4679760879291e11ed71c4371073bc1c43b01bff84d2b53bcb3466968737a922`, pinned; `calternal plugin-digest` reports `pinned` |

`tests/verify-v1-worker.mjs` was not rerun: it needs the hand-placed legacy source
described below and proves only the retired v1 path.

## Changed paths

- `crates/plugin-host/`: library manifest, documented registry, distinct canonical
  UUIDv7 account/space types, bounded JSON codec, worker session, tests and reports.
- `crates/core/src/plugin_manifest.rs`, registration in `src/lib.rs` and
  `src/bin/export-contract.rs`: strict manifest v1 DTOs and generator registration.
- `contract/plugin-manifest-v1.schema.json`, `packages/contracts/src/index.ts`:
  regenerated manifest schema and TypeScript declarations.
- `packages/plugin-sdk/src/{index,manifest-v1,worker-v2}.ts` and `test/`: data-only
  menu validation, long-lived bidirectional worker peer, shared wire vectors,
  and preserved legacy fixture coverage for the unchanged preview callers.
- `plugins/notes-insights/{manifest.json,src/worker.ts,tsconfig.json,test/worker.test.mjs}`:
  manifest v1, strict statistics schemas and long-lived broker-backed worker v2.
- Root `Cargo.toml`: this lane added only the `calternal-plugin-host` workspace
  dependency entry; other concurrent entries belong to their lanes.

Kind declarations include visible/hidden placement and claimed-text selection of
userdata-ssd versus userdata-hdd. Visible names reject non-NFC, separators,
leading dots, controls, reserved names and cross-plugin folded-name collisions.
Hidden placement is `.calternal/<plugin-id>/<kind>/` inside the selected space
userdata tree. appdata remains host database/state storage.

## Exact validation commands and results

All build, test, generation and formatting commands ran on
`ubuntu@calternal-dev-02-std`, after `cd ~/Developer/lanes/plugin-host`, reached by
`ssh ubuntu@calternal-dev-02-std '<command>'`. No build/test ran on hd01.
Initial sync used the requested command:

```sh
cd /home/kayg/Developer/Calternal-AI
git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf - | ssh ubuntu@calternal-dev-02-std 'mkdir -p ~/Developer/lanes/plugin-host && cd ~/Developer/lanes/plugin-host && tar xf -'
```

Owned files were subsequently tar-synced. Only owned source, formatting and
contract artifacts were copied back; no lockfile or built bundle was copied back.

| Exact command in the VM lane directory | Final result |
| --- | --- |
| `nix develop -c ./scripts/with-toolchain.sh cargo test -j4 -p calternal-plugin-host` | Pass; generated VM root lockfile, before final added regressions. |
| `nix develop -c ./scripts/with-toolchain.sh cargo test --locked -j4 -p calternal-plugin-host` | **16 passed, 0 failed, 0 ignored**; doc tests also pass. |
| `nix develop -c npm test --workspace @calternal/plugin-sdk` | **14 passed, 0 failed**; includes six preserved preview tests and eight new tests. |
| `nix develop -c node crates/plugin-host/tests/verify-v1-worker.mjs` | Pass; committed v1 worker still returns v1 statistics and GenUI using the updated SDK. |
| `nix develop -c npm test --workspace @calternal/notes-insights` | **2 passed, 0 failed**; real Node worker, multiple invocations per process and malformed/oversized frames. |
| `nix develop -c ./scripts/with-toolchain.sh cargo run --locked -j4 -p calternal-core --bin export-contract` | Pass; regenerated artifacts. |
| `nix develop -c ./scripts/with-toolchain.sh cargo run --locked -j4 -p calternal-core --bin export-contract -- --check` | Pass; artifacts match Rust models. |
| `nix develop -c env RUSTDOCFLAGS=-Dwarnings ./scripts/with-toolchain.sh cargo doc --locked -j4 -p calternal-plugin-host -p calternal-core --no-deps` | Pass; no warnings in owned crate/core documentation. |
| `nix develop -c ./scripts/check-docs.sh` | Pass; existing script does not yet list plugin-host, so the strict supplemental crate check above is necessary. Unrelated content snapshot had an unused-import warning. |
| `nix develop -c npm run check --prefix scripts/docs-tools` | Pass; SDK and worker reference coverage/links. |
| `nix develop -c ./scripts/with-toolchain.sh cargo test --locked -j4 --workspace` | **Failed, exit 101 before tests**: the concurrently written GenUI snapshot had syntax errors in `evaluate.rs:169` and `lexer.rs:65`. No full-workspace pass is claimed. |

Owned Rust files were formatted with `nix develop -c rustfmt --edition 2024
--config skip_children=true <owned-files>`. The explicit file list comprised the
five `crates/plugin-host/src/*.rs` files, `tests/contracts.rs`, and the three owned
core registration/model/export files. `git diff --check -- Cargo.toml
crates/core/src/lib.rs crates/core/src/plugin_manifest.rs
crates/core/src/bin/export-contract.rs contract/plugin-manifest-v1.schema.json
packages/contracts/src/index.ts crates/plugin-host packages/plugin-sdk
plugins/notes-insights` passes as a static hd01 check.

Earlier failures were corrected: strict unit-message unknown-field rejection
(5 passed/1 failed initially), ambiguous legacy/new validator imports, legacy SDK
tests reading the newly ported menu, TypeDoc inline union/re-export comments,
and two Rustdoc HTML warnings. New typed-schema/secret/cancellation regressions
remain enabled. No test was ignored to obtain a pass.

The initial workspace snapshot could not load because GenUI had no target yet.
A temporary two-crate workspace under `crates/plugin-host/.verification` in this
VM copy allowed early checks with
`nix develop -c ./scripts/with-toolchain.sh cargo test -j4 --manifest-path
crates/plugin-host/.verification/Cargo.toml -p calternal-plugin-host`.
Later final checks above used the actual root workspace. No other lane directory
was deleted. An initial `nix develop -c npm ci --ignore-scripts --no-audit
--no-fund` failed because the concurrent GenUI package was absent from the shared
lockfile; `nix develop -c npm install --ignore-scripts --no-audit --no-fund`
regenerated the VM npm lockfile successfully. Documentation tools were installed
with `nix develop -c npm ci --prefix scripts/docs-tools --ignore-scripts
--no-audit --no-fund`. No lockfile was copied back.

## Committed worker-v1 compatibility and coordinated wiring

The existing SDK v1 exports and wire types were retained. To verify the committed
worker rather than only the parser helpers, its source was read from git commit
`b4a126d2c18d716c6ea94f7f405c8fe1c56190cf` on hd01 and placed in the VM copy only:

```sh
git show b4a126d2c18d716c6ea94f7f405c8fe1c56190cf:plugins/notes-insights/src/worker.ts | ssh ubuntu@calternal-dev-02-std 'cd ~/Developer/lanes/plugin-host && cat > plugins/notes-insights/src/legacy-worker-verification.ts'
```

Then, from the VM lane root:

```sh
nix develop -c npm exec --workspace @calternal/notes-insights -- tsc --noEmit -p tsconfig.json
nix develop -c node crates/plugin-host/tests/verify-v1-worker.mjs
```

Both pass. The smoke test bundles that original source against the updated SDK
and checks actual protocol-v1 invocation identity, statistics and portable GenUI.
The original server digest pin was not edited. An initial esbuild CLI attempt
failed with an architecture error, and a root-level JS import failed because
esbuild is installed in the Notes Insights workspace. The final test uses that
workspace's esbuild JS API, as the existing plugin build does, and passes.

The coordinated wiring change must update `crates/server/Cargo.toml`,
`crates/server/src/plugins.rs` (registry, persistent worker transport and broker),
`crates/server/src/api.rs` (dispatch/delivery), `crates/server/src/authz/mod.rs`
(manifest policy rows), and `crates/server/trusted-plugins.json` (the rebuilt v2
bundle digest, in the same change). Its plugin discovery/result contract must
also switch in `crates/core/src/{models.rs,validation.rs}` and regenerate through
`crates/core/src/bin/export-contract.rs` into `contract/v0.schema.json`,
`contract/operations.json` and `packages/contracts/src/index.ts`. Update the
corresponding server/integration tests and generated root lockfile together;
coordinate client contract consumers with their owning lanes. Do not repin a v2
bundle while the launcher still speaks v1.

## Dependency additions

No npm dependency was added. New host crate dependency declarations:

| Dependency | Declaration | VM resolved version |
| --- | --- | --- |
| calternal-core | workspace path | 0.1.0 |
| calternal-fsio | workspace path; reused NFC/folding | 0.1.0 |
| serde | workspace `1` | 1.0.229 |
| serde_json | workspace `1` | 1.0.154 |
| thiserror | workspace `2` | 2.0.21 |
| uuid | workspace `1` | 1.29.2 |
| [jsonschema](https://docs.rs/jsonschema/0.61.2/jsonschema/) | `=0.61.2`, `default-features = false` | 0.61.2 |

The parent must merge generated locks. JSON Schema HTTP/file reference retrieval
is disabled; explicit external-reference regressions pass.

## Quality

- Modules separate identity, bounded serialization, registry and session lifecycle;
  the SDK separates menu validation from worker transport. Host effects use the
  `HostBroker` trait; tests provide fake brokers and a real Node peer.
- Registration compiles schemas and probes contribution indexes in
  O(new declarations * log registered IDs), allocating only new index keys.
  Grant inspection borrows in O(log plugins); exporting all policy rows costs
  O(all operation/schema bytes), with owned row allocations.
- Dispatch/callbacks cost O(frame bytes + compiled-schema evaluation + bounded-map
  lookup). Budget checking allocates no output buffer; actual encoding allocates
  one bounded buffer. No library-owned locks, filesystem I/O, fsync or network
  connection occurs. A callback has one broker effect/round trip; trusted adapters
  determine storage/transaction costs and recheck live authority atomically.
- Limits: 256 KiB frames/manifests, 32 active invocations, 32 queued frames per
  direction, 128 callbacks per invocation and 4096 lifetime invocation IDs before
  worker rotation. Chunk sequencing and admitted job handles are checked.
- Cancellation invalidates authority even if unrelated messages fill the output
  queue; inability to send cancel requires the supervisor to stop the worker.
  Queued replies/results recheck current grants; callback replay is refused.
- Schema evaluation/compilation complexity still depends on supplied schemas;
  deployment resource ceilings remain required before hostile plugin admission.

## Unfinished integration and limits

- Server wiring is deliberately absent. Its old loader skips the new Insights
  menu; its one-shot worker launcher and trusted digest are unchanged. The new
  backend returns statistics, without an inline GenUI tree or a new UI component.
- The parent must add `-p calternal-plugin-host` to both Cargo invocations in
  `scripts/check-docs.sh`; that script is outside this lane's ownership. Equivalent
  strict crate documentation checks passed here.
- No OS sandbox/process supervisor, scheduler, webhook route, theme-content
  sealing, durable job/receipt store or Collection-provisioning side effect is
  implemented. The broker adapter must enforce current method/target grants,
  scoped opaque handles and atomic effect checks. Secrets never enter menus.
- The single-item Insights read is bounded by the 256 KiB frame budget. Larger
  content needs a paged/handle-based host read contract or a revised transport
  budget before claiming support for the preview's entire document-size range.
- Native components and trusted grant surfaces belong to their respective lanes.
  Themes/settings/schedules/hooks here are declarations, never an activation bypass.
- Full workspace verification remains unfinished because of the concurrent GenUI
  compilation failure. These development checks are not performance benchmarks.

## Questions for the user

No answer blocks the completed library. Before schedules are activated, which
schedule contract should be supported: calendar cron with an IANA timezone
(requires DST/calendar semantics and their tests), elapsed intervals (simpler
scheduler, cannot express local calendar times), or both (larger contract and
validation surface)? The manifest currently retains a bounded opaque expression;
it does not select or execute any of these scheduling policies.
