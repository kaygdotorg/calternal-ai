# Plugin API: decisions so far

Working record of what kayg has decided; the design itself is still being settled.
Input: `docs/reviews/plugin-and-client-map-sol.md`.

## Decided (2026-11-17)

- Apps are plugins authored in TypeScript. The orchestrator is the first app; Notes second.
- No compatibility with bb plugins.
- A plugin fills the three shell regions (`docs/ui-shell.md`) and contributes kinds and
  collections addressed per `docs/addresses.md`.
- A message sent while offline is queued as a visible pending submission that can be
  edited or cancelled, and is submitted on reconnect after access is re-checked.
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
- Hooks may watch, block (proceed, wait, reject) and rewrite, within the plugin's own space
  and only when a manager enabled them. A rewrite may change content (message text,
  operation input) but never the caller, space or target; access is checked after it; both
  versions are recorded and shown as changed by the plugin; rewrites run one at a time in
  an order the manager sets.
- Context contributions: a plugin may attach context to an agent turn (Honcho memory is the
  model). It is labelled with its source, recorded with the turn, visible to the user and
  bounded by a deadline. It never changes what the user typed.
- Page-wide JavaScript is not supported. Its legitimate uses are covered instead by full-CSS
  themes, command and shortcut contributions, and the shell regions.
- Themes can be full CSS, with no network access. Trusted screens (sign-in, permission
  approval, secret entry) render in a protected layer that themes cannot restyle or cover.
- Userdata layout: each space has a hidden `.trash/` folder of its own. Collections, space
  and plugin settings, skills, automations, themes and custom components are files in the
  space's userdata (shape in `docs/storage-layout.md` once agreed), not database-only.
- File formats follow the existing app: settings are `.calternal/settings.json` (plain
  key/value; Markdown would add nothing, so this is the recorded JSON exception);
  collections and saved views are Markdown, as in ADR 0066 ("Views are Collections"): one
  human-readable Views file with a `## Name` section per view and `key: value` lines
  (including the view's `id`); unknown keys are kept. Skills and automations live hidden
  under `.calternal/`.
- An automation is a Markdown file under `.calternal/automations/`: frontmatter holds its
  id, `schedule` (a systemd timer calendar expression, for example `Mon *-*-* 09:00:00`),
  `timezone` (IANA) and agent settings; the body is the prompt. The app shows the schedule
  in plain words.
- Capabilities adopted from bb, bb-lite and pi-durable (2026-11-17): all 24 recommended
  items in `docs/reviews/extensibility-research-2026-11-17.md`, in their safe forms, built
  in priority order (durable waits, run-end work and sandboxed machine operations first).
  The 12 not-recommended items there are out. Themes are design tokens that every client
  applies, with CSS as a web-only extra.
- Secrets (2026-11-17): rotation is immediate (old handles stop working); a member's
  personal secrets are invisible to everyone else, managers included; a space's secrets
  are managed under a separate `secrets.manage` capability. How secrets are keyed (a
  server key, or keys derived from each person's passkey) is still open.
- Entitlements come from groups (2026-11-17): groups are named after the plans offered; an
  invite assigns its groups (existing app ADR 0059, 0060). A group decides access to the
  shared pool of cloud machines and the limits that apply (concurrent agent runs, storage).
  Limits are not arbitrary constants: product limits come from the plan and from real
  capacity; the server keeps only high, operator-configurable safety ceilings against a
  runaway plugin.
- Machines: when a machine goes silent mid-run the turn shows as interrupted and the
  person is told; work never moves by itself, and Resume restarts it from its checkpoint; while waiting for capacity they see their queue position.
- Jobs: a running job stops only when its account loses access to the space, not when a
  device signs out; any member who can read the space can watch its output; the person
  who started it or a Manager can cancel.
- Secrets are keyed with one key file per owner (account or space) in that owner's appdata
  folder; no server-wide key, no unlock step. On a person's own machines, provider logins
  stay on the machine. A plugin declares the secrets it needs in its manifest (name, label,
  why, space or personal, required or optional); they belong to the space that enabled the
  plugin. A secret request appears in a sticky bar above the composer, expires after 15
  minutes and can be asked again.
- Cloud machines keep a small warm pool of pre-started sandboxes.
- Drafts sync across a person's devices and stay until sent or erased. After sign-out,
  unsent edits are kept as drafts. After revocation the device offers an export of what it
  holds, then deletes it; revocation is immediate and never waits for the export.
- Clients do not sync. All state lives on the server; a client subscribes to a space's
  journal over one live connection, shows it, and caches what it needs (the existing app's
  model: `/api/v1/live`, resume with a bookmark, reload when the bookmark has expired).
  Drafts work the same way. The only things a client holds that the server lacks are
  edits typed while offline, which it sends on reconnect.
- Agent questions and approvals appear in the same sticky bar as secret requests and do
  not expire.
- Provider logins (2026-11-17): Calternal is an orchestrator, not a harness. It launches the
  providers' official, unmodified programs (Claude Code, Codex), and each person signs in
  to them through the provider's own flow inside their own sandbox. The login is saved by the
  provider's program in the person's own sandbox home, a persistent volume that follows
  them, so they sign in once. The server process never reads, copies or stores it. Evidence and provider wording:
  `docs/reviews/provider-terms-2026-11-17.md`.
- One way to run untrusted code (2026-11-17, measured in
  `docs/measurements/plugin-workers-in-sandboxes.md`): plugin workers run in containers
  started through the machine role, like agents; about 0.3 ms extra per operation, no
  network. Plugins never run directly on the server, first-party ones included.
- In the cloud each person has one microVM, with a container per thread
  (`docs/measurements/sandbox-hybrid.md`). CPU is shared by weights at both levels.
- Drafts live until sent or erased, capped by count and size per account. A message queued
  while offline stays until sent or cancelled; after 7 days it asks before sending.
- Plugin network access: none by default. A plugin declares destinations in its manifest;
  a Manager approves them at enable time; the plugin fetches through the server, which
  logs the request, attaches secrets by handle and may cache results. No raw sockets.
- Protocols that need long-lived connections (mail: IMAP, JMAP, SMTP) are Rust engines in
  the server behind a contract, with every outbound connection through one guard
  (the existing app's netguard, ADR 0071); the plugin is the product layer on top.
- Agent files live in each space's visible `AI/` folder (2026-11-17, replacing the earlier
  hidden location): `AI/Chats/`, `AI/Skills/<name>/SKILL.md`, `AI/Automations/<name>.md`,
  and the space's instructions at `AI/Instructions.md`. A person's own skills, automations,
  instructions and starred prompts are the same files in their personal space.

## Open

- Composer and chat experience (mentions, commands): needs its own design round.
- Skills and automations management: needs fleshing out.
- What a sandbox is on a cloud machine (being measured:
  `docs/measurements/sandbox-containers-vs-microvms.md`).

## Provider and risk contract (step 1)

Manifest v1 operations require `risk`: `read`, `reversible`, `irreversible`,
`external`, or `widens_access`. Read risk is consistent with `mutates: false`;
other classes require a mutating operation. There are no defaults or legacy
readers for missing declarations. The SDK and Rust host reject unknown classes.
Capabilities remain mandatory in every permission mode. The rules reviewer uses
the matrix in [manual turns](agent-turns.md), with a seam for a future model
reviewer restricted to irreversible actions.

A provider plugin additionally declares `provider` with protocol version one,
an executable basename inside the sandbox and its default model. See
[provider protocol](provider-protocol.md). This is machine-side metadata; the
server never launches or signs in to a provider. The first provider is
`calternal-claude-code`.
