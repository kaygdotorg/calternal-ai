# Private drafts and pending submissions

Implemented by lane/drafts for issue #17, 2026-11-17. Account identity comes only
from the authenticated session. These operations never accept an account selector.
The definitions in `calternal-core` generate the schemas and TypeScript types.
This is a disposable-data development preview.

## Operations

All calls use `/api/v1/operations` and its ordinary envelope.

| Operation | Input | Result |
| --- | --- | --- |
| `drafts.save` | `space_id`, `item_id`, `text` | `record`, server `revision` |
| `drafts.list` | `{}` | `records` |
| `drafts.get`, `drafts.delete` | `space_id`, `item_id` | `record` or null after deletion |
| `pending.queue`, `pending.edit` | `space_id`, `item_id`, `created_at_ms`, `request` | `record` |
| `pending.list` | `{}` | `records` |
| `pending.cancel`, `pending.confirm`, `pending.submit` | `id` (underlying request ID) | `record`, `result` |

Single-record replies always contain both `record` and `result`; absent values
are null. List replies contain `records`. Queue and submit retries of completed
input return the underlying ordinary receipt under `result`, without retaining
another copy of its content in the account database. Current write access is
checked before replay. Reusing an underlying request ID with different input is
a conflict. No terminal replay promise is added to cancel or draft replacement.

A draft key is the account and target item UUID, with the target's space UUID
checked on each operation. Item IDs are globally unique. A save is a complete
replacement: the last server-accepted write wins and returns a new opaque revision.
There is no merge engine, base-revision check or time expiry. A draft requires
current space read access and remains private even from other members of that space.

Pending envelopes carry their original creation time in Unix milliseconds and
stable write request ID. They must contain their full space UUID in the nested
input; an item-scoped write's `id` must match `item_id`. The normal dispatcher
resolves the real item and checks its scope again before writing. Only content-write
operations may be queued; account-input, directory and secrets operations cannot
be nested. Queueing checks current write access. An edit preserves identity,
target and original creation time.

After more than seven days, a queued record becomes `confirmation_required`.
This is a request addressed to the signed-in person, visible in `pending.list`
and the account feed. Submit returns that record without sending it. Confirm
attempts that exact envelope, after rechecking access; cancel removes it.
A newer draft written after queueing survives successful submission: only the
captured draft revision is erased. An envelope becomes immutable once an attempt
starts, including an uncertain outcome; retry the original request ID. A process
restart retries against the ordinary operation receipt, so it cannot repeat a
committed write. Pending records are removed only after successful dispatch or
explicit cancellation, and revoked-space input is erased rather than sent.

## Storage and ceilings

`appdata/accounts/<account-uuid>/input.db` is an owner-bound SQLite database,
mode 0600 under 0700 directories, following `calternal-secrets::LocalStore`.
It contains current input and content-free invalidations. It is appdata, not a
space file or userdata JSON exception. The ordinary per-space journal owns sent
write receipts. Each call opens only its account database and closes it again;
blocking workers handle SQLite, directory preparation, hashing and file locks.
Writes use IMMEDIATE transactions and FULL durability, committing state and its
invalidation together. There is no per-account worker/thread pool or global input
mutex. The inherited content receipt router is consulted for completed retries.

Proposed default safety ceilings (operator environment, read once at startup):

| Variable | Default | Meaning |
| --- | --- | --- |
| `CALTERNAL_DRAFT_COUNT` | 256 | Saved drafts per account |
| `CALTERNAL_PENDING_COUNT` | 256 | Unsent pending submissions per account |
| `CALTERNAL_INPUT_BYTES` | 16777216 | UTF-8 draft text plus serialized pending envelopes per account |
| `CALTERNAL_ACCOUNT_EVENTS` | 1024 | Retained content-free invalidation bookmarks per account |
| `CALTERNAL_LIVE_SPACES` | 16 | Space journals per server connection |
| `CALTERNAL_LIVE_LISTENERS` | 128 | Live connections per account or space |

Each value must be positive and fit a signed 32-bit integer. Individual draft
text still has the core document limit; operation envelopes retain the 2 MiB
transport limit. Bounded record metadata is additional to the input payload
budget. Lowering a ceiling preserves accepted input and allows equal-size or
shrinking replacements; growth is refused with a `too_large` error naming send,
erase/cancel and the operator settings. State-only age transitions do not grow
the payload budget. These safety ceilings do not implement group entitlements.

Account connection admission uses empty, reused owner-local advisory lock files,
held by the HTTP body and released on disconnect. Their count is bounded by the
highest configured listener ceiling; there is no stale process lease after a crash.
A subscriber buffers one 64-token page. The client buffers at most 64 KiB for the
account feed, 2 MiB for the space feed and 256 offline envelopes.

Membership removal, leaving and whole-space deletion commit authority first,
then erase affected accounts' input without holding the preview Store mutex.
Revocation does not depend on erasure succeeding. Account operations and active
account streams also check current access and erase inaccessible records, covering
changes made through independently opened directory handles. Idle account files
changed outside the HTTP path need that next access for physical cleanup. Sign-out
does not delete saved server drafts. Local offline retention and export/deletion
after revocation remain platform responsibilities; see the lane report.

## Account stream and client core

`GET /api/v1/account/live?bookmark=<opaque>` follows the signed-in account.
The optional bookmark is an unpredictable server-issued UUID, bound by membership
in that account's retained database history; it is never a caller-selected position.
SSE `data` contains `AccountFrame`: `changed`, `ready`, or `reset`. `changed`
invalidates the private account snapshot; reload `drafts.list` and `pending.list`,
then accept its bookmark. Events contain no draft text, message body or space ID.
Bookmarks from another account, pruned history or a rolled-back/missing event
produce `reset` and close the subscription. Reload authorized state before opening
without a bookmark. There is no atomic snapshot/bookmark lease in this slice.

Account polling replays indexed pages every 250 ms, rechecking the current session.
Expiry scans use a state/time expression index and decode only newly due
records; idle polling never deserializes every pending envelope. Bookmarks remain
stable across a server restart. No instantaneous cross-process publication fence
is claimed.

Rust `Client::subscribe_account` and `AccountConnection::{next,accept}` supply
cancellable consumption. `resume_after_reload` explicitly abandons a reset
bookmark after authorized reload. `FileBookmarks` reserves `account.md` in its
existing account-exclusive cache root. Native `subscribe_account` exposes the
same pull/accept boundary through UniFFI, with one outstanding frame.

Every authorized reconnect drains existing server pending input and the in-memory
`OfflineOutbox`. Outbox entries retain their offline creation time, full target
scope and request ID, and first become visible server pending input. Old entries
are transferred as confirmation requests, never sent automatically. Refusal or an
uncertain transport outcome keeps the local head. `OfflineOutbox::submissions` and `restore` (native `pending_input` / `restore_input`)
expose retention without refreshing the offline clock. Platform persistence must retain
the complete `PendingQueueInput`, including creation time; rebuilding it with a
fresh timestamp would bypass the age check. A device must not move it between
accounts. This lane does not claim platform durable storage or sign-out wiring.

## CLI

Both `calternal drafts ...` / `calternal pending ...` and `cl drafts ...` /
`cl pending ...` use the same operations and shared output module. Human tables
adapt to terminal width; `--json` retains full input/result records. Hints follow
the shared terminal/`--no-hints` rules.

```sh
calternal drafts save <item-uuid> --space <space-uuid> --text 'Unsent text' --token-file <private-file>
calternal drafts list --token-file <private-file> --json
calternal pending queue pending.json --token-file <private-file>
calternal pending list --token-file <private-file> --json
calternal pending confirm <request-id> --token-file <private-file>
```

A queue/edit input file holds the four fields in the operation table. `request`
is the ordinary validated write envelope; its stable `request_id` becomes the
pending ID. Use `-` for stdin. Never put a credential in that file or argv.
`--request-id` identifies the outer CLI operation; it does not replace the nested
write identity.
