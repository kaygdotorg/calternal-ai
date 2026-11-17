# Simplify lane report, 2026-11-17

Lane `lane/simplify` (Forgejo kayg/calternal-ai#11) applied the findings of
[simplification-review-2026-11-17.md](simplification-review-2026-11-17.md),
merged `dev` (live subscription, CLI pass 2, drafts and the account feed) and
ported that code onto the shared types, per-space locks, one error type and one
schema.

## Before and after

| Measure | Before | After |
| --- | --- | --- |
| Installation-wide locks on the request path | 3 (store mutex, content mutex, journal bridge thread), plus the account-input path reusing the store lock | 0; per-space slots, a lock-free `Store`, one connection per account call |
| Capability lookups per `notes.update` | 4 | 2 |
| Copies of IDs, caller-safe error, capabilities, time helpers | one per crate | one, in `calternal-types` |
| Plugin manifest and view formats | v0 and v1, with a lossy projection | v1 manifests, GenUI v2 source views |
| Request receipts | kept forever | 7 days by default (`CALTERNAL_RECEIPT_RETENTION_DAYS`) |
| Installation ID | read from the roots | pinned in server configuration (`CALTERNAL_INSTALLATION_ID`); roots must match |
| Lines against `dev` | | +8,373 / −10,383 (Rust: +5,204 / −6,803) |

## Gates (dev VM, final tree)

`cargo fmt --check`, workspace tests, conformance with `test-plugins` (38: the
37 from `dev` plus request IDs scoped per account and space), integration,
plugin, contract, docs gate and `contract/verify-cli.py` all pass.

## Not done

- Finding 15, remainder: `strict: bool` and `Option<Guard>` in `calternal-store`
  exist only for test constructors. Removing them means porting about 20 store
  tests from plain `epoch-seq` tokens to signed ones. That is more than an hour,
  so it is left for a follow-up lane.

## Simplification opportunities

- Store: make the guard non-optional (above); `begin_owned_save_activity` then
  stops returning `Option`.
- `account_input::Inputs` keeps its own clock type; it could take
  `calternal_auth::Clock` (or a `calternal-types` clock) instead.
- `publication_receipts` still gains one row per write with no bound (finding 11).
