# Claude Code provider

Machine-side TypeScript adapter for the installed, unmodified `claude` program.
It uses the documented print-mode stream (`-p --output-format stream-json
--verbose --include-partial-messages`), `--resume`, `--append-system-prompt` and
`--disallowedTools`. Unlike an Agent SDK integration, it launches the CLI directly
and handles no OAuth token, API key or provider login file. The person uses
`claude auth login` inside their account sandbox home; status is checked through
`claude auth status` (only exit status and the billing-method enum are used).

Build from the repository root: `npm ci && npm run build:plugins`. Install `dist/`
and `bin/` together in the sandbox image; put `bin/` on PATH. The Containerfile is
built from the repository root. The machine supplies its own read-only Calternal
CLI; Nix builds also expose the immutable Nix store for runtime dependencies.
Install the official Claude Code binary in the persistent account home, normally
`~/.local/bin/claude`. It is never installed, patched or signed in by the adapter.

The adapter accepts provider-v1 JSON lines on stdin and emits JSON lines on stdout.
EOF waits for the current turn. Stop sends SIGINT, then SIGKILL after two seconds.
Steer interrupts the current turn and launches another print invocation of the
same official session. Native workspace/web tools are pre-allowed inside the chosen machine execution boundary.
MCP auto-discovery is disabled so Calternal has one tool path. System-prompt
snapshots are disabled so resumed sessions receive the current instruction layers.
Native AskUserQuestion, Agent and Task tools are disabled;
the core instructions prefer Calternal operations for questions and helpers.
Permission denials become approval events; the adapter never approves them.
Raw tool arguments/results and stderr are discarded. Text deltas still contain
model output and must be treated as untrusted data.

Fake tests: `npm test --workspace @calternal/claude-code`. The real-account test
runs only with `CALTERNAL_TEST_REAL_CLAUDE=1`; the developer signs in themselves.
See [provider protocol](../../docs/provider-protocol.md) and
[manual turns](../../docs/agent-turns.md).

Official interface references:
- [Programmatic use](https://code.claude.com/docs/en/headless)
- [CLI reference](https://code.claude.com/docs/en/cli-reference)
- [Stream message and error types](https://code.claude.com/docs/en/agent-sdk/typescript)
