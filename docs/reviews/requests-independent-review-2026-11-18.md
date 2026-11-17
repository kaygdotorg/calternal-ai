# Independent review of the requests lane, 2026-11-18

Issue [#36](https://git.kayg.org/kayg/calternal-ai/issues/36), branch
`lane/requests-review` (cut from `lane/requests` at `718146f`), thread
`thr_b3rarakvj8`. Reviewed: [request contract](../requests.md),
[agent turns](../agent-turns.md), [agent instructions](../agent-instructions.md),
the lane's [own report](requests-2026-11-17.md) and the earlier
[provider review](provider-independent-review-2026-11-17.md). Code: `crates/server/src/requests.rs`,
`plugins/requests.rs`, `plugins/agent_access.rs`, `api/request_action.rs`,
`api/dispatch.rs`, `turns/access.rs`, `turns/mod.rs`, `threads/operations.rs`,
`crates/auth/src/schema.sql`, `crates/content/src/instructions.rs` and `files.rs`,
`crates/secrets` and `server/src/secrets.rs`, and the conformance cases.

No product code was changed. Each finding has a deterministic test that fails
on this branch.

## Findings, by severity

### 1. A this-thread grant lets an unreceipted irreversible action repeat (Medium)

Test: `crates/server/tests/requests_review.rs`
`a_this_thread_grant_for_an_unreceipted_irreversible_action_does_not_authorize_later_turns`.

`requests.approve` refuses `once` for `secrets.revoke`, because the operation has no
durable receipt (`supports_once`). That leaves `this_thread` as the only
approval the person can choose. The grant binds the operation, the input and the
original request ID (`action_scope`), and `with_grants` loads it into every
later turn of the thread until the thread is archived or deleted. With no receipt,
each replay of that request ID is a new execution. After the person provides the
secret again, a later turn can revoke it again without asking. Approve for me
says irreversible actions ask. Once was refused because a replay can't be fenced,
and this-thread allows exactly that replay with no bound.

The same grant shape covers receipted irreversible and access-widening actions
(`spaces.delete`, `spaces.set_role`, `spaces.transfer`). There, a replay of the same
request ID returns the stored receipt until the receipt window ends
(`CALTERNAL_RECEIPT_RETENTION_DAYS`, 7 days). After that, the docs treat the
retry as a new request, and the retained grant still matches it. For example,
the agent could promote a member again after an Owner demoted them. I found this
by reading the code; the test covers the unreceipted case.

Expected: an irreversible or widening approval authorizes one execution. Either
`requests.approve` accepts only `once` for those risk classes, or a this-thread
grant of that class is consumed on use. See the design point below.

### 2. Approving a stopped turn's request strands the running turn (Medium)

Test: `tests/conformance/requests_review_cases.rs`
`approving_a_stopped_turns_request_does_not_strand_the_current_turn`.

`agent.stop` cancels the turn and revokes its token, but leaves its pending
requests in the inbox. If the person approves one later, `agent_decide` joins the
thread's current `agent_access` row and calls `replace_scope`. That sets
`hash=NULL` on the token of the turn that is currently running. The dispatcher
only interrupts the request's own job (`route.job`), and that job is already
cancelled. The current turn stays `running` with a dead credential: every
`calternal` call returns 401 for the rest of the turn. `agent.resume` accepts
only `interrupted` turns, so nothing delivers a replacement.
Observed: status 401, state `running`.

The same leftover rows also cause these problems:
- An answer to a stopped turn's question is accepted and stored, and never delivered.
- The rows count toward the 32 and 256 bounds until the person resolves them.

Expected: stopping a turn withdraws its pending requests. A decision that
rotates the thread credential also interrupts the thread's current running turn,
the same way the `Running` branch does for the request's own job.

### 3. Leaving a space leaves hidden requests, grants and authority (Low to Medium)

Test: `tests/conformance/requests_review_cases.rs`
`leaving_a_space_erases_the_persons_requests_and_thread_authority`.
`deleting_a_space_erases_its_requests_and_thread_authority` passes, and is kept
as the passing counterpart.

Space deletion cascades through `agent_access.space REFERENCES spaces ON DELETE
CASCADE`. Membership exit doesn't: `spaces.leave`, `spaces.remove_member` and
demotion below `SpaceRead` remove no `agent_access` or `agent_approvals` rows.
The inbox hides rows from spaces the person can't read, and answer/deny return
404, so the person can't clear these rows. Pending rows never age out. The
consequences:
- The rows use up the 256-per-account and 32-per-thread request bounds and the
  1024-per-account authority bound.
- If the person is added back, the old pending requests, this-thread grants and
  saved thread mode all come back.

Observed: 1 request row and 1 authority row after leaving.
Expected: removing access erases the person's protected rows for that space,
in the same commit that removes the membership.

## Checked and found sound

1. **Who can see and answer.**
   - `personal_requests` and `personal_request_route` filter by
     `account = authenticated account`. Other members, Managers, the Owner, an
     admin and other spaces get 404, which looks the same as an unknown ID. The
     lane's conformance case covers this for accounts 0, 2 and 3.
   - A request in a space the person can no longer access is hidden and returns
     404.
   - Thread credentials are refused twice: in `requests::dispatch`
     (`Session::Agent`/`agent` for non-`Turn` rows) and in `check_operation`.
   - `requests.approve` requires `Session::Installation` with
     `require_fresh`. A Static development bearer can answer and deny, but not
     approve.
   - Plugin host calls have no generic operation call (`HostCallKind` covers
     items, jobs, checkpoints and secret handles only), so no plugin path
     reaches `requests.*`. The CLI uses the same operation.
   - A kind mismatch (approving a question, answering an approval) returns 404.
2. **Once.**
   - Claims run in one protected transaction with a conditional
     `UPDATE … claimed_id IS NULL`. Identical concurrent calls also serialize
     on the live job mutex. The lane tests the race and a restart between
     approval and use.
   - Claims are bound to account, space and thread, the app and write
     permission, and the action hash (operation and canonical input, plus the
     request ID for irreversible and widening actions).
   - A second request ID with the same action can't reuse a claimed grant.
   - The same request ID with a different input returns `request_reused`.
   - After a crash between the claim and the action's commit, a retry with the
     same request ID re-executes. That is safe only for receipted writers and
     reads, which is why once is refused for other writers. App-level once
     grants never cover high-risk actions.
3. **This thread.** The grant ends on archive and on delete (`agent_delete_thread`
   in `finish_lifecycle`). It survives mode changes, which the documents allow.
   It can't widen: read grants don't cover writes (`!mutates || g.write`), and
   apps must match exactly. The exception is finding 1.
4. **Waiting turns.**
   - Wake is conditional on `state='waiting' AND person_request=id`. A stopped,
     cancelled, archived or superseded turn doesn't resume, and a second
     decision doesn't start a second run.
   - Parking requires `runs` and `state='running'`.
   - A resumed run gets a new token, and old tokens fail `live_authority`
     because `runs` changed. Resume is pinned to the recorded machine (lane
     test).
   - Archive cancels the waiting job, and delete refuses while a turn is active.
   - When a bound is reached, `agent_wait_request` fails inside the park
     transaction. The transition rolls back, the turn keeps running and the
     agent gets `unavailable`, so it fails safe.
5. **Secret prompts.** `pending()` filters by `created_at > now − 15 min`.
   Values enter only through `/auth/secrets/provide`. The inbox row carries the
   name, label, reason and expiry, and the feed carries only an invalidation.
   Request rows in auth.db never hold the secret.
6. **AI/AGENTS.md.**
   - `space.instructions` requires `space.settings` for both read and write,
     and refuses thread credentials.
   - Item paths, kind folders and moves are checked by `is_reserved_space_path`
     (case-insensitive).
   - Plugin `write_item` goes through the same content placement.
   - The cap is 16 KiB on write and truncation at 16 KiB on read.
   - The write is journaled through the outbox, and the event omits the text.
     A symlink fails closed (lane test).
   - Agent workspaces have no sync into userdata in this tree, so there is no
     such path to check.
7. **Recovery.** Decisions, grants, claims and parking use `Agents::change`, the
   auth recovery barrier, and a failed commit returns an error before any
   acknowledgement. A crash between a decision commit and the jobs wake is
   repaired by `recover_request_waits` at startup. A crash between the request
   insert and the jobs commit leaves a pending request while the turn stays
   `running` with its lease released. Machine silence then interrupts the turn,
   and an explicit Resume reads the resolved outcome. The feed invalidation
   commits after the request, a gap the lane documents.

### Token rotation (question 4)

| Change | Rotates? | What the old token can still do |
| --- | --- | --- |
| Approve (`once` or `this_thread`) | Yes: `replace_scope` sets `hash=NULL` | Nothing. The current turn is interrupted only if it is the request's own job (finding 2). |
| New turn (`agent.run`, `threads.send`), any mode | Yes: `agent_prepare_scope` | Nothing |
| `agent.mode` in flight | Yes: `agent_scope_change` | Nothing. Explicit Resume. |
| Resume, wake after a decision | New token per run | Nothing (`runs` check) |
| Stop, finish, interrupt | Revoked | Nothing |
| Archive or delete thread | Row deleted | Nothing |
| Once consumption | **No** | It can replay the claimed request ID with the same input, which returns the stored receipt (or re-runs a read). It can't use the grant for anything else. Documented as unfinished; the exposure is limited to that replay. |
| Deny, answer | No (no access change) | Unchanged scope |
| Role demotion, leaving, sign-out | No rotation | Capabilities and the admitting session are rechecked on every call, so the token loses the access. Rows persist (finding 3). |

## Not checked

- I didn't run Podman sandbox execution: dev-02 has no Podman. The conformance
  cases use the simulated machine protocol.
- Whether a plugin manifest can declare a non-mutating `external` operation.
  If it can, `supports_once` treats it as a read, and a crash between the claim
  and the read receipt would repeat an external effect.
- Real-device passkey freshness timing; the fixtures use soft passkeys.
- The full workspace gate. The coordinator runs it.

## Design points the documents don't settle

- **Lifetimes per risk class.** Options:
  - (a) Accept only `once` for irreversible and widening actions. This is the
    simplest, but those actions then need receipts to be approvable at all;
    `secrets.revoke` would need one.
  - (b) Consume this-thread grants of those classes on first use. This needs
    the claim path on the grant table.
  - (c) Keep the current behavior and document it as "this request, retried for
    this thread".
- **A once grant left unused by a stopped turn** can be claimed by a later turn
  of the thread, possibly in another mode. The options are to bind it to the job
  (it is lost with the turn) or to the thread (current behavior).
- **Bounds and starvation.** The thread bound of 32 counts every account's rows
  in a shared thread, and it includes this-thread grants that never age out.
  After 32 retained grants a thread can never ask again until it is archived.
  The account bound of 256 includes answered rows for 7 days, so one busy space
  (8 threads × 32) blocks the person's agents in every space. Options:
  - Per-account-per-thread and per-space sub-limits. They cost one more count
    query each.
  - Leaving grants out of the request count, with their own bound.
- **Reading AI/AGENTS.md** requires `space.settings`, but every member's agent
  receives the text in its prompt. Options: `space.read` for reads (matches
  what agents see), or keep it Manager-only (no effective confidentiality).

## Simplification opportunities

- Store a secret prompt's recipient account as a column instead of parsing
  `requested_by` text (`request_actor`). The parse runs on every listing, and
  it breaks if the attribution wording changes.
- Remove a person's requests in one place, shared by stop, archive, delete and
  membership exit. Today, archive and delete use `agent_delete_thread`, space
  deletion relies on a foreign key, and stop and leave do nothing.
- `agent_decide` could interrupt "the thread's current turn" through one query
  on `agent_access.job`, instead of loading `route.job` and checking two states
  in the dispatcher.
- The `supports_once` list and the receipt declaration are two copies of one
  fact, as the lane notes. The risk-class rule for lifetimes could live in the
  same declaration.

## Commands and results

All runs were on `ubuntu@calternal-dev-02-std` (aarch64) in
`~/Developer/lanes/requests-review` through `nix develop -c`, synced with the
`git ls-files … | tar` pipe.

| Command | Result |
| --- | --- |
| `npm run build:plugins && cargo build --locked -j6 -p calternal-server --features test-plugins` | Passed |
| `cargo test --locked -j6 -p calternal-server --test requests_review` | 0 passed, 1 failed (finding 1) |
| `cargo test --locked -j6 -p calternal-server --test provider_review --test provider_access --test provider_security` | 8 passed |
| `CALTERNAL_SERVER_BIN=$PWD/target/debug/calternal cargo test --locked -j6 --manifest-path tests/conformance/Cargo.toml --test isolation requests_ -- --test-threads=1` | Lane's 2 cases passed. Review cases: deletion passed; findings 2 and 3 failed. |
| `cargo fmt --all -- --check`, `cargo fmt --manifest-path tests/conformance/Cargo.toml -- --check` | Passed |
