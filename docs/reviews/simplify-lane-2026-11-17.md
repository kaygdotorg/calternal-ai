# Simplify lane report, 2026-11-17

Lane `lane/simplify` (Forgejo kayg/calternal-ai#11) applied the findings of
[simplification-review-2026-11-17.md](simplification-review-2026-11-17.md),
merged `dev` (live subscription, CLI pass 2, drafts and the account feed) and
ported that code onto the shared types, per-space locks, one error type and one
schema.

## Before and after

| Measure | Before | After |
| --- | --- | --- |
| Installation-wide locks on the content, plugin and receipt paths | 3 (store mutex, content mutex, journal bridge thread), plus the account-input path reusing the store lock, plus the plugin-state writer | 0: per-space slots, a lock-free `Store`, plugin state in each space, one connection per account call |
| Installation-wide serialization that remains | | the catalog writer for a new item's first route (one FULL insert); the auth worker (every directory read and write, with recovery fsyncs); the machine registry mutex |
| Capability lookups per `notes.update` | 4 | 2 |
| Copies of IDs, caller-safe error, capabilities, time helpers | one per crate | one, in `calternal-types` |
| Plugin manifest and view formats | v0 and v1, with a lossy projection | v1 manifests, GenUI v2 source views |
| Request receipts | kept forever | 7 days by default (`CALTERNAL_RECEIPT_RETENTION_DAYS`) |
| Installation ID | read from the roots | pinned in server configuration (`CALTERNAL_INSTALLATION_ID`); roots must match |
| Lines against `dev` | | +8,373 / −10,383 (Rust: +5,204 / −6,803) |

## Independent review fixes

[simplify-independent-review-2026-11-17.md](simplify-independent-review-2026-11-17.md)
reproduced ten defects. Each fix is one commit (1 and 8 share one, as do 3 and 4),
accepted by the review's failing test, which now passes unchanged except where noted.

| # | Cause | Fix |
| --- | --- | --- |
| 1 | A re-saved draft updated only its body; its indexed space stayed stale | Space and item columns generated from the record body; a draft's space is fixed (owner decision) |
| 2 | Cold-start slot released only after the start's await | RAII reservation carried into the start task; a dropped worker stops its process |
| 3 | `machines` commands opened `machines.db` directly | Commands take the three roots and pinned ID; a running server is asked over its admin socket, a stopped one is opened through the guard |
| 4 | Machine authority committed outside the recovery log | Authority moved into `auth.db` (`calternal_auth::machines`) under the same barrier; no cross-database protocol |
| 5 | Directory receipts separate from space receipts | Every write in a space is admitted through that space's receipts under its lock (`Content::admit`) |
| 6 | Non-starter cancel read the directory on the async thread | Capability read moved to a blocking worker |
| 7 | Directory receipts keyed (actor, request ID), never pruned | Keyed (actor, scope, request ID) with `at_ms`, pruned at the receipt retention |
| 8 | Pending identity ignored the space | Pending identity is (space, request ID); selectors take `space_id` |
| 9 | Permanent publication keys outlived receipts | Receipt and event commit in one FULL transaction; publication keys removed |
| 10 | Plugin toggles used the installation-wide state writer | Enablement is a control value in the space's `content.db` |

Test adaptations: finding 4's test calls `admin::service_for(&store)` instead of
`admin::pool_grant(&appdata, …)`, because an appdata path alone cannot reach the
recovery log that finding 3 requires. The injection (a file where the log should be)
and the assertion are unchanged. Callers of `pending.*` and `machines` now pass the
new `space_id` and root arguments.

## Gates (dev VM, final tree)

`cargo fmt --check`, workspace tests (including the review's server and auth
probes), conformance with `test-plugins` (43: the 37 from `dev`, request IDs per
account and space, and the review's five cases), integration, plugin, contract,
the docs gate with `--locked` (so the standalone conformance lockfile is current)
and `contract/verify-cli.py` all pass.

## Not done

- Generated docs pages (`apps/docs/src/content/docs/admin/...`) still show the old
  `pending` and `machines` arguments: `npm run docs:site` needs the web app's
  font packages, which are the UI lane's to install.
- `secrets.*` writes keep their own owner-local idempotency, not the request
  admission above.

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
