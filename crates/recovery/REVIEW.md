# Independent recovery review — 2026-11-17

This review was written before changing implementation or tests. It reviews the
shared checkout's version-one protocol, not a production recovery guarantee.
Read: repository AGENTS.md, architecture proposal sections 2 and 4, overnight
progress and morning decisions, recovery protocol/source/tests, all five allowed
auth hook files, and source ADR 0062 lines 404–458 using `git show origin/dev`.
The newer user decisions supersede the proposal's old route/import requirements.

## Final disposition after expanded ownership

The original review below records findings **before implementation**. The user
expanded ownership to all required auth source on 2026-11-17 and explicitly
excluded sharing delivery/reset metadata. Implementation then resumed inline.
All four findings are fixed, with their regression tests retained:

- Newest-generation deletion: a fixed installation head pins the current intent
  and authenticated record index; no directory-prefix fallback remains.
- Uncommitted witness: prepare compares its witness with a separate read-only
  connection and refuses a second prepare. Replay uses committed visibility only.
- Passkey labels: rename now uses an IMMEDIATE transaction and the commit barrier.
- Login: credential counters and session creation now commit through the same
  barrier. `passkey::recovery_tests::login_rolls_back_credentials_and_session_when_recovery_refuses`
  exercises real WebAuthn registration/sign-in with the software authenticator;
  it failed before the fix and passes afterward.

Part 2 is implemented: transactional dirty-owner tracking, per-owner versioned
records, a fixed-size head, a fixed-depth authenticated index, exact retirement
lists, crash-safe bounded reclamation, and optional external checkpoint validation.
Ordinary single-account/space replacement is 81 filesystem fsyncs independent of
unrelated identities. This is a fixed-cost tradeoff, not a latency qualification.
PROTOCOL.md gives exact counts and threat/recovery limits; REVIEW-VALIDATION.md
contains current commands/results. There are no remaining product questions from
this review. External pin storage, validated rebuild and sandbox integration
remain host work, without being silently claimed by this crate.

The inventory below is a historical audit. Its formerly missing login/rename
paths are now covered; sharing touch/reset paths are intentionally excluded.
The user's future-rebuild rule is to discard sharing delivery state and force
every device to resync conservatively under current authority. The executable
pairwise test covers all 25 ordered pairs of create/remove/role/transfer/delete
through independent space workers. The shared transaction/publication argument
also applies to account/token/credential/session writes; their existing auth and
new login/recovery tests exercise those hooks, rather than claiming an exhaustive
cross-product of every individual auth API.

## Initial scope blocker (resolved)

`crates/auth/src/passkey.rs::finish_login_inner` updates the serialized passkey
(including its authentication counter), creates an installation using
`session::create`, then calls `tx.commit()` directly. Neither the normal login nor
the HTTP discoverable/reverification adapter invokes the recovery commit barrier.
This file is outside the permitted five auth files. A safe complete fix needs the
caller to retain the prepare/commit/complete interval through acknowledgement;
staging inside `session::create` cannot complete after its caller commits. The
user explicitly requires stopping and reporting when another path needs a change.
Consequently this lane stops at review/reproducers, without redesigning or fixing
the implementation. The sign-in finding is source-audited; a real WebAuthn login
reproducer is not yet implemented.

## Defects

1. **Deleting the newest generation silently restores revoked authority.**
   `intents()` derives the end of history from directory contents. Sequence
   continuity only detects holes before a surviving later generation. Removing
   the newest intent and completion leaves a valid prefix; even retaining its
   orphaned record files does not prevent acceptance. `validate_and_read` returns
   the preceding membership grant after an acknowledged removal. Reproducer:
   `deleting_newest_generation_must_not_resurrect_revoked_membership`.
2. **Prepare can publish an uncommitted witness.** Public replay/complete reject
   active transactions, but `prepare_inner` calls `replay_locked` using its active
   transaction. Two sequential prepare calls in the same transaction cause the
   second to finish the first using its still-uncommitted witness. Rolling back
   the transaction does not undo the first completion file, which is trusted on
   replay. This needs no concurrency and obeys the documented requirement to
   serialize direct trait calls. Reproducer:
   `repeated_prepare_must_not_publish_a_rolled_back_grant`.
3. **Passkey rename acknowledges metadata absent from recovery.**
   `passkeys::rename` is an autocommit UPDATE with no publication. This loses a
   current account record change on recovery even though it is not itself a
   credential grant. Reproducer: `acknowledged_passkey_label_must_reach_recovery`.
4. **Login credential/session publication bypass**, described above. A committed
   counter update is security state; it cannot be treated like last-seen telemetry.
   No end-to-end login regression test is supplied in this stopped lane.

The existing missing-version test deletes the *oldest* generation; it does not
cover defect 1. The existing uncommitted-witness test calls replay/complete on an
active transaction; it does not cover defect 2.

## Auth write inventory

| Write path | Current barrier coverage | Recovery disposition |
| --- | --- | --- |
| `setup::first_owner_token` replacement/issuance | `spaces::commit` | Recover hashed token, purpose, expiry, consumption/replacement |
| `setup::create_in` identity/token creation and first-token invalidation | `spaces::commit` | Recover account identity, server role, quota, current token state |
| `setup::recover_in` token replacement | `spaces::commit` | Recover replacement, so an older token is not resurrected |
| `setup::redeem` token consumption, optional first account, passkey, session and registration flags | `spaces::commit` in the same IMMEDIATE transaction | Recover credential and token-consumption authority |
| `passkeys::finish_add` | `spaces::commit` in the same IMMEDIATE transaction | Recover authenticator grant |
| `passkeys::remove` | `spaces::commit` in the same IMMEDIATE transaction | Recover revocation |
| `passkeys::rename` | **Missing**, direct autocommit UPDATE | Recover account metadata |
| `passkey::finish_login_inner`, both sign-in and reverification | **Missing**, direct transaction commit | Recover credential counter; installation authority policy needs explicit treatment |
| `session::create` | Caller-dependent: enrollment covered, login uncovered | Never infer coverage from this helper alone |
| `session::revoke`, including DAV-only wrapper | `spaces::commit_with` in the same IMMEDIATE transaction | Recover revocation; cache fence runs immediately after SQLite commit even on completion failure |
| `session::touch` last-seen/rolling expiry | Direct UPDATE, deliberately no barrier | Last-seen is disposable; restoring earlier expiry must only shorten authority, never extend expired authority |
| `session::mark_stale_installation` sharing reset marker | Direct INSERT OR IGNORE from touch, no barrier | Cannot classify as disposable without a rebuild proof for the inherited sharing reset protocol |
| `spaces::change_with_receipt`: create, rename, add, role change, remove, leave, transfer, delete, receipt insertion | `spaces::commit`, same IMMEDIATE transaction; receipt-hit branch also republishes | Recover changed space authority; receipt retries should not rewrite unrelated records |
| `Spaces::seed_development`: fixture accounts, directory and spaces | `spaces::commit`, same IMMEDIATE transaction | Disposable fixture entry point, nevertheless covered |
| `spaces::members` | Direct read-transaction commit | No writes; needs no publication |
| Rate-limit buckets, account failure backoff, ceremonies | Memory only (`limit.rs`, `ceremony.rs`) | Disposable transient admission state; no recovery publication |
| Invite consumption | No current invite mutation/redemption API in this auth crate; only inherited schema | No demonstrated coverage; future consumption must share the authority transaction and barrier |
| Schema/role seed migrations in `db::migrate` | Direct transaction commit outside the hook files | Startup under root locks; future security migrations need an explicit publication/adoption rule |
| Recovery witness table creation/insertion | Coordinator reconcile/prepare | Intent digest insertion uses exactly the caller's Connection/transaction |

`account.rs` delegates writes to the functions above. The HTTP auth adapter
delegates to those same setup/login functions; it does not repair login coverage.
No independent people/group/quota/share mutation modules currently exist in this
auth crate. This review does not certify external SQL writers in other crates.

## Crash and concurrency analysis

For a single correctly hooked mutation, intent durability precedes witness
insertion; a rollback removes the witness with the authority change. A matching
committed witness permits deterministic publication. Failure during any record
write leaves a pending intent, so the reader refuses a mixed inventory. Failure
before completion placement refuses recovery; after placement the complete
inventory must validate. A failed acknowledgement after commit can require retry.
These boundaries have existing fault tests, not physical power-cut evidence.

For every pair drawn from hooked account/token creation or consumption, passkey
addition/removal, explicit installation revocation, and space mutations, SQLite
serializes writers and the process commit gate retains ordering through completion.
The witness and changes use the same transaction at each covered call site.
This is a shared structural argument, **not** an executed exhaustive pair matrix.
Unhooked login/rename/touch do not acquire that publication gate. SQLite serializes
their writes but their acknowledgement is not bound to recovery publication.

After a process dies between commit and completion, the next hooked mutation
replays the previous intent before preparing its own. It can finish a committed
revocation. Ordinary session reads do not themselves replay; they use the current
database/cache. Restart must reconcile before admission. The documented host root
locks and startup integration remain prerequisites, not supplied by this crate.
Direct callers can violate the intended protocol via repeated prepare (defect 2).

## Cost and redesign requirements, not an implemented replacement

Current cost is installation-sized: full SQL capture and integrity scan, full
intent, private global bundle, and every space record; `4 + 3*(1+S)` filesystem
fsyncs per mutation plus SQLite durability. Replay validates retained history and
witnesses/intents/records grow without bound. Merely diffing full snapshots would
reduce writes while retaining installation-sized CPU/read cost.

A replacement needs transaction-local dirty account/space identities, bounded
record selection, one delta intent, a fixed-size head binding the current version
set (for example an authenticated persistent index), and reclamation only after
durably installing a head that no longer references retired nodes. A full manifest
rewritten each time is still installation-sized. Head publication and collection
must have separate crash tests; deletion of the newest files must be refused.
No replacement format, proof, exact new fsync count, compaction algorithm or cost
qualification has been implemented or claimed here.

Checksums and a local head cannot detect rollback of the *entire* recovery tree
to a previously valid image without a surviving independent monotonic anchor.
The installation UUID alone is not such an anchor. This is distinct from detecting
individual missing/rolled-back records or a deleted newest generation.

## Original questions for the user (both answered)

- Scope: permit the minimal login caller fix in `crates/auth/src/passkey.rs`,
  including its regression coverage, or have its owner land that fix first.
  The former adds one owned integration path; the latter blocks this lane until
  the change is available. Hiding the problem inside the session helper cannot
  provide an after-commit publication barrier.
- Sharing reset metadata: include it as protected recovery state (a dormant-device
  touch that inserts a reset would publish), or prove a conservative rebuild that
  forces every restored device through reset (more rebuild work, no touch-time
  publication). The reviewed docs do not settle that reconstruction behavior.

Validation results for this stopped review are recorded separately in
`REVIEW-VALIDATION.md`; the original VALIDATION.md describes the previous lane.
