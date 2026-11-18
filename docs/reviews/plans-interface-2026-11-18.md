# Plans interface, 2026-11-18

[Forgejo issue #56](https://git.kayg.org/kayg/calternal-ai/issues/56) tracks the
open interface. Operators can set compute, queue, and storage limits without a
plugin. The ordinary plugin SDK exposes the same resource observations and
operator-approved admission hook to community packages. An optional owner
scope reports the group's combined committed bytes through the existing read
call. That scope requires a package-specific operator grant.

Sandbox runtime uses fenced guest-clock intervals. Personal spaces attribute
work to the account. Shared spaces with a configured owner group attribute work
to that group. Confirmed time is retained for 35 days. The host reports an
unresolved end when it lacks a final clock observation. Measured guest memory
governs placement. An active-thread ceiling is optional.

Group storage adds two local counter reads and one read-only SQLite snapshot
per sibling space on growth against a configured ceiling. It adds no flush.
The group lock spans the existing local content commit so two member spaces
cannot both spend the same capacity. The writer reuses the canonical head
query for cold spaces that predate byte counters.

## Upgrade

The production baseline is c98705d7. Content.db steps 6 and 7 follow sharing
at version 5. Machines.db version 7 comes from the threads backend title-lease
change at 3da9cfcf. This lane adds versions 8 through 11 and introduces machine
runtime store version 1. The auth schema stays at 14. Each new transition runs
through the shared migration runner and has a transition fixture.

## Verification

Dev-01 ran the full command sequence at `4f277e03`. Plugin build, both
format checks, the documentation build and gate, the server build, and the
artifact and real-server CLI contract checks passed. Rust workspace tests
reported 682 passed, 0 failed, and 2 ignored. Plugin tests reported 32 passed,
0 failed, and 1 skipped. Integration reported 2 passed and 0 failed.
Serial conformance reported 138 passed and 2 failed. One reported pass was
the live Firecracker case, which returned early because no guest config
was set.

Both measured-memory fixtures used the production 20-second hold for a
quiet poll, which exceeded the test client's 15-second deadline. The
corrected fixture uses a two-second hold. Dev-02 at `965ca6e7` passed
both measured-memory fixtures and all five scoped conformance cases,
including the owner-bound storage read, refusal without a grant, and
storage enforcement after restart. The full follow-up remains queued
on Dev-01 slot 3.

The test `a_slow_write_in_one_space_does_not_delay_other_spaces` had failed
intermittently during shutdown on Dev-01. The content sweeper lifetime fix has
a targeted regression case.

## Remaining integration

The orchestrator's child-thread spawn operation and parent wait presentation
remain open. They can reuse the existing memory admission and wait reason.
The exact production and candidate image pair still needs release proof before
an unattended update.

## Owner decisions

- Decide which independent evidence can settle an unresolved sandbox stop.

## Simplification opportunities

Keep the per-space committed byte counter authoritative. If large groups make
sibling reads costly, evaluate a shared aggregate before adding another
persisted counter.
