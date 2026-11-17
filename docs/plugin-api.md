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

- Shape: every plugin ships a manifest (its "menu": kinds, operations, collections, views)
  that the host reads without running plugin code, plus TypeScript functions behind it.
- Execution: one `calternal` binary with a server role and a machine role. An agent always
  runs on a machine that connected out to the server (the user's own, or a cloud machine
  the operator enrolled), inside a per-thread sandbox that the machine role itself starts.
  The server never starts containers or VMs.
- Generated UI is native by default: a plugin or agent describes a component from the
  built-in primitives and each client draws it with real native controls. HTML/CSS in a
  sealed frame is the escape hatch for what the primitives cannot express.
- Accessibility is part of the component contract: built-in primitives are accessible by
  default, labels are required by the schema, and a component that fails the checks is
  not enabled.

## Open

- Which bb concepts become manifest entries: skills and instructions, agent providers,
  machine and workspace providers, schedules, webhooks, renderers per file type, mentions,
  themes, plugin-private storage, hooks.
- Whether custom components may run scripts, and how they are confined.
- What a sandbox is on a cloud machine (being measured:
  `docs/measurements/sandbox-containers-vs-microvms.md`).
