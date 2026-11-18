---
title: "Owner erasure"
---

<!-- Generated from: docs/owner-erasure.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

`POST /api/v1/auth/account/delete` deletes the signed-in account with a recent
passkey assertion. Send an empty JSON object for self-deletion. A signed-in
server administrator can send `{"account_id":"ACCOUNT_UUID"}` to delete
another account with the same recent passkey proof. Both paths require a
person's browser or native installation. The last active server administrator must first transfer
administration. Use the normal same-origin session and JSON request rules.
For CLI deletion the token must identify a browser or native installation
that recently verified a passkey.

A successful response is `{"account_id":"ACCOUNT_UUID","deleted":true}`.
`passkey_reverification_required` means the person must verify their passkey
again. `conflict` identifies shared spaces that need an Owner before the
deletion can proceed. The error includes the space names, UUIDs and transfer
command.

```sh
calternal people transfer --space SPACE_UUID ACCOUNT_UUID
calternal people delete --token-file PERSON_TOKEN
calternal people delete --account ACCOUNT_UUID --token-file ADMIN_TOKEN --json
```

The personal space and owned spaces with no other member are erased.
Memberships are removed from retained shared spaces. Invitations the deleted
account created in those spaces are revoked and remain visible to Managers
under the existing invite retention. Revoked links cannot be redeemed. Their history can still
refer to the erased account's UUID. Both direct membership and membership
through a group count when checking another member. An
administrator is subject to the same ownership rule.

Ownership transfer preserves the shared space and needs a willing current
member. Archiving would need a custodian and a retention policy. Deleting
shared spaces would destroy other members' content. The deletion request
refuses the shared-Owner case until the transfer is complete.

## Authority and storage ordering

1. One protected auth commit revokes the owner, related grants and
   credentials, and records the erased UUIDs as pending. A self-delete
   rechecks proof in that transaction. The commit callback closes the shared
   owner lifecycle and invalidates cached sessions.
2. Outside the auth worker, owner-local durable steps drain. New steps on
   the closed owners fail, including writes through retained journal handles
   and scheduled materialization, jobs, secrets and account input. Existing
   per-space locks continue to serialize space-local writes.
3. Cleanup erases protected keys, unroutes identifiers and removes the
   retired owners' appdata and userdata trees. The same cleanup removes
   placement authority, fences active leases, and durably queues work for
   affected machines.
4. The existing recovery coordinator compacts superseded frames containing
   removed authority. The pending flags clear in a protected commit. The
   erasure inventory retains only minimal erased UUID records and existing
   key fingerprints.

A crash after the first SQLite commit can recover through its witnessed
proposal. Startup restores the closed owner set and completes pending
cleanup before admitting the HTTP listener. Replaying key unlink, catalog
deletion, directory unlink or the final progress marker is safe.
`spaces.delete` uses the same lifecycle and recovery path.

## Machine-side erasure

Space deletion queues one instruction for each known machine workspace in
that space, including workspaces whose finished leases have expired from the
placement history. It preserves pool provider sign-in homes. Account
deletion queues removal of the entire machine-role directory for that
account, including `accounts/<account-uuid>/home` and its thread workspaces.
Existing queued workspace discards for the same owner are carried forward
by the durable erasure instructions.

The server completes local erasure without waiting for machines. When an
enrolled machine reconnects, it gets outstanding instructions and
acknowledges them after removal. A dropped response, reconnect, or machine
role restart can deliver the same instruction again. Transport delivery
alone leaves it pending.

Use `calternal machines list` on the server to see pending counts and the
`Operator wipe` flag for machines that have lost enrollment. Use
`calternal machine status` on the machine to query its pending count without
interrupting its role. When the server cannot be reached, the machine
reports the count as unavailable and the server-side list remains the
authoritative inventory.

The operator must wipe a revoked or permanently offline machine. Stop its
machine role, then use the UUIDs and the configured machine state directory
to identify the machine-role data. In direct mode the relevant paths are
under `<machine-dir>/sandboxes/accounts/<account-uuid>`. For a space
workspace remove the relevant `threads/<thread-uuid>` directories. For an
erased account remove the whole UUID directory, including its sandbox
`home`. A person-owned machine uses the same sandbox-owned paths. The
person's actual home, provider sign-ins outside the role, and other files
remain under their control.

This preview uses auth schema 10, machine database schema 5, and machine
protocol 4. Use fresh development state and matching server and machine role
binaries. Before replacing an earlier preview, stop its enrolled roles and
wipe any retired sandbox roots whose placements were stored by that preview.
