# Provider terms for an orchestrator that launches official coding-agent CLIs

Researched 2026-11-17. Every quote below was read from the cited page on that date unless marked "secondary". Not legal advice.

## Framing used throughout

Two different things are often both called "third-party tools". The providers treat them differently, so every answer below separates them.

- **(a) Third-party harness using subscription tokens directly.** The tool takes the user's subscription OAuth token and calls the model endpoints itself (OpenClaw, old OpenCode Claude login, "sub2api" proxies). Anthropic and Google prohibit this. OpenAI allows it only through its official "Sign in with ChatGPT" programme.
- **(b) Third-party tool that launches the official CLI.** The tool starts the provider's own unmodified harness (Claude Code, Codex CLI), which the user signed in to themselves. The tool never holds or uses the credential. Calternal-AI is this.

Headline results:

- **Anthropic has an explicit written rule for (b), including operator-hosted sandboxes.** It is allowed under conditions. Storing the user's Claude login on the operator side is prohibited.
- **OpenAI documents (b) on the user's own machines and has no written rule for a paid third-party host.** It has an official bring-your-own-subscription programme, but paid or remotely hosted apps must apply.
- **Google prohibits (a) by name and says nothing official on (b).**
- **Cursor has no subscription-token statement; its restriction is on reselling the service.**

## Anthropic (Claude Pro/Max/Ultra/Team/Enterprise with Claude Code)

Main source: Claude Code "Legal and compliance", https://code.claude.com/docs/en/legal-and-compliance (no date on the page). Archive snapshots show the section "Can customers offer Claude Code in their products?" and the "collect, store, or intermediate" sentence were absent on 2026-08-16 and present on 2026-09-06.

| # | Question | Status | Decisive text | Source |
|---|---|---|---|---|
| 1 | Official CLI with a personal subscription on the user's own remote machine or CI | Allowed with limits | "For CI pipelines, scripts, or other environments where interactive browser login isn't available, generate a one-year OAuth token with `claude setup-token`". The GitHub Actions doc says the token "authenticates with your Claude subscription, available on Pro, Max, Ultra, Team, and Enterprise plans". Limit: "Advertised usage limits for Pro, Max, and Ultra plans assume ordinary, individual usage of Claude Code and the Agent SDK." I found no stated cap on machines or concurrent sessions. | https://code.claude.com/docs/en/authentication (undated); https://code.claude.com/docs/en/github-actions (undated); legal page |
| 2 | Same, on a machine a third party operates and charges for | Allowed with conditions | "Unless we've mutually agreed otherwise, preinstalling or running Claude Code in your products or services (e.g. in hosted sandboxes or other agent infrastructure) requires agreeing to our Commercial Terms of Service and complying with the conditions below". Conditions: "The Claude Code binary must not be modified... customers may not remove, disable, or restrict any authentication method built into it"; "Customers may not pay for, resell, or intermediate Claude usage on their end users' behalf. Each end user must authenticate with their own Anthropic API key, Claude subscription plan credentials, or 3P inference provider credential". Also: "Nor does it prevent an end user from signing in to the unmodified Claude Code binary with their own Claude subscription, including where a platform hosts Claude Code". Naming: you may say in plain text that the product "runs Claude Code", but not use the name or logo in a way that suggests endorsement. | Legal page |
| 3 | Operator stores the user's subscription token and injects it into the user's sandbox | Clearly prohibited for the operator | "developers may not collect, store, or intermediate Claude.ai credentials or session tokens — sign-in to a Claude account must complete through Anthropic's own flow." Also: "Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, Max, or Ultra plan credentials on behalf of their users." A user putting their own `setup-token` into their own environment is documented (Codespaces secret, GitHub secret). | Legal page; https://code.claude.com/docs/en/devcontainer |
| 4 | Sanctioned path for third-party products | API key or cloud provider; hosting unmodified Claude Code as in row 2; otherwise prior approval | "Developers building products or services that interact with Claude's capabilities, including those using the Agent SDK, should use API key authentication through Claude Console or a supported cloud provider." Agent SDK overview: "Unless previously approved, Anthropic does not allow third party developers to offer claude.ai login or rate limits for their products, including agents built on the Claude Agent SDK." "For questions about permitted authentication methods for your use case, please contact sales". No general bring-your-own-subscription mechanism for third-party apps exists. | Legal page; https://code.claude.com/docs/en/agent-sdk/overview |
| 5 | Consumer versus Team/Enterprise | Different contracts | Consumer Terms cover Free, Pro, Max, Ultra; Commercial Terms cover Team, Enterprise, API. Consumer Terms s.2: "You may not share your Account login information, Anthropic API key, or Account credentials with anyone else. You also may not make your Account available to anyone else." Consumer s.3: no access "through automated or non-human means, whether through a bot, script, or otherwise", "Except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it". Commercial D.4: may not "resell the Services except as expressly approved by Anthropic". The "route requests" sentence names only Free, Pro, Max, Ultra; the "collect, store" sentence is not limited by plan. | https://www.anthropic.com/legal/consumer-terms (effective 2025-10-08); https://www.anthropic.com/legal/commercial-terms (effective 2025-06-17) |
| 6 | About 10 agents running 24/7 | Unclear, risky on consumer plans | Only "ordinary, individual usage" (row 1) and the plan limits: "Pro and Max plans have a five-hour session limit and a weekly limit." "Anthropic reserves the right to take measures to enforce these restrictions and may do so without prior notice." No numeric concurrency rule found. | Legal page; https://support.claude.com/en/articles/11145838 ("Updated over 2 weeks ago") |

Enforcement and clarifications in 2026:

- **2026-01-09 (secondary):** Anthropic began technically blocking subscription tokens in third-party harnesses. The Register quotes Anthropic's Thariq Shihipar: "Third-party harnesses using Claude subscriptions create problems for users and are prohibited by our Terms of Service." https://www.theregister.com/2026/02/20/anthropic_clarifies_ban_third_party_claude_access
- **2026-02-19 (archived primary):** the legal page then read "Using OAuth tokens obtained through Claude Free, Pro, or Max accounts in any other product, tool, or service — including the Agent SDK — is not permitted and constitutes a violation of the Consumer Terms of Service." That sentence was gone from the 2026-04-21 snapshot and is not in the current page.
- **2026-04-04 (secondary):** InfoWorld quotes Anthropic's customer email: "Starting April 4, third-party harnesses like OpenClaw connected to your Claude account will draw from extra usage" "instead of from your subscription". Boris Cherny: "our subscriptions weren't built for the usage patterns of these third-party tools". https://www.infoworld.com/article/4154435/ and https://venturebeat.com/technology/anthropic-cuts-off-the-ability-to-use-claude-subscriptions-with-openclaw-and
- **2026-06-15 (primary):** "We've paused the previously-announced changes to Claude Agent SDK usage. For now, nothing has changed: Claude Agent SDK, `claude -p`, and third-party app usage still draw from your subscription limits."
- **2026-10-07 (primary):** "when you're signed in with your Claude plan, the Claude Agent SDK and `claude -p` still draw from your subscription limits." Both from https://support.claude.com/en/articles/15036540

## OpenAI (ChatGPT Plus/Pro/Business/Enterprise with Codex)

| # | Question | Status | Decisive text | Source |
|---|---|---|---|---|
| 1 | Codex CLI with a ChatGPT plan on the user's own remote machine or CI | Allowed with limits | Headless login is documented: "prefer device code authentication (beta)... run `codex login --device-auth`", or "copy your cached credentials to the headless machine". Limits: "API keys are still the recommended default for automation." "Don't expose Codex execution in untrusted or public environments." The CI guide applies only when "only one machine or serialized job stream will use a given `auth.json` copy". | https://learn.chatgpt.com/docs/auth and https://learn.chatgpt.com/docs/auth/ci-cd-auth (undated) |
| 2 | Same, on a machine a third party operates and charges for | Unclear | No clause addresses a host that runs the unmodified Codex CLI for its users. The nearest text is the consumer rule "You may not share your account credentials or make your account available to anyone else", and, for the plan-usage programme, "If you're interested in offering it in a paid or remotely hosted app, complete the interest form". | https://openai.com/policies/terms-of-use/ (effective 2026-01-01, read via Wayback snapshot of 2026-11-16 because the live page returned 403); https://developers.openai.com/siwc/token-sharing-open-source |
| 3 | Operator stores the user's login token and injects it | Discouraged, not explicitly addressed for operators | "Treat `~/.codex/auth.json` like a password: it contains access tokens. Don't commit it, paste it into tickets, or share it in chat." The CI guide is for a "trusted private" runner. Under the official programme, tokens are to be kept "in protected local or self-hosted runtime storage". | Auth docs; SIWC docs |
| 4 | Sanctioned path for third-party products | Several, including an official bring-your-own-subscription programme | Sign in with ChatGPT: "For open-source developers: Let users run AI workloads in your tools with their ChatGPT plan, without requiring them to provide an API key." "Sign in with ChatGPT is currently available to selected commercial partners through a limited trial. ChatGPT plan usage is available to all open-source partners and selected private clients." OpenAI's own page lists OpenClaw, OpenCode, Pi and T3 as open-source integrations. Others: API keys; Codex access tokens ("currently supported for ChatGPT Business and Enterprise workspaces", "for trusted scripts, schedulers, and private CI runners"); service accounts ("available only on pay-as-you-go plans"); the Codex app-server ("Embed Codex into your product") and Codex SDK. | https://developers.openai.com/siwc/llms-full.txt; https://learn.chatgpt.com/docs/sign-in-with-chatgpt; https://learn.chatgpt.com/docs/enterprise/access-tokens |
| 5 | Consumer versus Business/Enterprise | Different contracts and credentials | Services Agreement 3.1: "Customer will not share Account access credentials or individual login credentials between multiple users. Customer may not resell or lease access to its Account or any End User Account." 3.2: "End User Accounts may only be provisioned to, registered for, and used by, a single End User." Access tokens and service accounts exist only on workspace plans. | https://openai.com/policies/services-agreement/ (updated 2025-12-01, effective 2026-01-01, via Wayback 2026-11-10) |
| 6 | About 10 agents running 24/7 | Unclear | Plan limits apply ("Weekly limits may also apply"; a turn in progress may finish "subject to fair use limits"). Terms prohibit to "circumvent any rate limits or restrictions". For shared plan usage: "the five-hour usage limit is shared across all apps". No numeric concurrency rule found. | https://learn.chatgpt.com/docs/pricing; Terms of Use; SIWC docs |

Staff statements (secondary, not verified verbatim): reporting from August 2026 says Codex lead Thibault Sottiaux attributed drained limits to "sub2api" tools that repackage a subscription as an API, and said official and supported open-source clients were unaffected. The one article I could load (mitrade, 2026-08-21) only paraphrases him.

## Google (Gemini CLI, Antigravity) and Cursor, briefly

| Provider | Finding | Text | Source |
|---|---|---|---|
| Google Gemini CLI | (a) clearly prohibited | "Directly accessing the services powering Gemini CLI (for example, the Gemini Code Assist service) using third-party software, tools, or services (for example, using OpenClaw with Gemini CLI OAuth) is a violation of applicable terms and policies." | https://github.com/google-gemini/gemini-cli/blob/main/docs/resources/tos-privacy.md (undated) |
| Google Antigravity | (a) clearly prohibited; (b) unclear | "Using third party software, tools, or services to access the Service (e.g. using OpenClaw with Antigravity OAuth) is a breach of this Agreement." The same section bars "using the Service in connection with products not provided by us", which could be read against (b). A forum question on exactly (b) (2026-07-20) has no Google staff answer. | https://antigravity.google/terms (date not found); https://discuss.ai.google.dev/t/175462 |
| Cursor | Resale prohibited; embedding the SDK supported | Terms 1.5: may not "rent, lease, lend, or sell the Service". Staff member Kevin Neilson (forum, 2026-05-04) called embedding the SDK in a product "a supported and explicitly intended use of the Cursor SDK", with the guardrail "you can't resell Cursor itself as a standalone service". Headless CLI authenticates with `CURSOR_API_KEY`. | https://cursor.com/terms-of-service (last updated 2026-09-03); https://forum.cursor.com/t/159741; https://cursor.com/docs/cli/headless |

## How bb and T3 Code do it

| | bb / bb-lite | T3 Code |
|---|---|---|
| Claude Code | Claude Agent SDK: "one streaming-input `query()` per thread". The installed bb passes only `BB_CLAUDE_CODE_EXECUTABLE` and `CLAUDE_CODE_OAUTH_TOKEN` through to the CLI; "Mint the token with `claude setup-token` for machines with no interactive login." | Claude Agent SDK (`@anthropic-ai/claude-agent-sdk`) with `pathToClaudeCodeExecutable` pointing at the installed `claude`. "T3 Code uses Claude Code's login and configuration." |
| Codex | "one `codex app-server` child per thread over JSON-RPC stdio" | Codex app-server, in two modes: the existing `codex login`, or a "managed" ChatGPT connection where T3 runs OpenAI's Sign in with ChatGPT flow itself (scope `chatgpt.tokens.use.direct`, issued `oaiapp_` client ID) and stores those tokens. |
| Who signs in | The user, in a terminal, with the provider's CLI. | The user with the CLI (`claude auth login`, `codex login`), or OpenAI's consent page for the managed mode. |
| Reads the subscription token itself? | Mostly no. One exception in bb-lite: the Codex title service "reads the Codex CLI's own `auth.json` (never written, never logged) and asks the Codex responses endpoint". That is type (a) behaviour. | Claude: none found in the files I checked. Codex managed mode: yes, by design, under OpenAI's programme. |
| Hosted mode | getbb.app is a relay plus a small bb-funded AI service for titles and voice. bb-lite can run hosted servers on Fly: the image holds "the Claude Code and Codex CLIs"; "Agents are not signed in. The owner signs Claude Code or Codex in from a terminal on the server"; the volume "keeps agent sign-ins". | None. T3 Connect is a relay; "credentials, and agent work stay on the remote machine". No charge: "There's no way to pay us to use T3 Code." |
| Statements on terms | None found in the guide, skills or bb-lite docs. | FAQ: "T3 Code uses those harnesses. It is not a harness". "We work with all of the labs and T3 Code is within the TOS of Claude Code, Codex, and Google Antigravity." On account pools: "If we add an auto-switcher, we are risking TOS bans for our users." These are T3's own claims, not provider statements. |

Sources: `bb guide providers` and the `claude-code-provider`, `bb-account`, `bb-cloud-ai` skills on this machine; https://github.com/get-bb/bb-lite (`docs/architecture.md`, `docs/hosted.md`, `REPORT.md`); https://github.com/pingdotgg/t3code (`README.md`, `docs/user/faq.md`, `docs/user/providers-claude.md`, `docs/user/providers-codex.md`, `docs/internals/providers.md`, `apps/server/src/orchestration-v2/Adapters/ClaudeAdapterV2.ts`, `apps/server/src/provider/CodexChatGptAuth.ts`).

Both precedents drive Claude Code through the Agent SDK rather than the bare CLI. Anthropic's text says products "using the Agent SDK, should use API key authentication", while its help centre says Agent SDK and "third-party app usage still draw from your subscription limits". That tension is unresolved in writing. Launching the unmodified `claude` binary directly is the form Anthropic's hosting clause names.

## Direct answer for an orchestrator that launches the official CLIs

**On the user's own machine: clearly fine for Claude Code and Codex.** The user installs or is given the unmodified CLI, signs in through the provider's flow, and the orchestrator starts and supervises it. Anthropic documents subscription use in terminals, CI and containers; OpenAI documents device login and headless use. Keep usage within "ordinary, individual usage" for Anthropic.

**On a machine the operator hosts and charges for, what changes:**

- **Anthropic: still allowed, now under written conditions.**
  1. The operator must agree to the Commercial Terms.
  2. The binary must be unmodified and no built-in sign-in method removed or restricted.
  3. Each user signs in with their own credential through Anthropic's flow, and usage bills to them.
  4. The operator must not pay for, resell or intermediate Claude usage. Charging for compute is not addressed as such; the clause is about Claude usage.
  5. The operator must not collect or store the Claude login. This rules out the "store the credential encrypted and place it into sandboxes" design.
  6. Naming is limited to plain-text statements such as "runs Claude Code".
- **OpenAI: nothing written either way.** The CLI's own login inside the user's sandbox is the documented headless path, but no text covers a paid host. The official programme points paid or remotely hosted apps to an interest form.
- **Google: do not offer hosted Gemini CLI or Antigravity on a subscription login without a written answer.**

## What is safe today, what is risky, what needs written confirmation

**(i) The user's own machines**

- Safe: launching unmodified Claude Code and Codex CLI that the user signed in to; a user-held `claude setup-token` or `codex login --device-auth` on their own VPS.
- Risky: reading `auth.json` or `.credentials.json` to call model endpoints directly (as bb-lite's title service does); account pools or auto-switching; ten agents around the clock on one consumer plan.
- Confirm: whether driving Claude Code through the Agent SDK with a subscription login in a distributed product counts as "ordinary use of Claude Code".

**(ii) Invite-only cloud machines run by one operator**

- Safe for Anthropic if the six conditions above are met and sign-in happens inside the sandbox through Anthropic's flow.
- Risky: any operator-side credential vault; copying one login into many sandboxes (OpenAI's guide assumes one machine or a serialized job stream per `auth.json`); a Claude login persisting on an operator-owned volume is implied by the hosting clause but not stated.
- Confirm: OpenAI's position on a third party hosting the Codex CLI with users' ChatGPT logins; Anthropic's view of login persistence on operator disks.

**(iii) Paid public cloud agents**

- Safe: Anthropic under the same conditions, with the Commercial Terms accepted; API-key or cloud-provider credentials for both providers; OpenAI Business/Enterprise access tokens for workspace users.
- Risky: pricing that looks like reselling model usage; marketing that implies endorsement; heavy 24/7 fleets on consumer plans, which Anthropic may act on "without prior notice".
- Confirm in writing before launch: Anthropic sales, that the product fits the hosting clause and that charging for compute is not "intermediating"; OpenAI, through the Sign in with ChatGPT interest form or an account contact, for paid remote hosting; Google and Cursor before offering either at all.

## Could not verify

- OpenAI policy pages on openai.com and help.openai.com returned 403. The Terms of Use and Services Agreement were read from Wayback snapshots (2026-11-16 and 2026-11-10); the help articles on Codex with ChatGPT plans and on Sign in with ChatGPT (20001542) were not read.
- No primary copy of Anthropic's April 2026 customer email, the January 2026 staff posts, or Sottiaux's August 2026 posts; these rest on press reports.
- The exact date Anthropic added the hosting clause (only bounded to 2026-08-16 to 2026-09-06), and the date of the Antigravity terms.
- Any stated limit on number of machines or concurrent sessions for either provider: none found, which is not proof none exists.
- Whether "Anthropic does not permit... route requests through Free, Pro, Max, or Ultra plan credentials" is meant to exempt Team/Enterprise seats.
- Anthropic's Service Specific Terms and Usage Policy were not read for relevant clauses.
- bb's main (non-lite) source code was not inspected; bb findings come from its installed guide and skills and from bb-lite. T3 Code's code was sampled, not audited.
- Google's Gemini Code Assist FAQ reportedly names prohibited tools; the page did not load usefully.
