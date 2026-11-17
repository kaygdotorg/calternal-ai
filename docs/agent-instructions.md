# How agents learn Calternal

Decided with kayg, 2026-11-17. Terms: [glossary](glossary.md). Style rule for everything
an agent is told: condensed. Mean a lot, say little.

## What an agent is given

| Layer | Contents | Read |
| --- | --- | --- |
| Core, ~300 words | Where it is (space, thread, machine, workspace); how to behave; the one path to Calternal | Always |
| Tool index | One line per operation it may call here, grouped by plugin, generated from manifests | Always; details on demand |
| Skills | Plugin API guide, CLI reference, component guide, each plugin's guidance | When the task needs them |
| Space, then personal instructions | Added by a space's Managers, and by each person for themselves | Always, after the core |

The core is modelled on bb's and T3 Code's skills. Each provider plugin may add a short
note for its model's habits.

## Rules

- **Behaviour.** Friendly and proactive: do the task all the way through; never stop after
  saying what you will do. Blocked: say why and ask.
- **One path.** The agent is told about exactly one way to reach Calternal. The CLI where
  there is a shell (the default); MCP only where there is none. The API is for code it writes.
- **Prefer Calternal's tools** when both exist: asking the person, starting helpers, notes
  and other apps, secrets, schedules. Its own tools stay right for workspace files,
  commands and the web.
- **Enforce, then tell.** Where a harness lets us switch off its duplicate tool, we do.
  Telling is the fallback.
- **Extending.** The plugin API guide ships as a skill; an agent asked to change the app
  loads it first.

## Access

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

## Space standing instructions

A space's standing instructions are `AI/AGENTS.md` (owner decision, 2026-11-18).
Only Managers and the Owner may write that file. It is reserved from all generic
kind placements and item writers, including Editor-level plugin operations.
The previous filename is not an alias. Reads remain bounded and descriptor-relative;
symlinks fail closed. Instructions provide context and never grant authority.

The preview reads an operator-provisioned file. A Manager-only write operation
still needs its public contract; the generic content API cannot be used as a
workaround. See the instruction-writer question in morning-decisions.md.

## Testing

Per provider, scripted tasks check that the agent asked through Calternal, spawned through
Calternal, requested access instead of failing or guessing, and finished the task. Run
with the UI once it exists, since that is where it is most useful to watch.
