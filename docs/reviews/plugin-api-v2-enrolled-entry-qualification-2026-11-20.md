# Plugin API v2 enrolled entry qualification

This is the test design for [Slice 4](https://git.kayg.org/kayg/calternal-ai/issues/96).
It follows the 14.33 build plan at `cc13ae21` and the Threads fake-host exchanges
at `6125ad96`. All tests use disposable data on dev-01. Report results against
the exact code and candidate image tested.

## First gate

1. Start an isolated server and enroll a real Podman-backed machine using the
   setup in `tests/conformance/worker_cases.rs`. Install an app and a separate
   contributed harness. Give the harness an operator-provisioned OCI image
   pinned by digest, independently hashed executable entry bytes, no login
   directories and execution network `none`. Use the enrolled carrier and its
   machine-owned teardown rather than calling the entry binary from the test.
   Reuse the TLS relay pattern in
   `crates/server/src/plugins/space_broker/product_fixture.rs` to stall the
   real carrier while leaving the TCP connection open.
2. Admit a job with the exact contribution input. Use the job context to start
   the attempt and issue `ContributionCall`. The entry must receive one
   `ContributionEntryInvoke` followed by LF, carrying the checked call and
   input. It emits an output at sequence 1 and an ok terminal at sequence 2.
   Exercise stream boundaries by splitting one JSON line across reads and
   placing two lines in one read. Check that the entry mount is executable even
   when the package asset lacks that mode, and that it cannot be modified.
3. Read the committed frames with `ContributionRead`, first without `after`
   and then with `after: 1`. Reads may batch both frames. Verify that replaying
   the exact contribution request and restarting the reader returns the same
   call and output, with one runtime launch. Once the owner writer is wired,
   drop an append acknowledgement and verify that replay commits no second
   append and launches no second process.
4. Run a pre-launch refusal matrix for a changed entry, wrong OCI manifest
   digest, changed registry source, unapproved hosted placement, a machine
   enrolled by another actor, and a mismatched attempt ticket. Count launches
   at the actual machine boundary. The fixture must observe zero launches and
   zero login mounts in each refusal arm. Check an escape-heavy input that
   crosses the encoded frame bound on admission and permits no job or prompt
   write.
5. Exercise an out-of-order sequence, changed replay, malformed or oversized
   output, duplicate terminal, output after terminal, and exit without
   terminal. Verify the accepted prefix and a single terminal result, with
   ambiguous or invalid frames producing no owner effect. Fill credits and
   the spool separately. At the spool cap, verify that the failure terminal
   still fits. With credits stalled, check bounded buffering and that control
   and lease expiry can still retire the attempt.

## Placement and lifecycle gates

- Under network `none`, attempt a connection to a fixture endpoint that a
  control process can reach. Verify that the endpoint observes no entry traffic.
  For a later approved scope, test an allowed endpoint and a denied endpoint,
  then narrow the scope while output is waiting for delivery.
- Seed synthetic markers for two profiles and two harness builds. Verify that
  the selected declared directory exposes only its marker. Run wrong-binding,
  symlink and overlapping-mount arms. Check that the entry has no shared tree
  mount and that neither protocol frames nor diagnostics expose the markers.
- Test Stop and revoke between output and terminal with the exact attempt
  ticket. Verify that already committed frames remain recorded and no later
  frame or owner effect is accepted. Keep the successor and quota held until
  the exact sandbox subtree and gateway are verified gone.
- Independently hang and kill the server, supervisor and watchdog using the
  fixture's own process handles. Observe freeze and teardown from outside
  those processes using the exact container identity, the gateway and a
  fixture egress sink. Try renewal from an old generation before and after
  the teardown cutoff. A positive thaw requires fresh, one-use renewal and
  evidence that it refers to the same live execution. After the teardown
  cutoff, no renewal may thaw it. Check leaks with
  `scripts/check-podman-tests.sh`.
- Exercise both cutoff orders from the generic engines proposal. Freeze by
  the earlier of the last supervisor heartbeat plus 2 seconds and the last
  authenticated renewal plus 5 seconds, reserving 250 ms for fencing. Verify
  teardown by the last renewal plus 10 minutes or, if earlier, Stop receipt
  plus 5 seconds. Test watchdog faults both before the reserved work window
  and immediately before each cutoff. Qualify short test clocks separately
  from the final runs at the specified cutoffs.
- Run the same dispatch and replay exchanges using the actual consumer at
  `origin/lane/threads-plugin` when slices 1 and 3 are wired. Qualify a
  supported provider image with the selected test login on an eligible
  personal machine after the scripted run qualifies.

## Existing boundaries

- Files. Consider `fsio::beneath::Folder` for checked package asset reads and
  `fsio::private` for attempt-owned staging. Reuse the Podman ownership
  primitive for removal by exact identity. The native provider-home helper
  establishes private backing directories and is the starting point for
  selected declared login mounts.
- Durability. Slice 1 owns the output spool and receipts. Slice 3 owns the
  durable attempt fence and quota. Use those commits for acknowledgements
  and cleanup reconciliation. Record actual reads and flushes once the
  launch, streaming and teardown paths are implemented.
- Locking. Use the existing per-slot lifecycle gate and bounded carrier
  credit. A stalled output consumer must leave the renewal and local
  deadline paths able to progress.
- Validation. Reuse the pinned shared types, installed package and schema
  validation, carrier byte-bounded encoding, and network admission. Verify
  OCI source and manifest identity separately from the entry asset SHA-256.
  Preserve Podman pull-by-verified-digest semantics. OCI digests and the
  storage engine's local image IDs are distinct identities.

Build and test only under the dev-01 lane directory with
`export TMPDIR=$HOME/gate-tmp` before `nix develop`. Use the candidate
release image and `localhost/calternal-ai:31fb41179460` with
`scripts/verify-additive-upgrade.sh` before reporting a passing first gate.

The entry-side own-file broker exchange is open on
[contract issue #78](https://git.kayg.org/kayg/calternal-ai/issues/78).

## Simplification opportunities

Keep image and entry pinning at the common package and placement boundaries.
Reuse the enrolled carrier for dispatch and delivery, and leave provider
protocol adaptation in the contributed package.
