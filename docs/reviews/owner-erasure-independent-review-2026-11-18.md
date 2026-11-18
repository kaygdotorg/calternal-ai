# Owner erasure: independent review

Independent Claude review of owner erasure and account deletion at `2971c11`
(commits `dd8cc13` and `2971c11`), for [issue #44](https://git.kayg.org/kayg/calternal-ai/issues/44).
The review reads the code against [owner erasure](../owner-erasure.md), the lane report
[owner-erasure-2026-11-18.md](owner-erasure-2026-11-18.md) and its fence inventory,
the secrets design in [plugin-api.md](../plugin-api.md), and the revoke/publication fence in
[morning-decisions.md](../morning-decisions.md). The failing tests are on
`lane/erasure-review`. No product code changed.

## Findings, by severity

| # | Severity | Gap | Failing test |
| --- | --- | --- | --- |
| 1 | High | Space content and provider sign-ins left on machines after erasure | Source finding, no test (see below) |
| 2 | Medium | A waiting lease is assigned to a machine after its space closes | `waiting_lease_is_not_assigned_after_its_space_closes` |
| 3 | Medium | An exec, with its scoped credential, is admitted for a closed space | `exec_is_refused_after_its_space_closes` |
| 4 | Low | A retained journal handle still reads a closed space | `retained_journal_reads_are_refused_after_owner_close` |
| 5 | Design | An administrator deletes another account without a fresh passkey | Not a test: the documents do not settle the expectation |

### 1. Machine-side erasure is incomplete (High)

Thread workspaces and the account's provider login home live on the machine under
`accounts/<account>/home` and `accounts/<account>/threads/<thread>`
(`crates/machine/src/sandbox/podman.rs:147`, `direct.rs:83`). A provider turn ends with
`Stop { keep_files: true }` (`crates/server/src/machines/provider.rs:129`, `state.rs:461`), so
its workspace stays on purpose. `Down::Discard` removes one thread workspace and keeps
the account home (`crates/machine/src/protocol.rs:270`). No message removes an account home.

`retire_owner` (`crates/server/src/machines/erasure.rs`) does not fill this gap:

- **Space erasure** ends the space's *live* leases with `keep_files=false`. Workspaces of
  earlier, finished turns in the deleted space are never discarded. They hold a working
  copy of the space's files.
- **Account erasure** revokes the account's own machines and ends its leases. On **pool**
  (operator) machines that ran the account's turns, the provider login home and every
  thread workspace stay. `DELETE FROM workspace_discards WHERE account=?1` also drops
  any discard already queued for that account, so not even those are delivered.

[owner-erasure.md](../owner-erasure.md) leaves to the operator only "independently retained
provider homes on an offline machine". Online pool machines that the server
controls are not in that exclusion. The person's provider credentials survive their deletion on
installation-run hardware.

There is no test because `retire_owner` and `erasure::finish` are crate-private. A
reproduction needs the conformance harness with a pool machine. The test would run a turn on
a pool machine, delete the account, then assert that the machine's next sync
carries a discard for the account home and each thread workspace. Fixing this needs a protocol
message, which is a design point (D1).

### 2. Late assignment between the close and `retire_owner` (Medium)

`erasure::finish` drains owner permits and only then calls `retire_owner`. `place_waiting`
runs on every machine `sync_begin` under the registry lock. It takes no owner permit for
the lease's account or space (`state.rs:395`). A lease admitted before the protected close is
still assigned to a machine that syncs in that window. The machine then gets `Down::Assign`
with the space and account and starts a sandbox. The window lasts as long as the
slowest admitted step that drain waits for. `reconcile` and `outbox` (envelopes queued before
the close) have the same boundary. The lane flagged this as unsure, and the test confirms it.

Test: `crates/server/tests/machines_service.rs::waiting_lease_is_not_assigned_after_its_space_closes`
fails with the `Assign` it received.

### 3. Exec admitted for a closed space (Medium)

`MachineService::exec_input` (`crates/server/src/machines/execs.rs`) checks only the lease
state and its account. It queues and sends `Down::Exec` for a closed space until
`retire_owner` ends the lease. The exec env carries the scoped Calternal credential
(doc comment on `exec`). `output()` already takes both permits, so the gap is on the input side only.

Test: `crates/server/tests/machines_service.rs::exec_is_refused_after_its_space_closes`.

### 4. Retained journal handle reads after the close (Low)

`SpaceStore::since` and `idempotency` (`crates/store/src/lib.rs:652-667`) open read-only
connections without a fresh owner permit. A handle obtained before the close still returns
the space's journal entries (titles and metadata) after `live_journal` already reports the
space as absent. The live route rechecks session and membership on each delivery
(`crates/server/src/live.rs:208`), so a person's stream is cut. This is defense in depth
for host code holding a handle, and it contradicts the inventory's "closed owner is
reported as absent". The Notes Trash probe (`has_expired`) is the same class. It only reads
an eligibility bit and its purge is fenced, so it is not listed separately.

Test: `crates/content/tests/lifecycle.rs::retained_journal_reads_are_refused_after_owner_close`
fails with "closed space journal still readable: 1 entries".

### 5. Administrator deletion needs no fresh proof (design point)

`Auth::erase_account` (`crates/auth/src/erasure.rs`) requires `check_fresh` (browser or
native installation, recent assertion) only when `actor == target`. An administrator deleting
another account needs only `Capability::PeopleManage` and a live installation of **any** kind,
including `cli` and `agent` (`schema.sql:35`). No code path issues `agent` installations
today, so this is latent. Still, a stolen administrator cookie or CLI token can irreversibly
erase any account without proof. See D2.

## Checked and sound

- **Fence coverage**: the inventory's 21 entries match the code. Every `content.db`, journal,
  collection, `input.db`, `agent-settings.db`, `jobs.db` and `secrets.db` opener takes a
  permit before opening, and retained writers re-admit per commit. Nothing is missing
  from the inventory beyond items 2–4. The feed reader is sound: `Reader::page` re-admits,
  and closing after tree removal checkpoints into the unlinked inode. It cannot recreate
  the directory, because SQLite creates no parent directories.
- **Read-only opens** cannot recreate a removed tree: a missing file fails to open
  and no parent is created. They can read during drain (item 4).
- **Restart ordering**: `restore_fence` closes every recorded UUID before secrets, jobs
  and machine binding open. `replay` runs before job recovery and `axum::serve`. Completed
  records stay closed forever. Every cleanup step tolerates a repeat (`NotFound` is ignored,
  and the `pending=1` guard makes the final update idempotent). A crash during replay repeats it.
- **Credentials at intermediate points**: installations are deleted in the auth commit and
  their cache entries are forgotten in its callback. `agent_access` and `agent_approvals` rows
  cascade from `users`. Machine `authenticate`, `session_finish` and `sync_begin` take the
  owner permit, which is closed at commit. A deleted space or account UUID cannot be
  re-inserted (`prohibit_erased_*_id`).
- **Shared-space rule race**: the owned-space check, membership and group reads, and every
  delete run in one `IMMEDIATE` auth transaction. Joins, group changes and transfers are
  written in the same database, so they serialize. The owner is always a member
  (`FOREIGN KEY(id,owner_account) … DEFERRABLE`), so no owned space escapes the check.
- **Existence hiding**: a non-administrator gets the capability refusal before the target
  lookup, so a missing account and another person's account look the same. Only an
  administrator sees `not found`.
- **Last administrator**: protected by `last_people_manager` in the same transaction.
- **Plugin host and operations**: account deletion is an HTTP route only, not an operation,
  so a plugin host call or operation dispatch cannot reach it. `spaces.delete` is
  `Irreversible` and goes through the agent reviewer's once-only approval.
- **Recovery log content**: `owner_erasures` holds only kind, UUID and pending. Key
  fingerprints stay in `owner_secret_keys`. `finish_erasure` compacts superseded frames
  before marking an erasure complete.

## Could not check in the time

- Readers that dereference a deleted Actor in retained shared spaces (author display,
  history, receipt replay). The conformance case reads the remaining member's content
  successfully, but I did not trace every author join.
- A conformance-level crash during `replay` with a pool machine attached.
- Other accounts' account-input drafts that reference an erased *private* space. A private space
  has no other member, so this should be empty.

## Design points the documents do not settle

- **D1. Machine-side erasure.** (a) Add `Down::EraseAccount{account_id}` and a space-scoped
  workspace discard, queued durably and retained until the machine acknowledges, like worker
  stops. This costs a protocol version and one row per affected machine. (b) Keep
  `workspace_discards` for erased accounts and space threads instead of deleting them,
  and add a home discard. The change is smaller but replays on reconnect. (c) Document the residue
  as the operator's responsibility for pool machines too. This costs nothing in code but
  contradicts "erasing … makes old data unreadable". Recommend (a).
- **D2. Administrator proof.** (a) Require `check_fresh` for the administrator as well.
  It costs one row read in the existing transaction and matches self-deletion. (b) Accept
  browser and native installations only, without freshness. (c) Leave as is. Recommend (a).
- **D3. Invites A created in B's shared spaces** are silently deleted
  (`DELETE FROM invites WHERE created_by=?1`). Options: keep pending invites, or revoke them
  and tell the space's Managers. Neither is decided.
- **D4. Late-window fence.** (a) Take owner permits in `place_waiting`, `reconcile`,
  `outbox` and `exec_input`. This adds per-lease lifecycle lookups under the registry lock, all
  in memory. (b) Call `retire_owner` from the commit callback, before drain. This is cheaper but
  puts a machines.db commit on the auth worker. Recommend (a).

## Commands and results

Ran on `claude@calternal-dev-01-compute02` in `~/Developer/lanes/erasure-review`, inside `nix develop`, with
`CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0`:

```sh
cargo test -j6 -p calternal-content --test lifecycle -- --test-threads=1
# 2 passed; 1 failed: retained_journal_reads_are_refused_after_owner_close
cargo test -j6 -p calternal-server --test machines_service -- --test-threads=1
# 5 passed; 2 failed: exec_is_refused_after_its_space_closes,
#                     waiting_lease_is_not_assigned_after_its_space_closes
```

The tests start no containers, and the leak check found none from this lane's
directory. The lane target was removed afterwards. Only the new tests were run, not the
full workspace and conformance gates, which the coordinator runs.

## Simplification opportunities

- `retire_owner` duplicates the account and space branches for lease and discard deletion.
  A single `matches` predicate over `LeaseRow` already exists, and could drive the SQL too.
- The three in-memory admission points that machines need (item 2 and 3) could share
  `owner_step` and a space twin, instead of open-coding `lifecycle.get().map(...)` at
  each site, as `admission.rs` and `sessions.rs` do now.
- `Content::admit_erasure` and `admit_checked` differ only in receipt handling. One
  admission with a receipt policy flag would remove a copy.
