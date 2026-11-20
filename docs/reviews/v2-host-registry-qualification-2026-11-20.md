# Plugin API v2 host registry qualification

Lane [#93](https://git.kayg.org/kayg/calternal-ai/issues/93). Contract questions
are tracked on [#78](https://git.kayg.org/kayg/calternal-ai/issues/78).

## Installed catalogue

- Install an owner and a contributor through the existing package installer.
  Use the Threads test host's exchange as the oracle. Resolve the exact
  package-qualified app, point, version and kind to the contributor's
  declared operation and pin both installed generations.
- Install a second owner with the same local app ID and distinct
  package-prefixed operations. Give each owner's point a different
  interface and confirm that registration and lookup keep the owners
  distinct. Also attempt to claim the other owner's operation ID.
  Try the wrong owner, point, version and kind, a foreign or missing
  operation, and separate input and output schema mismatches. Confirm any
  rejected install leaves the live catalogue intact.
- Install the contributor from both a directory and a packed archive.
  Reject a missing entry, a changed digest, and an unpinned archive asset.
  Refuse unchecked choices or choices from disabled packages. Confirm
  restart rebuilds the same eligible inventory from stored archives.
- Queue a call, uninstall its contributor, and attempt to claim it.
  It must fail before launch. Reinstall the byte-identical archive and
  pick from fresh choices. The queued call must never switch generations. Keep the
  pre-uninstall catalogue snapshot in the test to catch stale snapshot use.

## Call and spool

- Bind the checked choice to an admitted job and its exact attempt. Match
  the Threads fake host's JSON exchange. Admission pins the contribution
  input. The owning job opens a call, reads output with no cursor, and
  reads again after seq 1 for the terminal. It reuses the call on retry.
- Submit a valid size-boundary input, then an escape-heavy input one encoded
  byte over the limit. Meter the full call and entry frames including LF
  and assigned reference maxima. Expect 413 with
  `contribution_frame_limit` and no admission, prompt write, or launch.
  Reuse a request ID with changed input and attempt a second call with a
  fresh request ID and substituted input. Neither may change the pin.
- Reject a gap, invalid output data, a changed duplicate and post-terminal
  output. A lost producer ACK must replay the committed frame at the same
  sequence. A lost call ACK must preserve the same call across restart.
  A lost read reply must return the same committed suffix for the same
  starting cursor without incrementing the scripted launch counter.
- Commit output, then revoke the attempt before terminal. Reject the stale
  terminal and stale caller reads. Preserve the committed frame and any
  successful owner writes. Exercise both commit orderings when revoke races
  with a frame.
- Drive the spool to its byte and ingest limits. Leave room for one
  terminal frame, enforce the bounded response page and exercise expiry
  under the existing job receipt policy, including on a warm space.
  Park a waiting read without holding a SQLite guard and commit on both
  sides of waiter registration. Recheck the exact attempt before delivery.
- In the cross-slice gate, simulate loss of the owner's committed append
  ACK. Reuse the append receipt keyed to the call and sequence, verify one
  canonical append, and confirm the provider launch count remains one.
  This arm requires Slice 2's item append and Slice 3's writer fence.

## Development gate

Run focused tests and the docs and format gates on dev-01 from this lane's
directory. Run `scripts/verify-additive-upgrade.sh` with the exact lane
candidate and `localhost/calternal-ai:31fb41179460`.
Record the commands, candidate hash, and test counts.

Run a focused migration test that preserves a pre-existing job and receipt,
then opens the new spool through the same startup migration used by the
release inventory.

Reuse the package install row's monotonic revision to distinguish a
byte-identical reinstall. Keep file reads under the installer's bounded
`Folder` reader and SHA-256 asset checker. Keep the scripted spool
transitions transaction-scoped so Slice 3 can supply the exact attempt
fence in the per-space content writer transaction. Keep the broker wait
outside the transaction.

Registry lookup uses the existing atomic catalogue snapshot, one map
lookup and bounded in-memory filtering. It does zero database reads and
zero flushes. Interface validation uses cached offline schemas. Input
preflight uses two bounded JSON encodings and one validation against the
paired input signature, with zero reads or flushes. The installed catalogue
is capped at 128 packages and 32 contributions per package. On the
operator path, the existing immediate package transaction counts the
installed rows, inserts the archive and returns its revision, then
commits once. A refused install rolls back the archive row.

## First qualification

On dev-01, `cargo fmt --all -- --check` passed. The focused Rust runs
passed 14 catalogue tests, three v2 asset tests, and one contribution
preflight test. The tests use a fixture with the same prompt and output
shapes as the Threads fake host, including its offline item-locator
reference. Archive and directory entry pins are verified by the same
installer. No runtime launch is involved in these tests.

This milestone implements the installed registry and preflight primitive.
Job and alarm admission wiring, the durable spool, and call/read delivery
follow this registry-only image in the release check.

The exact ec3ff78c build passed `scripts/verify-additive-upgrade.sh` on
dev-01. The previous image was `localhost/calternal-ai:31fb41179460`
(`563931284f83`). The candidate was
`localhost/calternal-ai-registry:ec3ff78cdf82` (`878fc8a55c28`).
The rollback and forward data-preservation case and all three leak checks
passed. The proof is `~/Developer/lanes/v2-host-registry/upgrade-ec3ff78c.json`
on dev-01. A first run with a lane-deep temporary directory exceeded the
admin socket's Unix path limit. The same image pair passed with a short
private directory under the dev account's home.

## Scripted spool qualification on draft 14.36

The spool uses rusqlite inside the existing per-space content writer's
FULL transaction. The execution engine supplies the exact attempt and
generation check in that transaction. The broker uses the existing
execution command receipt for call reservation. The spool records the
launch claim before the outer broker dispatches and gives reconnect no
second claim. A packet replay reads the bound call and frame and has no
write or flush. A fresh packet reads its call and space credit, writes
the frame and both counters, and has one outer FULL commit. These costs
exclude the execution engine's reused fence check.

Read uses two indexed queries and has no flush. It bounds the page by
16 frames and the remaining broker envelope. Cursor wait and delivery
revalidation belong outside the writer transaction. The spool reserves
terminal capacity within each call's 1 MiB cap. Space storage is capped
at 128 MiB and 4,096 retained calls. Attempt and space ingest credits
come from the pinned catalogue and persist across reconnect. Retirement
starts the configured receipt lifetime, and one sweep removes at most
eight expired calls.

The isolated fixture applies the draft SQL through the additive
migration harness. Production registration awaits the item lane's
confirmation of content.db version 12 on #78. The job and alarm
admission adapters share one preflight gate. Their live entry points and
carrier delivery still need the combined item, execution and carrier lanes.

Dev-01 passed six spool tests, two envelope tests, the installed
admission test, and formatting under the shared lock. The admission
test checks both job and alarm callbacks for zero writes on oversized
input and refuses another app's choice and a byte-identical reinstall's
old generation.

## Simplification opportunities

Keep install and startup on the same asset pin validator. Build checked
choices from the catalogue's existing snapshot so choice reads need no
second installation database lookup. Share the envelope encoding with
Slice 4's entry ABI so admission and dispatch use the same byte counter.
