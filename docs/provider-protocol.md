# Provider adapter protocol v1

A provider plugin declares an optional `provider` entry in manifest v1:
`protocol_version`, an executable basename resolved inside the sandbox, and a
`default_model`. The host validates it without running code. The machine launches
the executable in the thread workspace. Providers have no server-side process.
The Claude provider is `calternal-claude-code`; Threads is `calternal-orchestrator`.

Rust types in `crates/core/src/provider.rs` generate the SDK types and the schemas
`contract/provider-v1.schema.json` and `contract/provider-events-v1.schema.json`.
The SDK reexports those types. No provider-specific authentication type exists.

Each UTF-8 stdin line is `{version:1,seq,command}`. Commands are check, start
(model, provider options, cwd, trusted instructions, optional official session to resume), send
(prompt plus labelled context blocks), steer (a new prompt), and stop. Command
numbers start at one and must increase by one. A line, including its newline, is
at most 64 KiB. There is one active turn per adapter. At most 32 context blocks
are permitted; source labels are at most 128 bytes.

Each stdout line is `{version:1,seq,event}`. Event numbers start at one per adapter
process, independently of machine exec and job stream sequence numbers. Events:
check, session, text_delta, tool_started, tool_output, tool_finished, question,
approval_request, usage, error, done. Lines are at most 12 KiB including newline.
Text and tool detail are at most 4 KiB; IDs and tool names 128 bytes; summaries and
safe error messages 512 bytes. JSON integers must be exactly representable in JS.
Unknown versions, malformed frames, sequence gaps and partial final lines fail
closed. Error classes distinguish transient, usage_limit, authentication,
refusal, permanent and unknown. Billing is api, subscription or unknown. A reset,
when reported, is Unix milliseconds. It is not guessed from error prose.

All tool events carry the same step ID, tool name and short target or
description. The Claude Code adapter emits `tool_output` for a nonempty tool
result and reports its text byte count. It omits the raw result. Completion
carries `failed` and a short detail. The server folds the scrubbed step notices
into a saved thread's Markdown, using starts and completions as checkpoint
boundaries.

Back-pressure is end to end: the adapter awaits stdout drain; the machine buffers
16 chunks of 16 KiB plus at most 256 KiB unacknowledged; machine output is fenced
and acknowledged by sequence; its server buffer is bounded. The one turn reader
publishes to the existing bounded job stream. A slow subscriber can lose old live
chunks and receives reset plus checkpoint; it does not expand server memory.
The adapter bounds raw CLI lines at 1 MiB and normalizes them before forwarding.
At most 128 tool steps may be pending; duplicate starts fail closed.
Provider crashes without a complete outcome become outcome-unknown. No login is
read from disk, copied to the server or included in protocol output. The
Claude adapter emits a Check result from its existing official status call
before a regular turn. The server keeps that result private to the requesting
account and excludes it from the shared turn stream.

A Codex adapter reuses this entire protocol, machine stdin/exec transport, job
streams/checkpoints, access tokens, instructions, risk policy, retry scheduler and
CLI. Its own plugin would supply an executable and model metadata; only official
Codex process control and event normalization differ.
