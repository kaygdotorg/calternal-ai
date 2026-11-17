# Open questions

Decisions waiting on kayg, newest first. Answer here or in chat; answered items move to
the decision record they belong to (`docs/plugin-api.md`, `docs/ui-shell.md`,
`docs/addresses.md`) and are removed from this list.

## Waiting now

| # | Question | Recommendation | Blocks |
| --- | --- | --- | --- |
| 1 | Sandbox boundary: one microVM per person per space, per workspace, or per person? | Decide from the hybrid measurement (issue #7) | Cloud agents |
| 2 | Containers now and microVMs before public sign-up, or microVMs from the start? | Containers inside a per-person microVM if #7 supports it | Cloud agents |
| 3 | A warm pool of pre-started sandboxes per cloud machine? | Yes, small | Subagent start time |
| 4 | Offline: nothing queued to run later (decided). Confirm drafts and cached reading only. | Yes | Client core sync |
| 5 | Client sync: how a client catches up after a gap (snapshot plus cursor, or leased replay)? | Snapshot plus cursor | Client core sync |
| 6 | Unsaved offline edits after sign-out or revocation: keep encrypted for recovery, or offer export then delete? | Offer export | Client core sync |
| 7 | Who owns a plugin's own secrets: the space that enabled it, or the plugin? | The space | Plugin secrets |
| 8 | Do pending secret requests expire? | Yes, after a day | Secrets |

## Design rounds not yet held

- Composer and chat experience: mentions, commands, context chips.
- Skills and automations management screens.
- The app shell on narrow screens, and the app switcher's exact form.
- The orchestrator plugin itself: threads, turns, subagents, review.
- Provider terms: whether personal AI subscriptions may be used from servers we operate and charge for.
