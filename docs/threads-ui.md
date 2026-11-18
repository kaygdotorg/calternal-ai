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
| Request bar (questions, approvals, secrets) | `ServerRequests` (`server/requests.ts`) on the requests lane's operations (`docs/requests.md`): `requests.list` (account-wide, `{}`), `requests.answer` `{id, answer}`, `requests.approve` `{id, lifetime: once or this_thread}`, `requests.deny`; a secret's value goes to `POST /api/v1/auth/secrets/provide`. The bar offers "Allow for this thread" only when the server's `detail.lifetimes` lists it (`lifetimesOf` reads that list and infers nothing; none listed means once only). Approving needs a fresh passkey proof on an installation session: a development bearer is refused and the server's words are shown. Refreshed when the feed says a turn started or stopped waiting |

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

For a fresh real account, each in plain words with the next step and no placeholder art (`ThreadNotices.svelte`, `Notice.svelte`, the sidebar's empty block, the thread's blank view). Fixture scenarios show them: `?scenario=empty|no-machine|queued|offline|signin|failed|usage-limit|permanent|retrying|retrying-api|retrying-plan|interrupted|interrupted-unsure` (`e2e/states.mjs`).

**From the server (turn readiness).** Two sources, both real:

| Source | Fields | Becomes |
| --- | --- | --- |
| `turn_reason` on `threads.list`, `threads.get` and the `data.turn_reason` of `threads.turn_state_changed` journal entries (`reasonNotice`) | `no_machine`; `waiting_for_capacity` (`queue_position`); `machine_offline` (`since_ms`); `provider_signed_out` (`provider`, `sign_in_command`); `retrying` (`attempt`, `next_attempt_ms`, `rule`: `provider_backoff`, `api_availability`, `subscription_reset`); `failed` (`class`: `transient`, `usage_limit`, `permanent`; `reset_at_ms`); `interrupted` (`resumable`, `why`, `since_ms`); `waiting_on_person` (the request bar draws it) | the notice for a turn that is waiting, retrying, failed or dropped |
| `agent.readiness` `{space_id}` (`readinessNotice`), re-read on the account feed's `readiness_changed` (`GET /api/v1/account/live?readiness=true`) | machines (`connected`, `available`, `last_seen_ms`, per-provider `check`: `installed`, `signed_in`, `sign_in_command`) and the pool | before a message is sent: no machine, every machine offline (the one seen last is named), the provider not signed in on the machine that is up (its own command; **Check again** calls `agent.check`). A provider with no observation yet is not called signed out |

The composer's machine list is the readiness snapshot's machines (and "Pool"). A thread with a turn reason shows that; otherwise the readiness notice shows while the thread is idle.

| Situation | Says | Next step |
| --- | --- | --- |
| No thread yet (sidebar and first view) | What a thread is (the first view); one line in the sidebar | **Start a thread** |
| Empty thread | Nothing sent yet; the agent shows its steps and asks before anything not allowed | **Write a message** (focuses the box) |
| No machine enrolled | Agents work on a computer you add, or one from the pool | **Open Machines** (Settings > Machines) |
| All machines busy | Your place in the queue; it starts by itself | none |
| Machine offline | Names the machine and when it was last heard from; the message is safe; nothing moves by itself | **Open Machines** |
| Provider not signed in | Names provider and machine; sign in once on the machine with the provider's own command; Calternal never sees the login | **Copy the command**, **Check again** |
| Retrying | By rule: provider backoff (attempt n of 5, 30 s up to 8 min, with a countdown), API availability (checks every 3 hours), the plan's limit (continues at the reset time) | **Try now** |
| Failed | By class: transient (already retried five times), usage limit (until about the reset time), permanent (stopped; retrying would not help) | **Try again** |
| Interrupted | The machine stopped answering; stopped at its last saved step; nothing moved. If it cannot be resumed safely, says so and offers nothing | **Resume** (when it can) |
| Request bar | Question: the agent waits and continues once answered. Approval: nothing runs until you decide; "for this thread" only when allowed. Secret: where the value goes | the buttons already on the bar |

Resume and Try again are `agent.resume` on the thread's last job.

## Backend follow-ups (for the requests and threads lanes)

- **Which lifetimes a request allows.** Resolved: the server sends `detail.lifetimes` (dev, 2026-11-18) and `lifetimesOf` uses only it.
- **A fixture provider that emits tool events.** The seeded installation's provider emits only `session` and `text_delta`, so a page reloaded mid-turn shows the Working row with no steps (a real provider's steps are replayed from the oldest retained event when a page connects without a bookmark; checked: the second connection returns the whole stream). A fixture with a few `tool_started` and `tool_finished` events would let `live-threads.mjs` check the steps.
- **Resuming a job stream after a bookmark** re-sent the `session` chunk in a probe (`?after=<id>` with the last id seen): worth a look, harmless to the UI.

The page's session code is the shell's: the adapter uses the shared `ServerClient` (bearer in development token mode, the browser's cookie otherwise, nothing of its own), and `/api/v1/auth/me` is called by the shell's `server/boot.ts`, not by the Threads adapter.

## Prepared for the delta read and `thread_full` (2026-11-18)

- `thread_full` (a checkpoint past 2 MiB fails the turn): `reasonNotice` maps it to the `thread-full` notice, "This thread is full", with a New thread action (`ThreadNotices`, fixture scenario `?scenario=full`). Sends are left to the server to refuse.
- Delta read: `ServerThreads.pull` re-reads the whole thread on each checkpoint (`load` is the one place that calls `threads.get`). When `threads.get` takes `since` and returns only the bytes one checkpoint added, `load` keeps the saved text and revision per thread and appends the returned bytes before parsing; a `reset` or a revision that does not follow from the kept one falls back to the full read. Waiting for the server's merge to fix the field names.
