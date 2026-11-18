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
- Feed recovery across request and bookmark commits. A bounded outbox in each
  source store preserves precise invalidations across a crash and adds recovery
  coordination. An account snapshot on reconnect keeps the inbox authoritative
  with fewer writes and can delay invalidation on an already open connection.

## Plugin worker placement and network contract (issue #29, 2026-11-17)

- Decide how shared-space workers receive trusted placement and capacity: an
  operator-owned service or explicitly space-trusted participant machines.
  Pending that contract, remote workers use operator pool machines only.
- For a server host without a container runtime, choose explicit enrollment of an
  operator machine or an approved automatic setup flow; there is no direct fallback.
- Define operator recovery for a permanently lost/revoked worker machine; pending
  teardown holds capacity until authenticated cleanup proof, without a timeout.
- Define manifest destinations, manager approval and brokered fetch together;
  ManifestV1 currently has none of these, so worker network remains disabled.

[Worker execution](plugin-workers.md#placement-and-unresolved-contracts) lists the
options, implementation costs and the placement revocation problem.

## People and access follow-ups (issue #37)

- **Server roles held by groups.** Keep the current server-role assignment with
  the installation operator, which limits online escalation but needs local
  provisioning, or add a protected role-grant operation with its own safeguards
  for the last administrator.
- **Local group managers.** Server group administrators can manage local
  memberships in the preview. Expose the source ADR's delegated group-manager
  workflow with its visible-people constraint, or keep group membership changes
  operator-scoped if attaching a group to a space should keep that tighter
  boundary.
- **Disabled owners.** Disabling preserves the sole Owner record and makes its
  grant inactive. Require transfer of shared spaces first, which can block
  emergency disable, or add a separate recovery action with explicit policy for
  shared and personal spaces.
- **Invitation lifecycle.** Space invites support the requested role and expiry.
  Extending expiry in place keeps the issued link alive, while revoking and
  replacing issues a new credential. Server-plan invitations carrying group and
  quota grants need an explicit server-scoped enrollment policy.
- **Visible people picker.** A visibility-filtered picker can help non-admins
  grant access to people whose accounts they can see. Inviting by link keeps
  the directory smaller and requires no lookup by name.

## Turn readiness (issue #39)

- Pool refresh scope. A single anonymous pool probe has bounded setup cost. A
  sweep across authorized pool machines gives direct coverage of every option
  and needs a concurrency limit and a partial-result contract.
- Visibility of provider changes made on the machine. A machine-local periodic
  check would report them promptly and consume provider processes while idle.
  Refresh on demand and checks during turns keep idle cost at zero, with changes
  becoming visible at the next observation.

## Long tool output (issue #41)

- **Storage and links.** A space-owned file item could reuse file addresses,
  current space access and downloads, with an additional item per large output.
  A thread-owned artifact could simplify cleanup when the thread is purged, with
  a new address, authorization check and download path. Define the Markdown
  reference and publication order for the selected model.
- **Retention and limits.** Keeping artifacts with the conversation preserves
  its links and can grow storage independently of Markdown. A separate quota or
  expiry bounds storage and needs an explicit expired-output state. Define
  the inline threshold and the point at which the artifact becomes durable.

## Wallpaper attribution and tracking (issue #49)

- **Same-origin image views.** Ask Unsplash about its documented view-beacon
  alternative, which retains server caching and adds provider integration work.
  A separately approved release-asset arrangement needs asset management and
  attribution. Direct hotlinking would change the decided CSP and expose browser
  requests to Unsplash.
- **Selection tracking.** Add a signed-in picker event that triggers the API call
  when someone selects a photo, even when its image is cached. This needs a
  client hook and a budget for selection events separate from rendition fills.
  The current contract reports first wallpaper use per photo per installation.


## Pool microVM networking and credentials (issue #51, 2026-11-18)

- Egress can use operator-created TAP devices and a dedicated firewall namespace.
  This supports ordinary provider and package traffic and adds privileged setup,
  DNS policy and a network isolation proof. A vsock egress broker keeps guest
  networking rootless and limits destinations in one place. It adds a proxy
  contract and requires compatibility checks for provider, CLI and package clients.
- Sharing account sign-in across space homes can use provider-supported
  credential-only storage where available. This requires verifying each official
  provider's refresh behavior. A local credential broker can maintain separate
  space homes and centralize refresh, with more sensitive components and
  concurrency rules.
