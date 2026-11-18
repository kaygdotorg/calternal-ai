# Provider sign-in from the app

Lane [#48](https://git.kayg.org/kayg/calternal-ai/issues/48). The app calls
`provider.sign_in.start` with a space, provider and optional thread and machine
selection. It renders the private `provider_sign_in` item from `requests.list`.
The answer operation forwards a code over the existing fenced interactive
machine pipe. Status contains only the request ID and outcome. Thread tokens
cannot use these operations or fetch the private exec output.

The official Claude Code adapter runs `claude auth login` in a disposable home.
It relays one provider-owned URL and supplies the browser's answer to the
same process. A clean exit requires `claude auth status` and machine-local
publication. The credential never goes over the machine link. Existing Linux
homes import the Claude OAuth record alone. The rest of the configuration
and transcripts remain in the old home. New homes scope settings and
transcripts by space and account. Personal Direct mode uses the distinct
default homes and retains its documented access to the host filesystem.
Filesystem isolation was tested on Podman.

A continuous signed-out readiness observation starts one automatic offer.
A failed turn starts sign-in on the machine that reported it. The owner's
next compose gesture can request another prompt. Signing in readmits the
latest failed turn on the same machine without another message. A thread's
attempted notice is written before the official command starts. Successful
completion updates the same line. A crash retains the attempt and drops the
volatile challenge.

The supported provider in this change is Claude Code. Context7 confirmed the
official terminal paste path, Linux credential file and macOS Keychain behavior.
The shipped development proof uses the scripted provider. A real provider
session and macOS Keychain behavior remain unverified by this Linux gate.
[Claude Code authentication](https://code.claude.com/docs/en/authentication) ·
[Claude Code remote login troubleshooting](https://code.claude.com/docs/en/troubleshoot-install)

## Review gate

| Item | Review |
| --- | --- |
| 1 | Reuses account, space, machine, error, job and request types. New wire types describe only sign-in inputs and result. |
| 2 | Uses existing crate dependencies and machine transport. |
| 3 | Shards the account inbox and releases its guards before filesystem, network or database work. Placement uses the existing machine registry. |
| 4 | SQLite, credential file access and revision writes run on blocking workers. The TypeScript adapter uses asynchronous child and file APIs. |
| 5 | Start resolves the account and machine and authorizes the space before scheduling. It performs one ordinary scratch lease admission. Pending relay rechecks session and membership around its bounded poll. A notice costs one revision-fenced content write at attempt and one at completion. |
| 6 | The operation and provider schemas use the existing cached catalog. |
| 7 | A challenge lives at most ten minutes. The provider process has a separate nine-minute timeout. Signed-out observations can be offered on explicit gestures for 24 hours. Volatile entries are bounded per account and shard. |
| 8 | Existing Linux credentials are adopted once into the new location. The adapter uses the same shared credential path for subsequent runs. This adoption preserves signed-in accounts during the owner-approved home split. |
| 9 | Request and scope lookups use UUIDs. |
| 10 | The provider manifest declares adapter support. The adapter selects the official command. The server independently validates the browser URL and current account authority at its trust boundary. |
| 11 | Sign-in operations, prompt listing, placement, teardown, readiness offers and saved-thread resumption all have runtime callers. |
| 12 | Reuses the existing machine lease, interactive input, content revision fence, per-account invalidation and provider status check. |
| 13 | Public entry points and wire fields carry reference comments. |
| 14 | Protocol documentation and generated contracts describe the same wire. |
| 15 | New host behavior is split between the provider inbox, flow and thread notice modules. |

## Simplification opportunities

- Keep application and provider-specific credential handling in the same
  machine-side adapter boundary. The host can use the existing private inbox
  and fenced exec transport for future providers.
- Route additional compose notifications through the same signed-out readiness
  observation as the existing typing operation.
