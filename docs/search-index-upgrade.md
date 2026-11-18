# Prepare a search index before upgrading

Proposed for issue #47. An ordinary upgrade keeps serving the last ready search
index while the next release prepares its own. The content journal stays
authoritative. The derived index, its manifests, and its watermark can be
discarded and built again.

## Prepare and publish

1. Admit one candidate per space. Record the space UUID, journal epoch,
   target release and schema, searchable kinds, and last incorporated
   journal position in the candidate. Use a bounded maintenance child of the
   active release with read-only access to the pinned roots and journal.
   The child has its own derived writer and uses no foreground slots. The
   old release retains installation ownership and serves current access rules.
2. Build the candidate under its own generation path on the same filesystem.
   Verify files against durable heads and catch up to the journal. Abort and
   reseed on an epoch or searchable-kind change. The pending generation is
   invisible to searches and can resume from its own committed cursor after
   interruption.
3. When ready, briefly fence publication for the space. Finish applying the
   admitted journal tail, verify the epoch and configuration again, and
   checkpoint and sync the candidate. Atomically replace the active-generation
   record and sync its directory. The new release opens the candidate using
   its own schema; old readers can finish against their pinned handles.
   Exercise the first new-release query while writes continue. Leave the old
   generation available for rollback until the upgrade is accepted.

## Disk admission and recovery

Reserve **live generation + candidate generation + peak WAL for both +
temporary scratch + the installation free-space floor**, summed across
admitted spaces. A candidate's main file and scratch space must have enforced
quotas. Retain the old files through the rollback window and count them as live
until deletion. Start with one candidate per installation while measuring peak
use. The current 100,000-note fixture uses about 79 MB in the checkpointed
database. This is a sizing input. Admission uses the configured ceiling and
measured WAL peak, not the fixture's size as a universal limit.

The candidate has one writer and no long-lived readers. Start with a proposed
64 MiB WAL allowance per candidate. Before beginning a batch, reserve
worst-case frames for its dirty-page ceiling, including frame headers. Cap
cache spill so repeated frames cannot exceed that reservation. Checkpoint and
truncate between batches if the allowance would be crossed. If a batch still cannot
fit, stop the build, keep the old generation active, and retry only after space
or configuration changes. Keep SQLite's live writer and its durability settings
independent of this allowance. The existing automatic checkpoint threshold is
a trigger, so it cannot substitute for a reservation or a hard byte bound.

| Crash point | Recovery |
| --- | --- |
| Before or during candidate build | The active record still names the old generation. Resume a committed matching candidate or discard it. |
| After catch-up or sync, before active record replacement | The old generation stays active. Check the candidate's epoch and current journal tail again before publishing. |
| During active record replacement | The synced candidate exists before either record can name it. Recover the last durable record, validate the named generation and catch up if needed. |
| After publication, during handoff or rollback | Keep both generations. Run the release matching the selected generation. Apply journal changes before publishing any rollback selection. Delete an abandoned generation only once no reader holds it and rollback is closed. |

When the next release finds a prepared matching candidate, it opens that index
on the first visit. A space with no ready index keeps today's lazy rebuild and
returns partial search results until ready. If preparation is interrupted for
a space that has a ready index, defer release cutover until its candidate
catches up. This keeps one server release active for the installation.
Before adoption, test with ongoing saves
and searches, including restart at each crash point and a full-disk refusal.
The acceptance measure is zero incomplete search pages caused by a prepared
upgrade. First-build latency remains a separate measurement.
