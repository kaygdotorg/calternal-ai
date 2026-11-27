# Persistent Chat preview

The preview runs on the Calternal development VM as the user service
`calternal-chat-preview.service`. Connect from your own computer.

```sh
ssh -L 24317:127.0.0.1:24317 claude@calternal-dev-01-compute02
```

Keep the SSH connection open and visit
[http://localhost:24317/host](http://localhost:24317/host). Use this exact
origin for enrollment and later sign-ins. The browser keeps its Secure,
HttpOnly, SameSite=Strict `__Host-` session cookie.

## First sign-in

Open the first-owner file privately on the VM at
`~/Developer/lanes/chat-preview/data/first-owner-code`. Enter its contents
on the setup screen, create a passkey, and store the recovery codes. The code
file has mode `0600` and a fresh code is issued on service start until the
first account exists. A code lasts 24 hours. If it expires before enrollment,
restart the user service. Once enrolled, use **Sign in with a passkey**.
The code file is removed on the next service start after enrollment.

The first completed sign-in installs the Chat and Claude Code harness plugins
in the owner's personal space. Create a chat, choose **Claude Code** under
**Ask an agent**, and send a message. The default provider echoes the message.
Reload or restart the user service and sign in again to check that the chat
and passkey survive. Restart stops in-flight runs and clears their live logs.

## Deploy or redeploy

Run `scripts/dev-chat-preview.sh` from a source checkout on the dev VM.
The script builds under `~/bin/lock-shared nix develop`, creates a release
under `~/Developer/lanes/chat-preview/build`, then switches
`~/Developer/lanes/chat-preview/current` to it. The persistent
`~/Developer/lanes/chat-preview/data` directory stays outside the build
and release paths. A failed start rolls the build link back when a previous
release exists. The current and previous build releases are kept for rollback.
Installed plugin versions carry forward with the personal space.

Use these commands on the VM to inspect and restart the preview.

```sh
systemctl --user status calternal-chat-preview
systemctl --user restart calternal-chat-preview
journalctl --user -u calternal-chat-preview
```

## Real provider configuration

The local image `localhost/calternal-claude-code-real:preview` is prepared
with the built adapter and the existing local Claude Code image as its
base. To select it, open an operator drop-in on the VM.

```sh
systemctl --user edit calternal-chat-preview
```

Set both values in the drop-in and restart the service.

```ini
[Service]
Environment="CALTERNAL_REAL_RUN_IMAGE=localhost/calternal-claude-code-real:preview"
Environment="CALTERNAL_RUN_SIGN_IN_DIR=/absolute/operator-managed/path"
```

The path names the operator's existing provider login directory, which has
the `.credentials.json` layout expected by the adapter. RunService binds
it read-only into the isolated run and gives the adapter a separate writable
home for transient files. Remove both settings and restart to switch back
to the fake. The image must be present locally before a run starts.

Egress stays at the exact plugin-declared name `api.anthropic.com` on port
443 through the host CONNECT proxy. Valid undeclared hostnames and names
with unsafe DNS answers are logged on rejection. The proxy records only
host names and forwarded byte counts.

A disposable host test on November 27 reached `api.anthropic.com` using
the read-only test sign-in. The CLI's local status reported signed in, but
the provider rejected the turn with `authentication_failed`. The proxy
recorded no refused host names. The next check is to renew the test sign-in
outside the sandbox and retry with the same read-only mount and egress
policy. Credential refresh inside the run can fail if it needs a write or
an undeclared host.

The single-turn Playwright script is `apps/web/e2e/host-real-provider.mjs`.
It skips unless both real provider settings are present, starts its own
disposable host, and sends one short prompt per invocation. Set `HOST_BIN`
if the host binary is outside `target/debug/calternal-host`.

## Browser verification

The Playwright test uses a disposable owner and chat on the same localhost
origin pattern. It tests browser cookie delivery, setup and sign-in responses,
and persistence across preview restarts. Chromium supplies a real virtual
WebAuthn authenticator. In WebKit, the same-origin Chromium authenticator
signs the test challenges and WebKit submits the credentials itself. Native
Safari passkey hardware requires separate verification.

At `http://localhost:24317`, Chromium passed setup, sign-in, and Secure
cookie delivery. The test kept the same passkey and chat after restarting
the preview. The Playwright WebKit build on the Linux VM reported a secure
context but exposed no native WebAuthn API. The host accepted WebKit's
setup and sign-in requests with the test authenticator's signatures, but
WebKit refused the Secure cookie on HTTP localhost. Use Chromium for this
preview. Native Safari behavior has yet to be verified.

The VM needed software EGL for the headless WebKit test. On this VM,
the following ran with port 24317 temporarily handed from the unclaimed
service to the disposable fixture. The service was started again afterward.

```sh
export CARGO_TARGET_DIR="$HOME/Developer/lanes/chat-preview/build/target"
export HOST_BIN="$HOME/Developer/lanes/chat-preview/current/calternal-host"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
export PREVIEW_TEST_PORT=24317
mesa=/nix/store/z5zn9mxnvik2s9h1bpnr1ipa27qhi9py-mesa-26.2.5
export __EGL_VENDOR_LIBRARY_DIRS="$mesa/share/glvnd/egl_vendor.d"
export LIBGL_DRIVERS_PATH="$mesa/lib/dri" LIBGL_ALWAYS_SOFTWARE=1
~/bin/lock-shared nix develop -c node apps/web/e2e/host-preview-auth.mjs
```
