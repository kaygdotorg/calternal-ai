# Plugin workers through machines

Issue [#29](https://git.kayg.org/kayg/calternal-ai/issues/29), 2026-11-17.
Worker frames remain protocol v2; the machine transport is v3. Disposable machine
lease databases use schema 3; older versions are refused, with no migration.

The server verifies and reads a self-contained UTF-8 bundle (at most 1 MiB), then
sends its source over the authenticated machine assignment channel. It never
starts Podman. The machine stages the source and uses the existing sandbox
backend and exec channel. Interactive exec input stays open; ordered, replay-
protected `Input` envelopes carry subsequent frames. Fences apply in both directions.
No paths or credentials accompany the source. Internal worker leases and raw
exec streams are hidden from person-facing machine routes; only checked plugin
results reach callers. Worker stderr is discarded at the machine boundary.

The worker profile has one read-only `/plugin` code mount, a read-only root,
`--read-only-tmpfs=false`, an isolated network namespace, no capabilities, no privilege
gain and no logs. It never mounts the account home, workspace, host CLI, Nix store,
appdata or userdata. The bundled seccomp allowlist additionally refuses socket
creation (including Unix and loopback sockets) and io_uring. Node's permission
model permits code reads only; child processes, native addons and writes remain
refused. The worker environment contains only the fixed locale.

Limits have one declaration in `sandbox/worker.rs`: 256 MiB memory (no extra swap),
1 CPU and 64 PIDs; V8's heap is capped at 128 MiB. There is one worker per
(plugin, space), at most eight physical workers including starts and retiring
sessions, with a semaphore held until authenticated teardown confirmation. Caller errors
are delivered before asynchronous cleanup, so a failed or unreachable machine
cannot hang an operation or silently release capacity. Failed machine cleanup is
retried; pending teardown is bounded by eight slots and has no timed expiry. At most 32 invocations are
admitted per worker; each frame is at most 256 KiB. Transport queues hold 32
outbound frames, 64 session events, and 16 input frames per exec at each machine
transport stage; exec output retains the existing bounded back-pressure.

Admission has a separate 10-second deadline covering prior cleanup, capacity
and controller setup. After capacity is reserved, the invocation deadline also
bounds cold launch and queueing. A timed-out launch is detached and holds its
permits until cleanup; it never receives the cancelled invocation.

A worker idles out after 60 seconds without an invocation, even if it emits
unsolicited output. Operations have a 3-second deadline; job run invocations keep
the existing 600-second checkpoint deadline. Sessions retire after 4096 calls.
Malformed/oversized output, process death or a deadline ends the worker and
returns a safe `unavailable` error to all affected callers. The next call starts
a clean worker; uncertain operations are never replayed automatically. Cancellation
invalidates host authority before any further callback or delivery.

Worker source stays only in memory and the ephemeral machine code directory.
Ending its lease removes its container and directory. Failed starts remove their
partial code directory. On machine startup, the backend removes orphaned worker
containers labelled with that machine directory only; other machine roots and
agent sandboxes are preserved. A server restart durably
fences worker leases into `stopping` rather than adopting them as agent sandboxes.
New worker admission waits for recovered teardown proofs, even across repeated
server restarts. Acknowledged cleanup changes the durable lease to `ended`. A machine
restart fences its execs through the existing boot-ID handling. Failed starts
remain unavailable; there is no automatic direct fallback or runtime pull.

## Placement and unresolved contracts

Remote workers currently admit only to operator-enrolled pool machines, and use
the existing initiating account's pool grant and machine capacity. This is a
restriction pending a placement contract: a participant-owned machine cannot be
trusted with later shared-space calls after its owner loses access. A participant
machine marked direct never runs remote plugin code. Configured machines still
choose containers for workers even when their owner chose direct agent execution.

Options requiring an owner decision:

- A space-owned operator worker service: separate capacity/entitlement from agent
  pool grants, with explicit trusted placement. Requires a service placement contract.
- Participant machines: requires an explicit space trust grant plus immediate
  placement-owner revocation checks, interruption rules and data disclosure terms.
- A server host without Podman: an enrolled operator machine can supply the
  runtime today. Automatic local machine enrollment/provisioning, or a disabled
  plugin state with setup guidance, requires a product decision. No server-side
  container path is added.

A revoked or permanently lost machine cannot supply authenticated cleanup proof.
Capacity remains reserved; an operator teardown attestation/recovery flow needs
an owner decision before such proof can be bypassed.

Manifest v1 has no destination declaration, manager approval record or `fetch`
host call. All plugin network is therefore disabled. A future network-contract
lane must define these together, reuse the existing app's netguard (ADR 0071),
and keep outbound connections exclusively in the trusted server broker. Nothing
in this lane supplies raw sockets or an unapproved fetch API.

## Development and tests

Provision `docker.io/library/node:24-alpine` on the enrolled machine before use;
the backend uses `--pull=never`. The image reference is centralized alongside the
limits. Operator-controlled digest pinning/provisioning remains a deployment concern.

`serve --development --node /absolute/path/to/node` explicitly selects the direct
sandbox adapter for disposable development; it provides no OS isolation. A
`test-plugins` build can select its local fixture backend with
`CALTERNAL_WORKER_TEST_BACKEND=direct|podman`. Neither mechanism is a production
fallback. These local adapters can expose a fixture's code directory read-only;
that path is never serialized in machine assignments. This preserves existing
fixture readiness files without sending host directories to remote machines.

The existing plugin tests can run unchanged with
`CALTERNAL_WORKER_TEST_BACKEND=podman cargo test -j6 -p calternal-server --test plugin_host`.
Container isolation tests skip with an explicit message if Podman is unavailable;
a missing worker image fails with the exact provisioning command. Development
measurements use the same sandbox interface; see the dated measurement report.
