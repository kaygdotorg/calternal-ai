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
