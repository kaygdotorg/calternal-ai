# Two durability barriers for thread appends

The 16 MiB writer uses four barriers for a send: a FULL content WAL commit,
a Markdown sync, a FULL space journal commit, and a FULL job admission
commit. In the locked night-6 profile, sync syscalls consumed 47.29 ms of
the 47.80 ms median spent inside those spans. The target is two durable
barriers for a send, including admission. Sent content must survive power loss.

| Option | Barriers and acknowledgement | Crash before the first barrier | Crash between barriers | Crash after the second barrier |
| --- | --- | --- | --- | --- |
| One per-space SQLite writer for the append | One FULL transaction holds patch bytes, placement intent, journal entry, retry receipt and initial job admission. A grouped Markdown sync is the second barrier. Acknowledge after both. | No message, event or job was accepted. Retry the request. | All logical state is durable. Recover the file from the intent, including a missing or torn suffix. Retry returns the same revision and job. | File and logical state survive. Recheck and retire an outstanding intent. Replay returns the same result. |
| Shared message commit with separate jobs | One FULL content and journal commit and one FULL jobs commit precede acknowledgement. Markdown sync is deferred and shared, so there are two foreground barriers and a third physical barrier per file group. | No message or turn was accepted. Retry the request. | Message and intent survive. Repair Markdown and resume admission using the saved key. An interrupted send can already have a visible message. | Both receipts survive. Repair Markdown if its group had yet to sync, then resume the admitted job once. |
| Durable redo log across the existing stores | A forced append to a redo log holds the message, event, receipt and job ID. Markdown sync is the second barrier. Acknowledge after both. SQLite updates replay from the log. | No accepted state. Drop any torn log suffix. | Reapply the logical records idempotently and repair Markdown from the durable redo record. | Replay any missing SQLite updates. Preserve the redo record until every store has a durable replay position. |

Prefer the single per-space SQLite writer. Keep the existing journal sequence,
receipt scope and initial job identifiers in the shared transaction. Commit
the original bytes and a checked placement intent before writing the file.
Hold the existing file lease through each complete record write and the group
sync. Defer the sync within a bounded group window, and finish it before the
first acknowledgement or exposure of those records to stream subscribers.
The scheduler releases the admitted job only after placement completes.

Retirement can join a later FULL commit or run as replayable bookkeeping.
A lost retirement must only repeat a check, never append the same bytes or
schedule a second job. Retain the patch and receipt until retirement is safe.
On reopen, verify the saved prefix and repair only a missing or matching
torn suffix. Preserve an outside edit and surface a conflict when the
witness differs. The server completes recovery before serving the file.

Colocating these records changes the stored authority and needs a versioned
migration and restore-point proof. Separate SQLite databases in WAL mode
cannot supply the proposed atomic transaction through ATTACH. Measure the
new barriers with the existing locked fixture and cut power at each boundary
before treating it as a release candidate.
