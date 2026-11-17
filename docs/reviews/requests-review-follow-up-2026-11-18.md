# Requests review follow-up, 2026-11-18

[Lane issue #32](https://git.kayg.org/kayg/calternal-ai/issues/32) ·
[Independent review](requests-independent-review-2026-11-18.md)

Code changes are in `6dc3d9d`. Commit `db39e13` fixes the provider resume
fixture's checkpoint wait. The branch carries the review's test and report
commits, cherry-picked as
`e731bd8` and `318f0c2`, and merges `origin/dev` at `f3c359c`, `f56c786`, and `a7a5275` in
`25ce321`, `4773a8e`, and `1b706d1` respectively.
The three findings reproduced before the changes. Space deletion already passed.

- An approval for an irreversible or access-widening action only accepts
  `once`. The protected admission transaction spends it before dispatch,
  including for writers without an operation-specific receipt. A stored
  response can answer an identical retry. After an unconfirmed failure or
  receipt expiry, execution requires a new approval. The review's original
  in-process test now asserts the explicit lifetime restriction before
  checking for authority in the next turn. Conformance also exercises a
  successful retry and a failed action on the same unreceipted writer.
- Stop and thread lifecycle transitions cancel pending requests. A decision
  must match the exact job's waiting checkpoint. Approval revokes that job's
  parked credential, and the wake issues a new credential. The stale approval
  case checks that the successor stays running with a working credential.
- Leaving a space, removal, or losing space read access erases that member's
  protected requests and saved thread access in the membership transaction.
  The jobs path interrupts that member's active turns. The conformance cases
  inspect the protected rows after leaving, removal, and whole-space deletion.

## Review gate

| Item | Evidence |
| --- | --- |
| 1. Shared types | Uses existing account and space IDs, operation risk, errors, fingerprints and capability declarations. |
| 2. Dependencies | Existing core, auth, jobs, content and machine boundaries. No dependency added. |
| 3. Locks | The existing protected auth worker remains the recovery barrier. A per-space jobs lock orders stop, current-request resolution and once admission. The per-turn async lock serializes the action and response publication. |
| 4. Async | Protected auth, SQLite, receipt reads, hashing and synchronous directory work run in blocking tasks. The per-turn mutex is async. |
| 5. Reads and flushes | Once dispatch takes one existing credential recheck, one journal receipt lookup, and one indexed running-job check. The protected admission reads the token and prior claim, then conditionally updates the matching grant with one protected commit. Success adds one journal publication for the response. The decision path uses one route lookup, one space capability check, one waiting-checkpoint read, and a protected transaction which reads the decision row and checks receipt identity before updating the decision and credential. Wake checks the parked state and rechecks scheduling state after the conditional update. Wake and feed publication each have their own conditional commit. |
| 6. Constant work | Existing cached schemas and policies remain. Lifetime eligibility is one match on the shared risk class. |
| 7. Retention | 32 request/grant rows per thread and 256 per account. Closed rows and spent claims age out on later requests after the operator receipt window, seven days by default. Pending requests keep the owner's no-expiry rule. Archive and delete erase grants. |
| 8. Compatibility | An auth schema bump uses the existing disposable-development schema path. No compatibility reader. |
| 9. IDs | Space targeting uses UUIDs. Membership cleanup keys rows by account and space. |
| 10. One declaration | Lifetime eligibility comes from the shared risk class. The operation-specific once allowlist is gone. |
| 11. Wiring | Decisions, command dispatch, lifecycle transitions, protected membership mutation, and the terminal jobs module all have production callers. |
| 12. Reuse | Uses the protected recovery transaction, job checkpoint, current run guards, and existing per-space journal receipts. |
| 13. Reference docs | Doc comments describe terminal job ordering and the cost of protected and journal writes. |
| 14. Matching docs | The [request contract](../requests.md), [instruction policy](../agent-instructions.md) and [turn credential table](../agent-turns.md) describe the implemented behavior. |
| 15. Formatting and size | Terminal job transitions have their own module. Workspace and stand-alone conformance formatting are checked separately. |

## Remaining work and decisions

- Rotating a live credential at once consumption still needs a delivery
  mechanism. Same-machine restart reuses bootstrap at the cost of interrupting
  the tool and coordinating replay. A sandbox-protected file or socket
  preserves the process and needs a provider and CLI credential protocol.
- Binding an unused once approval to its job expires it when that job stops.
  The current thread binding avoids a repeat prompt on a successor but
  carries the approval across a turn or mode change.
- Per-account-per-thread and per-space sublimits would reduce starvation at
  the cost of additional counts. Independent grant quotas would keep
  long-lived grants from filling the pending-request budget and require an
  additional retention policy.
- Keeping direct `AI/AGENTS.md` reads Manager/Owner-only retains the existing
  permission boundary. Allowing `space.read` would match the context handed
  to members' agents and widen direct access.
- A write that succeeds before its once response receipt commits may have an
  unknown outcome for the caller. The spent approval fences another
  execution. A source-store outbox or external outcome lookup is required
  to recover the response across that gap.
- Account-feed invalidation still follows the protected transaction.
  An outbox would make it recoverable across a crash and add a write.
  An authoritative snapshot on reconnect would defer notification on an
  already open stream. The web adapter remains with the UI lane.

## Validation

Runs used `claude@calternal-dev-01-compute02` and
`~/Developer/lanes/requests`, with `nix develop -c`. The source archive
used `git ls-files -co --exclude-standard -z | tar --null --ignore-failed-read -T - -cf /tmp/requests-review-merged.tar`
and transferred through SSH. The final retry extracted with `tar xmf -` to
refresh VM modification times. After the upstream plugin rename, the lane
removed the old plugin copies from its VM directory and ran
`npm ci --ignore-scripts --no-audit --no-fund`. The gate built the plugins
before Cargo tests.

| Command in the dev shell | Result |
| --- | --- |
| `npm run build:plugins` | Passed before the Rust gates and workspace retry. |
| `cargo test --locked -j6 --workspace --all-features --no-fail-fast` | 541 passed, one failed and one ignored on the merged production source. The first retry also hit the VM keyring quota. |
| `cargo test --locked -j6 -p calternal-server --all-features --test provider_turns -- --test-threads=1` | 4 passed after fixing the resume fixture. |
| `cargo test --locked -j6 --workspace --all-features --no-fail-fast -- --test-threads=1` | 536 passed, six failed, one ignored. The dev VM keyring quota blocked all six container starts. |
| `cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed. |
| `CALTERNAL_SERVER_BIN="$PWD/target/debug/calternal" cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml -- --test-threads=1` | 62 passed, zero failed. All seven focused request cases are included. |
| `npm run test:integration` | 1 passed, zero failed. |
| `npm run test:plugins` | 22 passed, zero failed. One signed-in provider opt-in skipped. |
| `cargo run --locked -j6 -p calternal-core --bin export-contract -- --check` | Passed after rebuilding the exporter. |
| `scripts/check-docs.sh` | Passed. Rust, TypeScript and GenUI documentation checks completed. |
| `cargo fmt --all -- --check` | Passed. |
| `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Passed. |

The first full workspace run failed
`expired_control_receipt_allows_a_new_request_after_restart`. The receipt read
can race the journal writer's startup pruning. The first run after merging
`f56c786` passed 541 tests, with one ignored and one failure in
`fake_official_cli_streams_checkpoints_crashes_stops_and_requires_explicit_resume`.
That fixture sampled the old machine checkpoint after the new run state was
already marked running. The request regressions passed in both full runs.
After the fixture fix, the provider suite passed. The dev account reached
200 of 200 kernel keyrings. Podman then failed before container startup with
`crun: join keyctl: Disk quota exceeded`, including in the latest serial retry. Its six failures were in
`calternal-machine` targets `plugin_workers` and `podman`. The provider
fixture and the expired-receipt fixture passed on the latest merge.

The first contract check used a cached exporter built before the upstream
personal-space field. Updating the VM source timestamp forced recompilation
and the check passed. Logs are under
`/tmp/requests-review-{full,final,retry,serial,merged}-*.log` on the dev VM.

Cleanup used `rm -rf -- target tests/conformance/target reference-docs/rust`
in the VM lane directory and checked that all three paths were absent.

## Simplification opportunities

- Once eligibility no longer needs a per-operation allowlist. The risk
  class controls retained approval, and every writer shares the spend path.
- Stop, native completion, and supersede reuse protected pending-request
  cancellation. Explicit rejection of stale decisions replaces the old
  successor-interrupt logic.
- A typed secret recipient column would eliminate text parsing on every
  inbox listing. That changes the independent secrets store schema.
- Decision handling resolves the owned route once and checks the exact
  waiting job before changing authority.
