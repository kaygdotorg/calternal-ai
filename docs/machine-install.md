# Install a personal machine

Copy the command in Calternal's machine setup. It fetches the installer from
calternal.com, shows what it will install, and asks for confirmation. You can
also run the public bootstrap yourself and enter a one-time code from the app
at the prompt.

```sh
curl --disable --proto '=https' --tlsv1.2 -fsS https://calternal.com/install |
  sh -s -- --server https://cal.example.org
```

HTTPS authenticates the bootstrap script from calternal.com. Set `--server`
to the Calternal instance you want to enroll with. The bootstrap verifies
that server's machine program with the release key pinned at calternal.com. The app command contains a
single-use code valid for 15 minutes. The prompt keeps the code out of shell
history. The command passes it via the installer's environment and the
installer passes it to enrollment via stdin.

The installer needs curl, OpenSSH with `ssh-keygen -Y verify`, and a running
Podman for the default sandboxed mode. It uses the existing enrollment command,
then installs a systemd user service on Linux or a LaunchAgent on macOS.
Everything belongs to your user. The machine directory holds your installed
binary, existing credentials, and workspaces. The service downloads and
verifies a matching release before it drains. It keeps syncing active leases
until they finish, then uses the existing machine ownership lock and atomic
file replacement to switch to the verified executable and restart. A machine
with a retained lease waits until that lease is released.

To remove the service and installed executable, use:

```sh
curl --disable --proto '=https' --tlsv1.2 -fsS https://calternal.com/install |
  sh -s -- --uninstall
```

Uninstall preserves enrollment and workspaces. Revoke the machine in Calternal
when you retire it. Use the same machine directory and XDG settings for install
and uninstall.

## Publish machine releases

Build the server and each machine executable with the same
`CALTERNAL_MACHINE_RELEASE_ID` and `CALTERNAL_MACHINE_RELEASE_PUBLIC_KEY`.
The ID is a unique release identifier, such as the deployment's Git revision.
`deploy/build-image.sh` uses `RELEASE_REVISION` by default. The key is the
first two fields of the owner's OpenSSH Ed25519 public key. Keep the owner's
private key on their signing system.

For each platform, place the executable at
`<release-dir>/<id>/<platform>/calternal`. Platform names are
`linux-x86_64`, `linux-aarch64`, `macos-x86_64`, and `macos-aarch64`.
Beside it, create a file called `manifest` with exactly five newline-terminated
lines. Put `calternal-machine-release-v1` on the first line, then the release ID,
platform name, exact size in decimal bytes, and lowercase SHA-256 digest.
The executable limit is 256 MiB. Generate the digest from the final published
binary, including any stripping or platform signing. The owner signs the
manifest with `ssh-keygen -Y sign -n calternal-machine-release-v1 -f KEY manifest`.
Publish the resulting `manifest.sig` beside it.

The official enrollment command uses the pinned bootstrap at
`https://calternal.com/install`. Independently signed builds can use their own
server's `/install` path and enter the code at the prompt.

Set `CALTERNAL_MACHINE_RELEASE_DIR` to the absolute, read-only release
directory on the server. A container deployment can mount it as a read-only
volume. The existing API router serves releases for the server's own build ID.
A build without a valid key, release ID, and configured release directory
offers no release artifacts. Co-located machines keep using the deploy-managed
binary and existing service. The installer alone opts the personal service
into self-update.
