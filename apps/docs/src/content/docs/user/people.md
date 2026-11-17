---
title: "People and access"
---

<!-- Generated from: docs/people.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Space and server authority are separate. The first account has the server
administration capabilities. Holding those capabilities gives no access to
another person's space. Space roles and capabilities come from the installed
role catalog. Read it with `roles.list`.

## Space grants

`spaces.members` shows direct accounts with their current display names and
direct group grants with each group's name and active member count. It does not
expand groups into individual accounts. `spaces.add_member`,
`spaces.set_role`, and `spaces.remove_member` take a space UUID and exactly one
`account_id` or `group_id`. Add and set-role also take a role. An account can
exercise capabilities from all its active direct and group grants. Removing a
grant removes only the access that grant provided. A person with access only
through a group cannot leave via `spaces.leave`.

A Manager can manage ordinary grants. Only the Owner can appoint or remove
Managers. There is exactly one Owner. `spaces.transfer` can appoint a current
account in the space, including an account with group-derived access. The former
Owner becomes a Manager. A group cannot own a space.

A space Manager can grant access only to a visible account or group. Accounts
can see their own identity and people with whom they share a group. Server
people managers can see all accounts. An unknown space and a space hidden from
the caller both return `not_found`. Group management requires its own server
capability.

## Invitations

`invites.create` issues one link for a space and one assignable role. Supply
`expires_at` in Unix milliseconds to choose an expiry, or omit it for seven
days. The creation receipt carries the UUID and expiry. Use `invites.list` to
copy the active link and `invites.revoke` with the record UUID to revoke it.
Managers can copy or revoke their own links. The Owner can manage all links
in the space and is the only person who can issue a Manager invitation. Treat
the link as a credential. The server sends no referrer on its invite page.

If you already have an account, sign in and call `invites.redeem` with the
secret from the link. The grant can widen a direct role and never reduces it.
If you are new to the server, pass the secret to the regular passkey enrollment
flow at `/api/v1/auth/setup/start` and finish with
`/api/v1/auth/setup/finish`. Starting a passkey prompt leaves the link usable.
Successful enrollment consumes it and creates the account, its separate
personal space, the destination-space grant, the passkey, and the session in one
protected commit.

A consumed, revoked, expired, or unknown invitation receives the same
redemption error. An exact retry of an authenticated redemption can replay its
private receipt while the account retains space access. Invite secrets are keyed for lookup and encrypted at rest
using the installation's private `system/invite.key`. Back up that key with the
installation state. Receipts and recovery records never carry plaintext links.
Expired invitation records disappear from the list and are pruned on the next
creation in the same space.

## Account and server directory

`account.set_profile` edits the signed-in account's display name. Login
identity is unchanged. The same Unicode and look-alike checks used for
enrollment apply to profile edits.

Server people administrators can call `people.list` and `people.disable`.
Disabling an account revokes its installations and makes its grants inactive.
The account and content remain recorded. The last active people administrator
cannot be disabled.

Server group administrators use `groups.list`, `groups.create`,
`groups.rename`, `groups.delete`, `groups.add_member`, and
`groups.remove_member`. These actions edit local groups. Deleting a group
removes its memberships and grants without deleting accounts or content.
Membership and role changes take effect on the next authorization check.

See the [CLI reference](/docs/admin/cli/) for commands and credential options.
