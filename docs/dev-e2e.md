# Dev-01 Claude Code bring-up

Run this lane as `claude@calternal-dev-01-compute02`, in
`~/Developer/lanes/e2e-dev`. The installation serves disposable data under
`~/calternal-e2e`. The existing app and production have their own deployments.

## Start, inspect, stop

From the lane checkout, run:

```sh
./scripts/dev-e2e/service.sh build
./scripts/dev-e2e/image.sh fixture
./scripts/dev-e2e/image.sh claude
./scripts/dev-e2e/service.sh start fixture
./scripts/dev-e2e/service.sh status
./scripts/dev-e2e/service.sh logs
./scripts/dev-e2e/prove.py fixture
```

The build uses the pinned Nix shell, runs the plugin build, and installs the
server binary in `~/calternal-e2e/bin`. The official image pins Anthropic's
unmodified Claude Code 2.1.326 package and uses the shipped adapter. The fixture
image replaces only the external Claude Code program with the shipped
account-free test program. Both images use a build context with only adapter
files and an anonymous Podman registry configuration.
[Anthropic installation documentation](https://code.claude.com/docs/en/setup#install-with-npm)

The three protected roots are `appdata`, `userdata-ssd` and `userdata-hdd`
under `~/calternal-e2e`. Their installation ID is
`91c2ed9a-5a98-4b01-ac58-7376ab499c34`. Start seeds the disposable owner and
guest through the existing `--development` option. It enrolls a machine
for the owner once, passes its one-use code over stdin, and waits for the
machine to report online. Passkey setup uses an empty installation as
described below.

The server listens on `127.0.0.1:34135`. The relay binds `34134` to dev-01's
NetBird IPv4 address on `wt0`, observed as `10.69.69.237`. The three
`calternal-e2e-*.service` user units write to the systemd journal, with
retention set by the host. Repeat `start fixture` to reuse the same account,
machine and roots. On host reboot rerun the start command.

Use `./scripts/dev-e2e/service.sh stop` before a rebuild or image switch.
A reset with `./scripts/dev-e2e/service.sh reset --yes` stops all three
lane units, removes the three roots, the lane machine enrollment, and the
enrolled account's thread containers and workspaces. The operator keeps
control of the former account's provider home. It stays under its old
account UUID and the fresh installation has a different account UUID.

## Provider sign-in handoff

Start prints the test account UUID and its host sandbox home:
`~/calternal-e2e/machine/sandboxes/accounts/<account-uuid>/home`. Podman
mounts that home as `/home/agent`. For the coordinator's authorized Linux
Claude Code login, the expected files relative to that home are
`.claude/.credentials.json` and `.claude.json`. Both should be owned by
`claude` with mode `0600`. Use mode `0700` for `home` and `home/.claude`.
The coordinator handles the sign-in files directly. The image and scripts
work without provider credentials and the server only authenticates
Calternal accounts and machines.
[Claude Code credential storage](https://code.claude.com/docs/en/iam#credential-management)
· [Claude Code settings files](https://code.claude.com/docs/en/settings#where-settings-files-live)

After the coordinator confirms placement, run:

```sh
./scripts/dev-e2e/service.sh stop
./scripts/dev-e2e/service.sh start claude
./scripts/dev-e2e/prove.py claude
```

The real proof creates a fresh thread so its Podman sandbox uses the Claude
Code image, submits one prompt, and reports only time to first text, elapsed
time and outcome. Existing thread containers retain their original image.
A saved thread lives under `~/calternal-e2e/userdata-ssd/spaces/<space-uuid>/AI/Chats/`.

## Web preview

The coordinator administers Caddy on dev-02. Its upstream is
`http://10.69.69.237:34134` over NetBird. Add these handlers inside the
existing `preview.calternal.cloud` site, ahead of the static SPA fallback.
Keep the preview's existing access controls. The UI may stay mounted at
`/ai/`; API routes must retain their absolute paths.

```caddyfile
@calternalDevBearer {
    path /api/v1/*
    header Authorization "Bearer *"
}
handle @calternalDevBearer {
    reverse_proxy http://10.69.69.237:34134 {
        header_up Host 127.0.0.1:34135
        header_up Origin http://127.0.0.1:34135
        flush_interval -1
    }
}
handle /api/v1/* {
    reverse_proxy http://10.69.69.237:34134 {
        flush_interval -1
    }
}
```

The first handler reproduces Vite's development-bearer Host and Origin
rewrite. The second preserves the browser Host and Origin for passkeys
and cookie sessions. SSE uses the same proxy routes.

When the Threads web client supports server mode, open
`https://preview.calternal.cloud/ai/?server=1` in the preview browser.
For the disposable account, the authorized operator can set `cal.devtoken`
in that origin's local storage with the contents of
`~/calternal-e2e/appdata/dev-tokens/owner.token` on dev-01. Keep the token
out of the URL, terminal arguments and logs. At the current base commit the
Threads store always uses its fixture client.

For a browser passkey, the configured relying-party origin is
`https://preview.calternal.cloud` and the RP ID is
`preview.calternal.cloud`. The browser needs a valid HTTPS certificate at
that host. An empty installation started without `--development` writes a
private `<appdata>/setup.token`. The owner-facing setup screen will need
to submit it through `/api/v1/auth/setup/start`, create a passkey with
`navigator.credentials.create`, and submit the result through
`/api/v1/auth/setup/finish`. The server issues a same-host Secure,
HttpOnly session cookie. The current web build lacks this ceremony UI.
The seeded fixture installation has already created its disposable users,
so it has no first-owner setup token.

## Bring-up results

| Observation | Resolution or open work |
| --- | --- |
| `calternal serve` only accepts a loopback listener | The lane-scoped socat relay binds specifically to `wt0`. A NetBird peer reached the auth status endpoint using the preview Host. |
| The shipped provider Containerfile expects a Claude installation in the home | The lane builds a bounded derivative image with the official npm package and verifies its version without mounting a home. |
| Older CLI documentation lists a single directory for machine administration | The enrollment script supplies all three root flags and the pinned installation ID, as required by the current CLI. |
| Fixture output, Stop and file placement | The first send finished and streamed. The second streamed while running and `threads.stop` drove it to `cancelled`. The Markdown included both user messages and provider output. |
| Second send omitted `--resume` | Fresh `threads.send` jobs do not carry over the previous Claude session. The fixture check verifies this against actual process arguments. This matches the open turn-engine item in [Threads](threads.md#bounds-and-remaining-integration). |
| Web Threads and owner passkey setup are unwired | The web client and WebAuthn ceremony remain UI integration work. The backend fixture proof uses the disposable account. |

See [Forgejo #34](https://git.kayg.org/kayg/calternal-ai/issues/34) for the
active qualification status.
