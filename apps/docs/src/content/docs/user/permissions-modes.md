---
title: "Permission modes"
---

<!-- Generated from: docs/agent-instructions.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Accepted design; the complete approval experience is not a released feature.


The `calternal` command in a sandbox carries a token for one thread. It acts as the
person, never beyond what they can do, and dies with the thread's work. Each thread has a
permission mode, chosen in the composer:

| Mode | The agent may | Asks the person |
| --- | --- | --- |
| Least access | Use the thread, its workspace, and the app it was opened from | For anything else: a grant names the app, read or write, and once, this thread, or always in this space |
| **Approve for me** (default) | Act with the person's access in the space; each request is reviewed automatically | Only for high-risk actions: messaging outside, spending, publishing or sharing outside, and anything that widens access. Asked once per scope, not repeatedly |
| Full access | Everything the person can do in the space, with no approvals | Never |

- The mode is about approvals only. Where the agent runs is a separate setting of the
  machine: in the cloud always inside the person's sandbox; on their own machine sandboxed
  or direct, as they chose for that machine. Both show as their own chip in the composer
  and in the thread's details.
- Automatic review in Approve for me is rules, by the risk class each operation declares
  in its manifest: reads and changes that can be undone are allowed; changes that cannot
  be undone ask; effects outside Calternal ask once per scope; anything that widens
  access always asks. Nothing inside the sandbox needs review. A reviewing model may be
  added later for the cannot-undo class only, able to allow or pass to the person.
- Approvals appear in the request bar on a trusted form the agent cannot answer.
- A mode requested with `agent.run` or `threads.send` applies directly to that
  turn, wider or narrower, and is saved as the thread mode (owner decision,
  2026-11-18). Choosing it on a send requires no additional passkey check.
- Every change of access or mode rotates the token and ends the old one. A thread
  credential cannot choose its own mode; only the person can do that. Explicit
  in-flight mode changes retain the existing fresh-person and Resume checks.
- What an agent reads (a note, a web page, a mail) is data, never an instruction, in every mode.
- Everything is recorded as "an agent for <person>, in thread <id>".
