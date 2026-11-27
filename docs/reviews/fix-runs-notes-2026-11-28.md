# Run, egress and preview fixes · 2026-11-28

## Format addition for the coordinator

A harness plugin can declare `replies: owner.kind.operation` in `plugin.md`.
It can give one fully qualified operation per session target kind. Each
operation must already appear in `calls`, its kind must appear in `reads`,
and its owner must declare an operation slot the harness fills. The recipient
is an unconditional coded operation that accepts an `item` link to its kind and
`text`, with any other inputs optional. The host resolves this selection by
target kind at run start and delivers completed text through ordinary
scope-checked dispatch. Omitting the declaration leaves the run's output
available in the log.

The opt-in run mount preserves the configured credential file's basename
under `CALTERNAL_PROVIDER_LOGIN_DIR`. The adapter chooses the credential
filename and its internal home layout. The machine supplies the fixed
mount point and bounded scratch locations.

## Morning questions

- Is mounting a single operator-managed credential file into a run bound to
  one account, space, local image ID and operator-installed program sufficient
  for a disposable test, or should real sign-in stay disabled until a broker
  can verify the service's identity? A mounted credential lets the adapter
  read its bytes. A broker would keep those bytes outside the container but
  needs provider-aware request mediation. Keep real mode off by default
  pending that decision.

- Are four simultaneous tunnels, a five-second handshake, a thirty-second
  idle limit, a three-minute tunnel lifetime and a 64 MiB aggregate transfer
  ceiling appropriate for preview runs? A slow provider or large conversation
  can hit these limits and need a new tunnel or run. Raising them would retain
  host resources longer. The preview uses these bounds pending owner review.
- Which additional operator networks should egress exclude? An installation
  can set `CALTERNAL_RUN_EGRESS_EXCLUDE` to a comma-separated list of CIDRs
  for its own publicly addressed internal networks. The proxy pins the
  checked address and port 443. Service-level enforcement needs a broker
  that can distinguish services sharing that address.

## Simplification opportunities

- Collapse provider run lifetime and host publication cancellation into one
  run state with independently observable exit and cancellation.
