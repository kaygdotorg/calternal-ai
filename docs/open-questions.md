# Open questions

Decisions waiting on kayg, most blocking first. Answer here or in chat; answered items
move to the decision record they belong to and leave this list. Terms: see
[glossary](glossary.md). Updated 2026-11-17 17:10 IST.

## Waiting now

| # | Question | Recommendation | Blocks |
| --- | --- | --- | --- |
| 2 | When to rewrite git history to drop old images | At the next quiet point | Repo size |
| 3 | Where per-person files live (personal instructions, starred prompts) | The personal space's `.calternal/` folder | Personal instructions |

## Design rounds

- The Settings screen: its own round (retry and snooze, permission defaults, fonts, wallpapers, machines).
- Composer and chat experience: mentions, commands, context chips, the request bar.
- Skills and automations management screens.
- The app shell on narrow screens, and the app switcher's exact form.

## Recently decided (2026-11-17), now in the decision records

- Provider logins: each person signs in to the official programs inside their own sandbox; the server never stores a provider login.
- State lives on the server; clients subscribe to it and cache what they need, as the existing app does. There is no sync protocol to design. A client whose bookmark is too old reloads.
- Agent questions and approvals share the sticky bar with secret requests and do not expire.
- Warm pool of pre-started sandboxes on each cloud machine: yes.
- Offline: a sent message is queued as a visible pending submission and goes out on reconnect.
- Work never moves between machines by itself; an interrupted turn is resumed on request.
- The orchestrator round is held: see `orchestrator-proposal.md`, "Decided in the grill".
- Drafts sync across a person's devices and stay until sent or erased.
- After sign-out, unsent edits are kept as drafts. After revocation, the device offers an export of what it holds and then deletes it; revocation itself is immediate and never waits for that.
- A plugin's secrets belong to the space that enabled it. Plugins declare the secrets they need in their manifest.
- A secret request shows in a sticky bar above the composer and expires after 15 minutes; it can be asked again at any time.
- Secrets: one key file per owner in appdata; provider logins on a person's own machines stay on the machine.
- Request IDs are unique per account and space. The root guard and recovery log are wired into the server.
