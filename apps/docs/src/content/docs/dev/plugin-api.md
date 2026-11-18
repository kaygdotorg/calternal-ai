---
title: "Plugin API decisions"
---

<!-- Generated from: docs/plugin-api.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Working record of what kayg has decided; the design itself is still being settled.

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
- File formats follow the existing app. Settings use versioned
  `.calternal/settings.json`. JSON is the ADR 0055 exception because it
  preserves typed nested values, including unknown values that an older
  reader must keep when writing one setting. An unreadable or newer file
  must remain untouched. Collections and saved views are Markdown, as in
  ADR 0066 ("Views are Collections"): one
  human-readable Views file with a `## Name` section per view and `key: value` lines
  (including the view's `id`); unknown keys are kept. Skills and automations live hidden
  under `.calternal/`.
- An automation is a Markdown file under `.calternal/automations/`: frontmatter holds its
  id, `schedule` (a systemd timer calendar expression, for example `Mon *-*-* 09:00:00`),
  `timezone` (IANA) and agent settings; the body is the prompt. The app shows the schedule
  in plain words.
- Capabilities adopted from bb, bb-lite and pi-durable (2026-11-17): all 24 recommended
  items in (internal evidence), in their safe forms, built
  in priority order (durable waits, run-end work and sandboxed machine operations first).
  The 12 not-recommended items there are out. Themes are design tokens that every client
  applies, with CSS as a web-only extra.
- Secrets (2026-11-17): rotation is immediate (old handles stop working); a member's
  personal secrets are invisible to everyone else, managers included; a space's secrets
  are managed under a separate `secrets.manage` capability. Keys are per owner as specified below; no passkey-derived key or unlock step.
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
  Content writes carry `data.actor`, the authenticated account and its delegated
  route (agent thread or plugin, if any). Clients render this attribution using
  the admitted actor rather than inferring it from the operation or its payload.
  Drafts work the same way. The only things a client holds that the server lacks are
  edits typed while offline, which it sends on reconnect.
- Agent questions and approvals appear in the same sticky bar as secret requests and do
  not expire.
- Provider logins (2026-11-17): Calternal is an orchestrator, not a harness. It launches the
  providers' official, unmodified programs (Claude Code, Codex), and each person signs in
  to them through the provider's own flow inside their own sandbox. The login is saved by the
  provider's program in the person's own sandbox home, a persistent volume that follows
  them, so they sign in once. The server process never reads, copies or stores it. Evidence and provider wording:
  (internal evidence).
- One way to run untrusted code (2026-11-17, measured in
  (internal evidence)): plugin workers run in containers
  started through the machine role, like agents; about 0.3 ms extra per operation, no
  network. Plugins never run directly on the server, first-party ones included.
- In the cloud each person has one microVM, with a container per thread
  ((internal evidence)). CPU is shared by weights at both levels.
- Drafts live until sent or erased, capped by count and size per account. A message queued
  while offline stays until sent or cancelled; after 7 days it asks before sending.
- Plugin workers have no sockets. A manifest requests distinct named, content
  fetch, open internet and private grants. The trusted host enforces the current
  per-space approval and remote-content preferences. See the
  network contract.
- Protocols that need long-lived connections (mail: IMAP, JMAP, SMTP) are Rust engines in
  the server behind a contract, with every outbound connection through one guard
  (the existing app's netguard, ADR 0071); the plugin is the product layer on top.
- Agent files live in each space's visible `AI/` folder (2026-11-17, replacing the earlier
  hidden location): `AI/Chats/`, `AI/Skills/<name>/SKILL.md`, `AI/Automations/<name>.md`,
  and the space's instructions at `AI/AGENTS.md`. A person's own skills, automations,
  instructions and starred prompts are the same files in their personal space.


## Durable job waits

The author reference
shows how a replayable job run returns a checkpoint and waits for an
Inbox answer, an approval, a webhook delivery or its deadline. The
worker invocation ends while the job waits. Interactive operations
return a job ID from the existing `start_job` broker call to use waits.

Park, cancellation and delivery use the same jobs.db connection and
state fence. The first signal commits with the transition to admitted
and, for Inbox decisions and webhooks, with the replay receipt.
Duplicate deliveries cannot win the same wait again. The normal job
scheduler rechecks the original sign-in, capability, package revision
and activation. Once admitted, ordinary job restart and effect-receipt
rules apply. After a later checkpoint the signal is no longer included.
Signal payloads are limited to 16 KiB and are retained with the owning
job. Job retention and account erasure also remove their waits.

### Signed webhook delivery

Declare a webhook `id` and schema `operation` in `webhooks`, and include
the same `id` in `secrets`. Configure at least 32 random key bytes for the
package's space secret of that name through the trusted secret flow. The
host uses the declared operation's cached input schema for the delivery.
It hands the payload to the resumed run.

Park with `{kind: "webhook", route, correlation}` using a fresh UUID for
each wait. The sender posts JSON to
`/api/v1/plugin-webhooks/<space>/<package>/<route>/<job>/<correlation>`.
Set `X-Calternal-Timestamp` to Unix seconds and `X-Calternal-Signature`
to the hexadecimal HMAC-SHA256 of the timestamp, LF, `POST`, LF,
exact path, LF and raw body bytes. Requests accept a five-minute clock
skew and up to 16 KiB of body bytes. Verification has a short body
timeout and a 60 attempts/minute budget per cached space. A 202
acknowledges the first accepted delivery or an identical replay.
Keep signing keys out of worker state and logs.

## Personal plugin settings

A package may declare `scope: "personal"` on a typed setting. A signed-in
space member can read and edit only their own override through
`plugins.personal_settings.get` and `plugins.personal_settings.set`.
Address the space UUID, package and key. For edits submit the last stored
`base`, the next `value` and a request ID. Null restores the manifest
default. A stale base conflicts and an identical retry replays its receipt.
The worker’s existing setting broker resolves the authenticated person's
value. An edit wakes that person's account feed without publishing the value.

## Space plugin settings

Declare a key in the manifest's `settings`, including its primitive type and
default. A Manager uses `plugins.settings.get` and `plugins.settings.set` with a
space UUID, package ID and key. For a write, echo the last read `value` as
`base` and send the new `value`. Null clears the override. Concurrent edits
to one key keep the current value and retain up to eight alternatives. To
resolve them, include the observed alternatives in `resolve` with the
chosen value. Use one request ID per edit and reuse it for network retries.

A worker declares `read_setting_v1` and calls `readSetting(context, "label")`.
The host supplies the invocation's package and space. A worker gets only
the effective setting value. The authorized space feed publishes a setting
invalidation with its package ID and key, without including the value.
The shell can use this to re-invoke a generated slot view.

Space userdata stores these overrides in `.calternal/settings.json` under
the `calternal-plugin-settings/v1` tag. An individual value is limited to
4 KiB and a settings document to 1 MiB. One durable pending edit is allowed
per space. On first settings access after a crash, the host finishes the
file exchange, publishes the feed entry and receipt, and removes the
intent. Syntax damage returns defaults with a file-and-line repair hint.
Once the file is valid, recovery reapplies the pending key and keeps other
edits. A competing edit to the same key becomes a conflict. Unrelated
operations stay available during repair.

## Author diagnostics

`calternal plugin new` creates a directory under the current directory and
prints the root worker, fixture test and next commands. Source the local
development env before using `calternal`, Node.js or npm. `calternal plugin dev`
accepts `APPDATA`, `SPACE_ID` and `ACCOUNT_ID` as defaults for its flags.

An invocation that fails a plugin input schema check returns a bounded JSON
pointer and the expected type or constraint. The request values stay out of
the diagnostic. The space selector is required in the CLI input for a
space-targeted plugin operation. Host job lookups accept a job UUID only.
The CLI points to discovery when input is invalid. A busy worker means the
host reached a live admission bound. Retry durable writes with the same
request ID when an invocation slot is free.

The SDK fixture captures a subprocess's exit status or signal and up to
4 KiB of its stderr. Fixture diagnostics are local to the author. The live
worker protocol still treats a failed handler as a failed invocation.
