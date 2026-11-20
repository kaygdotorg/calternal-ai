# Plugin API v2 enrolled entry qualification

This is the test design for [Slice 4](https://git.kayg.org/kayg/calternal-ai/issues/96).
It follows the entry ABI pinned in revision 14.34 and the Threads fake-host exchanges
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

## First implementation boundary

The scripted path uses a source-qualified, already provisioned OCI digest on
an acting account's own enrolled Podman machine. Machine protocol 9 negotiates
the dedicated carrier. The runner verifies its exact ticket, package and digest
before launch. It mounts only the read-only entry, enforces network `none`,
and forwards output to the carrier. Login custody and entry callbacks are
pending. The transport's callers are
Slice 1's contribution dispatch and durable spool and Slice 3's attempt fence
and cleanup reconciliation. The fixed Podman runtime timeout limits this
scripted path. The independent freeze, thaw and recovery deadlines still need
qualification.

The carrier reservation and claim use the existing machine scheduler and
authenticated session. The reservation takes one brief scheduler lock, queues
one control message and adds no database reads or flushes. Claim reuses existing
session authentication. Frame handling has no database reads or flushes. Launch
uses one local image inspection, one in-memory asset hash and one write of the
verified bytes to an exclusive staged inode. Asset staging has no content
flush. Creating the private staging parent on cold setup can sync its parent
directory. These counts exclude the container engine's internal metadata I/O.
Each launch stages one asset bounded by the carrier, with one bounded exec queue
and one 16 KiB line buffer. Verified release removes the stage. Machine startup
sweeps any prior owned containers and stages.

On dev-01, the entry codec suite passed three tests and the machine suite passed
33 tests with its performance-only measurement ignored. The enrolled HTTPS
test exercised a real personal Podman placement, verified the wrong actor,
hosted placement and wrong ticket refusals, and received the ordered output and
terminal from the pinned entry. A Stop arm exercised mismatched and matching
cancel tickets and checked stage removal. The Podman, Firecracker and provider
sign-in leak checks passed. Six unrelated registry tests fail on the base's
legacy reading-list fixture, tracked on [contract issue #78](https://git.kayg.org/kayg/calternal-ai/issues/78).

## Restart and dropped connections

The owner requires an enrolled entry to continue locally while its server is
unreachable. The machine now owns the checked output independently of the
carrier. A fresh carrier can attach to the retained space, ticket, installed
package digest and original launch reservation. The server must persist that
reservation with the exact attempt and machine boot before dispatch. The
replacement receives only frames after the host's highest contiguous committed
sequence. The machine accepts a lost-ACK advance through a submitted frame and
refuses a cursor that skips unsent output or rewinds behind a previous ACK.
The machine returns an explicit attachment proof and the highest buffered
sequence before replay. This bounds catch-up even for a quiet entry.
Carrier credit and commit acknowledgements have separate meanings.

The machine records checked stdout in a private append-only working log and
publishes each complete batch after a data flush. The recorder runs independently
of carrier and commit credit. The relay holds one decoded record and eight
submitted byte offsets. Reconnect translates the committed sequence into a byte
position using those offsets and reads one bounded record at a time. There is no
backlog-dependent in-memory allocation or rescan. Intake uses no database reads
or flushes. It writes and synchronizes batches of approximately 128 KiB or 8 ms
of output. Replay reads one four-byte record length and at most one 16 KiB payload.
The recorder reuses the private-directory and directory-sync helpers from
`fsio`, the shared entry frame validator, and the existing machine carrier's
credits and per-ticket slot lock. The slot lock covers only admission and lookup.
Log creation syncs the log directory once, with one extra parent-directory sync
when first creating that directory.
The machine's available filesystem space bounds the backlog. The scripted
60-second deadline and an exact Stop can end the run earlier. After 24 hours of
continuous server unreachability, the machine stops any live entry and keeps its
log. The same running machine service can still relay retained output after exact
reattachment. A machine service restart preserves the log for inspection.
Local I/O and teardown errors also leave the file for inspection and removal by
the machine operator.
[Recovery handoff and owner decision](https://git.kayg.org/kayg/calternal-ai/issues/78)

The enrolled recovery fixture drops the machine's HTTPS carrier after the first
frame, lets the same sandbox finish writing 320 frames with 3.84 MB of text, and
checks the retained file before proving a fresh signed session for the same
boot. It checks refusal of a different launch reservation, every frame of the
ordered suffix after a lost acknowledgement, and cleanup.
The fixture timed out waiting for the offline sandbox to finish with the
previous in-memory queue.
On dev-01, six machine tests and the enrolled HTTPS fixture passed with the
digest-pinned Node image. Formatting and the Podman, Firecracker and provider
sign-in leak checks passed.
The fixture supplies the commit cursor directly. The product server-crash and
pending-Stop gates require Slice 3's durable runtime binding and Stop delivery
and Slice 1's committed producer cursor. Those dependencies also govern the
product thread's state and an updater's idle check.

Build and test only under the dev-01 lane directory. Set
`TMPDIR=$HOME/gate-tmp` before `nix develop` and take the shared
`/tmp/calternal-dev01.lock` when running builds or tests. Deploys use fresh
installs until the Threads cutover.

Revision 14.34 settled the entry-side broker exchange raised on
[contract issue #78](https://git.kayg.org/kayg/calternal-ai/issues/78).
Exercise the private Unix stream at `/run/calternal/broker.sock` for declared
own-file calls, with correlation by callback ID. Refuse extra identity
fields, undeclared methods, an additional connection, and effects or replies
after the attempt is fenced.

## Provider path

`EntryProgram` now carries the operation's declared host methods, the approved
login directories, the resolved execution network and the run's own deadline
(at most seven days). `EntryProgram::check` refuses overlapping, escaping or
malformed login paths and hostnames before anything is staged.
`entry::harness_declaration` reads one harness contribution's pins from a
checked manifest for the host that builds the program.

- Broker. A declared method list gives the sandbox one stream at
  `/run/calternal/broker.sock` (a read-only mount of a private run directory).
  The machine accepts one connection and decodes each line as
  `WorkerCallMessage`. It refuses extra fields, duplicate open IDs and malformed
  lines by closing the stream. It answers an undeclared method with 403
  `forbidden` and a 129th open call with 429 `retry_after`, both locally. Each
  accepted call gets a fresh carrier UUID, so the host never sees entry
  identifiers. Calls go before output on the carrier. While the server is
  unreachable they wait. After exact reattachment every unanswered call is
  resent in order, relying on the host's request receipts for replay. Stop and
  the end of the run drop the relay first, so no reply follows the fence.
- Logins. The home is a private tmpfs. Each declared directory is mounted
  read-write from
  `accounts/<account>/harness-logins/<package>/<contribution>/<sha256(path)>`.
  Backing directories never nest and are never shared between contributions.
  The machine reads no login bytes. Space-scoped bindings are not accepted.
- Egress. The sandbox always has `--network=none`. For an approved network, the
  only way out is an HTTPS CONNECT proxy at `/run/calternal/egress.sock`,
  advertised as `CALTERNAL_EGRESS_SOCKET`. It admits only approved hostnames,
  or any public hostname for `any_website`, on port 443. It resolves once on the
  machine and refuses the whole answer if any address is non-public, using the
  server's netguard ranges, which moved to `calternal_types::public_address`. It
  allows 16 connections per run and closes with the run.
- Clocks. The run deadline and the 24-hour unreachable-server cutoff are both
  tested on tokio's paused clock, so the tests do not wait in real time.

Each broker call holds at most one 256 KiB frame, with at most 128 open, which
bounds the relay's memory to about 32 MiB. The broker path makes no database
reads or flushes. Login preparation costs one stat chain plus one directory sync
for each new directory.

On dev-01, the enrolled HTTPS test ran three provider arms with
`docker.io/library/node@sha256:e9f565d3…`. It checked one host-answered
callback, a local refusal, a forged field closing the stream, login backing that
persists for one contribution and is fresh for another, and that no egress
socket is offered under `none`. A real Podman egress test saw only `lo`. It
received 403 for an unlisted host and for an approved but unresolvable one, and
a refused direct TCP connection.

Still open:

- The Threads Claude Code package still declares a fixture digest, and its entry
  throws. Its entry must speak this ABI. It must bridge a local TCP proxy to the
  egress socket for `HTTPS_PROXY`, set `CLAUDE_CONFIG_DIR` under a declared
  login directory, and declare its execution network.
- Nothing yet signs a person in or installs the provider CLI into the new
  per-harness backing.
- The server-kill and persisted-Stop tests wait on the durable launch binding
  (#94) and the host-durable cursor (#93).

## Simplification opportunities

Keep image and entry pinning at the common package and placement boundaries.
Reuse the enrolled carrier for dispatch and delivery, and leave provider
protocol adaptation in the contributed package.
