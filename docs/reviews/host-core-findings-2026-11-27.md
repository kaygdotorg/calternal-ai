# Host core, slices 1 and 2 · 2026-11-27

[Forgejo #115](https://git.kayg.org/kayg/calternal-ai/issues/115) tracks this lane.

## Reuse and review

- Host spike `39016818` supplied the dispatcher, per-space writer,
  authority ledger, record header and keyed disk checks. The host uses
  `auth` for bearer-session and current personal-space authority checks,
  and `types` for IDs, actors, caller-safe errors, time and paging.
- Record spike `1cf1172d` supplied the positioned write and file data
  flush. The host pins the item using `fsio` directories, then reopens
  that inode on Linux for the positioned write. The only lock held
  during durability work is scoped to the space. HTTP and asynchronous
  worker adapters move blocking work off the async executor.
- `fsio` also supplies a private state directory, the process lock,
  portable names, durable staged placements and directory flushes.
  `content/src/publication.rs` supplied durable intents and replay. Its
  outbox is coupled to its item model, global routing and sync journal.
  The host uses one rusqlite ledger per space to commit its heads,
  stamps, receipts and publication intent together.
- `calternal-format` compiles bundles once on install and once on open.
  It parses item frontmatter, preserves untouched YAML on write,
  validates saves and evaluates authorized rows with record stamps and
  viewer state. The host supplies its UUIDs, times, permissions, file
  placements and receipt keys. An empty compiled collection is cached
  for ad hoc queries, since the engine API currently constructs
  collections through the bundle parser.

## Choices and remaining boundaries

- **§1.1, developer installation.** The local operator attests the
  directory and any reserved official plugin ID. The installed bytes
  are pinned by a keyed digest. Source attestation and passkey approval
  are after the goal. Installed code stays dormant and updates are
  disabled. The isolated install probe and exact slot-selection
  authorization will be hooked up with the worker installation path.
- **§2.3, §3.2 and §9, item placement.** The host accepts literal kind
  folders and simple filename fields. A rename uses the same durable
  publication and retains the ID. General link rewriting and
  case-only moves still need a placement and link-index pass. The
  host refuses unsupported placement declarations at install.
- **§2.8, §4.3 and §8.1, records and authority.** The public route
  admits the personal-space owner. The two-writer test starts with two
  separate trusted ledger contexts to verify authorship and refusal
  of another author's edit or removal. The Markdown displays the
  authenticated name, escaped for the header. The ledger supplies
  author IDs, anchors and original timestamps. Record edits overlay
  submitted fields and preserve omitted fields or body. An outside
  record gets an `outside` stamp and its contents are withheld.
- **§2.9 and §4.3, coordinator's eleventh set.** Unread counts use
  the ledger ordinal. The engine receives the number of live
  records at or before a saved position, even after the marked
  record is removed. Replace the Chat fixture's `read through: time`
  with `read through: record anchor` and its unread formula with
  `count(records after state.read through)`. The test exercises
  that version on a disposable copy. The checked-in fixture's time
  formula continues to parse.
- **§8.1, coordinator's twelfth set.** The worker wire uses `op` tags.
  Append receipts include an anchored Markdown `link` with a
  percent-encoded, space-relative path. The anchor is the durable
  reference if the item moves. The SDK can carry the link to the
  selected harness without rebuilding it from a path.
- **§2.8 and §8.1, coordinator's thirteenth set.** The header ends in
  ` <!--ca1 <22 base64url characters>-->`, a **34-byte** suffix.
  The characters encode a 128-bit truncated keyed BLAKE3 tag over
  the item, ordinal, byte offset, actor and complete original record
  bytes. An idempotent append derives the existing 16-character
  Crockford anchor from the gesture with the host key and binds that
  anchor in the tag. Unkeyed appends keep random anchors. The file
  carries neither the host key nor the raw gesture key. The personal
  space allows recovery to derive the owner and test shell or
  owning-plugin provenance. Broader writer populations will need an
  authoritative actor source before this recovery path can admit them.
- **§8.1, crash adoption.** Recovery re-syncs a complete verified tail
  before committing its head, stamp and tag-keyed receipt. The saved
  retry key proves itself by regenerating the anchor, and the host
  compares the normalized record bytes before promoting the receipt
  to the usual key. Torn, altered and displaced tails remain outside
  changes. A recovered timestamp has the UTC minute stored in the
  Markdown. The ledger ordinal determines transcript order.
- **Review gate 7 and §2.20.** Receipts and the action log have the
  configured seven-day default. Item removal has a 30-day Trash
  period and expiry runs in bounded batches on access. The worker
  for inactive spaces comes after the goal. Live heads and records
  retain their stamps. Staged Markdown is removed after publication
  and is never stored in the ledger.
- **Fourteenth set.** The JSON route preserves base-ten decimal
  digits across the format boundary. Plugin assets and frames are
  outside this lane's install scope. Unclassified bundle entries are
  refused instead of using their extension to grant data-only status.
  The old server's remote-media classifier is private to its proxy,
  so adding asset support needs a shared content classifier.
- **Engine API gaps.** The pure record reader now skips the opaque
  header suffix. The host verifies it against the ledger. The engine
  does not expose its field codec for an isolated record
  write or an ad hoc collection constructor. The host uses its
  compiled record field declarations and one cached collection
  template. Save-time contexts for more complex rules need state
  and authorized link lookup data. Slot inputs, event delivery and
  scoped runs attach in their assigned lanes.

## Verification

All commands ran on `claude@calternal-dev-01-compute02`, in
`~/Developer/lanes/host-core`, under the plan's gate locks and
`nix develop -c`.

- At `1f6fb741`, the first six host tests passed using the real Chat
  fixture. At `d224fa50`, 13 tests passed, including roll-forward
  publication and append recovery.
- After merging the workspace format engine, `cargo test --locked -p
  calternal-format -p calternal-host -j3` passed 22 format tests and
  16 host tests. Workspace formatting, strict crate Clippy and crate
  Rustdoc passed as well.
- For 1,000 warmed, idempotent appends, `crates/host/count-flushes.sh`
  reported **0 item-content reads, 759 ledger read syscalls, 0
  authority read syscalls, 1,000 item data flushes and 1,033 ledger
  flushes**. The extra 33 ledger flushes come from periodic SQLite
  WAL checkpoints. The normal append commits one file and one ledger
  flush. The trace uses the trusted caller entry point. It includes
  the authority queries but skips HTTP session resolution.
- At the logical level a warm append reuses the cached item head and
  makes two authority probes, an indexed receipt lookup and an indexed
  recovered-receipt lookup. An unkeyed append skips the last two. The
  cold append verifies the existing file and ledger stamps before the
  positioned write. Query evaluation reads the cached fields, record
  stamps and viewer state without loading Markdown. A direct read loads
  the item once before matching record spans against their stamps.

## Simplification opportunities

- Expose an ad hoc collection constructor and a field codec in the
  pure engine to remove the host's template and type adapter.
- Keep query index fields in the engine's typed representation
  and convert to JSON only when assembling a response.
- Use the same ledger ordinal for unread positions, record paging
  and later live subscriptions.
- Reuse the publication intent for additional atomic placements
  without introducing a second journal or copying Markdown into receipts.
