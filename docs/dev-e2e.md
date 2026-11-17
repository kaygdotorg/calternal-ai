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
files and an anonymous Podman registry configuration. The server uses the
explicit direct worker adapter for disposable TypeScript plugin calls.
Claude Code turns run in the machine role’s Podman sandbox.
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
systemctl --user stop calternal-e2e-machine.service
./scripts/dev-e2e/service.sh start claude
./scripts/dev-e2e/prove.py claude
```

The real proof creates a fresh thread so its Podman sandbox uses the Claude
Code image. It submits one prompt with retries disabled for that turn and
reports time to first text, elapsed time and outcome. Existing thread containers retain their original image.
A saved thread lives under `~/calternal-e2e/userdata-ssd/spaces/<space-uuid>/AI/Chats/`.
To check continuation, use `./scripts/dev-e2e/prove.py claude-continuation`
in place of `prove.py claude`. It verifies a two-message exchange and reports
only timing and outcome.

## Browser installation and Caddy

Use a separate empty installation for the real browser passkey. On dev-01,
alongside the seeded service, run:

```sh
./scripts/dev-e2e/service.sh build browser
./scripts/dev-e2e/service.sh start-browser
```

This installs a separate binary and pairs three roots under
`~/calternal-e2e/browser` with installation ID
`825bd622-44d8-4171-b5f7-c90a8702b34e`. The browser server listens on
`127.0.0.1:34136`. Its NetBird relay binds to `wt0` on port `34137`.
`service.sh stop-browser` stops just those two units. `service.sh logs`
includes both installations. The fixture reset only resets the seeded
installation.

Inside the coordinator’s existing `preview.calternal.cloud` Caddy site,
route the absolute API paths ahead of the static SPA fallback:

```caddyfile
handle /api/v1/* {
    reverse_proxy http://10.69.69.237:34137 {
        header_up Host preview.calternal.cloud
        flush_interval -1
    }
}
```

Keep the existing preview access controls and a valid TLS certificate for
`preview.calternal.cloud`. Keep `/api/v1/` in every upstream path. Caddy passes
through the incoming `Origin` and `Cookie` and the outgoing `Set-Cookie`.
Do not add a Vite-style Origin rewrite to this handler. The web UI may stay
under `/ai/`. The browser sends `Origin: https://preview.calternal.cloud` on
its JSON POSTs. Use `/ai/?server=1` when enabling the server-backed Threads
adapter, with a browser profile free of the development bearer. The server’s `--origin https://preview.calternal.cloud`
sets that exact accepted origin and derives RP ID `preview.calternal.cloud`.
Setup and sign-in set the host-only `__Host-calternal_session` cookie with
`Secure; HttpOnly; SameSite=Lax; Path=/` and a server-configured lifetime.
The browser stores the cookie and sends it on same-origin API and SSE requests.

The separate seeded service remains on NetBird port `34134`. Its developer
bearer flow works through Vite on dev-01 when `CAL_SERVER` points at
`http://127.0.0.1:34135`. Use a dedicated browser profile for that flow.
If testing incremental text in Threads, enable **Stream messages as written**
in Settings.

### Morning steps

1. Open the private link for the first-owner screen. Its form is
   `https://preview.calternal.cloud/ai/?server=1#setup=SETUP_CODE`, with the
   code from the file below in the fragment. You can also open
   [the preview](https://preview.calternal.cloud/ai/?server=1) and paste the
   code into the form.
2. Create your passkey and stay signed in. The server creates your Personal
   space during setup. In the browser’s developer tools,
   open storage for `https://preview.calternal.cloud` and copy the value of
   `__Host-calternal_session` for the next step.
3. Run the post-setup script and paste the session value at its hidden
   prompt. Clear the clipboard after pasting. The script uses the session
   only in memory against dev-01’s loopback server. It enrolls the browser
   account’s machine, verifies a fixture turn, switches the machine to the
   Claude Code image, and prints the private sandbox home.

   ```sh
   ssh -t claude@calternal-dev-01-compute02 'cd ~/Developer/lanes/e2e-dev && ./scripts/dev-e2e/after-owner-setup.sh'
   ```

4. Sign in to Claude Code in that home. The coordinator can place a reduced
   sign-in copy there using the file names and modes above. Alternatively,
   run the interactive SSH command printed by the script. It has this
   form with the account UUID filled in. The container mounts the same
   private home as the account’s thread sandboxes. The server receives no
   provider credentials.

   ```sh
   ssh -t claude@calternal-dev-01-compute02 'podman run --rm -it -e HOME=/home/agent -v /home/claude/calternal-e2e/browser/machine/sandboxes/accounts/ACCOUNT_UUID/home:/home/agent localhost/calternal-e2e-claude:dev claude auth login'
   ```

5. Back in the preview, open a new Thread in your space and send a message.

### First owner token

The fresh server starts without `--development` and writes the one-use
token to `/home/claude/calternal-e2e/browser/appdata/setup.token` while its
account directory is empty. Only the coordinator opens the private file
as `claude` and transfers it privately to the owner. Use the form or the
link’s `#setup=` fragment, which the browser keeps out of HTTP requests.
Keep the code out of query strings, the console, and logs. An unredeemed
token expires after 24 hours.
Restarting the empty installation replaces the unused token. The file path
is safe to report. After setup, check `GET /api/v1/auth/status`; the used
file can remain and its value is invalid. The post-setup script discovers
the account and its provisioned Personal space through the signed-in API.
The enrollment target is
`http://127.0.0.1:34136` from dev-01, with a machine directory under
`~/calternal-e2e/browser/machine`. Any Claude Code sign-in for that account
belongs in that machine directory’s account-specific sandbox home.

### Setup and sign-in request shapes

All paths below are relative to `/api/v1/auth`. POST JSON with
`Content-Type: application/json` from the preview origin. Ordinary API and
SSE requests should use the cookie, with same-origin browser credentials.

| Method and path | Request body | Success |
| --- | --- | --- |
| `GET /status` | — | `{"setup":true,"signed_in":false}` on an empty installation. Both fields are booleans. |
| `POST /setup/start` | `{"token":"<setup token>","name":"<owner name>"}` | `{"ceremony":"<opaque id>","options":{"publicKey":{…}}}` with WebAuthn creation options. |
| `POST /setup/finish` | `{"ceremony":"<same id>","credential":{…},"device":"<optional label>"}` | `{"user":{"id":"<account UUID>"}}` and the session cookie. |
| `POST /login/start` | `{}` | `{"ceremony":"<opaque id>","options":{"publicKey":{…}}}` with WebAuthn request options. |
| `POST /login/finish` | `{"ceremony":"<same id>","credential":{…},"device":"<optional label>"}` | `{"user":{"id":"<account UUID>"}}` and a new session cookie. |
| `GET /me` | — | `{"user":{"id":"<uuid>","name":"…","username":"…","role":"…","capabilities":[]},"installation":"<uuid>"}` when signed in. |
| `POST /logout` | `{}` | HTTP 204 and a cookie clear. |

Use `options.publicKey` for `navigator.credentials.create` during setup
and `navigator.credentials.get` during login. Decode the challenge and
any credential IDs and user ID from base64url to binary where WebAuthn
expects buffers. Setup requires a discoverable passkey. Login uses
discoverable credentials and returns the user handle in the assertion.
Serialize the resulting public-key credential for the finish call with
base64url binary fields. Setup sends `id`, `rawId`, `type`, and
`response.clientDataJSON` and `response.attestationObject`. Login sends
`id`, `rawId`, `type`, and `response.clientDataJSON`,
`response.authenticatorData`, `response.signature`, and
`response.userHandle`. The browser should preserve any optional
WebAuthn fields used by its authenticator. Ceremonies are single-use and
expire after five minutes. Error responses carry
`{"error":{"code":"…","message":"…","retryable":false,"request_id":"…"}}`.
The authoritative examples are [the account routes](../crates/server/src/account_http.rs)
and [the software passkey conformance fixture](../tests/conformance/isolation.rs).

### Provider job SSE envelope

`GET /api/v1/jobs/<job-uuid>/events` returns SSE. The `chunk` payload
contains the stream sequence at `seq` and the provider protocol envelope
at `data`. The view consumes `data.event`; its `type` is
`text_delta`, `tool_started`, `tool_finished`, or another provider event.
For example:

```text
id: <epoch>:4
event: chunk
data: {"seq":4,"data":{"version":1,"seq":2,"event":{"type":"text_delta","text":"Example partial text"}}}

```

The two sequence numbers are independent. Placement and notice chunks
carry a plain `data.type` without a provider envelope. A changed provider
or selected machine emits a notice on the same stream, for example:

```text
event: chunk
data: {"seq":5,"data":{"type":"notice","message":"Starting a new provider session because the machine changed."}}

```

`snapshot` carries
`{"job":{…},"epoch":"…"}`, `state` carries `{"job":{…},"requests":[]}`,
and `end` carries `{"job":{…}}`. The `state` and `end` payloads expose
the job’s lifecycle. Live text and steps come from `chunk.data.event`.
See the [provider event schema](../contract/provider-events-v1.schema.json)
and [job stream implementation](../crates/server/src/plugins/job_api.rs).

## Bring-up results

| Observation | Resolution or open work |
| --- | --- |
| `calternal serve` only accepts a loopback listener | The lane-scoped socat relay binds specifically to `wt0`. A NetBird peer reached the auth status endpoint using the preview Host. |
| The shipped provider Containerfile expects a Claude installation in the home | The lane builds a bounded derivative image with the official npm package and verifies its version without mounting a home. |
| Older CLI documentation lists a single directory for machine administration | The enrollment script supplies all three root flags and the pinned installation ID, as required by the current CLI. |
| Fixture output, Stop and file placement | The first send finished and streamed. The second streamed while running and `threads.stop` drove it to `cancelled`. The Markdown included both user messages and provider output. |
| The workspace container test reported discard failure | Its three-second test lease expired during a successful Podman Stop that took 12.1 seconds. The container test now uses the standard server timing. Synthetic-machine tests keep their short timeouts. |
| Second send omitted `--resume` | The thread persists a session binding. The live fixture verifies `--resume` in the second Podman process. The real two-message check requires confirmed sign-in for the current seeded account. |
| Threads decoded a flat provider chunk | The merged browser adapter reads `chunk.data.event` for provider output. Rendering the separate placement and session notices remains a UI follow-up. |
| Browser setup and sign-in need their own first owner | Caddy reaches the separate browser server. The post-setup script passed with a fresh software passkey and its provisioned Personal space. The preview’s actual owner ceremony remains to be exercised. |
| Podman startup exhausted the dev-01 account’s kernel keyring quota | The instrumented worker test saw `crun` reject 14 starts with exit 126. Individual failures took 301–1,912 ms, and the test surfaced a 10-second worker-drain timeout. Adjusting a test deadline would leave the sandbox startup fault in place. |

See [Forgejo #34](https://git.kayg.org/kayg/calternal-ai/issues/34) for the
active qualification status.
