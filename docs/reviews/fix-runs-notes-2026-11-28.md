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

## Simplification opportunities

- Collapse provider run lifetime and host publication cancellation into one
  run state with independently observable exit and cancellation.
