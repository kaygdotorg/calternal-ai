# Receipt pruning across outbox recovery — November 18, 2026

[Forgejo #52](https://git.kayg.org/kayg/calternal-ai/issues/52)

The journal writer previously pruned at open, before the content database
could expose its outbox. When a journal commit survived and its outbox row
survived retirement, dormancy past the receipt lifetime made the first reopen
forget the completed request. Recovery then appended a second event. The
regression reproduced two `threads.checkpoint` events carrying the same append.

Content now reads every outbox pin before issuing the prune command. The space
slot stays held through the writer reply. The writer encodes each typed
`(account, request ID)` pair with the same private key function used by commit
and lookup. At first open the command runs before outbox replay. The existing
content worker requests it at hourly maintenance for active spaces. A failed
pin read sends no command. The journal writer never prunes on its own.

The existing FULL content, journal and retirement boundaries still precede
acknowledgment. The fence adds no clock check, query or command to a warm save.
A cold recovery or maintenance pass adds one outbox key scan and one indexed
delete on the existing per-space writer. The maintenance pass holds its slot
through the delete so a caller cannot retire or retry with stale pins.

## Retention for owner sign-off

The configured age, seven days by default, makes an ordinary receipt eligible
for deletion at the next prune. A receipt that protects a durable outbox row
has **no finite wall-clock ceiling while that row survives**. It becomes
eligible after FULL retirement and is removed on the next eligible prune,
normally the next hourly pass while open or next open after dormancy. An
unreadable pin set suspends pruning for that space until it can be read.
Existing owner erasure removes the journal with the space. A hard time limit
while an intent can still replay would allow the duplicate again.

## Verification

| Boundary | Expected and observed result |
| --- | --- |
| Durable checkpoint survives interrupted retirement and dormancy | Old code appended twice. Fixed reopen returns the original reply and serves one append event. |
| Forward clock jump at boot | The same cold recovery path preserves the pinned original reply and the single event with an advanced wall clock. |
| Outbox read fails | An expired receipt survives the attempted prune. |
| Maintenance races retirement and a retry | The busy slot defers maintenance. Retry replays the original reply, and a later eligible pass deletes the retired, expired receipt. |
| Prune with two accounts using the same request text | The pin protects only its account's receipt. The other expired receipt is removed and both journal events remain. |

The debug HTTP test aborts the server at the journal boundary, ages the
surviving receipt and verifies that restart returns the original reply with
one journal event. The first gate run exposed a blocking-channel panic in
the in-process authorization fixture. The fixed command uses the existing
journal bridge, and all ten authorization cases pass. The final dev-01 gate
passed 609 workspace tests, 123 conformance tests and the separate opt-in
real-server test. Plugin, integration, contract, docs, formatting and Podman
leak checks passed.

## Simplification opportunities

Keep retention scheduling in the content lifecycle and one key encoder inside
the journal. The prune fence makes startup protection independent of writer
timers and exposes the same ordering for recovery and maintenance.
