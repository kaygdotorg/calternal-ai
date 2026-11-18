# Deploy Calternal-AI at calternal.cloud

Calternal-AI serves its web app and API at `https://calternal.cloud/`. Traefik
keeps its existing TLS configuration and sends HTTP to `10.70.3.162:8080`.
The host runs both roles as its lingering `calternal` user.

```mermaid
flowchart LR
  Browser -->|HTTPS| Traefik
  Traefik -->|LAN HTTP| Server
  subgraph Host["calternal-cloud · rootless Podman + user systemd"]
    Server["calternal-ai server container"]
    Machine["calternal-ai-machine host service"]
    Sandboxes["Agent and plugin containers"]
    A["SSD · appdata"]
    S["SSD · userdata-ssd"]
    H["HDD · userdata-hdd"]
    Machine -->|Starts| Sandboxes
    Server --- A
    Server --- S
    Server --- H
  end
  Machine -->|Outbound HTTPS| Traefik
```

The image is built on x86_64 dev-01 using the repository's pinned Nix inputs and
Cargo/npm lockfiles. It contains a static Calternal binary, the web build,
trusted plugin manifests and digest-verified worker code, and an HTTP health
probe. The final image has no shell, Node runtime or package manager. The
web build's optional downloaded wallpaper assets depend on its cache and
availability at build time.

The machine runs the same static Calternal binary on the host. It starts
sandbox containers with Podman and provides only the assigned workspace and
provider home to an agent. The server's roots, the old app's files and the
Podman socket are outside those mounts. Putting the machine in a container
with the rootless user's Podman socket would give that container control
over the server's containers and bind mounts. The chosen host service keeps
that control with the operator's user account. Both roles share that Unix
account, so co-location depends on the container boundary and host trust.

On the 4-core, 8 GB host, the machine admits two sandbox slots **total** across
agent turns and plugin workers. Each agent sandbox has 1 CPU, 1 GiB memory and
256 PIDs. Each plugin worker has 1 CPU, 256 MiB memory and 64 PIDs, with
networking disabled. Operate the deployment with sandboxed machine mode.

## Deploy and roll back

From the dev-01 build checkout run `deploy/deploy.sh --target deploy/cloud.conf`.
It builds and sends the image via SSH, stages the units and matching machine
binary, stops both services, and promotes the image to `:current`. It verifies
HTTP readiness on the published address and runs the container probe. If
the new release fails those checks, it restarts the previous release. A
successful deploy retains one previous release under `:rollback`.
`deploy/rollback.sh --target deploy/cloud.conf` swaps the releases and checks
health. Add `--dry-run` to see the SSH calls before running either script.

Provider sign-in needs a matching Claude sandbox image. Build the plugins and
run `deploy/build-provider.sh` on the build VM from the same revision as the
server and machine binary. Stage the rebuilt sandbox image on the target before
activating the release. The account's login stays on the machine under
`sandboxes/accounts/<account>/provider-login`, and its transcript home is
scoped to the space.

Deploy and rollback reuse the three data roots. A schema mismatch can refuse
startup, including when an older binary opens data initialized by a newer one.
Per-space stores that open lazily may first report a mismatch when used.
Automatic rollback switches code and units. Recovering data requires a
coordinated backup and an operator decision. See **Decisions before real data**
below.

## Backups and service controls

Keep the installation UUID in `/srv/calternal-ai/server.env` with the backups.
The SSD VM backup should cover `/srv/calternal-ai/appdata`,
`/srv/calternal-ai/userdata-ssd`, the unit and configuration files, and
`/srv/calternal-ai/machine`. This includes private credentials and provider homes.
Verify that the VM backup also captures the separate disk mounted at `/mnt/hdd`
and its `calternal-ai/userdata-hdd` tree. Schedule a separate backup if the HDD is
outside the VM backup. For a coordinated pre-deploy restore point, stop both
roles before taking the snapshot. Keep the old app's backup policy in force.

Run on the host as `calternal` to inspect or stop the service:

```sh
systemctl --user status calternal-ai.service calternal-ai-machine.service
journalctl --user -u calternal-ai.service -u calternal-ai-machine.service -f
podman healthcheck run calternal-ai
systemctl --user stop calternal-ai-machine.service calternal-ai.service
systemctl --user start calternal-ai.service calternal-ai-machine.service
```

A manual stop lasts until the next start or boot. To stop across boots, disable
the machine service and move the server's `.container` file out of the Quadlet
search path, then run `systemctl --user daemon-reload`.

## Cut over from the existing app

**Device sync pauses at cutover.** The existing container's calternaldav
listener on port 993 stops with it. Calternal-AI has no DAV bridge and publishes
only the web port. The owner's phone and Mac will stop syncing CardDAV contacts
and CalDAV reminders and calendars with `calternal.cloud` until that plugin is
available here. Restoring the old container restores its DAV listener.

1. Record the Traefik **source** address observed by the backend. Keep the
   public Host header intact, have Traefik replace untrusted forwarding
   headers, and restrict access to the LAN port to Traefik using the host's
   existing network policy. Confirm rootless Podman with cgroup v2 and pasta.
   If you need to identify the source IP, watch the connection metadata on the
   target while requesting the old app's `/healthz` through its public URL.
   As the administrator, provision the mount points:

   ```sh
   # Optional source-IP check while another terminal requests /healthz.
   tcpdump -ni any -c 5 'tcp dst host 10.70.3.162 and dst port 8080'
   loginctl show-user calternal -p Linger
   test "$(uname -m)" = x86_64
   mountpoint -q /mnt/hdd
   install -d -m 0700 -o calternal -g calternal /srv/calternal-ai /mnt/hdd/calternal-ai
   ```

2. On the target as `calternal`, create the public configuration once. Replace
   `TRAEFIK_IP` with the recorded IPv4 address. Its `/32` is the only trusted
   forwarding peer. The origin supplies the relying-party ID
   `calternal.cloud`.

   ```sh
   umask 077
   TRAEFIK_IP=REPLACE_WITH_TRAEFIK_SOURCE_IPV4
   (
     set -C
     printf 'CALTERNAL_INSTALLATION_ID=%s\nCALTERNAL_ORIGIN=https://calternal.cloud\nCALTERNAL_TRUSTED_PROXIES=%s/32\n' \
       "$(cat /proc/sys/kernel/random/uuid)" "$TRAEFIK_IP" > /srv/calternal-ai/server.env
   )
   test "$(stat -Lc %a "$HOME/calternal-secrets.env")" = 600
   podman info --format '{{.Host.CgroupsVersion}} {{.Host.RootlessNetworkCmd}}'
   podman pull docker.io/library/node:24-alpine
   ```

3. On the dev-01 build host, stage the server and machine sandbox images
   before taking the old app down. Set `CALTERNAL_DEPLOY_HOST` to the
   coordinator's authorized SSH endpoint if it differs from the default.
   Build as a non-root operator with access to the dev host's Podman.

   ```sh
   export CALTERNAL_DEPLOY_HOST=calternal@calternal-cloud
   revision=$(git rev-parse --short=12 HEAD)
   export RELEASE_REVISION=$(git rev-parse HEAD)
   export SOURCE_DATE_EPOCH=$(git show -s --format=%ct HEAD)
   nix develop .#deploy -c deploy/build-image.sh "localhost/calternal-ai:$revision"
   deploy/build-provider.sh localhost/calternal-ai-claude:current
   podman save localhost/calternal-ai-claude:current | ssh "$CALTERNAL_DEPLOY_HOST" podman load --quiet
   deploy/deploy.sh --target deploy/cloud.conf --image "$revision" --initialize --dry-run
   ```

4. On the target as `calternal`, stop and disable the old generated service
   by retaining its Quadlet under a disabled filename. Keep
   `/srv/calternal-app/data` intact. Then run the deploy command from dev-01.

   ```sh
   systemctl --user stop calternal.service
   mv "$HOME/.config/containers/systemd/calternal.container" \
      "$HOME/.config/containers/systemd/calternal.container.cutover-disabled"
   systemctl --user daemon-reload
   ```

   ```sh
   deploy/deploy.sh --target deploy/cloud.conf --image "$revision" --initialize
   curl -fsS https://calternal.cloud/api/v1/auth/status
   curl -fsS -o /dev/null https://calternal.cloud/
   ```

5. As the `calternal` deploy account, request a fresh code in a private
   terminal on the target.

   ```sh
   podman exec calternal-ai /usr/local/bin/calternal setup-code --appdata /data/appdata
   ```

   The command uses the running container's installation ID. Open the address
   it prints and register the first owner before the code expires. Repeating
   the command replaces the previous unused code without a restart. Server
   startup preserves a valid mode-0600 `setup.token`. Deployment normally
   prints the command and address. The installed user service automatically
   enrolls this host in the instance pool. The first administrator gets an
   initial pool grant through the existing authorization mechanism.

   The deploy script creates `/srv/calternal-ai/machine-handoff` with mode 0700,
   owned by `calternal`, and mounts only that handoff directory in the server.
   The machine service generates and saves its signing key in its own private
   state and atomically publishes a mode-0600 `request.json` containing its
   UUID, public key and machine label. After the first administrator has a
   passkey, the server commits the pool identity and the initial grant together,
   then publishes a mode-0600 `enrolled` file with that UUID. The machine
   validates the UUID, saves its credentials, and removes both handoff files.
   Replaying the same UUID and key across a service restart returns the same
   enrollment. No signing key or enrollment code crosses the shared mount.
   Existing machine credentials take precedence.

   The onboarding screen can use `machines.onboarding` to see the enrollment
   and placement availability. Use `machines.verify` to confirm that the
   sandbox starts and check the provider login.

6. To undo the cutover, on the target as `calternal`, stop the new roles
   and restore the old Quadlet on the same port. Confirm the old app through
   Traefik. Keep the new roots for recovery and diagnosis.

   ```sh
   units=$HOME/.config/containers/systemd
   if systemctl --user cat calternal-ai.service >/dev/null 2>&1; then
     systemctl --user stop calternal-ai-machine.service calternal-ai.service
     systemctl --user disable calternal-ai-machine.service
     mv "$units/calternal-ai.container" "$units/calternal-ai.container.cutover-disabled"
   fi
   mv "$units/calternal.container.cutover-disabled" "$units/calternal.container"
   systemctl --user daemon-reload
   systemctl --user start calternal.service
   curl -fsS https://calternal.cloud/healthz
   ss -ltn 'sport = :993'
   ```

## Additional machines

Set `CALTERNAL_MACHINE_INSTALL_IMAGE` in the server's environment to the
standard sandbox image accessible from enrolled hosts. A host's
`localhost/…` image tag only works where that image is installed locally.
The signed-in person can request `machines.install` to get a shell script
with a 15-minute, single-use code for their own machine. It needs a Calternal
binary for that platform and a running Podman engine. The script enrolls
over the public server URL and installs a systemd user service or a launchd
agent. It reruns without replacing an existing identity for that server.

## Decisions before real data

- **Schema transitions.** Choose a maintenance-window export and import
  with a rehearsed restore, or keep this release family on one schema until
  a supported transition is agreed. Export and import require tooling and
  downtime. Keeping the schema stable limits which releases can ship.
- **Existing app data.** Choose whether it stays in its separately retained
  installation or needs an explicit import project. An import requires a
  mapping and validation of accounts, files and permissions.
- **Backups.** Confirm HDD coverage and select a retention and restore policy
  for the SSD, HDD and sensitive machine state. Retaining more coordinated
  restore points uses more storage.
