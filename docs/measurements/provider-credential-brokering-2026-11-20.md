# Provider credential brokering · 2026-11-20

The [sealed generic-engines proposal](../proposals/plugin-api-v2-generic-engines.md)
requires the upstream credential to stay in a supervisor-owned transport.
The adapter, official CLI and agent tools may receive only a local handle
under that design. A CLI with an empty login directory can still receive a
real bearer through its environment, a helper or an RPC. That arrangement
does not satisfy the sealed isolation requirement.

| Shipped provider and login | Result | Cutover implication |
| --- | --- | --- |
| Claude Code with Claude subscription OAuth | No supported third-party subscription-token broker found. Anthropic explicitly bars developers from collecting or intermediating Claude.ai credentials. The ordinary gateway transport works in a synthetic test with the login file unreadable. | Keep native login in the current engine. A Console/API or supported cloud gateway has a different credential and billing model. |
| Codex with native ChatGPT login | The native login file and refresh are owned by Codex. The documented external-auth RPC and plan-usage App Server example give Codex a real access token. Neither proves an opaque native-login broker. | Keep native login in the current engine. The separately consented Sign in with ChatGPT plan-usage program offers a public inference route to investigate under its own registration and terms. |

## Claude Code

- **Start and run.** On Linux the official CLI reads
  `~/.claude/.credentials.json` or the file under `CLAUDE_CONFIG_DIR`.
  macOS uses Keychain with a protected file fallback. The shipped
  integration uses the official private sign-in flow and a machine-local
  profile. The CLI refreshes short-lived subscription access during
  ordinary use and requires sign-in again when the grant cannot refresh.
  A keychain protects storage and still returns credentials to the
  program. [Authentication](https://code.claude.com/docs/en/authentication)
  · [Long-lived CI host](https://code.claude.com/docs/en/self-hosted-environments-testing)
  · [Current adapter](../../plugins/calternal-claude-code/README.md)
- **Brokerable surface.** `ANTHROPIC_BASE_URL` and
  `ANTHROPIC_AUTH_TOKEN` can present a revocable gateway handle. The
  shipped adapter already supports that shape. `apiKeyHelper` returns
  a credential to the CLI and is useful only if the output is a gateway
  handle. `CLAUDE_CODE_OAUTH_TOKEN` gives the CLI the actual
  subscription credential. In supported gateway mode, billing follows the
  upstream credential, with no automatic reuse of the saved subscription.
  [Gateway configuration](https://code.claude.com/docs/en/llm-gateway-connect)
  · [Subscriptions and gateways](https://code.claude.com/docs/en/llm-gateway#subscriptions-and-gateways)
  · [Adapter environment](../../plugins/calternal-claude-code/src/adapter.ts)
- **What blocks live qualification.** Anthropic distinguishes a user
  signing in to the unmodified CLI from a third party collecting or
  intermediating their subscription credentials. The requested bearer
  injector would take custody of those credentials, so the dev probe
  did not open or use them. No documented delegated refresh or
  handle-only native subscription mode was found. No claim about
  device binding follows from the local gateway result.
  [Anthropic legal and compliance](https://code.claude.com/docs/en/legal-and-compliance#authentication-and-credential-use)

## Codex

- **Start and run.** Native browser/device login returns credentials
  to Codex. It reads `auth.json` under `CODEX_HOME` or the OS credential
  store, depending on `cli_auth_credentials_store`; ephemeral mode keeps
  auth in process. Codex refreshes managed ChatGPT sessions automatically.
  The current Calternal profile explicitly selects file storage and
  links the machine-local auth and lock files. Its command-tool policy
  denies those paths and the Codex home. This protects the tool path
  while leaving the provider runtime credential-bearing.
  [Codex auth](https://developers.openai.com/codex/auth)
  · [Current profile](../../plugins/calternal-codex/src/profile.ts)
- **Brokerable surface.** The custom Responses provider can use a local
  handle via its `env_key`. App Server's experimental
  `chatgptAuthTokens` accepts an access token and account ID and can ask
  the host to replace that token after an auth error. It keeps refresh
  credentials with the host but gives Codex the access bearer. The
  published plan-usage App Server recipe similarly gives the child the
  access token in its environment and restarts/resumes on token renewal.
  Neither is an opaque handle for the native login.
  [App Server auth protocol](https://developers.openai.com/codex/app-server)
  · [Plan-usage App Server recipe](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)
- **What blocks live qualification.** OpenAI documents registration,
  explicit plan-usage consent and the public Responses endpoint for this
  third-party program. It directs paid or remotely hosted apps to an
  interest form and does not authorize substituting native
  `auth.json` tokens for that grant. Account/workspace context and
  entitlement checks still apply. I did not replay a native bearer
  against ChatGPT's private endpoints, test refresh against a live
  account or infer a device-binding requirement from device-code login.
  A broker with its own supported consent and refresh lifecycle is a
  candidate for separate qualification, not evidence about the existing
  login. [Plan usage program](https://developers.openai.com/siwc/token-sharing-open-source)
  · [Supported inference endpoint](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)
  · [OpenAI Terms of Use](https://openai.com/policies/terms-of-use/)

## Disposable dev-01 probe

Ran under `nix develop -c` on `claude@calternal-dev-01-compute02`.
Reused the official Claude Code 2.1.326 binary from the local dev image
and verified the official Codex Linux x64 0.170.0 archive against the
repository's SHA-512 pin. Neither experiment used a real account. No
real sign-in file was opened or copied. The fixture used a read-only
Podman container with no external network, a fresh temporary home, and
two loopback endpoints. A supervisor-owned process read a generated
canary from a mode-0600 file in a private directory. A separate
unprivileged UID ran the driver and official CLI. The driver and a
child tool process each attempted to open the canary. The tool also
checked that its fresh home had no provider login record. No
provider-generated tool call was exercised.

The driver supplied only a synthetic gateway handle. The gateway
accepted only the fixture's inference paths, replaced the handle with
the canary and forwarded to an upstream simulator that verified it.
The simulator returned the provider's native streaming response shape
with a known text and terminal outcome. The native/no-gateway controls
used the same denied file and fresh home. The Codex missing-handle
control kept the local provider configured and omitted only its handle.
That distinguishes the auth input check from the offline native
control, which retries the unreachable network. This tests credential
placement and real CLI stream handling, with synthetic protocol
responses and no provider-side authentication, entitlement, refresh,
device binding or egress qualification.

| Official binary | Run count | Run | Driver/tool file reads | Terminal result | Gateway / upstream requests per run |
| --- | ---: | --- | --- | --- | --- |
| Claude Code 2.1.326 | 1 | Native, empty home | Both denied | Explicit sign-in failure | 0 / 0 |
| Claude Code 2.1.326 | 1 | Synthetic gateway | Both denied | Expected text and successful completed result | 1 / 1 |
| Codex 0.170.0 | 1 | Native, empty home | Both denied | No completion on network-disabled control | 0 / 0 |
| Codex 0.170.0 | 1 | Synthetic gateway, handle omitted | Both denied | Explicit missing-credential failure | 0 / 0 |
| Codex 0.170.0 | 2 | Synthetic Responses gateway | Both denied | Expected assistant text and `turn.completed` | 1 / 1 |

Claude was driven through `claude -p --output-format json` with a tool-free
turn. Codex was driven through `codex exec --json` with a custom
`wire_api = "responses"` provider and a read-only tool profile. These were
protocol drivers under the unprivileged identity. The shipped Calternal
adapters and provider-generated tool calls remain outside this proof.

## Owner decision

If a subscription provider cannot meet the transport-only rule through
a supported route, may its **official** program read its own login
inside one person's sandbox on that person's own enrolled machine?
For example, a person could continue using their Claude subscription
through the official CLI. This preserves sign-in and automatic refresh
and keeps the Calternal adapter out of credential custody if the two
processes are separately isolated. It leaves the official program
credential-bearing, and a sandbox or tool-isolation failure has higher
consequences. The transport-only rule would keep this route disabled and
require a supported broker, separate API billing or a provider agreement.
Keep the existing engine behavior until both providers qualify or the
owner approves a personal-machine exception. The probe does not support
an exception for hosted or shared credentials.

## Simplification opportunities

Reuse the existing gateway handle for supported upstream credentials.
Keep provider sign-in and refresh on a separate authority path from
tool execution, and treat a missing supported login mode as an
admission condition rather than a retry.
