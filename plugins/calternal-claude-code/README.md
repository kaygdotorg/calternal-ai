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
built from the repository root. Linux machines supply their own read-only Calternal
CLI; Nix builds also expose the immutable Nix store for runtime dependencies.
On other host platforms, the image must include a matching Linux Calternal CLI;
a native host executable is not mounted into a Linux container.
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

The fake provider can also run through the actual rootless container layout, with
the mounted Calternal executable checked inside it. On the dev VM, build plugins,
run `test/build-image.sh localhost/calternal-provider-fixture` from this package,
then set `CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-provider-fixture` for
`cargo test -j6 -p calternal-server --test provider_container`. The fixture builder
uses a bounded context and an empty registry auth file. It installs only the fake
CLI and never signs in. The test removes only its fresh account's containers.

Before Rust provider-turn tests, run `npm run build:plugins`. Those tests fail at
entry with that command if `dist/main.js` is absent. The sandbox launcher emits a
permanent `adapter_not_built` error with the same command when compiled output is
missing; an absent launcher reports `adapter_missing`. Both end as failed setup,
with the message retained in the checkpoint, rather than outcome-unknown.
