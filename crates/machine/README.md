# calternal-machine: the machine role

A machine (your own Mac or Linux computer, or a cloud VM the operator enrolled)
connects **out** to the server, reports capacity, and runs each assigned thread
in a per-thread sandbox that it starts itself. The server never connects to a
machine and never starts containers or VMs. The server side lives in
`crates/server/src/machines.rs`.

## Use

```sh
# On the server host, or from the signed-in account (POST /api/v1/machines/codes):
calternal machines code --appdata APPDATA --userdata-ssd SSD --userdata-hdd HDD --account <account-uuid>   # or --pool
# On the machine:
scripts/install-machine.sh --server https://cal.example.org --code - < code.txt
# which runs, in order:
calternal machine enroll --server https://cal.example.org --code CODE
calternal machine run                    # --backend auto|podman|direct; default auto follows owner setting
calternal machine status
# From a signed-in client:
calternal machine verify MACHINE_UUID --space-id SPACE_UUID --token-file PERSON_TOKEN
```

Machine directory: `$CALTERNAL_MACHINE_DIR`, else `$XDG_DATA_HOME/calternal/machine`,
else `~/.local/share/calternal/machine`. It holds `credentials` (`key: value` lines,
mode 0600: server, machine id, owner, Ed25519 private key) and `sandboxes/accounts/<account>/threads/<thread>/work` and a persistent `sandboxes/accounts/<account>/home`.
Exit code 3 means the machine was revoked.

Verification runs one provider readiness check through the enrolled channel. A
machine owner or pool administrator gets the sandbox status, provider install
and sign-in status, and official sign-in command as available. Add `--json` for
the structured outcome.

## Design

- **Control plane** uses version 4 long-poll over HTTP(S) ([`protocol`](src/protocol.rs))
  and the server's request guard. Both directions carry sequence numbers and
  acknowledgements, so a dropped request loses nothing. The cost is one held request
  per machine, plus one round trip of latency when output and work cross.
- **Identity**: the key pair is made on the machine; the server stores the public key.
  A session is a signed single-use challenge that yields a bearer token, held in memory
  on both sides.
- **Leases and fencing**: every assignment carries a fence. The server refuses output
  with an old fence or from another machine (409). After `lease_ttl` without contact,
  the machine removes its disposable containers and keeps their mounted workspaces.
  The server fences silent leases. Only an explicit resume makes an interrupted lease
  eligible for placement again.
- **Sandboxes** ([`sandbox`](src/sandbox/mod.rs)): a trait with start, exec
  (streamed output), freeze, resume, stop, release and resource limits. The Podman
  backend runs one rootless container per active thread or worker slot, with a disposable
  writable layer, work mounted at `/work`, and `--memory`/`--cpus`/`--pids-limit`.
  The role reports physical and available memory with each heartbeat. Linux reads
  `/proc/meminfo` once and macOS queries free pages with `vm_stat`, on a blocking
  worker. The server checks the cached sample and counts reservations under the
  existing placement lock, with zero additional storage reads per placement.
  Hosts reserve 2 GiB for a standard sandbox and 2, 4, or 8 GiB per turn on the
  selected account guest profile. The guest also budgets its own turn limits
  against usable guest RAM. A missing or stale sample cannot admit new work.
  Operators can optionally set `--max-sandboxes` for turns and workers. A
  successful poll parks provider containers idle for a lease TTL, 90 seconds by default.
  Startup sweeps orphan containers scoped to the locked sandbox directory. The direct
  backend runs as the machine user with no isolation, only after the
  account owner marks that machine direct. Pool machines refuse direct execution.
  The microVM backend implements the same trait through an authenticated vsock
  channel to a per-account Firecracker guest.
- **Back-pressure**: at most 256 KiB per exec in the channel and 256 KiB unacknowledged
  on the machine, plus at most 1 MiB unread on the server. When the reader stops, the
  process blocks on its next write.

## Tests

`cargo test -p calternal-machine`: direct backend lifecycle, freeze, back-pressure and
credentials. `tests/podman.rs` prints `SKIPPED` and passes when Podman or the image
(`CALTERNAL_TEST_IMAGE`, default `docker.io/library/alpine:3`) is missing; it never pulls.
End-to-end tests with both roles are in `crates/server/tests/machines.rs`.

Provider stdin and the account-qualified home mounts are described in
[manual turns](../../docs/agent-turns.md). Provider release removes the container
and retains mounted workspace files. Destructive machine-stop operations still
remove the thread workspace.

Explicit disposal: `calternal agent discard <job-id>` requests machine-confirmed
removal of a stopped turn's workspace/container. It preserves the account home.
The machine owner can stop the role and use `calternal machine discard
--account-id <uuid> --thread-id <uuid> --dir DIR`; a directory lock excludes a
live role. Never-sent requests are durable; interrupted in-flight deletion asks
for inspection/retry. Automatic cleanup of preserved work and plan disk accounting
are follow-ups. There is no timed expiry of unpreserved work.

Exec environment names must be POSIX identifiers. Podman receives name-only
`--env NAME` arguments and resolves values from its child environment; credentials
never appear in the engine argv. Direct execution sets environment values with
the process API, and configured execution delegates to those same backends.
Enrollment codes travel over HTTP; use `CALTERNAL_ENROLL_CODE` or `--code -` with
stdin when invoking the machine CLI so the code stays out of the caller argv.

Plugin workers use these same backend and exec interfaces with a separate confined
profile: no network, read-only code only, no persistent home or host data mounts,
256 MiB / 1 CPU / 64 PIDs. Interactive execs return a bounded stdin sender;
machine `Input` messages retain ordering and fencing. Details, lifecycle, test
commands and placement restrictions: [plugin workers](../../docs/plugin-workers.md).

The approved item-head profile opens a separate per-space data stream with
HTTP/1.1 Upgrade on the enrolled TLS origin. Machine sync delivers the
single-use reservation and controls placement. The runtime verifies the
leased space and generation before starting the package in the existing
worker sandbox. Revoking the session or losing the short space-channel
lease tears down that placement. [Protocol and verification](../../docs/reviews/space-channel-2026-11-19.md).

## Harness runs

The run service binds an installed plugin executable to a session target and
starting actor. It starts a rootless network-isolated Podman container with a
read-only image, plugin bundle and workspace. Bounded temporary mounts contain
writable run state. Run input checks the starting actor and returns queue
acceptance. Output events have a monotonic run sequence and a terminal exit.

The host authorizes Stop against the run's original scope. Target removal or
failed publication can terminate a known run by ID after the target disappears.
Termination signals the pinned init process of the private PID namespace and
waits up to 1.8 seconds. Failed sandbox cleanup is reported and retried.
Startup sweeps stale containers and private run artifacts for the owned root.

The CONNECT proxy admits the plugin's exact declared hostnames on port 443.
It resolves absolute DNS names and rejects any answer containing an address
outside the global-unicast policy or inside operator-supplied exclusions.
It dials the checked numeric address. Four simultaneous tunnels share a
64 MiB transfer budget. Each tunnel has a five-second handshake, thirty-second
idle and three-minute lifetime limit. The fake provider and an injected
resolver and dialer exercise this path.

Real sign-in is an opt-in development mode for one account, space and trusted
installed program. The operator supplies one private credential file and a full
local image ID. The fake image tests use a synthetic credential file.
Executable program bindings are supported. Image bindings return an explicit
unsupported error.

On the dev VM, the service tests are `cargo test -p calternal-machine --test
run_service`; CONNECT policy tests are `cargo test -p calternal-machine --test
egress_proxy`. Both use the local fake only.

With `--space-vm-config PATH`, the item-head carrier uses an offline 256 MiB,
two-vCPU Firecracker guest. Use the digest-pinned image assembled by
`deploy/microvm/build-image.sh` with the Node runtime. The guest stages package
code on private tmpfs and starts Node through a restricted process launcher.
The launcher installs the worker seccomp profile, isolates mounts, processes
and networking, drops privileges, and gives each worker a 192 MiB memory
limit with no swap. Node caps old space at 96 MiB and each semi-space at 4 MiB.
The remaining guest memory is available to the kernel,
the control agent and the reseed helper. Restores launch fresh Node by
default. Set `prestarted_worker: true` to park a clean trusted launcher before
checkpointing. The restored guest must acknowledge kernel reseed before the
launcher can refresh and load a package. Before publishing its output link,
the image build requires the release image's randomness, permissions and heap
proof and the native isolation proof on a test image built from the same inputs.

The small VM JSON accepts `idle_seconds` (1 through 86400, default 900) and
`resident_memory_mib` (default `memory_mib` × `max_guests`). The machine reserves
256 MiB of guest RAM for each placement. A full machine sleeps the least
recently used idle guest before admitting another space. Calls arriving during
sleep join the same teardown before the next wake. `calternal machine status`
reports resident count, reserved guest RAM and the configured idle window.



First-owner deployments start their machine service before setup. The server
and that service share an owner-only directory for the public-key enrollment
handoff. The machine role retains its private key. Use `calternal machine
onboarding` to see its attachment and capacity and `calternal machine verify`
to run a sandbox diagnostic.

A person can get the installer through the app's `machines.install` operation
or `calternal machine install`. It enrolls to that person's account with a
15-minute code, copies the supplied Calternal binary and sets up a user-level
systemd service or launchd agent. Install the matching binary and have the
Podman engine running first. Use `sh installer.sh --binary /path/to/calternal`
when the binary is outside PATH.


The opt-in `microvm` pool backend runs this same rootless Podman implementation
inside a Firecracker guest per account. It uses the same isolated space homes
and Claude login directory in the guest's account disk. The guest boots
offline by default. An operator can attach a TAP with explicit egress
rules. The deployment guide covers the preview, KVM privileges, pinned
image, and firewall. [Deployment guide](../../docs/deploy.md#choose-the-machine-sandbox-backend)
