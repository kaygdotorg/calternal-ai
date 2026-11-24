# Agent rules: research and a proposal · 2026-11-24

Status: **proposal** for the owner (round 27 of `docs/morning-2026-11-21.md`:
"auto-approve for me is the default, we need to build a really good rule
system, taking inspiration from other open source agents"). Nothing here is
decided; the choices are questions in §4.

All pages were read with Firecrawl on 2026-11-24. Claude Code, Codex and
Antigravity, the harnesses we ship ourselves (round 28), are covered in
depth.

## 1. How other agents decide what an agent may do without asking

The three harnesses we ship ourselves (round 28) come first and in depth;
the others are a shorter comparison.

### 1.1 Claude Code

Sources: [permissions](https://code.claude.com/docs/en/permissions),
[sandboxing](https://code.claude.com/docs/en/sandboxing),
[hooks](https://code.claude.com/docs/en/hooks-guide).

- **Unit:** a tool, optionally narrowed by a specifier: `Bash(npm run *)`,
  `Read(./.env)`, `Edit(src/**)`, `WebFetch(domain:example.com)`,
  `mcp__server__tool`, `Agent(name)`. Deny and ask rules may also match one
  top-level input parameter (`Agent(model:opus)`), but never a tool's main
  content field, because a compound command would bypass it.
- **Scopes:** managed settings (organisation), user `~/.claude/settings.json`,
  project `.claude/settings.json`, local `.claude/settings.local.json`, and
  the session.
- **Defaults and modes:** `default` (shown as Manual) reads in the working
  directory without asking and asks before shell commands, edits and web
  fetches; `acceptEdits` accepts edits and common file commands in the
  working directory; `plan` only reads; `auto` lets a background classifier
  check actions against the request instead of asking; `dontAsk` denies
  whatever would ask; `bypassPermissions` skips prompts except for a few
  protected actions. Managed settings can disable `auto` and
  `bypassPermissions`.
- **Writing and storing:** JSON lists `allow`, `ask`, `deny` in the settings
  files. **Deny, then ask, then allow; the first match in that order wins**,
  and specificity does not change it: an allow cannot carve an exception out
  of a deny.
- **Asking and "always":** the prompt offers Yes, "Yes, and don't ask again
  for: npm test *", and No; Tab adds a note sent to the model. Permanent
  answers for commands and domains are saved to `.claude/settings.local.json`;
  file edits are remembered only until the session ends. `/permissions` lists
  every rule with the file it comes from.
- **Hooks:** a PreToolUse hook runs a command before the prompt and may deny,
  force a prompt or skip it, but never overrides a deny or ask rule.
- **Sandbox:** off by default; when on, shell commands and their children run
  inside an OS boundary (Seatbelt on macOS, bubblewrap and a network
  namespace on Linux): writes only in the working directory and added
  folders, network only through a local proxy checking an allowed-domain
  list that starts empty. In auto-allow mode a sandboxed command needs no
  prompt.
- **Weaknesses, as the docs state them:** a Bash rule "isn't a security
  boundary" (`Bash(curl *)` does not stop `/usr/bin/curl …` or
  `sh -c 'curl …'`); Read, Edit and WebFetch run outside the sandbox, so
  `denyRead` does not stop the Read tool; an allow rule also approves an
  unsandboxed retry; reads default to most of the machine, credential files
  included.

### 1.2 Codex CLI

Sources: [approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security),
[sandboxing](https://learn.chatgpt.com/docs/sandboxing),
[rules](https://learn.chatgpt.com/docs/agent-configuration/rules).

- **Unit:** two separate controls. A **sandbox mode** says what commands can
  touch: `read-only`, `workspace-write` (edit the workspace, network off
  unless `sandbox_workspace_write.network_access = true`) or
  `danger-full-access`. An **approval policy** says when to stop and ask:
  `on-request` (ask only to cross the sandbox) or `never`. **Rules** are
  command prefixes that decide what may run outside the sandbox:
  `prefix_rule(pattern = ["gh", "pr", "view"], decision = "prompt")`.
- **Scopes:** config layers: managed and team configuration, user
  `~/.codex/config.toml` and `~/.codex/rules/`, project `<repo>/.codex/`,
  which loads only for a trusted project.
- **Defaults and presets:** `workspace-write` with `on-request` in version-
  controlled folders, `read-only` elsewhere or until the folder is trusted.
  The presets are "Ask for approval", **"Approve for me"** (the same sandbox,
  with eligible approval requests sent to an automatic reviewer agent,
  `approvals_reviewer = "auto_review"`), "Full access"
  (`danger-full-access` with `never`) and Custom.
- **Writing and storing:** `.rules` files in a `rules/` folder beside each
  config layer; `decision` is `allow`, `prompt` or `forbidden`; **the most
  restrictive matching rule wins**; a rule may carry `match` and `not_match`
  examples that Codex checks when it loads the rules.
- **Asking and "always":** allowing a command in the interface writes
  `~/.codex/rules/default.rules`; "smart approvals" propose a prefix rule
  during an escalation, which the docs ask people to review carefully.
- **Protected paths:** `.git`, `.codex` and `.agents` inside a writable root
  stay read-only, so the agent cannot rewrite its own settings or history.
- **Weaknesses:** rules are experimental and prefix-only; shell wrappers are
  split only when the script is a simple chain; an automatic reviewer's
  approval is a model's judgment.

### 1.3 Antigravity

Google's agent is **Antigravity** today: an app, an IDE, IDE extensions and
the **Antigravity CLI** (`agy`), which replaced Gemini CLI for unpaid and
Google One users on 2026-06-18 (notice on the Gemini CLI policy page). Its
rules are documented. Sources:
[agent permissions](https://antigravity.google/docs/permissions/),
[agent settings](https://antigravity.google/docs/agent-settings/),
[CLI `/permissions`](https://antigravity.google/docs/cli/commands/permissions/),
[CLI execution modes](https://antigravity.google/docs/cli/modes/),
[terminal sandbox](https://antigravity.google/docs/sandbox).

- **Unit:** a permission resource written `action(target)`, one engine for
  the app, the CLI and the IDE: `read_file(path)`, `write_file(path)`,
  `read_url(domain)`, `execute_url(domain)` (the browser agent),
  `download_url(domain)`, `command(prefix)` or `command(regex:…)`,
  `mcp(server/tool)`. Write implies read; a denied read also denies write.
- **Scopes:** Project, Shared (across all Antigravity products) and Global,
  edited in the CLI's `/permissions` manager or in
  `~/.gemini/antigravity-cli/settings.json`; Gemini Enterprise adds
  organisation controls.
- **Defaults and presets:** **Request Review** (always ask, no sandbox),
  **Default** (commands run without asking inside the terminal sandbox,
  which sees only the workspace and temp folders and has no network; outside
  it they ask), **Auto** (the same, and commands that must leave the sandbox
  are evaluated automatically: "safe, reversible commands" run, risky or
  irreversible ones ask) and **Turbo** (no sandbox, no prompts, the whole
  filesystem). The CLI's modes are `default`, `accept-edits`, `auto` (its
  default since 1.5.7 where available) and `plan`. Rules are layered on top
  of the preset and always take precedence.
- **Writing and storing:** three lists, Deny, Ask and Allow; "Deny: the action
  is blocked immediately". Granted paths and domains are compiled into the
  sandbox: a `read_url` grant becomes an allowed outbound host for `curl` and
  `npm`, a `write_file` grant a writable folder.
- **Asking and "always":** a card shows the request, and the person may edit
  the target before allowing it (widen `src/a.ts` to `src/`); outside the
  workspace a prompt offers "Yes, and always allow non-workspace access"; a
  file edit shows its diff and stays open until answered. Every upload from
  the browser agent needs a one-time approval, even in Turbo.
- **Weaknesses:** command rules match prefixes; for constructs that can hide
  a command (`$(…)`, backticks, brace expansion) a rule needs an exact match,
  which the docs list; Auto's "safe, reversible" is an automatic judgment;
  the forum has reports of allow-listed commands still prompting.

### 1.4 Shorter comparisons

| Agent | Unit | Default | Ordering | Notable | Weakness |
| --- | --- | --- | --- | --- | --- |
| **OpenCode** ([permissions](https://opencode.ai/docs/permissions/)) | `{action, resource, effect}`: read, edit, shell, subagent, external_directory, MCP; plugins may add actions | No match means ask; paths outside the project allowed | **Last match wins** | An operation touching several resources is denied if any is denied | Order decides; a later broad rule silently widens access |
| **Gemini CLI** ([policy engine](https://geminicli.com/docs/reference/policy-engine/), [trusted folders](https://geminicli.com/docs/cli/trusted-folders/)) | Tool name, command prefix, or a regular expression over arguments as JSON | Modes default, autoEdit, plan, yolo | Tiers Default < Workspace < User < Admin, plus priority 0–999 | "Always" never widens a stricter mode; a global deny hides the tool from the model | Workspace tier disabled because a project could grant itself rights; folder trust off by default |
| **Goose** ([modes](https://goose-docs.ai/docs/guides/managing-tools/goose-permissions/), [tool permissions](https://goose-docs.ai/docs/guides/managing-tools/tool-permissions/)) | A tool of an extension: Always Allow, Ask Before, Never Allow | **Autonomous** | Per tool over the mode | Smart Approval approves "low-risk" actions | No approval by default; risk is the model's judgment |
| **Cline** ([auto approve](https://docs.cline.bot/features/auto-approve)) | Categories (read, edit, safe or all commands, browser, MCP) | All off | Toggles | Notifications when approval is needed | "Cline does not use a fixed allowlist": the model marks each command `requires_approval` |
| **Roo Code** ([auto-approving](https://roocodeinc.github.io/Roo-Code/features/auto-approving-actions/)) | Categories plus allowed and denied command prefixes | Off | Lists | One master Enabled switch pauses everything | Prefix lists over shell text |
| **OpenHands** ([security](https://docs.openhands.dev/sdk/guides/security)) | Each action with a risk level from an analyzer | Set in code: AlwaysConfirm, NeverConfirm, ConfirmRisky | Policy over analyzer | Policy kept apart from the risk judge | The judge may be a model |

**What recurs:** three answers (allow, ask, deny); several scopes merged with
a fixed precedence; "always" saved as a written rule; a person-readable list
of rules; modes that change the defaults. **What fails:** rules over shell
command text (every system says, or shows, that a different spelling slips
through); "safe" decided by the model (Cline, Goose's Smart Approval);
last-match-wins ordering (OpenCode); projects granting themselves rights
(why Gemini disabled its workspace tier and Codex trusts project files only
for trusted projects).

## 2. Our rules

### 2.1 Why rules are about operations

On Calternal an agent changes nothing except through plugin operations over
MCP, the API or the CLI (round 16), and every operation is checked at one
call table (§5 of the proposal). A rule about `calternal-mail.send` cannot
be spelled differently to slip past it, the way `/usr/bin/curl` slips past
`Bash(curl *)`. **So a rule is about operations, and it is a security
boundary**, which none of the shell-rule systems above can claim.

A rule has four parts:

| Part | Values | Example |
| --- | --- | --- |
| Decision | `allow`, `ask`, `deny` | `ask` |
| Operation | `plugin.operation`, with `*` for every operation of a plugin; `add`, `change`, `remove` for the free operations of any kind | `calternal-mail.send`, `remove`, `calternal-money.*` |
| Conditions (optional) | `on <kind>`, `in <collection>`, `where <condition>` (the format's expression language), `outside this chat` | `on transactions where amount > 500 EUR` |
| Who (optional) | `for <harness>`, default every agent | `for calternal-chat-codex` |

**Precedence: deny, then ask, then allow; the most restrictive match wins**
(Claude Code and Codex), never "last match wins". A person's rule cannot
widen past the ceilings that do not change: the starter's own rights, the
run's scope, the space's Agent access switch, and the risky actions that
always go through the server's sheet with a passkey (installing or widening
a plugin, sharing outside the space, picking a part that sends data out,
installing web code).

### 2.2 What else runs need

| Run | What rules cannot see | What is needed |
| --- | --- | --- |
| **A run with a handed folder** (an editable-outside kind's folder) | The run writes files in the folder directly, not through operations | Rules on the folder's files: `allow change in Notes/Work`, `ask remove in Notes`, `deny change in Notes/Archive`. The server enforces them, because the folder is served to the sandbox by the server |
| **A sandboxed run's outside traffic** | Hosts reached through the server's proxy | The plugin's declared destinations are the ceiling; rules may narrow them (`deny fetch github.com for calternal-chat-claude-code`) |
| **A "direct, full access" run on the person's own machine** (personal space only) | Shell commands, local files and the network on that machine | The server cannot enforce anything there. Two honest options (question R5): leave the harness's own permission system in charge, or translate our rules into the harness's files (`.claude/settings.json`, `.codex/rules/`) as a best effort, labelled as such |

### 2.3 Our own harnesses inside our sandbox

Claude Code, Codex and Antigravity each have their own rule system. Inside a
hosted sandbox the sandbox and our operation rules are the boundary, so the
harness runs in its own permissive mode (`bypassPermissions`, Codex's full
access inside our box, Antigravity's Turbo) and its tool calls reach the
server as operations, which our rules decide. Running both rule systems would
ask the person twice for the same action. On the person's own machine
("direct, full access", personal space only) the harness's own rules are the
only ones that apply (question R5).

### 2.4 "Auto-approve for me", exactly

The owner's default. Proposed meaning:

| | Personal space | A space with members |
| --- | --- | --- |
| Reading inside the run's scope | allow | allow |
| Free `add` and `change` | allow | allow on items the starter wrote; ask on items others wrote |
| Free `remove` | allow (history keeps every version; Trash holds files) | ask |
| Coded operations of installed plugins | allow | allow |
| Replying in the chat the agent was asked in | allow | allow |
| Operations that reach other people (sending mail, posting in another chat, notifying) | ask | ask |
| Files in a handed folder | allow change and add; ask remove | allow change and add; ask remove |
| The server's risky actions | the server's sheet with a passkey, always | the same |

Anything the agent was not given in its scope is simply not reachable; no
rule changes that.

### 2.5 Where rules live and how they are shown

- **A person's rules** are one Markdown file in their personal space,
  `Agents/rules.md`, applied to their runs in every space. **A space's
  rules** are `.calternal/agent-rules.md`, written by the Owner or a
  Manager; they bind every member's runs in that space, and a person's rule
  can narrow them but never widen them (question R2).
- The file is a plain list a person can write by hand:

  ```markdown
  ---
  rules: agents
  ---
  - ask calternal-mail.send
  - deny calternal-money.* on transactions where amount > 500 EUR
  - allow change in Notes/Work
  - ask remove
  - deny fetch github.com for calternal-chat-codex
  ```

- **The prompt** is the server's sheet, with four answers: **Allow once**,
  **Allow for this run**, **Always allow** (which writes one line to the
  person's `Agents/rules.md`), and **Don't allow** (with an optional note to
  the agent, as Claude Code allows). An "Always" that would conflict with a
  space rule is not offered.
- **Settings** shows every rule as a sentence ("Agents ask before sending
  mail"), grouped by where it comes from (you, this space, the server's
  fixed rules), with the file it lives in.

## 3. What we take, and what we do not

| From | Taken | Not taken |
| --- | --- | --- |
| Claude Code | Deny, then ask, then allow; "don't ask again" writes a rule; a note with an answer | Rules over command text; hooks that run code to decide |
| Codex | "Approve for me" as a named default; the sandbox and the approval rules as two separate controls; `match` and `not_match` examples a rule can carry (proposed for `calternal check`); the agent cannot rewrite its own rules folder | Prefix rules over commands |
| Antigravity | One `action(target)` engine for every surface; grants compiled into the sandbox (an allowed host becomes an allowed outbound host); editing the target in the prompt before allowing | Turbo's "no isolation"; an automatic "safe, reversible" judge |
| Gemini CLI | "Always" scoped so it never widens a stricter mode; a project or space cannot grant itself rights beyond its tier | Priority numbers; regular expressions over arguments |
| OpenCode | Plugins can name new actions (our operations already are that) | Last match wins |
| Goose, Cline, Roo Code | A master pause switch | Model-judged "safe" or "low risk" |
| OpenHands | Policy separate from any risk judge | A model as the judge (question R4) |

## 4. Questions for the owner

**R1. What exactly does "auto-approve for me" include?**
*What:* the default rule set for a person's own runs (§2.4).
*Why now:* it is the first thing every agent run meets.
*Example:* you ask Claude to clean up your notes; it wants to remove 40
duplicates and email a summary to Lee.
*Options:* (a) **as in §2.4**: everything allowed in your personal space
except reaching other people; in a shared space also ask before removing or
changing others' items. Cost: a few prompts in shared spaces. (b) Allow
everything below the server's risky sheet, everywhere. Cost: an agent can
remove a colleague's items or mail anyone without asking. (c) Ask before
every change. Cost: the opposite of what you asked for.
*Recommendation:* (a).

**R2. Do a space's rules bind every member's runs in it?**
*What:* the Owner or a Manager writes `.calternal/agent-rules.md` for the
space; a member's own rules apply on top.
*Why now:* it decides whether a shared space can protect itself from a
member's permissive agent.
*Example:* the family space denies `calternal-money.*` for agents; your own
rules allow it.
*Options:* (a) **space rules are a ceiling a member's rules can only
narrow**. Cost: a member cannot loosen them. (b) A member's rules win for
their own runs. Cost: one member's agent can do what the space forbids.
*Recommendation:* (a), as Gemini and Codex keep a project from granting
itself more than its tier.

**R3. Are rules for all agents, or per harness?**
*What:* whether `for calternal-chat-codex` is part of the rule language.
*Why now:* it decides the shape of every rule and the prompt's "Always".
*Example:* you trust Claude to send mail but not a new community harness.
*Options:* (a) **all agents by default, with an optional `for <harness>`**.
Cost: one more word in the language. (b) Rules per harness only. Cost: every
new harness starts from nothing.
*Recommendation:* (a); "Always" from a prompt is written for that harness
only.

**R4. May an automatic reviewer answer some prompts?**
*What:* Codex's "Approve for me" and Claude Code's auto mode route prompts to
a reviewing model instead of the person.
*Why now:* it decides whether "auto-approve" means "rules" or "rules plus a
judge".
*Example:* an agent asks to send an email; a reviewer model approves it
because it matches what you asked for.
*Options:* (a) **no reviewer: the rules decide, and only the person answers
prompts**. Cost: more prompts for actions no rule covers. (b) A reviewer
model answers prompts the rules leave open. Cost: the decision is a model's
opinion, the weakness found in Cline and Goose.
*Recommendation:* (a) now; a reviewer can be a plugin later, filling a slot,
without changing the rules.

**R5. What do our rules do for a "direct, full access" run on your own machine?**
*What:* the server cannot see or stop shell commands there.
*Why now:* those runs exist (personal space only, round 11).
*Example:* Claude Code runs on your Mac with full access and wants to run
`rm -rf build/`.
*Options:* (a) **leave the harness's own permissions in charge, and say so on
the run**. Cost: two rule systems for one person. (b) Translate our rules into
the harness's own files as a best effort. Cost: a translation that can drift
and looks like a guarantee it is not.
*Recommendation:* (a); full access means the harness's rules, and the run
says so.

**R6. Does "Always allow" ever expire?**
*What:* a remembered answer stays in `Agents/rules.md` until removed.
*Why now:* old "Always" lines accumulate.
*Example:* you allowed an agent to post in a work chat in 2026 and forgot.
*Options:* (a) **never expires; Settings shows each rule with the date it
was added and when it last applied**. Cost: stale rules stay. (b) Expire
after a year unused. Cost: prompts return without warning.
*Recommendation:* (a).

**R7. Do our own harnesses run in their permissive mode inside our sandbox?**
*What:* Claude Code, Codex and Antigravity each ask the person before
actions by their own rules; inside our hosted sandbox our rules already
decide every operation, folder write and outbound host (§2.3).
*Why now:* it decides whether a person is asked once or twice for the same
action, and what the harness plugins set when they start a run.
*Example:* Claude Code in a hosted run wants to edit a file in the folder
you handed it; Claude Code's Manual mode and our rule would both ask.
*Options:* (a) **the harness runs in its permissive mode
(`bypassPermissions`, full access inside the box, Turbo), and only our rules
ask**. Cost: the harness's own protections inside the box (protected paths
such as `.git`) no longer apply unless our rules repeat them. (b) The harness
keeps its own default mode as a second layer. Cost: double prompts and two
rule files to keep in step.
*Recommendation:* (a), with our default rules protecting `.git` and the
harness's own settings folders inside a handed folder.
