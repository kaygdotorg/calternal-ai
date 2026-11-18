# Machine onboarding and personal enrollment

Lane [#48](https://git.kayg.org/kayg/calternal-ai/issues/48). The co-located
handoff commits a pool enrollment and the first administrator's existing
account-level pool grant in one protected authority transaction. The machine
generates the signing key on the host and publishes only its UUID and public
key. Onboarding reports placement availability. `machines.verify` runs the
sandbox and provider diagnostic.

The personal installer uses the existing enrollment path and a 15-minute code.
It copies a platform-matched binary supplied by the person and starts a user
service with the configured image. It consumes the code through stdin and
preserves an existing identity enrolled to the same server.

## Review gate

| Item | Result |
| --- | --- |
| 1 | Uses the shared account, machine ownership, error, and authorization types. |
| 2 | Reuses the existing crate dependencies. The machine owns the local signing key and depends on no server code. |
| 3 | The installation-scoped enrollment commit holds the same machine registry guard used for network enrollment, so a confirmed UUID immediately works for signed sessions. No per-space lock or connection was added. |
| 4 | Handoff I/O, auth authority, and the machine directory lock run on blocking workers. |
| 5 | Onboarding reads one capability and takes one registry snapshot with zero flushes. Creating an installer takes one protected code commit and no machine round trip. The handoff has one protected enrollment commit and uses atomic file publication. |
| 6 | Reuses the cached operation catalog and embeds the canonical script at compile time. |
| 7 | Codes expire after 15 minutes and follow the existing expired-code pruning. The handoff uses one request and one acknowledgement and removes both when the machine saves its durable credentials. |
| 8 | No schema version change, aliases, or migration readers. Existing machine credentials are the standard enrollment format. |
| 9 | Operations use the authenticated account and enrollment UUIDs. The instance-wide operations need no space argument. |
| 10 | Reuses the existing pool grant and enrollment-code authority. Group compute authorization remains open. |
| 11 | Deployment units start the handoff. Both operations are routed and exposed through CLI commands and discovery. |
| 12 | Reuses the protected auth commit, private atomic file helpers, machine owner lock, readiness predicates, and standalone installer. |
| 13 | Public types and entry points have reference comments. |
| 14 | The CLI, machine, and deployment docs describe the same contract. |
| 15 | New server implementation lives in separate modules. The existing machine service and conformance harness exceed 600 lines. |

## Pending decisions

- Group compute authorization. Configure an initial group at deployment to
  give the administrator explicit membership and limits, or define a default
  owner group in the product. The second option requires a stable group policy
  and rules for multiple memberships. The current increment grants the first
  administrator access through the existing per-account pool grant.
- Installer distribution. Publish signed artifacts for each supported Linux and
  macOS architecture, or bundle the binary in the installers or macOS app.
  The former needs a release index and verification keys. The latter requires
  maintaining packages per platform. The script uses a supplied binary today.
- Native macOS isolation. A launchd template is available. Shipping a macOS
  process sandbox needs a backend and device verification. The present script
  uses Podman and needs the engine running.
- SSH enrollment. The same single-use code and script can be used after a
  person authorizes an SSH target and its host key. The agent would need a
  permission to connect, a trusted way to deliver the matching binary and
  script, a protected stdin channel, and a readiness check. Agent-driven
  SSH enrollment is outside this implementation.
- Hosted provider login. A link-only flow lets the official CLI complete the
  provider's own callback or device flow. A paste-back flow additionally sends
  an authorization code through the app and needs provider confirmation about
  this relay. Anthropic's published rules allow use of the unmodified binary
  under conditions and restrict credential intermediation. [Claude Code legal
  and compliance](https://code.claude.com/docs/en/legal-and-compliance)
- Sandbox home scoping. The existing Podman and Direct backends mount
  `accounts/<account>/home` for every space. The accepted model gives each
  account a separate home in each space. Enabling sign-in with the current
  mount would share a login across spaces. For existing machines, starting
  with empty per-space homes requires re-authentication. An operator-assisted
  transfer could preserve a login for one explicitly selected space, with a
  confirmation step and extra recovery work.
- Provider sign-in, automatic retry after expiry, and the single thread notice
  remain unfinished. The live login exchange needs a dedicated private exec
  path, an account-private inbox entry, bounded teardown, and a readiness check
  in the same sandbox home. The ordinary turn stream and checkpoints are shared
  with authorized space members and cannot carry login challenges.

## Simplification opportunities

- Keep the standalone and app-generated installer coupled to one shell source.
- Use the same readiness projections and diagnostic command for onboarding and
  machine management once group compute authorization can supply the caller's
  effective entitlement.
