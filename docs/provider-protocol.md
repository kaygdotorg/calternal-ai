# Provider adapter protocol v1

A provider plugin declares an optional `provider` entry in manifest v1:
`protocol_version`, an executable basename resolved inside the sandbox, and a
`default_model` and `models` containing IDs, display names and reasoning
levels. The host validates and caches it at startup without running code. The machine launches
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
check, session, text_delta, reasoning_started, reasoning_delta,
reasoning_finished, tool_started, tool_output, tool_finished, question,
approval_request, usage, error, done. Lines are at most 12 KiB including newline.
Text and tool detail are at most 4 KiB; IDs and tool names 128 bytes; summaries and
safe error messages 512 bytes. JSON integers must be exactly representable in JS.
Unknown versions, malformed frames, sequence gaps and partial final lines fail
closed. Error classes distinguish transient, usage_limit, authentication,
refusal, permanent and unknown. Billing is api, subscription or unknown. A reset,
when reported, is Unix milliseconds. It is not guessed from error prose.

A reasoning block is ordered and non-overlapping. A start precedes deltas,
and a finish carries the elapsed millisecond count observed by the adapter.
No thinking event is fabricated when the program supplies no block. Claude
Code maps print-mode partial streams from `content_block_start` with a
`thinking` block, `thinking_delta` and the corresponding block stop. It omits
signatures and encrypted block payloads. The host applies the same issued-token scrub
as text output, with an independent streaming redactor for reasoning deltas.
Claude Code receives the chosen reasoning level as its documented
`--effort` option. The profile's own setting applies when the option is absent.

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

## App sign-in

A provider that declares `sign_in: true` implements a private `--sign-in`
mode on its manifest executable. The host starts this mode in a fenced scratch
sandbox. The program may write one bounded JSON challenge line with `url`,
`display_code` and `needs_code`. An authorization code is supplied to
the original process on stdin. Exit 0 means the official CLI's own status
check succeeded. The program keeps all other output on the machine.

`provider.sign_in.start` selects a provider, space, and optional thread and
machine. `requests.list` carries the private `provider_sign_in` challenge.
`provider.sign_in.answer` supplies a code, `provider.sign_in.status` reads the
result, and `provider.sign_in.typing` offers sign-in again when a failed turn's
owner resumes composing. A pending request expires after 10 minutes. The
private machine process has its own timeout. The host validates the HTTPS
origin and path against the installed provider's operator-approved registration.
A device challenge sets `needs_code: false` and supplies the display code
for the user to enter on the provider's page. The server keeps challenge data
in memory and exposes only the current person's requests.

Normal execution uses a home per space and account. The Claude Code adapter
links only the shared machine-local login file into the space's configuration
directory. Podman mounts the shared directory read-only during a turn. Direct
uses distinct homes under the same Unix account and retains its documented
host filesystem access. Existing Linux homes import the Claude OAuth record
alone. Other state stays in its original directory.

## Installed harness runtimes

Install the provider manifest through the ordinary package review, installation
and space activation commands. The runtime image is a separate immutable,
operator-installed artifact. A newly installed provider needs an active tested
runtime. The package store's migration preserves the bundled provider's existing
image selection and private login layout.

Install both release candidates with the same lifecycle commands on their manifest
executable. `--harness-version` reports the installed official CLI version,
`--harness-update` describes the exact package release and public sign-in metadata,
and `--harness-smoke-test VERSION` checks the candidate in a scratch sandbox.
The host requires the expected version and a bounded successful result before
offering an update. The installation administrator calls `harnesses.list`,
`harnesses.stage`, `harnesses.activate`, and `harnesses.rollback` to query and
change the runtime pin. Routes work through the standard operation endpoint.

Managed leases carry a provider ID and an explicit login scope. The sign-in
recipient is the person who initiated the request. Account scope is supported by
the current placement code. A future space login needs owner/manager settings,
an authorized space-scoped placement and profile volume, per-member override,
revocation and erasure, scoped readiness and session-binding checks. The current
machine rejects unsupported space-scoped assignments before mounting any profile.

Interactive runtimes accept `agent.steer`, `agent.approval`, and a bounded graceful
`agent.stop`. Approvals expire and apply only to the pending request in its current
run. The adapter may refuse a native approval that would bypass the sandbox.
