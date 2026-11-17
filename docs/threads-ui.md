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
| Request bar (questions, approvals, secrets) | `ServerRequests` (`server/requests.ts`): `requests.list`, `requests.answer`, `requests.approve` (`once` or `thread`), `requests.deny`; refreshed when the feed says a turn started or stopped waiting. **The wire shape is issue #32's stated goal, not yet in dev**; `requestOf` and `answerInput` are the one place to change. A server without them answers `not_found` once and the list stays empty |

Tested against a recorded-response double (`threads-client.test.ts`) and, end to end, against a seeded installation (`e2e/live-threads.mjs`).

## What the server still lacks

- Where a thread runs: no operation names or records a machine; the choice is kept on the page and the server picks one.
- Requests (#32) are not in dev.
- Changes and Run tabs have no operation.

## The client interface

`ThreadsClient` (`apps/threads/client.ts`) is the threads themselves (list, get, create, delete, messages, watch, send, stop, rename, archive, unarchive) plus three smaller interfaces a server
serves from separate operations: `ChoicesClient` (provider and model, permission, machines, where it runs; on the server these travel with `send`), `RequestsClient` (list, watch, answer) and
`PanelsClient` (Changes and Run). The split is type-level, with no change in behaviour; the server adapter delegates requests to its own small class and the fixtures implement all four in one.
A further step, not taken because it would only add code today: components taking the narrow interface they use (the request bar takes `RequestsClient`, the composer `ChoicesClient`), so a screen can run
on a server that serves only part of it.
