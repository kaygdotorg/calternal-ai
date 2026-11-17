# Plugin API: decisions so far

Working record of what kayg has decided; the design itself is still being settled.
Input: `docs/reviews/plugin-and-client-map-sol.md`.

## Decided (2026-11-17)

- Apps are plugins authored in TypeScript. The orchestrator is the first app; Notes second.
- No compatibility with bb plugins.
- A plugin fills the three shell regions (`docs/ui-shell.md`) and contributes kinds and
  collections addressed per `docs/addresses.md`.
- Starting an agent run needs a connection. Offline, a client can read cached threads and
  keep drafts; nothing is queued to run later.
- UI: the product ships a rich library of native components that plugins use by default.
  A plugin or an agent may also bring its own components written in HTML and CSS, created
  dynamically. This supersedes "portable GenUI only" as the early scope.

## Open

- The shape of the API (manifest plus handlers is the proposal).
- Whether custom components may run scripts, and how they are confined.
- What a sandbox is on a cloud machine (being measured:
  `docs/measurements/sandbox-containers-vs-microvms.md`).
