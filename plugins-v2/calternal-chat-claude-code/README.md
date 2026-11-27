# Claude Code chat harness

The revision 9 manifest binds program ID `calternal-provider-claude-code` to
`programs/calternal-provider-claude-code` inside this plugin. The wrapper starts
the local egress bridge, opens the host run-control socket, then delegates Claude
stream handling to the existing `plugins/calternal-claude-code` adapter installed
in the container image.

The container has `--network=none`. The bridge forwards only to the per-run host
CONNECT proxy. The host proxy allows the exact `sends out:` hostnames, resolves
them itself for every CONNECT, rejects unsafe IP ranges, and records host and
byte counts. The fake provider test redirects the checked numeric endpoint to a
local test server. There is no real Claude invocation or sign-in.

Build the adapter and fake image on the dev VM, then run the fake integration
tests under the shared lock:

```sh
~/bin/lock-shared nix develop -c npm ci
~/bin/lock-shared nix develop -c npm run build:plugins
~/bin/lock-shared nix develop -c plugins-v2/calternal-chat-claude-code/test/build-echo-image.sh
CALTERNAL_TEST_PROVIDER_IMAGE=localhost/calternal-chat-claude-code-fake:dev \
  ~/bin/lock-shared nix develop -c cargo test --locked -p calternal-machine \
    --test egress_proxy --test run_service --test runs_harness
```

The egress tests cover an allowed host, an undeclared host, and a private resolver
answer. The run-service tests verify the allowed tunnel reaches its local test
server, direct networking is blocked, actor-bound input, ordered output, and a
process-family Stop within two seconds.

The machine mounts the plugin bundle, proxy socket and host-lifetime socket
for the wrapper. The fake image supplies the adapter and fake provider only.
Stop signals the pinned init process of the private Linux PID namespace and
waits for its process family to exit. Podman removes the container artifacts
afterward.
