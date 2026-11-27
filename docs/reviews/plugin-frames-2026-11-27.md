# Plugin operation frames · 2026-11-27

Slice 3 of the [Chat-first build plan](../proposals/chat-first-build-plan-2026-11-27.md)
uses a private UTF-8 JSON line pipe per installed plugin and space. The
host starts a worker for `operations.ts` on demand and closes it after
idle. Standard output is reserved for frames. Standard error is private.
The worker receives no host credentials. An enrolled launcher must provide
the filesystem, network and process boundary before running installed code.

## Exchange

Each line has a `v: 1` envelope. `invocation` is a fresh UUID generated
by the host on each delivery. `id` is a positive callback sequence number
starting at 1 per delivery. A worker sends one callback at a time and awaits
the reply. The host accepts only callbacks for the current invocation and in
sequence. Each result carries exactly one of `result` or `error`.

| Direction | Frame fields in addition to `v` and `invocation` |
| --- | --- |
| Host → worker | `type: "invoke", selector, input, me, space, event` |
| Worker → host | `type: "call", id, call, input` |
| Host → worker | `type: "reply", id, result` or `type: "reply", id, error` |
| Worker → host | `type: "complete", result` or `type: "complete", error` |

`event` is `{kind: "call" | "change", key, item, before?, request?}`,
with `item: null` permitted for an itemless operation. Selectors are the
declared operation name for a call and the declared kind or collection
for a change. `me: {id,role}`, `space: {id}` and `event` come from the
host and are read locally as `s.me`, `s.space` and `s.event`. The
authorized context stays on the host when it routes a callback. Worker
frames have no authority fields.

Errors are `{code,message,next}`. Codes are `not-allowed`, `not-found`,
`invalid`, `conflict`, `too-large`, `limit`, `unreachable`,
`needs-sign-in`, `stopped` and `timeout`. A crashed worker fails only
its invocation. The next delivery can start a fresh process. Host-owned
receipts make the event key and the zero-based ordinal of each `s.write`
stable across replay, independent of the invocation and callback IDs.
The call route scopes the receipt by verified person, space and plugin
and checks matching retry input. Each `s.write` commits its own change
list atomically.

## Calls

| Call | Input | Result |
| --- | --- | --- |
| `s.query` | `{from,where?,sort?,group?,totals?,fields?,cursor?,limit?}` or `{collection,cursor?,limit?}` | `{rows,totals,cursor}` |
| `s.read` | `{item}` | `{id,kind,fields,computed,body,records,version}` |
| `s.write` | `{changes,preview?}`. Each change holds the `op` tag and its fields from format §8.1 and lane B’s route. | `{changes:[{id,version,anchor?}]}` |
| `s.state` | `{op:"get",item,key}` or `{op:"set",item,key,value}` | `{value}` |
| `s.call` | `{operation,input}`. `operation` is a qualified name listed in `calls:`. | The operation’s result |
| `s.run` | `{program,scope,targets}` | `{id,status}` |
| `s.run.send` | `{id,input}` | `null` |
| `s.run.stop` | `{id}` | `null` |

## Run handoff for lane D

Start example:

```json
{"v":1,"invocation":"b2851f86-5530-40e5-9f9f-ef4aae7c9daa","type":"call","id":1,"call":"s.run","input":{"program":"calternal-chat-claude-code","scope":"session","targets":["81414d2c-eb7d-4fac-a0e9-c4d5c75ae3c1"]}}
{"v":1,"invocation":"b2851f86-5530-40e5-9f9f-ef4aae7c9daa","type":"reply","id":1,"result":{"id":"db73a529-7b37-4b11-b189-c56f2112287b","status":"running"}}
```

The SDK attaches `send(input)` and `stop()` to the returned handle.
`status` is the returned snapshot. Subsequent handle calls route by run
ID through the same host check. The ID is a reference. Trusted event
context supplies the person and space, and the run service resolves
scope, targets, sending and stopping authority. Lane D owns those checks.
