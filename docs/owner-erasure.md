# Owner erasure

`POST /api/v1/auth/account/delete` deletes the signed-in account with a recent
passkey assertion. Send an empty JSON object for self-deletion. A signed-in
server administrator can send `{"account_id":"ACCOUNT_UUID"}` to delete
another account. The last active server administrator must first transfer
administration. Use the normal same-origin session and JSON request rules.
For CLI self-deletion the token must identify a browser or native installation
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
Memberships are removed from retained shared spaces. Their history can still
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
   placement authority and fences active leases.
4. The existing recovery coordinator compacts superseded frames containing
   removed authority. The pending flags clear in a protected commit. The
   erasure inventory retains only minimal erased UUID records and existing
   key fingerprints.

A crash after the first SQLite commit can recover through its witnessed
proposal. Startup restores the closed owner set and completes pending
cleanup before admitting the HTTP listener. Replaying key unlink, catalog
deletion, directory unlink or the final progress marker is safe.
`spaces.delete` uses the same lifecycle and recovery path.

The server revokes enrollment and access for machines that are no longer
owned by a live account. Independently retained provider homes on an offline
machine and exports outside the installation are under their operator's control.
