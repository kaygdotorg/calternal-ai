# Threads screen on the real server

`apps/web/src/server/threads-client.ts` is `ThreadsClient` (the one interface the Threads app talks to) over the orchestrator's operations (`threads.list/get/create/send/stop/archive/delete`,
[threads.md](threads.md)) and the job event stream ([agent-turns.md](agent-turns.md)). `?server=1` selects it (development token mode, as Notes); the fixtures stay the default, for the preview and the tests.
Response shapes and the Markdown are in `server/thread-file.ts`; `server/threads-client.test.ts` runs the adapter against a recorded-response double of the server (shapes from `crates/server/src/threads`).

| Screen | Server |
| --- | --- |
| Sidebar list | `threads.list` (space from the shell's current space), re-read every 5 s while watched and after each change here |
| Open a thread | `threads.get`: the Markdown is parsed once per revision; messages are read in ranges from the parse |
| Send | `threads.send` with the caller's request id; the composer's permission mode, provider and model are sent with it (the server applies them directly) |
| Working state and steps | the job's SSE stream (`text_delta`, `tool_started`, `tool_finished`); at its end the file is re-read for the saved answer. The open file is re-read every 3 s, so a reload mid-turn still shows checkpointed text |
| Stop, archive, delete | `threads.stop`, `.archive`, `.delete` |

## Gaps the UI cannot close alone (for the backend)

- `threads.list` has no per-thread job state, model or message count: a turn started elsewhere shows idle until its file changes; the model is only known after a send here.
- The Markdown has no per-message time: every message shows the thread's creation time, the last one its update time.
- No operation renames or un-archives a thread, lists machines or records where a thread runs, or lists requests (questions, approvals): those calls return empty or say so. Picking a machine is kept locally.
- Finding a thread's running job after a reload needs an operation or a field on the thread; until then only turns started on this page are followed live.
- Lists and the open file are polled; a per-kind journal subscription would replace both.
