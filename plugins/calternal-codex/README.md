# Codex provider

Calternal runs the official Codex CLI using `codex app-server --listen stdio://`
inside a managed harness sandbox. The plugin maps saved native sessions, model and
effort choices, steering, interruption, and bounded events to the same provider
protocol used by Claude Code.

The host mounts Least access workspaces read-only on Podman and on MicroVM's
inner Podman. The adapter also uses a named Codex read-only permission profile.
Approve for me permits workspace writes and asks for native escalations. The
adapter declines decisions that would escape the protected tool roots. Full
access uses the same writable roots with Codex approvals set to `never`.
Calternal's operation capabilities and machine containment apply in every mode.

Sign in with the private device-code flow, or use the official
`codex login --device-auth` in the assigned private sandbox. The CLI owns its
credential file and refresh lock. Separate provider homes hold native sessions.
The adapter never prints credentials, account payloads, or raw diagnostics.
Automatic titles remain disabled per Forgejo #46.

Use `npm run build:plugins` then
`deploy/build-provider.sh localhost/calternal-ai-codex:current calternal-codex`
on the development VM. Review and install this manifest through the same package
path as other providers. Stage the exact image through the Admin harness
operation before enabling it in a space. See [provider protocol](../../docs/provider-protocol.md)
and the [official App Server documentation](https://learn.chatgpt.com/docs/app-server).
