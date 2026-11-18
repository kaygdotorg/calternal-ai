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

The production baseline is c98705d7. Content.db steps 7 and 8 follow live rooms
at version 6. Machines.db version 7 comes from the threads backend title-lease
change at 3da9cfcf. This lane adds versions 8 through 11 and introduces machine
runtime store version 1. The auth schema stays at 14. Each new transition runs
through the shared migration runner and has a transition fixture.

## Verification

Dev-01 slot 3 completed the full command sequence at `965ca6e7`. Plugin
build, both format checks, the documentation build and gate, the server build,
and the artifact and real-server CLI contract checks passed. Rust workspace
tests reported 686 passed, 0 failed, and 2 ignored. Plugin tests reported 32
passed, 0 failed, and 1 skipped. Integration reported 2 passed and 0 failed.
Serial conformance reported 140 passed and 0 failed. The live Firecracker
case returned early because no guest config was set, so 139 cases ran.
Both scratch checkouts were removed, and no lane process remained.

An earlier serial run timed out in both measured-memory fixtures. A quiet
poll could hold for 20 seconds, beyond the test client's 15-second deadline.
The corrected fixtures set a two-second server hold. They passed on Dev-02
and in the full gate. The scoped plugin tests verified an owner-bound usage
read, denial without an operator grant, and enforcement after restart.

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
