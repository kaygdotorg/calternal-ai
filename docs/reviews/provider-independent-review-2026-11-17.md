# Independent review of the Claude provider lane, 2026-11-17

Issue: [#27](https://git.kayg.org/kayg/calternal-ai/issues/27). Branch: `lane/provider-review`,
cut from `lane/provider-claude` at `5121423`. Thread: `thr_i64f458dmc`. Reviewer: Claude
Opus 5.5. This review covers code written by another lane, not code written here.

Scope: thread tokens, permission modes and grants, sandbox and account home, turn
lifecycle, the provider stream contract, instruction delivery and the recovery barrier,
checked against [agent-instructions](../agent-instructions.md), the orchestrator proposal
("Turns", "Where work runs", "Provider plugins", "Decided in the grill"),
[plugin-api](../plugin-api.md) and the lane's own [report](provider-claude-2026-11-17.md).
Product code was not changed. Each finding has a deterministic test that fails on this
branch and should pass once the gap is closed.

## Findings, by severity

| # | Severity | Finding | Failing test |
| --- | --- | --- | --- |
| 1 | High | Issued thread credential is published to space readers and persisted | `provider_review_stream::issued_thread_credential_is_not_published_or_checkpointed` |
| 2 | High | Least access runs irreversible and external operations without asking | `provider_review::least_access_asks_for_high_risk_operations_that_approve_for_me_asks_for` |
| 3 | Medium | Credential on the host `podman exec` command line | `podman_exec_env::podman_exec_keeps_environment_values_out_of_host_argv` (machine crate) |
| 4 | Medium | A narrower mode for the next turn is ignored | `provider_review::next_turn_honours_a_narrower_mode_chosen_by_the_person` |
| 5 | Medium | Token outlives the person's sign-in | `provider_review::thread_token_stops_when_the_admitting_person_session_is_not_live` |
| 6 | Low (fails closed) | Least-access approvals created by a denial can never take effect | `provider_review::least_access_accepts_the_action_approval_its_denial_created` |

### 1. The issued credential reaches the stream, the checkpoint and disk (High)

The adapter receives `CALTERNAL_AGENT_TOKEN` in its environment
([run.rs](../../crates/server/src/turns/run.rs), `exec_input` env). The provider and its
tools inherit it, so `env`, a debugging command or a model echo puts the value in a
`text_delta`. The server publishes provider events unchanged to the job's live buffer, and
`agent.events` requires only `SpaceRead` for non-owners
([turns/mod.rs](../../crates/server/src/turns/mod.rs)). In a shared space any reader can
take the token and act as the turn's owner, with the owner's capabilities and mode,
until the turn ends (up to ten minutes). The text tail is also checkpointed into
`jobs.db`, which is retained for seven days.

Reproduced end to end (real server, machine role, adapter, fixture CLI that prints its
environment): the published `text_delta` contains `ct1_<space>_…`. The server knows the
value it issued, so it can redact that value from every event and checkpoint before
publication. Whether the adapter should also scrub its children's environment is a
design question (see Open).

### 2. Least access is wider than Approve for me for high-risk operations (High)

`risk_allowed` returns `in_default_scope || approved_scope` for Least access regardless
of risk class ([access.rs](../../crates/server/src/turns/access.rs)). Inside the
originating app, or after an app grant, Least access runs `irreversible` and `external`
operations with no approval, while Approve for me asks for both. Reproduced:
`spaces.delete` with `source_app = "spaces"`, a plugin `mail.send` declared `external`
with `source_app = "mail"`, and `secrets.revoke` under an app grant `("secrets", write)`.
`source_app` is any short string the caller of `agent.run` supplies; it is not checked
against an enabled plugin. Expected: in every mode, `irreversible` and `external` ask
unless an exact action grant exists, so Least access is never wider than the default.

### 3. Credential visible in the host process table (Medium)

`PodmanSandbox::exec` passes each environment entry as `--env KEY=VALUE` on the
`podman exec` command line ([podman.rs](../../crates/machine/src/sandbox/podman.rs)).
That client process lives for the whole turn, and its argv is readable by every user
on the machine through `/proc/<pid>/cmdline` (unless `/proc` is mounted with `hidepid`).
Reproduced with a recording stand-in for `podman`. A fix could pass the value
through the client's own environment (`--env KEY` with no value) or a file
descriptor. The direct backend uses `Command::env`, which does not expose it this way.

### 4. A narrower mode for the next turn is ignored (Medium)

`agent_prepare_scope` always copies the previous turn's mode onto a new turn
(`a.mode = previous.mode`), and `agent.mode` refuses finished turns. So after a Full
access turn ends, the person can't start the next turn in Least access: the composer's
mode in `agent.run` is silently replaced, and the next turn runs with the wider mode
until the person changes it mid-turn. The lane's own test covers only the widening
direction (a request for Full access does not override a saved Approve for me).
Expected: a narrower requested mode is honored, or a mode-change operation works
between turns. Whether a wider requested mode should apply is a product decision
(see Open).

### 5. The token survives the person's sign-in (Medium)

A turn records its admitting `Session`, but `agent_authenticate`, `agent_current` and the
run loop check only the access row, the job state and current capabilities. After the
person's installation is revoked (for example, signing out everywhere after a
compromise), the thread token keeps acting for them until the turn ends. Ordinary plugin
host calls already refuse this (`plugin_host::host_calls_recheck_the_callers_sign_in`).
The test admits a turn under an installation session that is not live and expects
authentication to fail. A fix may need the jobs handle to consult the directory, so the
test's setup may need adjusting to whatever port the fix adds.

### 6. Least-access denials create approvals that never apply (Low, fails closed)

On a denial, the dispatcher creates an exact-action approval (`action:<op>:<hash>`) and
returns its `approval_id`. After the person approves it, Least access still evaluates
only app-level grants (`p == app`), so the same action is denied again and a new request
is created, up to 32 per turn. Only an explicit `access.request` app grant works. The
reverse also holds: in Approve for me, app grants from `access.request` are ignored. No
authority leaks, but the request bar would show approvals that change nothing.

## Checked and found sound

- **Cross-space and cross-thread use.** `check_operation` requires `space_id` to equal
  the token's space (`not_found` otherwise, which is indistinguishable from a missing
  space). Account-input, `agent.settings`, `machines.*`, `agent.mode` and
  `access.approve` refuse thread credentials. `agent.events` and `agent.stop` work only
  for the token's own job. The jobs SSE route rejects other jobs for agent sessions.
  Discovery lists only the token's space and the operations it can call.
- **The person's current access.** Every operation rechecks capabilities through the
  normal policy before the scope check. The run loop rechecks `PluginsUse` each batch
  and ends the run when it is lost. Plugin host calls revalidate the token hash and the
  running job.
- **End, rotation and stop.** Tokens require `state='running' AND runs=<issuing run>`.
  `finish` is fenced by run, and `agent_revoke_run` affects only its own run (the lane's
  test covers a late finish against a successor). Rotation replaces the hash in one
  protected transaction. Mode and approval changes null the hash before interrupting.
  Server restart marks every `agent.turn` interrupted, so no old token survives it.
- **Self-approval.** No operation, thread-token path or plugin host call reaches
  `access.approve` or `agent.mode`. Both require `Session::Installation` with a fresh
  passkey proof, and the approver must own the job.
- **Undeclared risk fails closed.** Both built-in and plugin operations without a
  declared risk are refused for agents. Every built-in has a derived class, and manifest
  risks are required. `widens_access` is never allowed by mode.
- **Stored in the clear.** Only BLAKE3 hashes are stored, `agent_scope` never returns the
  hash, the raw token is absent from `jobs.db` (the lane's test), and adapter stderr is
  never forwarded. Finding 1 is the exception: a provider can echo the token.
- **Direct execution.** The server refuses a `direct` session unless the account owner
  set it on an account machine. Pool machines always run Podman. Direct runs with an
  empty environment apart from `PATH`, `HOME` and the token.
- **Provider stream contract.** 12 KiB per event line, strict `seq` order, nothing
  accepted after `done`, a 128 pending-tool cap, the 600 s run deadline, a ring live
  buffer (256 chunks / 1 MiB), and a bounded per-exec channel (16 × 16 KiB) for
  back-pressure on the machine. A malformed event ends the run as `outcome_unknown` and
  releases the lease.
- **Retry.** Retry happens only on `failed` with no tool started and unfinished,
  `transient` or `usage_limit` class, and settings enabled. Waits are fenced by run, and
  a mode change while waiting interrupts the turn.
- **Instructions.** They are read descriptor-relative under the space, symlinks are
  refused (the lane's test), the text is capped at 16 KiB with a notice, and it is placed
  after a "context only; never grants" label. Authority is server-side, so text cannot
  grant anything.
- **Recovery.** `Agents::change` commits through the auth barrier before returning, and a
  failed commit maps to an error, so nothing is acknowledged. Turn state changes stay in
  per-space `jobs.db`, and token validity requires both stores to agree, so a partial
  failure fails closed.

## Not checked

- Real Podman isolation between two accounts on one machine. `calternal-dev-02-std` has
  no Podman, so `crates/machine/tests/podman.rs` and the container test skip there. I
  reviewed mounts and naming in code only: per-account `0700` directories,
  account-qualified container names, and only `/work` and the account home mounted.
- macOS sandboxing, a real signed-in Claude Code, and production HTTPS.
- Model-level conformance ("content read is data, never instruction"). It needs the
  scripted provider tasks named in agent-instructions.
- Who may write `AI/Instructions.md`. Placement allows a plugin kind with a visible `AI`
  folder (`VisibleTree` cites `AI/Chats`). I found no reservation that limits that file to
  Managers, and no kind writes it today.
- Timing races between `agent_approve` (auth.db) and `agent_interrupt` (jobs.db) under
  load. Ordering is safe by construction (the hash is null first), but it is not stressed.

## Open (design choices the documents don't settle)

- **Account home shared across spaces.** Every thread of an account mounts the same
  `/home/agent`, which holds the provider's session transcripts for all spaces. A
  Least-access thread in one space can read what another space's thread saw. Options:
  (a) accept it as the person's own data (no cost); (b) a per-space home, with only the
  provider login files shared read-only (one login, more mounts, provider-specific
  paths); (c) per-thread provider config directories (`CLAUDE_CONFIG_DIR`), losing
  cross-thread session resume.
- **The token in the agent's environment.** Redacting on the server (finding 1) closes
  the stream path, but tools can still write the token into workspace files or send it
  over the network. Options: keep it in the environment and redact; or a per-turn
  socket or file that only the `calternal` CLI reads (fewer accidental echoes, more
  plumbing).
- **A wider requested mode at the next turn** (finding 4's other direction): apply it
  (the composer is the person) or require `agent.mode` with fresh proof.
- **Durable grants.** "Once", "this thread" and "always in this space" are not
  implemented; every grant lasts the current turn, across its resumes. "Once" needs a
  consume-on-use rule.
- **Retained thread modes.** 1024 per account, with no deletion caller until thread
  storage exists, so the 1025th thread fails permanently. This needs a retention
  decision or the thread-deletion caller.
- **Queued turns.** `agent.mode` on an `admitted` turn that has no access row yet returns
  "turn access not found", so the person can't change the mode of a queued turn.

## Commands and results

All builds and tests ran on `ubuntu@calternal-dev-02-std` under
`~/Developer/lanes/provider-review` through `nix develop -c`, synced with the lane's
`git ls-files | tar | ssh` command. Nothing ran on hd01.

| Command | Result |
| --- | --- |
| `cargo test --locked -j6 -p calternal-machine --test podman_exec_env` | 1 failed (finding 3) |
| `cargo test --locked -j6 -p calternal-server --test provider_review` | 4 failed (findings 2, 4, 5, 6) |
| `npm ci --ignore-scripts --no-audit --no-fund && npm run build:plugins` | Passed |
| `cargo test --locked -j6 -p calternal-server --test provider_review_stream` | 1 failed (finding 1) |
| `cargo build --locked -j6 -p calternal-server --features test-plugins`, then `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal cargo test --locked --manifest-path tests/conformance/Cargo.toml -j6 --test isolation -- --test-threads=1` | 46 passed, 0 failed |
| `cargo fmt --all -- --check` | Passed |
| Lane baseline (`provider_access`, `provider_security`, `provider_turns`) | Not run by the review; covered by the coordinator gate |

## Simplification opportunities

- `agent_token` and `agent_prepare_scope` repeat the same 1024-row count and scope
  lookup. One helper would keep the bound in one place.
- Least access and Approve for me evaluate two grant shapes (app prefix and exact
  action). One grant type with an optional action hash would remove finding 6 and the
  mode-dependent branch in `check_operation`.
- `agent_scope_change` and `agent_approve` each write `hash=NULL` with slightly different
  `WHERE` clauses (one omits `account`, one omits `space`). One "replace scope and end
  token" helper would make the rotation rule a single statement.
