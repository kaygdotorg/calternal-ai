# Agent rules: research and a proposal · 2026-11-24

Status: **proposal** for the owner (round 27 of `docs/morning-2026-11-21.md`:
"auto-approve for me is the default, we need to build a really good rule
system, taking inspiration from other open source agents"). Nothing here is
decided; the choices are questions in §4.

All pages were read with Firecrawl on 2026-11-24.

## 1. How other agents decide what an agent may do without asking

| Agent | Unit of a rule | Scopes | Default | How a rule is written and stored | Asking and "always" | Known weakness |
| --- | --- | --- | --- | --- | --- | --- |
| **Claude Code** ([permissions](https://code.claude.com/docs/en/permissions), [sandboxing](https://code.claude.com/docs/en/sandboxing), [hooks](https://code.claude.com/docs/en/hooks-guide)) | A tool, optionally narrowed: `Bash(npm run *)`, `Read(./.env)`, `WebFetch(domain:example.com)`, `mcp__server__tool`; deny and ask rules may match one top-level input parameter | Managed (organisation), user `~/.claude/settings.json`, project `.claude/settings.json`, local `.claude/settings.local.json`; session | Mode `default` (Manual): reads in the working directory free; shell commands, edits and fetches ask. Other modes: `acceptEdits`, `plan`, `auto` (a classifier reviews actions instead of the person), `dontAsk`, `bypassPermissions` | JSON lists `allow`, `ask`, `deny`; evaluated deny, then ask, then allow; first match in that order wins, specificity does not matter | The prompt offers Yes, "Yes, and don't ask again for: npm test *", No; permanent answers are written to `.claude/settings.local.json`; edits are remembered only for the session. PreToolUse hooks may deny, force a prompt or skip it, but never override a deny rule | The docs say a Bash rule "isn't a security boundary": `Bash(curl *)` does not stop `/usr/bin/curl …` or `sh -c 'curl …'`. File and web tools run outside the OS sandbox; an allow rule also approves an unsandboxed retry |
| **Codex CLI** ([approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security), [sandboxing](https://learn.chatgpt.com/docs/sandboxing), [rules](https://learn.chatgpt.com/docs/agent-configuration/rules)) | Two controls: a sandbox mode (`read-only`, `workspace-write`, `danger-full-access`) and an approval policy (`on-request`, `never`); rules are command prefixes (`prefix_rule(pattern = ["gh","pr","view"])`) | Config layers: team or managed, user `~/.codex/`, project `<repo>/.codex/` (only for trusted projects) | `workspace-write` with network off and `on-request`; untrusted folders start `read-only`. Presets "Ask for approval", **"Approve for me"** (same sandbox; eligible requests go to an automatic reviewer agent), "Full access" | `.rules` files in a `rules/` folder; `decision` is `allow`, `prompt` or `forbidden`; the most restrictive match wins; each rule may carry `match` and `not_match` examples checked at load | Adding a command to the allow list in the interface writes `~/.codex/rules/default.rules`; "smart approvals" propose a prefix rule during escalation, which the docs ask people to review carefully | Rules are experimental and prefix-only; `.git`, `.codex` and `.agents` inside the workspace stay read-only so the agent cannot rewrite its own settings |
| **OpenCode** ([permissions](https://opencode.ai/docs/permissions/)) | An action (`read`, `edit`, `shell`, `glob`, `grep`, `subagent`, `external_directory`, MCP tools) on a resource (path, command, URL, agent ID); plugins may add actions | Configuration layers, then global, then per-agent rules | No matching rule means `ask`; the default policy allows paths outside the project | Ordered `{action, resource, effect}` entries in `opencode.jsonc`; **the last match wins**; an operation touching several resources is denied if any is denied | `ask` waits for the client; a reject can reject every pending request in the session | Last-match-wins makes order matter, so a broad rule appended later silently widens access; the shell scanner cannot analyse `eval` or malformed syntax |
| **Gemini CLI** ([policy engine](https://geminicli.com/docs/reference/policy-engine/), [trusted folders](https://geminicli.com/docs/cli/trusted-folders/), [sandbox](https://geminicli.com/docs/cli/sandbox/)) | A tool name (wildcards for MCP servers), optionally a command prefix or a regular expression over the arguments as JSON; rules may apply only in some approval modes | Tiers Default, Workspace (currently disabled), User, Admin; a tier's base plus a priority 0–999 orders rules | Modes `default`, `autoEdit`, `plan`, `yolo` | TOML files in `~/.gemini/policies/`; `decision` is `allow`, `deny` or `ask_user`; the highest priority wins; a global deny hides the tool from the model | "Allow for all future sessions" is scoped by mode: an approval in a stricter mode applies to it and the more permissive ones, never the other way. Trusted folders are asked once per folder and saved in `~/.gemini/trustedFolders.json` | Workspace policies are disabled because a project could grant itself rights; folder trust is off by default; regular expressions over JSON arguments are easy to get wrong |
| **Goose** ([permission modes](https://goose-docs.ai/docs/guides/managing-tools/goose-permissions/), [tool permissions](https://goose-docs.ai/docs/guides/managing-tools/tool-permissions/)) | A tool of an extension | Per session; a default for new sessions in settings | **Autonomous by default**; Manual Approval; Smart Approval (risk-based: low-risk actions approved, others flagged) | Per tool: Always Allow, Ask Before, Never Allow, set in the app or `goose configure` | Asked per tool call in Manual or Smart mode | The default is no approval at all; "low risk" is the model's judgment, not a rule a person can read |
| **Cline** ([auto approve](https://docs.cline.bot/features/auto-approve)) | A category: read project files, read all files, edit project files, edit all files, run safe commands, run all commands, browser, MCP | Per installation | Everything off; the docs recommend turning on only "Read project files" | Toggles in settings; "YOLO mode" approves everything | Asked per call when the category is off; notifications when approval is needed | **The model marks each command `requires_approval`**: "Cline does not use a fixed allowlist", so "safe command" is the model's opinion |
| **Roo Code** ([auto-approving actions](https://roocodeinc.github.io/Roo-Code/features/auto-approving-actions/)) | A category (read, edit, run approved commands, browser, MCP, switch modes, subtasks, follow-up answers), plus allowed and denied command prefixes | Global, with a master Enabled switch | Off until enabled | Tiles in a dropdown; `roo-cline.allowedCommands` and `roo-cline.deniedCommands` prefix lists | The master switch pauses every auto-approval while keeping the selections | Prefix lists share the shell-string weakness; the docs warn auto-approve can mean "data loss, file corruption, or worse" |
| **OpenHands** ([security](https://docs.openhands.dev/sdk/guides/security)), chosen because it separates the policy from a risk judge | Each action, with a risk level from a security analyzer | Per conversation, set in code | Policies `AlwaysConfirm`, `NeverConfirm`, `ConfirmRisky` | Code (`conversation.set_confirmation_policy(ConfirmRisky())`) plus an optional analyzer, which may be a language model | The program's own confirmation handler asks | The risk level comes from an analyzer that may itself be a model; nothing a person writes down |

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

### 2.3 "Auto-approve for me", exactly

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

### 2.4 Where rules live and how they are shown

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
| Codex | "Approve for me" as a named default; `match` and `not_match` examples a rule can carry (proposed for `calternal check`); the agent cannot rewrite its own rules folder | Prefix rules over commands |
| Gemini CLI | "Always" scoped so it never widens a stricter mode; a project or space cannot grant itself rights beyond its tier | Priority numbers; regular expressions over arguments |
| OpenCode | Plugins can name new actions (our operations already are that) | Last match wins |
| Goose, Cline, Roo Code | A master pause switch | Model-judged "safe" or "low risk" |
| OpenHands | Policy separate from any risk judge | A model as the judge (question R4) |

## 4. Questions for the owner

**R1. What exactly does "auto-approve for me" include?**
*What:* the default rule set for a person's own runs (§2.3).
*Why now:* it is the first thing every agent run meets.
*Example:* you ask Claude to clean up your notes; it wants to remove 40
duplicates and email a summary to Lee.
*Options:* (a) **as in §2.3**: everything allowed in your personal space
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
