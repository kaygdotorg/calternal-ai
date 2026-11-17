# Incremental security recovery implementation plan

User-authorized inline execution; no subagents, commits, server/core/UI edits,
or changes to auth migrations 1–15. Scope expanded to auth source on 2026-11-17.

1. Preserve the independent review and failing tests. Add a real software-passkey
   login test: failed recovery prepare must roll back both credential counter
   and new installation. Fix login and rename to use the existing commit barrier.
2. Split recovery into protected storage, record selection/dirty tracking,
   authenticated fixed-depth radix index, and coordinator. Keep public snapshot
   reading as a reconstructed logical image for future owner-controlled rebuild.
3. Install transactional dirty-key triggers once at configuration. Partition by
   account, space, group, invite, and fixed global policy. Exclude sharing delivery
   bookkeeping and telemetry. No full-database capture on subsequent mutations.
4. Write one delta intent, durably mark the fixed head pending before SQLite
   commit, materialize changed records and index paths, then atomically replace
   the head. Resolve pending decisions only through committed witness visibility.
5. Reclaim only resources explicitly retired by the published head's intent;
   retain its manifest and current tree. Inject crashes around every collection
   boundary. Never scan or rewrite the installation on an ordinary mutation.
6. Run boundary/replay regressions, pairwise concurrency, missing/rollback/head
   damage, exclusion, constant fsync cost and bounded-retention tests. Update
   PROTOCOL.md and review validation with exact counts, assumptions and limits.
7. Run recovery/auth suites, required docs gate and recovery rustdoc through the
   toolchain wrapper with Cargo at most -j4. Report remaining failures plainly.

The head is fixed-size (root hash, sequence, current/pending intent references).
Index keys are one namespace nibble and 32 UUID nibbles: a record replacement
writes at most 34 small immutable index nodes regardless of installation size.
This trades a higher fixed fsync count for bounded work and a simple reclamation
proof. Full-tree rollback still needs an independent monotonic external pin;
the surviving auth database also supplies a commit anchor while it exists.

Execution complete: all seven steps delivered inline. Final workspace validation
passes 362 tests (including 28 recovery tests); 3 existing server ignores remain.
Auth passes 93 tests, required documentation gates and recovery rustdoc pass.
See REVIEW-VALIDATION.md for exact commands, red/green evidence and limitations.
