# Provider permission-mode correction, 2026-11-17

Issue [#16](https://git.kayg.org/kayg/calternal-ai/issues/16), thread
`thr_y4g4pnn3b5`. This corrects finding 2's overly broad “every mode” wording in
[the preceding qualification](provider-review-fixes-2026-11-17.md).
The owner decision in [agent instructions](../agent-instructions.md#access) governs.

Full access permits irreversible and external actions without an approval.
Approve for me permits read/reversible operations, requires an exact external
scope and either an exact irreversible grant or advice from its irreversible-only
reviewer hook. The current rules-only reviewer asks. Least access requires the
default scope or a grant for read/reversible operations and an exact grant for
irreversible/external operations. App grants never substitute for those exact
sensitive-action grants in either approval mode. Access widening keeps its
existing behavior: mode alone refuses it; the exact-action path is unchanged.

The authority check passes only exact grants for sensitive effects into the
shared matrix. There are no discarded parameters: the reviewer, operation and
input are used by the Approve-for-me irreversible branch. The Full access row and
explanatory text in `docs/agent-turns.md`, the shared mode comment and generated
contract agree. Two regressions cover the authority boundary and reviewer matrix;
the six original review regressions remain in the affected suites.

## Qualification

All commands run only on `claude@calternal-dev-01-compute02`, inside
`~/Developer/lanes/provider-claude`, through `nix develop -c`. The worktree was
synced using the previously recorded tracked/unignored tar-over-SSH command.

| Command | Result |
| --- | --- |
| `npm run build:plugins` | Passed before Cargo tests |
| `cargo fmt --all` | Passed; formatted files copied back |
| `cargo run --locked -j6 -p calternal-core --bin export-contract` | Passed; generated contract copied back |
| `cargo test --locked -j6 -p calternal-server --lib --test provider_review --test provider_review_stream --test provider_access --test provider_security --test provider_turns --no-fail-fast` | 19 passed, including both new regressions |
| `cargo test --locked -j6 -p calternal-machine --lib --test podman_exec_env` | 12 unit tests and 1 independent argv regression passed |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 47 passed |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed; artifacts match |
| `scripts/check-docs.sh` | Passed |
| `cargo fmt --all -- --check` and `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Both passed |

This is affected-suite qualification plus conformance, as requested; workspace,
plugin and integration gates from the preceding report were not rerun. No real
Claude account was used. The fifteen-item review assessment is unchanged by this
bounded in-memory policy correction: no new type, dependency, storage, I/O, lock,
read or flush is introduced. Existing authority and operation-risk primitives
are reused; comments and the generated contract change together. Owner Open
items remain unchanged; no new question needs a decision.

The worktree and VM matched across 1,042 SHA-256 file comparisons, excluding this
subsequently written report. After qualification, the lane's `target` (7.3 GiB),
standalone conformance `target` (1.8 GiB) and `reference-docs` (878 MiB) were
removed; their absence was verified.

## Simplification opportunities

The authority boundary now supplies grants to the existing risk matrix instead
of maintaining its own irreversible/external decision branch. The separate
exact-action widening path preserves the owner's existing rule.
