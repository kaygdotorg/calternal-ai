# Per-space channel and broker — 2026-11-19

Issue [#86](https://git.kayg.org/kayg/calternal-ai/issues/86) is open. The
change pushed in `f3633ca9` is strict decoding on the existing private
worker pipe. The server's worker adapter already calls `decode_frame` on
the bounded line and stops the affected process on a decoding error. The
decoder now rejects invalid UTF-8, duplicate keys after escape decoding
and more than 32 object/array containers on one path. It parses bytes once,
then maps the checked values to the existing typed envelope. Cost per frame
is linear in input size, with zero filesystem and database reads and zero
flushes. The process adapter's existing pipe I/O is unchanged.

## Reuse and integration

The paused attempt supplied the recursive Serde visitor. Its outer
`plugin_id`/`plugin_revision`/`invocation_id` header and message-level
ticket differ from §3's `package`/`digest`/`ticket` header. Its
`HostCallKind::ReadItem` uses the private `read_item` contract, which
looks up the global catalog route and loads a full item through the current
store. V2 `item_head` is a
metadata-and-revision read. The paused header, sequence helper tied to that
header, broker wire enum export and tests keyed to that format were left
out. The duplicate-key, nesting and invalid-UTF-8 cases were applied to the
live child decoder. The existing bounded encoder and worker wire vectors
still apply to the private format.

The machine pieces to reuse are the enrolled Ed25519 session, bearer
authentication, capacity, placement and sandbox teardown. Use
`machine::Link` for the configured origin and TLS peer verification
and the existing worker sandbox factory for package startup. A space
channel needs authority derived from a live assignment to the enrolled
machine. A bearer proves machine identity, while the assignment binds
the machine to the particular space. The spike's loopback listener and
direct subprocess startup are disposable test mechanisms.

The current server worker transport sends stdin through
`MachineService::interactive_input` and uses the machine output service
for stdout. This takes the installation-wide machine registry lock on
frame traffic. Keep those APIs on the existing worker path while building
the new carrier. Use enrollment and assignment checks at handshake and
deliver lifecycle invalidation to the per-space endpoint so subsequent
channel traffic can stay independent of the machine registry.

`Spaces::capabilities` and `capabilities_async` both read on the shared
authority worker. The room cache epoch in `calternal-types` is
installation-wide and is explicitly a cache fence. The v2 delivery check
needs a per-space grant revision tied to actual grant changes, alongside
session, activation, cancellation and enrollment invalidation. Calling
the current broker through the new socket would still use the shared
authority worker and would skip the required v2 delivery fence. The lane base had no per-space grant revision with invalidation wired
through the authority mutations. Revision 3 below adds that handle. An
`item_head` adapter can build on content's indexed live metadata
and revision access. The adapter should return both from one space-scoped
head lookup and hide a wrong kind, missing item or foreign target with the
same opaque error.

## Proposed numbered revisions for morning review

1. **Carrier bootstrap and control grammar (§3).** Define the private
   handshake and how the exact application header represents space-wide
   control without a package or invocation. A renewal that borrows one
   package's ticket would stop working when that invocation completes.
   Specify the client's supported major, host-issued epoch, assignment
   binding, lease duration and both credit windows. Define explicit
   space-scoped controls for renewal and fencing and package-scoped
   credit return. Preserve one sequence per direction across data and
   control. The conservative implementation choice is the enrolled
   connection's TLS origin and a bounded upgrade handshake, with no
   authority on unknown control or failed binding. The alternative is a
   separate authenticated listener, which needs its own deployment and
   peer-verification configuration. Review the exact grammar before a
   second implementation starts depending on it.
2. **Receipt replay for a read-only first method (§8).** Keep
   `item_head` read-only and cover fresh admission and stale-ticket refusal on
   reconnect. A proven receipt replay must send the same request ID through
   carrier admission with a new ticket and observe one durable effect. Issue
   #86 tracks that work. The direct fixture query into the receipt store was
   removed because it would pass without a carrier.

3. **Delivery grant revisions (§3).** Track awake space assignments in the
   shared auth writer, bound to the enrolled machine UUID. Connection-local
   SQLite triggers mark the affected handles before authority writes.
   Publish a new clean revision after the write job returns to autocommit,
   including after a rollback. Close the handle when its machine changes
   enrollment, its assignment moves, or the auth worker exits. Session
   deletion and updates to revocation, expiration or ownership invalidate
   the relevant spaces. New sign-ins and token rotation preserve existing
   tickets. The first carrier deployment uses the installation's one
   root-locked authority writer. This adds no on-disk authority copy.
   Tickets bind the handle and runtime epoch as well as the revision.
   Revisions may restart with a new handle. The carrier must separately
   verify assignment and machine session identity.

The frame path compares the retained revision using one atomic load, zero
storage reads, zero flushes and no shared worker or lock. Assignment setup
uses one auth-worker job and a statement with two primary-key existence
probes. Authority writes retain their existing commit and flush count.
Space-scoped triggers use a memory lookup, and account, group and role
changes use indexed relationship queries to identify affected spaces.
The tracker is bounded to 1,024 active assignments and reclaims dropped
handles before refusing a new assignment. Its registry mutex only covers
in-memory operations on the write and assignment paths.

4. **Enrolled upgrade and lane windows (§3).** Reserve one carrier for a
   space on the existing worker placement and send its single-use nonce
   over machine sync. The runtime uses HTTP/1.1 Upgrade on the configured
   TLS origin with the current enrolled bearer and redirects disabled.
   The host checks machine, exact session, space, lease fence and reservation
   before the stream receives authority. The first bounded LF object offers
   major 1 and echoes the binding. The host confirms the same binding,
   fresh epoch, package pins, four-second lease, and initial credit windows.
   Use explicit null package, digest and ticket for space-wide renew and
   fence. Package-scoped credit returns encoded bytes (including LF) and
   frame slots. Control is prioritized before unsent data. Package lane
   credits are bounded to two maximum encoded frames and eight outstanding
   frame slots. The control window is 4 KiB, including one coalesced credit
   return. The writer owns the one direction-local sequence. It reserves the
   largest sequence encoding at admission, then refunds the unused bytes at
   write time. The receiver charges the bounded line to the space receive
   budget before decoding and settles credit against the package window.
   Socket write and flush share a 300 ms deadline. Local fencing and the
   remote four-second lease run independently of application callbacks and
   physical writes.

Carrier processing reads and parses one bounded NDJSON object at a time.
Each sent object uses one write-all and one transport flush. The transport
runs entirely in memory, without SQLite reads or storage flushes after
setup. It checks the registered pin and package credits under a short
per-space memory lock. Setup uses existing machine registry and authority
lookups. The first machine placement attaches one package. The carrier
can allocate distinct windows for up to eight pinned packages.

5. **Read invocation tickets (§3).** Sample the existing protected rights
   and sign-in at admission. Require both space read and plugin use. Bind
   one opaque ticket to the actor, installation, package digest, invocation
   operation, item and kind, the assignment's retained grant handle and
   revision, and the channel epoch. The audience is the admitted actor.
   Lifetime is 60 seconds from mint, shortened to the remaining installation
   lifetime when needed. Thread delegation and item-only sharing stay outside
   this first scope. Authorization compares the saved handle revision and
   expiry on callbacks and delivery. Each invocation after reconnect takes
   fresh rights and receives a new ticket.

After the caller's identity is verified, warm admission takes one capability
query and, for an installation session, one primary-key query on the space's
retained read-only WAL connection. Both queries share a snapshot. Admission
checks the grant revision before and after the snapshot. Each delivery check
uses one atomic grant load, one monotonic time sample and local comparisons.
Warm admission and delivery incur zero storage flushes and use neither the
machine registry nor the auth worker.

6. **First item-head operation (§4).** Register a trusted, space-specific
   operation and verified no-egress worker source through the host's startup
   registry. The normal operation dispatcher can select it. Existing registry
   operations keep priority. The first profile admits one pinned package
   per placement and one item callback per ticket. The terminal result echoes
   the checked head. Its callback only accepts the ticket's exact kind and
   item. The machine starts the package with the existing worker sandbox
   and relays it on a capability-free child pipe. The child sees its own
   invocation ID and callback ID. The supervisor alone retains the ticket.
   The host rechecks the grant at the content query, on its return and when
   dequeuing outbound replies. Guarded writes also recheck on each write and
   flush poll. Revocation aborts a partially written line and closes the stream.
   The normal manifest installer remains on the
   existing path until it can supply validated v2 approvals.

A warm item-head callback reuses Content’s per-space visibility pool and performs one
indexed metadata/revision read, zero catalog or shared authority reads,
and zero storage flushes. The callback loads metadata and revision only.
Cold wake prepares the recovered space and keeps a weak reference to its
cache entry. An evicted slot opens through the normal recovery path. A
callback holds a lifecycle permit while borrowing the slot. Erasure drains
those permits and fences the carrier via the existing placement retirement
before removing the cached SQLite connections. The first pool checkout may
open a read-only content connection. Cold preflight uses
the authority worker before placement. The enrolled upgrade uses it once
more to bind the verified assignment to a grant revision. Ticket mint checks
authority again after package startup.

7. **Fault-test boundaries (§8).** Limit kernel buffers and hold the receiver
   after observing a physical prefix. Require the watchdog to close before
   delivering the newline. Exercise explicit fencing on an unstalled
   carrier. Keep the TLS peer open during the lease test so EOF and cancellation
   cannot explain the process exit. Exercise a fresh read after reconnect
   and refuse a ticket that was valid on the previous epoch.

8. **Awake reuse (§§2–3).** The initial profile places one pinned package per
   space, holds one enrolled channel and multiplexes up to four tickets.
   The scheduler admits up to 32 active or reserved carriers.
   A space-local wake gate coalesces placement and waits up to the existing
   ten-second worker startup limit for Ready. Reclaim the placement after
   60 seconds without an application frame when no ticket is in flight. This keeps the placement bounded
   and preserves worker and machine capacity. A longer idle period would
   reduce cold starts at the cost of reserving a machine slot longer.

9. **Shared package credit (§3).** Bound multiplexed concurrency to half
   the package frame window and bound each encoded Reply or Complete to one
   quarter of the byte window. The host and machine wait for data credit
   without taking the control path's reserved capacity. The machine stops
   draining package stdout while it waits. This bounds memory to one pending
   frame per host ticket and one for the machine relay.

## Verification and remaining work

The following milestones were verified on
`claude@calternal-dev-01-compute02` under `nix develop -c`.

| Commit | Verification |
| --- | --- |
| `f3633ca9` | Existing child decoding and its rejection tests. |
| `b5d0e5fa` | 107 auth tests. An in-flight session write fences its affected space before commit. An unrelated space stays current. |
| `66fb93ad` | 24 plugin-host tests, server all-targets/all-features check, and single-use enrolled assignment regression. |
| `e0246791` | 107 auth tests and ticket denial for missing plugin-use rights, wrong epoch, expiry and a moved grant revision. |
| `03e173d7` | Three broker regressions and 24 plugin-host tests. The normal dispatcher, enrollment and server-side HTTP Upgrade exchange reach a real Node process. The reconnect fixture rejects the old ticket and admits a fresh read. |

The physical carrier tests cover an explicit fence without a stalled write,
a mid-frame stall with the write watchdog, and an old-epoch frame carrying
the expected next sequence. A two-package transport fixture holds one callback
and checks control priority and the second package. Another test checks that
a stale queued reply consumes neither delivery nor sequence, including a
physical interruption after a guarded write puts out a prefix.

The Linux integration fixture uses the normal dispatcher and server router,
one scheduler and enrollment, the machine runner and TLS Link upgrade,
and the direct sandbox's process startup and teardown. It uses an isolated
loopback TLS relay and supplies the absolute Node executable to the direct
sandbox fixture because worker execution clears PATH. It exercises four
concurrent tickets with the largest admitted Reply on one placement, a
second space sharing those components, package reaping after loss of
renewals, and closing the carrier and SQLite handles on space erasure. The bounds are regression
assertions on development infrastructure.

Open on issue #86 are receipt replay through carrier admission and
ticket fencing on package disable or uninstall. The approved v2 read route
has no production caller. The SDK and package integration lane
([#84](https://git.kayg.org/kayg/calternal-ai/issues/84)) can supply validated
v2 approvals. The first product profile has one package per awake space.
Distinct package windows have transport coverage. A placement serving
multiple package processes remains future work.

## Simplification opportunities

Keep one bounded JSON parser and two typed envelopes. Use existing sandbox
startup and teardown behind a narrow placement interface. Derive the space
from the active assignment and the item target from the ticket so callback
input carries no second source of authority.
