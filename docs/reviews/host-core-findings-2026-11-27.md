# Host core, slices 1 and 2 · 2026-11-27

Work and merge blockers are tracked in [Forgejo #115](https://git.kayg.org/kayg/calternal-ai/issues/115).

## Reuse

- Host spike `39016818` supplied the single dispatcher, a per-space
  writer, an authority ledger alongside a rebuildable index, fixed record
  headers and keyed checks of disk record regions. This host verifies
  sessions with `Auth::identify`, looks up current personal-space
  authority through `auth`, and uses the shared account, space, actor,
  error, clock and paging types where applicable.
- Record spike `1cf1172d` supplied the positioned append followed by one
  file flush. The host writes the bytes before acknowledging the SQLite
  commit. It opens the item through pinned directories and reopens that
  inode for writing on Linux.
- `fsio` supplies owner-only state preparation, pinned-directory access,
  the space lock, portable naming, staged placement and directory
  durability. `content/src/publication.rs` supplied the persistent intent
  and replay pattern. Its outbox is coupled to its own item model, global
  catalog and journal, so the new ledger owns the narrower per-space
  intent and receipt together.
- Lane A's API sketch and its engine spike `5e113206` establish the
  split between evaluated definitions and host-owned storage and
  authority. The branch has initial line and expression code. The host
  currently parses the Chat vectors through its temporary adapter.
  Replacing it needs a ready `parse_plugin` and an item writer that can
  preserve unknown YAML, comments and body ranges. For query evaluation
  the engine also needs authorized record positions, viewer-state
  positions and the host's `edited` metadata. For runtime operations it
  needs typed inputs and an answer for the operation's checked targets.

## Choices and open questions

- **Format §2.9 and §4.3, coordinator's eleventh set.** Use the stable
  ledger ordinal for `count(records after state.<key>)`. Preserve the
  viewer's ordinal when their marked record is removed. The Chat
  fixture's two replacement lines are `read through: record anchor`
  and `unread: = count(records after state.read through)`. The checked-in
  fixture still uses its timestamp expression. The host exercise verifies
  the proposed lines on a disposable copy as well as the checked-in
  vectors. Operation inputs, send-and-start and program binding are
  handled by the format, plugin process and runs lanes.
- **Format §2.8 and §8.1.** Record writes carry `{fields?,body?}`. Edit
  overlays submitted fields and retains omitted fields and body. The
  ledger retains the original author, time and ordinal. Redaction keeps
  stamp metadata and removes the record's live bytes. A record-shaped
  outside region is presented with `author: outside` and its content is
  withheld. Further writes to that item refuse until the disk contents
  are reconciled. The Markdown header uses the authoritative account ID
  as a display fallback. A display-name lookup needs to be attached to
  the trusted actor context before integration.
- **Build plan slices 1 and 2, authority.** The public route admits only
  the owner of their single personal space. The two-writer test starts
  with two authorized ledger contexts to exercise stamping and non-author
  refusal. A person outside that route's scope cannot use an item UUID
  to discover or change it. Shared-space access stays with the later
  sharing slice.
- **Format §2.3, §3.2 and §9, item placement.** The temporary adapter
  supports a literal kind folder, the default title pattern and a simple
  field pattern. Changing the title updates that placement and retains the
  item ID. Full link rewriting, size and character validation of
  declarations, and byte-preserving YAML edits need the engine and
  additional tests. Case-only moves keep the existing path on this lane.
- **Format §1.1 and §1.1a, installation.** The local operator attests
  official bundles and the host pins installed content by a keyed digest.
  Coordinator question before exposing installation to clients: what
  attestation binds a plugin ID to its source, and which host proof
  represents approval of risky installs? A locally selected directory is
  only sufficient for the privileged developer path. Updates and wider
  declarations stay disabled here.
- **Format §8.1, crash between append flushes.** The normal path publishes
  the durable file first and then commits its verified offset, author,
  history and receipt in SQLite. After power loss in that interval the
  item fails closed with an uncommitted tail. The coordinator needs to
  decide whether to pay for a durable pre-intent, or accept the
  two-flush path with a provable recovery marker. A pre-intent protects
  the ability to continue writing after interruption at the cost of an
  extra flush. Without one, the host cannot safely distinguish an
  incomplete append from changes made outside the host. This blocks
  merge of the append path until the policy and recovery are settled.
- **Review gate 7 and format §2.20.** Receipts default to 7 days and use
  `CALTERNAL_RECEIPT_RETENTION_DAYS`. Item removal goes to 30-day Trash.
  This slice checks expiry in bounded batches on access. Historical
  author metadata for live records and item heads stays attached to
  those objects. The slice's separate append-only action log is pruned
  with receipts for now. Coordinator decision needed on longer
  action-log retention and a Trash expiry worker for inactive spaces.

## Verification

Run on `claude@calternal-dev-01-compute02` from
`~/Developer/lanes/host-core`, using `~/bin/lock-shared nix develop -c …`
for host checks. The test and syscall receipts will be recorded here.

## Simplification opportunities

- Delete the local declaration and expression adapter when the format
  engine can validate the complete Chat bundle and return indexed rows.
- Keep record positions in the generic ledger and use the same ordinal
  for unread counts, page anchors and later live subscriptions.
- Reuse the publication intent for additional transactional placements
  without adding a second history database or storing Markdown in receipts.
