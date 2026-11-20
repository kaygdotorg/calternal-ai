# Plugin API v2 execution qualification design

Slice 3, Forgejo [#94](https://git.kayg.org/kayg/calternal-ai/issues/94).
The initial exchange map is from the revision 14.33 build plan at `cc13ae21`
and the Threads fake host at `6125ad96`. Implement the tests with the shared
Rust identities on the host base. The lane is based on revision 14.35 at
`36d49713`. Revision 14.34 pins the
initiating installation through renewal and puts archive and its execution
fences on the same per-space content writer.

## Storage checkpoint

`calternal_content::execution` supplies the execution writer operations and
their additive SQL. The content-store tests drive the admission and Stop
states, retained receipt replay and exact teardown settlement. They run against
disposable SQLite stores. The product broker, renderer activation, live grant
and installation hooks, cross-store fences and authenticated machine proof
still need integration. The test callback models the live check result and
the exact teardown key, and does not qualify the enrolled carrier.

The server adapter in `plugins::execution_v2` compares Send operands with
the checked installed declaration and placement, injects the one item field
before run validation, and calls contribution preflight before admission.
Claim produces the host-owned invocation context. The owning-job handle
reserves an attempt and replays its transition cursor. Protected Stop
selection issues one context per eligible target. Resolve rechecks the
exact target and dispatch requires confirmation for the same member,
installation, lease, component and item, then writes the receipt with the
fence. Slice 1 supplies the installed-package resolver, frame validator and
carrier callback registration. Slice 6 supplies the active renderer lease
and confirmation-proof resolver. Their input authority and proof checks are
fixture callbacks in the server adapter test.

Slice 2 owns registration of `execution/schema.sql` in the per-space migration
and calls `archive_targets` and `fence_archive` from its item archive writer.
Slice 3's broker and dispatcher use the admission, claim, promotion, start and
effect checks. Slice 6 supplies the protected Stop context and calls the
receipt-and-fence boundary. Slice 4 supplies authenticated teardown and uses
the exact-attempt cleanup identity. The installation revocation hook belongs
to Slice 3. Leave the execution path gated until these callers are wired.

Warm operation costs exclude cold opening, recovery backlog and the upstream
authority checks. Fresh Send makes one bounded space count and one indexed
slot read. Fresh Stop makes one exact indexed target lookup. The shared receipt
path probes the outbox before the effect, reads the prior journal receipt, then
reads the committed outbox row, probes the journal before publishing, and
checks that the outbox drained. The journal writer validates the receipt again
inside its commit. Fresh requests have three FULL commits using the inherited
content receipt path: effect plus outbox, journal publication, then outbox
retirement. Exact replay has one outbox probe, one receipt lookup and zero
flushes once recovery has drained. Promotion and claim each read one matching
pin, decode it once and make at most one FULL writer commit. Runtime and grant
validation I/O still needs measurement after integration.

Each unsettled item has two slots. The host supplies its per-space admission
capacity. Each retained pin is bounded by the shared host frame limit. A
fenced attempt occupies its head slot and admission capacity until exact
teardown; it requires no second unbounded cleanup queue. A settled intent is
removed. Its admission and Stop receipts share the existing configured
retention and outbox pinning policy.

## First qualification

Run through the server's broker and worker invocation paths with a fresh
disposable space and the real per-space SQLite store. The enrolled-entry
integration can substitute a deterministic carrier at the runtime seam for
these tests. Exercise the same sequence as the packaged Threads worker.

1. Send a checked `start_job(ExecutionJobStart)` and capture its
   `ExecutionJobReceipt`. Resolve the installed job and run-operation schemas,
   inject the checked top-level target once, and dispatch the owning job with
   `ExecutionJobContext`. Reject duplicate target fields, expired or foreign
   choices, and oversized contribution envelopes before admitting the job.
   Assert no prompt write or launch on refusal. Include a payload whose JSON
   escapes push the encoded frame over the limit.
2. Cut the process immediately after admission commits and before reply.
   Reopen the same data directory and replay the same checked Send. Require
   one intent, one reservation and the same job reference. Change an operand
   under the same request and require refusal. A fresh request must observe
   the existing queue occupancy.
3. Admit a head as actor A and race two checked Sends for the successor slot
   as actor B. Require one successor receipt and one `pending_limit`. The
   queued job receives no invocation or runtime lease. Exercise
   `execution_start(ExecutionStart)` from the claimed head and require
   `starting` plus a stable execution ref and cursor on retry. The carrier
   sees no entry dispatch until the typed call matches the pinned contribution
   input. A changed input or forged job context has zero dispatches.
4. Resolve, confirm and dispatch protected Stop through `execution.control`.
   Reject raw client refs, foreign contexts and a stale context after target
   replacement. A member without retirement authority cannot stop another
   starter's job. Exercise both Stop arms (pending intent and active attempt).
   Crash after the fence and receipt commit, before dispatch or reply. A
   same-request replay returns the committed result. Cleanup recovers by exact
   attempt and generation. Repeated dispatch or acknowledgement cannot retire
   another attempt or release capacity twice. Probe old output, terminal,
   renewal and egress on both sides of the commit. A frame committed earlier
   stays committed.
5. Stop the head alone while another actor's successor is queued. Withhold
   teardown proof and require the successor to remain queued. Reject proof
   from a different execution, placement or generation. After verified
   teardown, require a live promotion check and a separate live claim check.
   Test device and grant loss between admission and promotion and between
   promotion and claim. No old choice can be replaced by the newly available
   machine, profile or harness.

Use the debug-only `CALTERNAL_FAULT` child-process pattern from
`crates/server/tests/http.rs` and coordinate race arms with explicit
barriers. Verify durable state after reopening, alongside
counts at the dispatch, cleanup and quota boundaries. A rollback arm faults
before commit and sees zero writes and no remote cleanup. Run on dev-01 using
the lane directory, `TMPDIR=$HOME/gate-tmp` and `nix develop`.

## Integration qualification

- Exercise the fence table with an unrelated active item, a pending successor
  and a retained receipt in every applicable arm. Erasure prevents new work on
  the erased scope. Membership, grant, session and package or placement
  invalidation retire only the affected bindings. Audience widening retires
  personal authority, incompatible saved bindings and uncommitted derived
  output before the new audience can read. Terminal and deadline paths hold
  capacity until proof. Cutover closes admission and refuses the writer switch
  until attempts and unresolved intents drain. Assert no fresh child effect
  on ancestor-authority loss while child paths are disabled.
- With Slice 2, archive with a head and successor started by different actors.
  Bind confirmation to the whole set and refuse a changed set, including
  insertion or replacement of a successor. After commit and restart, recover
  archive, both retirements and the cleanup work. Undo cannot resurrect their
  authority. Run the same fence oracle for every authority-ending catalogue
  row on an integrated route. For cross-database changes include a crash
  between the durable space fence and authority publication.
- With Slices 1 and 4, commit the first contribution frame and its item append,
  lose the append acknowledgement, then replay. Require a single append and a
  single launch. Stop before terminal, then require the committed bytes to
  survive while the retired attempt can publish no further output. This
  follows the fake-host tests for append replay and Stop before terminal.
- With the Inbox, race a one-use approval against dispatch, cancellation and
  grant loss. Match approval to the same checked effect and attempt. Verify
  zero tool dispatches at the first Send gate. Child and Resume paths also
  remain disabled.
- Run `scripts/verify-additive-upgrade.sh` with the actual release image
  `localhost/calternal-ai:31fb41179460` and the exact candidate image. Keep
  its proof outside the repository in the dev-01 lane directory. The existing
  conformance case covers a space store already present at upgrade and a
  store first opened by the candidate. Inspect both for the new tables and
  confirm the older writer still serves its own data after rollback.

## Running Threads survive server and link interruptions

The owner decision is recorded in [the November 20 notes](../morning-2026-11-20.md).
The behavioral reference is bb 0.50.0, which tracks pending delivery of user
commands across reconnect and waits for idle before automatic updates.
[bb 0.50.0 release notes](https://getbb.app/changelog#0-50-0)

These five cases are pending. Run them through the real Threads Send, queued
follow-up, transcript and protected Stop paths with an installed harness on
an enrolled development machine. Use a deterministic harness that records
each accepted input and emits tagged, numbered output on both sides of the
fault. Keep its evidence outside the server process. Observe the actual
thread status from an attached client and again on reconnect. Record the
attempt ID and generation, process identity and launch count, input and
Stop receipts, output sequences and committed transcript offsets. Use
explicit barriers at delivery and commit boundaries. Stop only the test's
server PID, preserving its data directory and machine process. Dev-01 uses
the shared `/tmp/calternal-dev01.lock` with `flock -s -w 7200`, `nice -n 10`,
at most three build jobs and at most three test threads.

| Test | Fault and pass criteria | Dependencies |
| --- | --- | --- |
| **A. Server dies while the harness is producing output** | Commit an initial output frame, then kill the server while the machine is still emitting. Observe fresh numbered output produced while the server is absent. Restart against the same data and reattach by exact proof with fresh renewal. Require one launch, the same attempt and process, a contiguous transcript containing each offline frame once in order, and no failed status on either side of reconnect. Repeat with the output persisted but its acknowledgement withheld when the server dies. | Slice 3 restores the exact attempt and checks live authority. Slice 4 proves the process and fence survived and retains unacknowledged output on the machine. Slice 1 replays ordered contribution frames and deduplicates the committed boundary. Use Slice 2's writer to verify the final transcript. |
| **B. Sends straddling restart reach the harness once** | In separate runs send a message or queue a follow-up immediately before the kill, after the client has observed the server is unavailable, and immediately after startup while reconciliation is pending. Reuse the same request ID after ambiguous admission or a lost reply. Cut once before admission and again after the harness accepts the input but before its acknowledgement reaches the server. Require one durable intent, one initiating record and one harness acceptance per Send. A queued follow-up retains its FIFO position and waits for the head to settle. The client retains the offline Send's identity until the server can accept it. | Slice 3 owns durable admission, receipt replay and successor promotion. Slice 1 owns contribution dispatch and call replay. Slice 4 supplies exact delivery acknowledgement or deduplication across carrier reconnect. The Threads client path must preserve the offline submission and stable request ID. |
| **C. Confirmed Stop survives and stays on its attempt** | Confirm Stop against attempt A and kill the server after the fence and receipt commit, before remote dispatch or reply. Restart, recover the outstanding Stop, and require A to stop with its committed output preserved. Then let a successor start as B. Deliver a delayed copy of A's Stop after reconnect, including an exact receipt replay, and verify B keeps producing output. Also race a stale A context that never committed against the replacement and require a refusal with no Stop sent to B. | Slice 3 owns the protected target, Stop receipt and durable cleanup. Slice 4 proves exact Stop delivery and teardown. Slice 1 verifies that late fenced frames cannot commit and that previously committed output stays visible. |
| **D. The enrolled-machine link drops mid-output** | Leave the server and machine processes alive and sever only the enrolled link. Verify the machine emits additional tagged output during the drop. Reconnect with a fresh transport, then require the same exact proof, launch count, ordered transcript and nonfailed status as in A. Repeat with a lost acknowledgement and a duplicate frame at reconnect. | Slice 3 reconciles the attempt across the link change. Slice 4 handles carrier reconnect, proof and machine-side retention. Slice 1 deduplicates and delivers the committed output in order. |
| **E. Automatic updates wait for idle and tolerate a forced update** | Query the updater's server status with an active run and during unsettled reconnect. It must withhold the idle result. Let the active work and any runnable successor settle, then observe the no-run-in-progress result using the updater's actual query. Race a new Send against the idle decision and verify it is admitted under a running server or retained for retry. In a separate run, apply the candidate server update while the enrolled harness is mid-output and require A's continuity, order and status checks across the version change. | Slice 3 owns the reconciled run status and admission boundary. The updater consumes that status. The live version-change arm needs Slice 4 proof across the restart and Slice 1 ordered replay, plus the actual old and candidate server artifacts. |

Use independent proof-loss arms for A and D. Change the process identity or
fence generation, expire renewal, or revoke authority while disconnected.
Require the existing freeze and retirement rules to hold. A failed proof
commits interruption before cleanup dispatch. The old attempt cannot thaw
later, and capacity stays reserved until exact teardown is verified. The
positive continuity arms verify actual progress during the outage within
the authorized runtime window. The contract owner needs to pin the outage
budget and how it relates to autonomous freeze, teardown and the update gate.

## Existing primitives

- Files and durability: use the existing per-space SQLite opening and upgrade
  machinery, `calternal_store::migrations`, and the content writer for the
  archive head and file boundary. The current Jobs transaction and content
  publication are separate durability boundaries. Archive needs a shared
  writer transaction with Slice 2.
- Locking: reuse the per-space owner lifecycle guard, per-space serialized
  SQLite writer, and existing blocking pool. Keep the exact state transition,
  receipt and cleanup identity in the commit. Dispatch cleanup after releasing
  the writer and lifecycle guards.
- Validation: reuse the checked installed registry, `space_ticket` live grant
  revision check and existing receipt fingerprint. Reuse the conditional job
  claim and durable wait comparison in the Jobs store. The native lease
  checkpoint shows the run fence, and the machine worker reservation shows
  how to hold capacity through durable teardown proof. Carry those checks into
  the generic engine without depending on native provider input or phases.
- Bounds to verify: two unsettled intent slots per item, the existing per-space
  active-job admission bound, configured receipt retention, and quota-held
  cleanup entries bounded by admitted capacity. Retain unresolved cleanup
  until proof. Measure indexed reads and FULL commits for admission, replay,
  promotion, claim and Stop on the implementation before reporting.

## Native cutover inventory

After draining native attempts and proving parity, remove the native turn
interpreter and its dispatch from `turns/mod.rs`, `turns/run.rs` and
`turns/thread_admission.rs`. Remove native `turns/admission.rs`,
`turns/lease.rs`, `turns/control.rs`, `turns/input.rs`,
`turns/instructions.rs`, `turns/phases.rs`, `turns/progress.rs`,
`turns/reason.rs`, `turns/retry.rs` and `turns/tool_steps.rs` after
porting needed generic authority and teardown behavior. Replace the native
`claim_turn` and placement checkpoint wrappers and remove the native
`agent.turn` restart dispatch. Retire the server-owned title launch and
provider-session wrapper in `threads/title.rs` and
`threads/provider_session.rs`, and the native turn presentation projection in
`threads/turn_events.rs`, `threads/index.rs` and `threads/operations.rs`.

Move live auth and shared placement functions still called outside Turns to the
generic authority or machine code before removing their old homes. Coordinate
`turns/{launch,readiness,redact,verify}.rs` and provider output adapters with
Slice 4, native automation callers with Slice 7, and native route aliases with
Slice 6. Preserve thread files, enrollment and the Inbox. This lane leaves the
native interpreter in place until cutover.

## Simplification opportunities

Use one conditional claim and one exact-attempt retirement path for all
installed execution jobs. Reuse the writer's receipt and recovery machinery
where the same transaction must also change an item. The runtime proof can
then authorize generic execution while the plugin owns titles, phases and
provider presentation.
