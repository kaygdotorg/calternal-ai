# Tools on shared turns

A shared turn keeps the member's Calternal authority and the Owner's execution
sponsorship. Keep provider credentials on the execution machine. The current
shared Claude path stays tool-free until the machine can run tools across a
verified isolation boundary.

| Option | Credential boundary | Trade-off |
| --- | --- | --- |
| Machine-local inference broker | The harness uses a disposable, lease-scoped gateway credential. A broker outside both sandboxes adds the provider credential on a fixed upstream route. | Keeps the provider credential out of the harness and the tools. Requires provider-approved gateway authentication and a separate tool executor. |
| Native sign-in in a separate harness identity | The official harness owns the login and uses it directly. It delegates tools to an executor with its own filesystem, UID and PID namespace. | Preserves supported native sign-in where available. Requires a harness interface that can delegate every enabled tool and prevent local hooks or tools from running. |
| Credential helper inside the harness sandbox | The helper returns a credential to the harness process. | Useful for rotating a gateway credential. Returning a provider credential here gives the harness possession of it and provides no isolation from tools sharing its identity. |

Claude Code documents `ANTHROPIC_BASE_URL`, gateway credentials in
`ANTHROPIC_AUTH_TOKEN` or `ANTHROPIC_API_KEY`, and an `apiKeyHelper`
that prints the gateway credential to stdout. Its Messages gateway supports
`/v1/messages` and optional token counting. Anthropic's usage policy restricts
third-party intermediation of Claude.ai subscription credentials. Use an
approved API or cloud credential for a live broker. [Gateway setup](https://code.claude.com/docs/en/llm-gateway-connect) · [Messages compatibility](https://code.claude.com/docs/en/llm-gateway-protocol) · [Usage policy](https://code.claude.com/docs/en/legal-and-compliance#usage-policy)

Codex documents a custom provider's `base_url` and either an `env_key`
or an `auth.command` whose stdout provides the bearer token. A Responses
gateway can use either for its own lease-scoped credential. OpenAI documents
a separate user-authorized ChatGPT-plan path for app-server. Qualify that
flow independently before any shared-account use. [Codex configuration](https://developers.openai.com/codex/config-reference) · [ChatGPT-plan app-server](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)

**Recommendation.** Start with the broker and an approved API credential.
Reuse fenced machine leases, the bounded provider stream and the existing
account-scoped agent authority. Permit only fixed provider hosts, methods and
paths. Replace caller auth, refuse redirects and revoke the harness's route
at lease end. Mount the gateway socket only into the trusted harness. Disable
its built-in tools, workspace hooks and arbitrary MCP servers. Route every
approved tool through a distinct sandbox identity with its own mounts and PID
namespace, no broker endpoint, a clean environment and closed inherited file
descriptors. Pass the member's scoped Calternal authority only to tools that
need it.

**Acceptance tests.** Drive each stock harness through a mock provider and
verify a tool call completes and its workspace change persists. Use random
canaries and a failing positive control to test file reads, process state,
environment, inherited descriptors and broker reachability from the tool.
Confirm a mock upstream receives the provider header and neither harness nor
tool output contains it, even on provider errors. Reject foreign and expired lease credentials, request replays, dynamic
destinations and redirects. Close both
sandboxes on revocation or restart. Then repeat the functional turn with a live,
disposable, approved provider credential.

**Dev-01 prototype.** Stock Claude Code completed one MCP tool call through a
separate Podman executor. The tool could not open the reduced login file, see
the broker process, find a leaked login descriptor or environment variable,
or reach the broker. A synthetic file and environment positive control was
detected. The broker opened the reduced login file without reading its bytes
and injected a generated canary into both authenticated mock upstream calls.
This tests the boundary and the gateway protocol against a mock upstream.

**Implementation.** The current Owner can enable the space's brokered route
with `space.machine.set` and `tools_enabled: true`. The machine uses the fixed
Claude Messages or Codex Responses route and allows one workspace MCP tool.
The Podman executor gets separate PID, mount, and network namespaces and
the workspace mount under a non-root container UID. Only Full access mounts
the workspace writable. The Codex adapter pre-approves this one MCP tool so
it can run with interactive approval disabled. [Codex tool approval configuration](https://developers.openai.com/plugins/build/plugins#configure-installed-server-policy)
