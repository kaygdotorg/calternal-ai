# Luna plugin author trial and follow-up

Four completed transcripts were read through their BB command events.
The independent verifier passed the reading list and checklist on their
first attempts and the composer chip on its repeat. The first chip
verification failed its combined default-and-change criterion. The
trial failed the friction criterion because every task needed failed
commands and product-source lookup.

| Run | Verifier | Completed command events | Nonzero exits |
| --- | ---: | ---: | ---: |
| 1, reading list | 6/6 | 49 | 11 |
| 2, checklist | 7/7 | 49 | 6 |
| 3, composer chip | 5/6 | 21 | 8 |
| 3b, composer chip repeat | 6/6 | 25 | 8 |

Counts are command item IDs in completed events. The coordinator's
checklist count was 48. Shell batches sometimes exited zero after an
earlier command printed an error, so the exit totals undercount the
friction. These are author workflow observations, with no causal timing
claim from the development machine.

| Defect and transcript evidence | Change |
| --- | --- |
| The guide call omitted `space_id` in runs 1, 2 and 3b. Reading-list calls repeated it. Run 2 also sent the unsupported `space_id` to `jobs.get`. Generic schema errors suggested inspecting a turn. | Plugin schema failures now disclose the JSON pointer and an expected type or constraint, omitting submitted values. Scope and job lookup checks name their fields. CLI input errors point to the schema and preserve machine-readable details. The skill shows which inputs include a space. |
| Run 1 saw `ERR_MODULE_NOT_FOUND` for esbuild. Run 2 saw a closed fixture subprocess. Run 3 saw an invocation failure from a thrown handler error. All got the dependency-install hint. | CLI distinguishes missing tool, missing npm dependency, compiler failure and fixture failure. Fixture subprocess failures include the exit status or signal and bounded stderr. The guide explains invocation failures and callback stubs. |
| Runs 1, 3 and 3b searched for Node, npm or the CLI and tried incorrect absolute paths or shells. Missing tools also appeared after changing to a fresh SSH shell. | Kit emits absolute tool paths in its env. The skill loads the env in each shell before commands and explains bootstrap without the kit. Scaffold prints its next commands plus its actual worker and test paths. |
| Run 1 tried `src/worker.ts` and a `test/` directory. Run 3 scaffolded under its SSH home before moving into the assigned project. Run 3b also looked for a `src/` entry. | The skill starts in the assigned project and names the root-level files and the generated manifest. |
| Runs 1 and 2 inspected the SDK's typed handler source after TypeScript rejected valid JSON Schema shapes. Run 1 also used the wrong handler parameter for context while editing. | The reference describes the two handler arguments, the literal inference subset and the plain worker-handler fallback for wider schemas. |
| Run 1 inspected the broker, contracts, SDK, notes and guide worker sources for item reads, writes, pagination, titles and generated lists. It searched GenUI sources for names and labels. | The reference documents exact broker payloads and return fields, the request and kind scopes, and usable `List`, `Text`, `Row`, `Stack`, `Metric` and `Badge` signatures. |
| Run 2 read the jobs fixture, broker, scheduler, runtime and worker session for the start call, run envelope, checkpoint, streams and results. Its first job failed because its schema allowed only an object for a checkpoint that starts as null. It found the generic busy message while inspecting the worker session. | Document the run envelope, null checkpoint, handler role, job lookup and SSE. A schema failure is distinct from saturation. A busy reply now has its own reason and retry guidance. The existing server log identifies schema-rejected job runs by operation and caller-safe field diagnostic. |
| Runs 3 and 3b read SDK fixtures and worker declarations for settings callbacks, escaped generated text, and the composer slot. Product source in the kit included the near-identical composer fixture. | The public guide covers the slot and setting contract. The refreshed kit contains only public SDK artifacts, guide documents and the seed metadata and bundles needed to serve it. It excludes product source and the composer fixture. |
| Run 3 passed an empty `--appdata` and got a clap message without the supplied env-file context. | Development flags also read the trial's `APPDATA`, `SPACE_ID` and `ACCOUNT_ID`. Missing or empty values identify the required flag or variable and direct the author to load the environment. |
| Runs 1, 3 and 3b had SSH or here-document quoting errors. | The skill explains how to send multiline source to a remote shell without local expansion. |
| Run 3 left `Pinned label ✓` in the setting. Its transcript shows that exact label rendering successfully before teardown. The original verifier also passed a recreated fixture with that stored value and with a stored value containing both quotes and ✓. | The original failure remains unattributed. The verifier checks the current edit base, explicitly rejects conflicts, compares decoded view string literals, checks a marker containing both quotes and ✓, and reports the stage of a failure. It restores the prior override if it still owns the test edit. |

## Next gap

Readiness, logs and timing for plugin authors is the next priority.
The planned contract and proof are on issue #50 for review before
implementation. The initial scope is one package revision's readiness,
bounded author diagnostics and phase timings with the same activation
and disclosure rules for every publisher.

## Verification

The lane includes `origin/dev` at `a7cd5dc`. Build and test commands ran
under `nix develop -c` in the dev-02 lane, with four build jobs. The first
workspace run hit a fixture port collision. The machine test picked a free
port, then its server exited because the address had been taken before
bind. The isolated machine case passed, and the serialized workspace
rerun passed.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Passed before Rust tests. |
| `cargo build --locked -p calternal-server --features test-plugins -j4` | Passed. |
| `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal cargo test --locked --manifest-path tests/conformance/Cargo.toml -j4 -- --test-threads=1` | 125 passed, including field diagnostics and no input-value echo. |
| `cargo test --locked -p calternal-server --all-features -j4 --test machines other_accounts_cannot_use_see_or_touch_a_machine -- --test-threads=1` | Passed on its own. |
| `cargo test --locked --workspace --all-features --no-fail-fast -j4 -- --test-threads=1 --skip pool_machines_serve_only_granted_accounts` | 604 passed. The Podman-only case belongs to dev-01. |
| `cargo test --locked -p calternal-plugin-host -j4` | 18 contracts and one manifest test passed. |
| `cargo test --locked -p calternal-server --all-features -j4 --test plugin_host -- --test-threads=1` | 18 passed. |
| `npm run test:integration` and `npm run test:plugins` | Two integration and 29 plugin tests passed. The real signed-in provider test is opt-in. |
| `cargo run --locked -p calternal-core --bin export-contract -j4 -- --check` | Passed. |
| `cargo build --locked -p calternal-cli -j4` and `python3 contract/verify-cli.py` | Passed. |
| `CARGO_TARGET_DIR=target npm run docs:site` and `scripts/check-docs.sh` | Passed with 344 generated source pages. |
| `cargo fmt --all -- --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Passed. |
| `python3 scripts/check-plugin-parity.py` | The established 45 source references remain. |

On dev-01, the isolated kit refresh ran `npm run build:plugins` and
`cargo build --locked -p calternal-server --features dev-plugin-drafts -j4`.
`~/plugin-trial/smoke.sh` exercised all three negative verifier paths,
tool discovery, missing space and missing appdata diagnostics, and leak
checks. A separate run-0 proof used the curated SDK to scaffold, install
dependencies, check, test, live-load and invoke an ordinary starter.
The revised verifier passed all chip checks, including an edited label with
quotes and Unicode and preservation of its original stored override.
The build snapshot and composer fixture were removed from the kit after
the proof. No model was used for either proof.

## Review gate

| Item | Evidence |
| --- | --- |
| 1 | Reuses the shared caller-safe error and space IDs. The diagnostic separates protocol failure from input rejection. |
| 2 | Uses the existing JSON Schema and Node process interfaces. No new dependency edges. |
| 3 | No new global worker or lock. Diagnostics are produced within the existing worker admission and local CLI paths. |
| 4 | Existing worker threads and CLI subprocess handling own blocking I/O. |
| 5 | The worker performs one cached input-schema validation. Diagnostics are built on failure. The change adds no persisted reads or flushes. |
| 6 | Uses the same validators compiled once per manifest revision. |
| 7 | The pointer and local stderr capture are bounded. No additional stored state or retention. |
| 8 | No alternate wire format, fallback parser or ignored compatibility test. |
| 9 | Space routing continues to use the admitted UUID and live grants. |
| 10 | A shared busy diagnostic covers the genuine admission bounds. Schemas remain authoritative. |
| 11 | The server and CLI use the new diagnostics. Tests exercise the public worker and fixture paths. |
| 12 | Reuses the existing admission path, error envelope, generated discovery and local fixture host. |
| 13 | Documents the runtime diagnostic and fixture capture in Rust and TypeScript. |
| 14 | Includes regenerated reference pages and the rebuilt guide bundle pin. |
| 15 | Rust formatting passed. Schema diagnostic formatting lives in its own module. |

## Simplification opportunities

Generate more of the author reference directly from the runtime broker and
GenUI contracts when those schemas can express the existing payloads.
The trial shows which examples need to stay easy to find.
