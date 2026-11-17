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
- An HTML component may run its own scripts, inside a sealed frame.
- An agent backend (Codex, Claude Code, others) is a plugin that implements the provider
  contract.
- Must-have manifest entries, by kayg's priority: themes (most important: the app must feel
  like the user's own), machine and workspace types, schedules (automations), webhooks and
  HTTP routes, skills and instructions, plugin-private storage, secrets requested through a
  trusted host prompt (the plugin gets a handle, never the value; OpenBao can back it
  later), asking the user a question mid-run and similar quality-of-life features.
- A component can be registered as the renderer for a file type, a code block or a message
  action; that is the same dynamic-component mechanism, not a separate one.
- The app has management screens for plugins, skills and automations.
- Machines: cloud users never see machines. People who bring their own can add one from the
  UI by giving SSH access; installation and enrolment are then automatic.
- Deterministic first: installers and provisioning are fixed scripts; an agent is called in
  only with the failure log when a step fails.
- Sandbox images include NetBird and Tailscale clients able to run in userspace networking
  mode, so a cloud sandbox can reach machines on the user's own private network.
- Generated UI reuses the types of the GenUI plugin used in bb
  (`github.com/ComicBit/bb-plugin-genui`, Apache-2.0: lexer, parser, AST, evaluator,
  component catalog, validation) as the starting point for the component description
  format.
- Hooks may watch and block (proceed, wait, reject) within the plugin's own space, enabled by
  a manager. No rewriting; bb's own dispatch hook does not rewrite either.
- Page-wide JavaScript is not supported. Its legitimate uses are covered instead by full-CSS
  themes, command and shortcut contributions, and the shell regions.
- Themes can be full CSS, with no network access. Trusted screens (sign-in, permission
  approval, secret entry) render in a protected layer that themes cannot restyle or cover.

## Open

- Composer and chat experience (mentions, commands): needs its own design round.
- Skills and automations management: needs fleshing out.
- What a sandbox is on a cloud machine (being measured:
  `docs/measurements/sandbox-containers-vs-microvms.md`).
