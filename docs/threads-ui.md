# Threads screen on the real server

`apps/web/src/server/threads-client.ts` is `ThreadsClient` (the one interface the Threads app talks to) over the orchestrator's operations (`threads.list/get/create/send/stop/archive/delete`,
[threads.md](threads.md)) and the job event stream ([agent-turns.md](agent-turns.md)). `?server=1` selects it (development token mode, as Notes); the fixtures stay the default, for the preview and the tests.
Response shapes and the Markdown are in `server/thread-file.ts`; `server/threads-client.test.ts` runs the adapter against a recorded-response double of the server (shapes from `crates/server/src/threads`).

| Screen | Server |
| --- | --- |
| Sidebar list | `threads.list` (the shell's current space): turn state, provider, model, mode, message count, last activity, parent. One operation, no file read. Kept current by the space's live journal (`server/space-feed.ts`), never polled |
| Turn state | applied in place from `threads.turn_state_changed` journal entries (`data.state`, `data.job_id`) |
| Open a thread | `threads.get`: the Markdown is parsed once per revision (message times from the markers); messages are read in ranges from the parse. A saved message or checkpoint of the open thread re-reads it (coalesced 120 ms) |
| Reload mid-turn | `threads.get` names the running job; the page follows its job stream from the last event it saw |
| Send | `threads.send` with the caller's request id; the composer's permission mode, provider and model travel with it (the server applies them directly) |
| Working state and steps | the job's SSE stream: provider events are nested, `chunk.data.event` (`text_delta`, `tool_started`, `tool_finished`); at its end the file is re-read for the saved answer |
| Stop, rename, archive, unarchive, delete | `threads.stop`, `.rename`, `.archive`, `.unarchive`, `.delete` |
| Request bar (questions, approvals, secrets) | `ServerRequests` (`server/requests.ts`) on the requests lane's operations (`docs/requests.md`): `requests.list` (account-wide, `{}`), `requests.answer` `{id, answer}`, `requests.approve` `{id, lifetime: once or this_thread}`, `requests.deny`; a secret's value goes to `POST /api/v1/auth/secrets/provide`. The bar offers "Allow for this thread" only when the approval may last (irreversible and access-widening actions are single-use: `lifetimesOf`). Approving needs a fresh passkey proof on an installation session: a development bearer is refused and the server's words are shown. Refreshed when the feed says a turn started or stopped waiting |

Tested against a recorded-response double (`threads-client.test.ts`) and, end to end, against a seeded installation (`e2e/live-threads.mjs`).

## What the server still lacks

- Where a thread runs: no operation names or records a machine; the choice is kept on the page and the server picks one.
- Changes and Run tabs have no operation.

## The client interface

`ThreadsClient` (`apps/threads/client.ts`) is the threads themselves (list, get, create, delete, messages, watch, send, stop, rename, archive, unarchive) plus three smaller interfaces a server
serves from separate operations: `ChoicesClient` (provider and model, permission, machines, where it runs; on the server these travel with `send`), `RequestsClient` (list, watch, answer) and
`PanelsClient` (Changes and Run). The split is type-level, with no change in behaviour; the server adapter delegates requests to its own small class and the fixtures implement all four in one.
A further step, not taken because it would only add code today: components taking the narrow interface they use (the request bar takes `RequestsClient`, the composer `ChoicesClient`), so a screen can run
on a server that serves only part of it.

## Empty, missing and failed states

For a fresh real account, each in plain words with the next step and no placeholder art (`ThreadNotices.svelte`, `Notice.svelte`, the sidebar's empty block, the thread's blank view). Fixture scenarios show them: `?scenario=empty|no-machine|offline|signin|failed|retrying|interrupted` (`e2e/states.mjs`).

| Situation | Says | Next step |
| --- | --- | --- |
| No thread yet (sidebar) | A thread is a conversation with an agent that works on your files and notes, with only the access you allow | **Start a thread** |
| Empty thread | Nothing sent yet; the agent shows its steps and asks before anything not allowed | **Write a message** (focuses the box) |
| No machine enrolled | Agents work on a computer you add, or one from the pool | **Open Machines** (Settings > Machines) |
| Machine offline | Names the machine; the message is safe and waits; nothing moves to another machine by itself | **Open Machines** |
| Provider not signed in on the machine | Names provider and machine; sign in once on that machine; Calternal never sees the login | **Copy the command** (`claude auth login`) |
| Turn failed | What failed; Calternal already retried five times (30 s up to 8 min) | **Try again** |
| Turn waiting to retry | Counts down; attempt n of 5; keeps checking afterwards or waits for the plan's limit to reset | **Try now** |
| Turn interrupted | The machine stopped answering; stopped at its last saved step; nothing moved | **Resume** |
| Request bar | Question: the agent waits and continues once answered. Approval: nothing runs until you decide; "for this thread" lasts until the thread is deleted. Secret: where the value goes | the buttons already on the bar |

What the server can tell today: a failed or interrupted turn comes from the turn state (`failed`, `interrupted`) with generic words; "no machine", "machine offline" and "provider not signed in" need a machine-readiness read the server does not offer yet (the real adapter shows none of them rather than guess), and the retry countdown needs the retry fields of the job checkpoint. Resume is `agent.resume` on the thread's last job.
