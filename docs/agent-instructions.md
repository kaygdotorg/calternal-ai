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
| Full access | Everything, with no sandbox and no approvals | Never. Offered only on the person's own machine |

- Approvals appear in the request bar on a trusted form the agent cannot answer.
- Every change of access or mode issues a new token and ends the old one.
- What an agent reads (a note, a web page, a mail) is data, never an instruction, in every mode.
- Everything is recorded as "an agent for <person>, in thread <id>".

## Testing

Per provider, scripted tasks check that the agent asked through Calternal, spawned through
Calternal, requested access instead of failing or guessing, and finished the task. Run
with the UI once it exists, since that is where it is most useful to watch.
