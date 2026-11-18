# Durable plugin waits

2026-11-18 · [Plugins lane #50](https://git.kayg.org/kayg/calternal-ai/issues/50)

A replayable job can persist a checkpoint and wait for an Inbox answer,
an approval, a signed webhook or a deadline. The host releases the run
slot when the worker returns. `jobs.get` and the job event stream expose
the reason and deadline. The recipient sees questions and approvals
through `requests.list`, which also supplies the job ID for the prompt.
The browser Inbox renders thread-bound requests and needs a Claude client
change to display standalone job requests.

## Store and correctness

- Jobs use their existing per-space FULL SQLite transaction, startup
  recovery, run claim, original session and package revision fence,
  checkpoints, cancellation and brokered effect receipts. The Inbox uses the
  existing account request snapshot, answer, approve and deny operations,
  request fingerprints, fresh approval proof and invalidation feed.
- The wait transition and checkpoint commit together. Delivery commits the
  first matching signal, its replay receipt and readmission together.
  The deadline uses the same update predicate. Cancellation fences pending
  signals and removes unresolved wait data. A duplicate can retrieve a
  receipt without adding a second run. Recovery can re-invoke a worker after
  a crash with the same checkpoint and signal. Receipt-backed host effects
  replay through their existing checkpoint scope.
- Jobs.db moves from 2 to 3 through the shared migration runner. The original
  jobs table shape and writes are preserved. Older servers cannot service
  signal waits, so production activation requires exact release-pair testing
  and a rollout decision. No auth.db layout changes.
- At most 32 parks per space, eight per package in a space, 64 runs per job
  and seven days per wait. A job fails with a caller-safe explanation if its
  park is refused. Checkpoints are bounded to 64 KiB and encoded signals to
  16 KiB. Resolved waits follow existing final job retention and are erased
  with the account. Decision replays use the configured receipt interval.
- Signed ingress binds the space, package, declared route, job, correlation
  nonce, time and body. The host uses the existing audited space secret,
  validates the manifest's cached input schema, and rechecks activation and
  the submitting session before signaling.

## Reads and flushes

Park uses one per-space FULL transaction with a bounded quota read, a
fenced checkpoint update and a wait insert. Webhook parks also check the
correlation key. A deadline task performs one indexed pending check and
uses a clock timer without polling SQLite. A delivered decision uses one
FULL transaction with receipt read, conditional outcome update and
readmission. A replay performs the receipt read without a write. Inbox
invalidations use the existing separate account commit. A resumed job
with a saved checkpoint has one indexed outcome read. Waiting summaries
are looked up only for rows in the parked state.

## Verification

- Dev-02 development preflight inside `nix develop`. Three store tests
  passed for forward migration and transaction rollback, signal replay
  after restart, and quota release on cancellation. Eight HTTP job tests
  passed, including four for approval, deadlines, typed answers and
  signed webhook replay. The initial HTTP run failed until the changed
  test worker and official bundles were pinned to their generated digests.
- The dev-01 slot 1 run at `c5aaf3de` passed the plugin build and both
  formatting checks. The Rust workspace passed 671 tests with 2 ignored.
  Plugin suites passed 32 tests with 1 skipped. Both integration tests and
  all 141 serial conformance tests passed. The generated contract and CLI
  contract checks passed. Podman, Firecracker and provider sign-in cleanup
  checks passed. The strict source scan reported the same 48 package
  references as its merged baseline.
- The initial docs run exposed an absolute target-directory issue, a missing
  Git origin in the verification checkout and undocumented SDK fields. The
  corrected `npm run docs:site` and `scripts/check-docs.sh` passed on dev-02
  at `dd62cc00` with 356 documentation sources and 357 pages built. The
  docs tip includes the subsequent account follow-up from dev. A full
  runtime gate for that tip is pending.
- A quoting error invoked the local docs script during retry setup. That
  command was stopped during the SDK build and the dependency directories
  it created were moved to the system Trash. All reported verification
  came from the development VMs.
- The merge with `stack/night-1` at `893a5965` rebuilt the worker pins
  and passed all eight focused jobs HTTP cases on dev-02. The standalone
  Inbox renderer remains a Claude client follow-up.
- The dev-01 author kit at `6342052a` passed the standard smoke and a
  fresh draft's deadline wait. The draft exposed its checkpoint while
  parked and returned the same checkpoint with a deadline signal on
  resume. Both runs removed the server, runner, containers, listener
  and disposable data. The initial kit setup failed because the isolation
  helper linked schema 11. The refresh now builds the helper and server
  from the same source.
- Follow-up work on scoped reactive values and installed-package
  diagnostics is recorded in the
  [settings and diagnostics review](plugin-settings-diagnostics-2026-11-18.md).

## Simplification opportunities

Store the Inbox form once. The current two readers share a wait row, but
the form appears in both its signal and presentation data. A narrower
persisted signal descriptor could remove that duplication.
When replacing a scaffold's starter operation, update its generated slot
and README together. A job-oriented scaffold could omit the starter slot.
