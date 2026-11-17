# Morning decisions and next slice

2026-11-16. Questions were deferred overnight as requested. The localhost preview uses disposable data; it does not resolve these product decisions by implication.

## Decisions to make next

1. **First real-use deployment:** owner-operated/invite-only, or public calternal.cloud? Recommended next step: owner-operated trusted-account deployment, while independently proving the hostile-tenant sandbox before public hosting. The current preview has fixed development tokens, not real sign-in.
2. **Existing data:** must importing current Calternal notes be part of the first daily-use build? Inherited file-first recovery/tiering guarantees remain required; they were not waived. Keep real data out until the inheritance and migration gates pass. The disposable SQLite-backed preview is not a new production storage decision.
3. **First Apple app and device:** Mac or iPhone first, and which reference device? Rust contracts and a generated Swift validation facade now exist. Native primitives/editor work can begin with Claude; functional native networking, sync, cancellation and 120 Hz validation still need a real Apple build and device.

## Engineering work that does not need another product decision

- Bring the existing Notes editor/offline implementation across behind the space-aware contracts; measure account/space separation and reconnect behavior.
- Add the Codex/Claude harness integration and durable execution states. Choose the first enrolled execution machine explicitly; do not silently copy provider credentials. Keep credential-isolation and provider terms gates before delegated/shared use.
- Replace development tokens with passkeys/OIDC and the agreed protected recovery path before real accounts.
- Prove OS-sandboxed TypeScript workers in both the hosted and self-hosted deployment shapes. Runtime permission flags alone do not satisfy this.
- Establish real device/dataset budgets and traces before claiming the 120 Hz target. Current local debug HTTP/worker timings are smoke measurements only. Fresh-process plugin calls are asynchronous and must stay away from per-frame UI work.
- Expand portable GenUI and native client contracts incrementally; keep the browser TypeScript and Apple UI Swift. No Wasm requirement.

## Explicit preview limits

No AI conversations, voice, Honcho memory, bb import, remote machine enrollment, PR lifecycle, full offline sync or production migration are simulated. Activity is browser-session-local, and the Notes editor is a textarea. An externally changed Markdown copy blocks further writes; there is no import/unblock flow yet. Ordinary users should not treat this preview as their primary store.

## Account input retention (issue #17, decided 2026-11-17)

Drafts stay until sent or erased, bounded by count and payload size per account.
Pending submissions stay until sent or cancelled; after seven days the person
confirms before sending. Saved server drafts survive sign-out. Revoked-space
input is never delivered or sent and is removed with access; devices offer local
export and then delete without delaying revocation. No further retention decision
is needed to implement this policy.

[Account input](account-input.md) describes the backend/client-core implementation.
Remaining engineering work includes durable platform offline retention and
sign-out/export integration, general addressed agent-request publication, atomic
snapshot/bookmark leases, and the strict revoke/publication fence. These are not
proofs supplied by the drafts slice.

## Threads follow-ups (issue #28, 2026-11-17)

The grill already decided one Markdown file per thread, with long tool output
kept separately and linked; file splitting is not an open product choice.
The remaining artifact shape needs a decision: reuse generic `/files/<uuid>`
attachments (one existing address kind, requires file upload/materialization),
or declare a Markdown thread-output kind (human-readable logs and versioning,
adds a kind and renderer). Keep long output disabled until that contract exists.

Engineering follow-ups that need no product decision: incremental provider
steering over machine input, tail/block indexing for the accepted one-file
layout, and reducing the inherited repeated content head loads on each send.

The backend currently exposes manifest-owned native thread operations, as Notes
does. The public TypeScript product bridge still needs a contract: a narrow
agent-turn host call (small authority surface, another vocabulary entry) or a
same-authority operation broker (one route for plugins, additional recursion,
deadline and receipt proofs). The lane has not invented that public API.

Cross-turn continuation also needs the turn-engine bridge: reuse a provider session
only for the same account/provider (small context cost, session identity lifecycle),
or hand off explicit conversation context (portable across accounts/providers,
requires a bounded context/summary policy). Current sends admit fresh jobs.

## Standing instructions and requests (owner direction 2026-11-18)

The requests lane implements the narrow `space.instructions` operation under
`space.settings`. It writes the reserved Markdown with a durable placement
intent and space journal receipt. A general protected-file writer is outside
this direction.

- Credential rotation after consuming a once approval. A same-machine provider
  restart can reuse the existing credential bootstrap but interrupts the calling
  tool and needs a receipt-only replay path for the old invocation. A protected
  local credential socket or file avoids the restart and changes how each
  provider and CLI receives its credential.
- Once approval for writers without a durable receipt. An operation can add a
  recoverable intent and receipt before becoming eligible. An outside service
  could supply a stable idempotency key and a result lookup instead, adding
  service-specific delivery and reconciliation requirements.
- Feed recovery across request and bookmark commits. A bounded outbox in each
  source store preserves precise invalidations across a crash and adds recovery
  coordination. An account snapshot on reconnect keeps the inbox authoritative
  with fewer writes and can delay invalidation on an already open connection.
