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
scripts/install-machine.sh --server https://cal.example.org --code CODE
# which runs, in order:
calternal machine enroll --server https://cal.example.org --code CODE
calternal machine run                    # --backend auto|podman|direct; default auto follows owner setting, --max-sandboxes 10
calternal machine status
```

Machine directory: `$CALTERNAL_MACHINE_DIR`, else `$XDG_DATA_HOME/calternal/machine`,
else `~/.local/share/calternal/machine`. It holds `credentials` (`key: value` lines,
mode 0600: server, machine id, owner, Ed25519 private key) and `sandboxes/accounts/<account>/threads/<thread>/work` and a persistent `sandboxes/accounts/<account>/home`.
Exit code 3 means the machine was revoked.

## Design

- **Transport**: version 2 long-poll over HTTP(S) ([`protocol`](src/protocol.rs)). There are no
  new dependencies on either side, it passes any proxy in front of the server, and it
  reuses the server's request guard. Both directions carry sequence numbers and
  acknowledgements, so a dropped request loses nothing. The cost is one held request
  per machine, plus one round trip of latency when output and work cross.
- **Identity**: the key pair is made on the machine; the server stores the public key.
  A session is a signed single-use challenge that yields a bearer token, held in memory
  on both sides.
- **Leases and fencing**: every assignment carries a fence. The server refuses output
  with an old fence or from another machine (409). After `lease_ttl` without contact,
  the machine freezes its sandboxes. The server fences the leases and marks them interrupted. Only an explicit
  resume operation makes them eligible for placement again.
- **Sandboxes** ([`sandbox`](src/sandbox/mod.rs)): a trait with start-or-adopt, exec
  (streamed output), freeze, resume, stop, release and resource limits. The Podman
  backend runs one rootless container per thread, with the base image by reference, the
  container's own writable layer, a work directory at `/work`, and `--memory`/`--cpus`/
  `--pids-limit`. The direct backend runs as the machine user with no isolation, only after the
  account owner marks that machine direct. Pool machines refuse direct execution. A microVM backend would implement
  the same trait.
- **Back-pressure**: at most 256 KiB per exec in the channel and 256 KiB unacknowledged
  on the machine, plus at most 1 MiB unread on the server. When the reader stops, the
  process blocks on its next write.

## Tests

`cargo test -p calternal-machine`: direct backend lifecycle, freeze, back-pressure and
credentials. `tests/podman.rs` prints `SKIPPED` and passes when Podman or the image
(`CALTERNAL_TEST_IMAGE`, default `docker.io/library/alpine:3`) is missing; it never pulls.
End-to-end tests with both roles are in `crates/server/tests/machines.rs`.

Provider stdin and the account-qualified home mounts are described in
[manual turns](../../docs/agent-turns.md). Provider release stops processes while
retaining workspace files; destructive machine-stop operations still remove the
thread workspace.

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
