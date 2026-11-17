---
title: "Threads"
---

<!-- Generated from: docs/orchestrator-proposal.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Accepted orchestrator decisions; this is not a claim that the complete thread experience is implemented. Later decisions below take precedence over earlier proposal text.


- The plugin's ID is `calternal-orchestrator`. Every official plugin is `calternal-<name>`.
- Child threads are visible in the sidebar under their parent and archive automatically
  when they finish. A child's file records the thread and the message that spawned it
  (`spawned-by:` in frontmatter), permanently, so an archived child can always be traced
  to its origin and a parent can list every child it ever started.
- A request can carry a list of questions: it is a form described with the generated-UI
  components and drawn by the app. Approvals and secret entry stay fixed, trusted forms.
- The CLI is non-interactive; there is no TUI. The web UI is built at the same time as the
  orchestrator so there is a surface to plug it into and test.
- Offline sending is queued after all: a message sent while offline is a visible pending
  submission on the account that can be edited or cancelled, and is submitted on
  reconnect with its request ID, re-checking access first. (Restores the 2026-11-16
  decision.)
- Work does not move between machines by itself, on the pool or anywhere. If a machine
  goes silent, the turn shows as interrupted and the person is told; Resume starts it
  again from its last checkpoint, on that machine if it is back or on another one.
  (Restores the 2026-11-16 decision; the automatic re-placement built in the machine
  role becomes resume-on-request.)
- Cals come after plain threads.
- One file per thread; long tool output is kept outside the file and linked. Long threads
  are the UI's speed test.
- On a person's own machine they choose per machine: sandboxed, or direct access. On
  macOS the sandbox is the system's own process sandbox, the one Claude Code and Codex
  already use. The app offers, in a friendly way, to add the person's machine so agents
  can work on it.
- A space's permissions apply to every plugin in it, the orchestrator included. A thread
  in a shared space is visible to members who can read the space.
- A coding subagent gets its own worktree.
- Agent sandboxes have outbound network access (they must reach the provider and install
  packages); a space can restrict it to an allow-list. Plugin workers have none.

- When a turn fails with a provider or network error it retries automatically: 5 attempts,
  waiting 30, 60, 120, 240 and 480 seconds. After that, on API-key billing it checks every
  3 hours whether the provider is back; on a subscription it snoozes the thread until the
  limit's reset time and resumes by itself. Both are on by default and configurable.
- The right side is a flat sidebar like the left, not a card.
- Message text is not streamed to the screen by default; the finished message appears at
  once while steps and a working state show progress. Streaming is a setting.
