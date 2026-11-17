# Journal following design (unbuilt)

The server remains sync and saved-merge authority. No offline cache, journal
endpoint, subscription, local file mirror or outbox is implemented in this crate.
This design does not depend on the preview content store moving to per-space DBs.

Use one ordered owner per (server identity, account UUID, space UUID). Persist the
opaque signed cursor from `calternal-store` with the local changes it covers in a
single local transaction. Store validates space binding, epoch, entry hash and
rollback witnesses; clients never forge cursors or compare signed strings as
sequence numbers. Keep account/private-profile streams separate from space data.

Subscribe before catch-up, buffer bounded live hints, replay journal pages from
the last committed cursor, then reconcile the buffer by journal sequence. Push is
a hint. Reconnect/foreground always replays; duplicate delivery is idempotent.
A slow consumer or sequence gap triggers catch-up, never cursor advancement over
unapplied entries. Topic filters need server-issued continuation cursors rather
than an assumption that all visible entries have consecutive global sequences.

Expired/rollback cursors require an explicit reset: stop publication, obtain a
consistent authorized snapshot and cursor, atomically replace clean derived
state, then replay. Preserve dirty original bytes, bases and pending request IDs
through reset. Revalidate authority before sending; revocation stops replay and
must never move another account's edits into a new session. Signing out drops the
active credential and network owner; unresolved edits require an approved local
retention/export policy. Unknown formats remain opaque; unsafe merges preserve
both copies. A journal position is not a content durability or room save receipt.

Questions for the user / protocol owner:

- Snapshot/reset protocol: atomic snapshot+cursor (server snapshot cost) or a
  bounded snapshot followed by replay under a retention lease (lease lifecycle and
  explicit expiry handling)? No HTTP journal/reset contract exists yet.
- Local dirty data after revocation/sign-out: retain encrypted private recovery
  data (platform secret and expiry policy needed) or require an explicit authorized
  export before deletion (more interaction, possible device storage pressure)?
- Apple multi-process ownership: one broker process (IPC/lifecycle work) or shared
  transactional storage with fenced leases (recovery and concurrency work)?

These choices do not block the in-memory online operation client. No implementation
or durability, offline, sync or performance qualification is implied by this note.
