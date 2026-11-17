# Turn readiness and reasons

Forgejo issue [#39](https://git.kayg.org/kayg/calternal-ai/issues/39) · BB thread
`thr_2rbqpvhdbt`

The cached readiness read shows account-owned machine options and authorized
anonymous pool options. The existing Check operation refreshes a particular
owned machine or an anonymous pool placement. Saved turn reasons use the same
checkpoint projection in thread detail, list and journal delivery. The
account feed sends private readiness invalidations. Implementation commit
`5ead5df` is followed by the generated site pages and this verification record.

## Read and write costs

| Path | Reads | Durable writes |
| --- | --- | --- |
| Warm readiness | One existing capability lookup, one indexed space SQL statement covering plugin activation and any thread pin, and one indexed registry snapshot | None |
| Thread list | Existing bounded content index page and indexed job seeks in one statement, now selecting only checkpoint reason fields | None |
| Thread detail | Existing document and latest-job reads. No extra reason read | None |
| Turn reason publication | Existing outbox page of at most 64 rows and ordinary live item/authority checks | Outbox captured in the existing job commit. One existing journal append per event and one outbox retirement per page |
| Blocked placement | Existing lease check plus indexed candidates on each placement poll | One fenced checkpoint when the reason or queue position changes. Zero thread file work for a reason-only checkpoint |
| Check refresh | Existing bounded provider command through one fenced lease | Ordinary job and lease commits. Observation and private invalidation stay in memory |

Check observations expire after five minutes and are capped at 128 per machine.
The owner index uses one identifier per enrolled machine. Assignment counts are
kept under the same machine registry lock as the leases they summarize. Account
watch entries are weak and pruned by the sweep. Pending reason events follow the
existing seven-day outbox retention and journal receipt settings. Readiness adds
zero durable flushes on a cached read.

The lane uses fresh preview data for jobs schema version 2. Version 1 files are
refused at startup. Checks outside Calternal become visible when the provider
runs its next check. The current pool probe selects one eligible host.

## Verification

The verified base is `origin/dev` at `11d713f`, including People and Threads.
Commands ran inside `nix develop -c sh` in
`claude@calternal-dev-01-compute02:~/Developer/lanes/readiness` with
`CARGO_TARGET_DIR="$PWD/target"`, `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal"`
and `CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-e2e-fixture:dev`.

| Command in the development shell | Result |
| --- | --- |
| `cargo run --locked -j6 -p calternal-core --bin export-contract` | Generated the wire artifacts. |
| `npm run build:plugins` | Passed before Cargo tests. |
| `UPDATE_GOLDEN=1 cargo test --locked -j6 -p calternal-cli --test readiness --test threads` | Two goldens passed. The workspace suite verified the resulting snapshots again. |
| `cargo build --locked -j6 -p calternal-server -p calternal-cli --features test-plugins` | Passed. Used by conformance. |
| `cargo test --locked -j6 -p calternal-server --test provider_turns readiness:: -- --nocapture` | Passed. Covers saved reasons, Check refresh, a pinned wait and private journal delivery. |
| `cargo test --locked --workspace --all-features --no-fail-fast -j6` | 563 passed, 1 opt-in ignored. |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" target/debug/deps/real_server-12561f9524ee5da4 --ignored` | The opt-in real-server client test passed. |
| `cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 73 passed. Covers cross-account and cross-space hiding, entitled pool visibility and grant withdrawal, private live frames, and capability enforcement. |
| `npm run test:integration` | 1 passed. |
| `npm run test:plugins` | 22 passed, 1 signed-in Claude opt-in skipped. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed. |
| `python3 contract/verify-cli.py` | Real-server CLI smoke passed. |
| `env CARGO_TARGET_DIR="$PWD/readiness-gates/docs-validation-target" sh scripts/check-docs.sh` | Passed. |
| `cargo fmt --all -- --check` and `rustfmt --edition 2024 --check tests/conformance/agent_cases.rs` | Passed after formatting the touched Rust files. |
| `npm ci --prefix apps/docs --ignore-scripts --no-audit --no-fund` | Passed. |
| `node scripts/docs-site/generate.mjs` and `node scripts/docs-site/generate.mjs --check` | Generated and checked 292 pages. |

The formatter used the NUL-delimited list of changed paths. On the VM it ran
`tr '\000' '\n' < readiness-gates/final-changed.list | sort -u | grep '\.rs$' | xargs -r rustfmt --edition 2024 --config skip_children=true`.

After the gates the lane verified that its 52 recorded Podman container names
were absent and removed `target`, `tests/conformance/target`,
`reference-docs/rust` and `readiness-gates/docs-validation-target`.

A fixture caught a malformed SQL literal in the waiting transition. The
corrected transition records the request ID and waiting state together. The
preliminary isolation fixture tried to change a role before a membership
existed. It now uses the fixture's preprovisioned Editor membership. An earlier
docs run reused stale Cargo metadata after a tar overlay preserved source
timestamps. A clean docs target passed. The final source was extracted with
`tar xmf` before reusing the lane targets.

## Review gate

| # | Verification |
| --- | --- |
| 1 | Reused account and space IDs, UUIDs, error/capability types and time/hash helpers. Provider events and readiness share one Check payload type. |
| 2 | Added types to core and implementation to existing server/CLI modules. No crate or dependency was added. |
| 3 | Reason reads use the existing per-space connection. The short machine snapshot uses the existing installation-owned registry lock because enrollment and placement are installation-scoped. No database I/O or await occurs during that snapshot. Existing durability lock costs remain documented in the machine service. |
| 4 | SQLite, registry access, credential files and provider process work stay on their existing blocking or machine paths. |
| 5 | The path counts above use the existing document/job reads. List and journal parse only the reason projection. |
| 6 | The catalog is cached. The outbox schema and list statement are initialized once and SQLite caches prepared statements. |
| 7 | Check cache, subscriptions and outbox retention are bounded as above. Probe workspaces use ordinary fenced teardown and existing receipt retention. |
| 8 | Fresh preview jobs schema 2, with no legacy reader or migration path. |
| 9 | Space and thread selectors are UUIDs. Account and pool placement reuse enrollment authority. |
| 10 | The catalog owns the operation ID and schemas. Server policy derives the required capability and space scope. The reason module owns the checkpoint projection. |
| 11 | Wire types are used by the generated operation contract, machine registry and read path. The CLI, journal and account feed all call wired paths. |
| 12 | Reused the existing jobs checkpoint and outbox, agent Check event, fenced lease, SQLite content index and signed account feed. The reference app's ADR 0012 and live route were consulted read-only. |
| 13 | Public API docs state meaning and costs. |
| 14 | Thread, provider and CLI reference docs and docs-site sources cover the new fields and commands. |
| 15 | The placement wait and progress writes are factored out of the provider reader. |

## Open decisions

The options and costs for polling provider status and probing all pool hosts are
in [morning decisions](../morning-decisions.md#turn-readiness-issue-39).

## Simplification opportunities

- The placement reason and the run view can share a single registry call for
  blocked turns. That would eliminate the second short lock on each placement
  poll while keeping the public machine view untouched.
- The jobs outbox could share an idempotent publisher primitive with other
  journals once their retention and crash behavior align.
