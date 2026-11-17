# Independent review and incremental recovery validation — 2026-11-17

The original audit was written before implementation in [REVIEW.md](REVIEW.md).
After the initial scope stop, the user expanded ownership to required auth source
and settled the sharing bookkeeping question. Both parts of the lane then resumed
inline. [PROTOCOL.md](PROTOCOL.md) describes the version-two protocol and proof.

## Paths changed by this review/fix lane

- `crates/recovery/src/lib.rs`: delta coordinator, fixed pending/current head,
  committed-only witness resolution, bounded collection, optional external pin.
- `crates/recovery/src/model.rs`: documented public snapshot/error/step types,
  checksummed encoding and fault/durability observation boundaries.
- `crates/recovery/src/storage.rs`: pinned no-follow private filesystem access,
  fsio atomic writes, idempotent unlink and directory durability.
- `crates/recovery/src/index.rs`: immutable authenticated fixed-depth radix index,
  affected-path replacement, exact retired-path inventories.
- `crates/recovery/src/state.rs`: closed table ownership inventory, transactional
  dirty-key triggers, indexed per-owner capture, explicit disposable-state exclusions.
- `crates/recovery/src/tests.rs`: original fault regressions adapted to the new
  layout, defect reproducers, constant-cost/retention/collection/rollback/pair tests.
- `crates/recovery/{PROTOCOL,REVIEW,REVIEW-VALIDATION,PLAN,PORTING,VALIDATION}.md`:
  final protocol, original audit/disposition, plan, source/validation records.
  The original VALIDATION.md is explicitly marked as a historical v1 report.
- `crates/auth/src/passkey.rs`: IMMEDIATE login transaction, recovery commit
  barrier for counters/session creation, real software-passkey refusal regression.
- `crates/auth/src/passkeys.rs`: IMMEDIATE rename transaction and metadata barrier.
- `crates/auth/src/spaces.rs`: recovery interface/commit documentation, including
  empty-token no-op publication semantics.
- `crates/auth/src/session.rs`: documentation of intentionally excluded touches
  and the required future conservative sharing reset/resync behavior.

No server/core/UI files, auth HTTP adapters, manifests/dependencies or migrations
1–15 were changed by this lane. No subagents, BB threads, commits, pushes, external
messages, `.dev-data` or requests to port 3210 were used. All fixtures are disposable
temporary directories; Cargo commands were sequential with at most four jobs.

## Defects and red/green evidence

| Defect | Regression and observed original failure | Final behavior |
| --- | --- | --- |
| Newest-file deletion resurrected revoked membership | `deleting_newest_generation_must_not_resurrect_revoked_membership`: reader accepted the prior snapshot | Fixed head requires its exact manifest/root; deletion refuses |
| Repeated prepare published a rolled-back grant | `repeated_prepare_must_not_publish_a_rolled_back_grant`: recovered state contained an uncommitted member | Own uncommitted witness cannot drive replay or a second prepare |
| Acknowledged passkey label absent from recovery | `acknowledged_passkey_label_must_reach_recovery`: expected `After`, read `Before` | Rename publishes the protected account record |
| Login bypassed publication | `passkey::recovery_tests::login_rolls_back_credentials_and_session_when_recovery_refuses`: login succeeded despite refused prepare | Real WebAuthn login fails and rolls back both counter and new installation |

The initial recovery command returned exit 101 with 14 passed / 3 failed, proving
those three tests exposed the old implementation. The new real login regression
also returned exit 101 before its hook fix and passes afterward. The original cost
regression observed 2 versus 14 materialized records instead of one; the retention
regression also failed before replacement. No test was ignored or marked should-panic.

## Cost and retention evidence

- `publication_record_cost_is_independent_of_other_accounts_and_spaces`: exactly
  one affected space record at both installation sizes.
- `fsync_cost_is_constant_for_one_account_or_space_replacement`: **81 completed
  filesystem fsyncs**, for either an account or a space replacement, with zero
  versus twelve unrelated accounts/spaces.
- `account_receipt_and_space_change_publish_together_and_retry_has_no_fsyncs`:
  **150 fsyncs** for two affected namespaces; retry returns its saved receipt
  with **zero** additional filesystem fsyncs.
- `completed_transaction_retention_is_bounded`: twenty alternating role changes
  retain a bounded transaction inventory.
- `every_index_and_compaction_boundary_can_be_interrupted`: 146 separate injected
  failures (34 before/after index-node writes, 36 before/after retired unlinks,
  3 before/after collection directory syncs). Repeated replay preserves the
  revocation, leaves one intent/five records and at most 34 nodes per live record.
- Additional tests cover aborted-intent collection, next-request completion of a
  committed revocation, stale record/index replacement, a missing head, and full
  tree rollback with/without an independent checkpoint or surviving DB witness.
- `concurrent_space_mutation_pairs_never_publish_a_mixed_snapshot` covers 25 ordered
  create/remove/role/transfer/delete pairs through independent auth DB workers.
  Existing primary/replay fault matrices cover membership grants/revocations,
  roles, transfers and passkey removal. This is not an exhaustive cross-product
  of every individual account API; the same transaction/publication argument
  covers those hooks and their auth regression suites.
- Telemetry/reset tests prove last-seen/rolling-expiry/last-used and sharing reset
  changes cause zero filesystem publication, while a recovered expiry remains
  conservative. Rebuild must discard delivery state and resync every device.

## Exact commands and results

Commands ran from `/home/kayg/Developer/Calternal-AI` on the shared checkout.

| Command | Result |
| --- | --- |
| `./scripts/with-toolchain.sh cargo test -p calternal-recovery -j4` | Exit 0: 27 passed, 0 failed, 0 ignored; 0 doctests. This full run preceded the additional receipt regression below. |
| `./scripts/with-toolchain.sh cargo test -p calternal-recovery -j4 account_receipt_and_space_change_publish_together_and_retry_has_no_fsyncs` | Exit 0: 1 passed, 27 filtered; verifies the final added recovery test. |
| `./scripts/with-toolchain.sh cargo test -p calternal-auth -j4` | Exit 0: 93 passed, 0 failed, 0 ignored (38 unit, 38 account, 2 migration, 15 space); 0 doctests. |
| `scripts/check-docs.sh` | Exit 0: Rust check/doc and TypeScript reference gates pass. The script does not enumerate recovery, so recovery rustdoc is checked separately. |
| `./scripts/with-toolchain.sh cargo test --workspace -j4` | Exit 0: 362 passed, 0 failed, 3 existing ignored tests across 37 suites. Includes all **28 recovery tests** on the final source. Output redirected to `/tmp/calternal-recovery-workspace-review.log`. |
| `./scripts/with-toolchain.sh cargo doc -p calternal-recovery --no-deps -j4` | Exit 0: recovery reference generated without diagnostics. |
| `./scripts/with-toolchain.sh rustfmt --check --edition 2024 crates/recovery/src/lib.rs crates/recovery/src/tests.rs crates/auth/src/passkey.rs crates/auth/src/passkeys.rs` | Exit 0; recovery child modules are included by rustfmt's module traversal. |
| `git diff --check -- crates/auth/src crates/recovery` | Exit 0. |

The workspace run emitted existing server unused-mut/missing-documentation warnings,
outside this lane's source. Auth/recovery compiled without new diagnostics.
The three existing ignored tests are in other lanes' server fixtures:

- `old_name_is_ignored_and_cleared_once_without_touching_host_session`: obsolete
  cookie compatibility deliberately omitted under the user's decision.
- `upgrading_schema_six_removes_cookie_state_and_keeps_passkeys`: old schema
  compatibility deliberately omitted; the source fixture is retained for reference.
- `revoking_another_device_signs_it_out`: server protected account mutation receipt
  replay is not ported; source retry assertions are retained. The distinct auth
  library test with the same short name passes in this run.

No check in the final validation set failed. The earlier deliberately failing
regressions are reported above as red/green evidence, not concealed as passes.

## Remaining limits and questions

No unresolved product questions remain: both scope and sharing-reset decisions
were supplied by the user. All four defects and Part 2 are implemented.

- This lane does not implement validated empty-state rebuild, namespace recovery,
  root-role checks, server startup integration or an external checkpoint store.
- A pending head plus lost authoritative DB remains ambiguous and is refused.
- Individual missing/stale records, nodes, manifests and a missing head are
  detected. A complete old tree remains internally valid without independent
  freshness evidence. A surviving DB witness rejects tree-only rollback; loss or
  rollback of both stores requires an independently retained, rollback-resistant
  checkpoint updated before acknowledgement. The external pin is an explicit
  host integration requirement, not claimed as already deployed.
- The fixed-depth node-file design removes installation-size work at the cost of
  a relatively high fixed fsync count. There is no production latency/performance
  qualification or physical power-cut test claim.
- Linux descriptor-relative fsio is required. Root locks, trusted ancestry and
  protection from same-UID plugins remain host/sandbox responsibilities.
- Current per-owner records still scale with that owner's current credentials,
  installations and receipts. Bootstrap/rebuild validation scale with the initial
  inventory. Recovery history/versions do not grow with successful mutations.
- Unknown future security tables require an explicit ownership/trigger mapping;
  future auth write APIs must use the barrier. Sharing remains disabled/unported;
  a future rebuild must discard its delivery bookkeeping and resync every device.
