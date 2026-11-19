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
authority worker and would skip the required v2 delivery fence. The lane base exposes no per-space
grant revision with invalidation wired through the authority mutations. An
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
2. **Receipt replay evidence for a read-only first method (§8).** Keep
   `item_head` read-only and exercise its fresh admission and stale-ticket
   rejection after reconnect. Use an existing content receipt writer in the
   carrier integration fixture for a separate lost-reply assertion. A
   repeatable item-head lookup has no durable effect to count. Adding a
   mutation to the first shipped broker surface would expand the
   commit-fencing and recovery scope. Keep that surface at one read and
   require the fixture to demonstrate one durable effect on replay.

## Verification and remaining work

On `claude@calternal-dev-01-compute02`, the exact `f3633ca9` source
passed `nix develop -c cargo fmt --all --check` and
`nix develop -c cargo test --locked -p calternal-plugin-host` (22 tests).
After installing the pinned JavaScript dependencies with `npm ci`,
`nix develop -c ./scripts/check-docs.sh` passed workspace and conformance
checks, rustdoc and the JavaScript documentation checks. Rustdoc emitted
warnings in unchanged content and shared-types modules. The server's
existing `cargo test --locked -p calternal-server --test plugin_host --
--test-threads=1` integration suite passed all 18 tests, including
malformed child output and the current worker broker path.

The authenticated duplex endpoint, bounded per-package queues and control
credit, leases and epochs, current-rights tickets, and item-head broker
are still open. The physical mid-frame stall, concurrent callback progress,
immediate local fence, isolated second space, remote expiry and
fresh-ticket receipt replay are spike evidence only. None has been
reproven against product code in this lane. The carrier tests still need to
show control and a second callback within 500 ms p95 while a callback is
blocked, a local fence within 500 ms p95, and watchdog close and independent
Space B progress within 2 s p95 during a physical stall. Both package
processes must stop within 5 s of the last renewal in every run. Replay
must reject prior-epoch frames and tickets and preserve one effect.

## Simplification opportunities

Keep one bounded JSON parser and two typed envelopes. Use existing sandbox
startup and teardown behind a narrow placement interface. Derive the space
from the active assignment and the item target from the ticket so callback
input carries no second source of authority.
