---
title: "Operation catalogue"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Generated operation schemas. Capability and risk are shown only when the catalogue declares them; absence is not a grant.

- [spaces.list](./spaces-list/): List current account memberships
- [spaces.create](./spaces-create/): Create an owned space
- [spaces.leave](./spaces-leave/): Leave after transferring ownership if necessary
- [spaces.delete](./spaces-delete/): Delete directory records; content cleanup is a later step
- [notes.list](./notes-list/): List up to 100 notes
- [resolve](./resolve/): Resolve an item address with current access
- [notes.get](./notes-get/): Read a note
- [notes.create](./notes-create/): Create a Markdown note
- [notes.update](./notes-update/): Update a note against an opaque base revision
- [notes.history](./notes-history/): Read bounded note history
- [plugins.list](./plugins-list/): List registered plugins
- [plugins.enable](./plugins-enable/): Enable a trusted plugin
- [plugins.disable](./plugins-disable/): Disable a trusted plugin
- [plugins.preview](./plugins-preview/): Preview a trusted plugin using fixture data
- [notes-insights.summarize](./notes-insights-summarize/): Compute word, character and line statistics
- [jobs.get](./jobs-get/): Read a plugin job's durable record
- [jobs.cancel](./jobs-cancel/): Cancel a plugin job and its active child jobs
- [secrets.request](./secrets-request/): Ask for a secret; a person supplies the value through the trusted prompt
- [secrets.list](./secrets-list/): List personal secret metadata and requests; space metadata requires secrets.manage
- [secrets.revoke](./secrets-revoke/): Destroy a personal secret or request; space management requires secrets.manage
- [spaces.members](./spaces-members/): List direct account and group grants.
- [spaces.transfer](./spaces-transfer/): Transfer ownership to a current account in the space.
- [spaces.add_member](./spaces-add-member/): Grant a person or group access to a space.
- [spaces.set_role](./spaces-set-role/): Change a direct account or group grant.
- [spaces.remove_member](./spaces-remove-member/): Remove a direct account or group grant.
- [roles.list](./roles-list/): List the space roles and their capabilities.
- [account.set_profile](./account-set-profile/): Change your own display name.
- [people.list](./people-list/): List server accounts.
- [people.disable](./people-disable/): Disable an account and revoke its sessions.
- [groups.list](./groups-list/): List server groups.
- [groups.create](./groups-create/): Create a local group.
- [groups.rename](./groups-rename/): Rename a local group.
- [groups.delete](./groups-delete/): Delete a local group and remove its grants.
- [groups.add_member](./groups-add-member/): Add a person to a local group.
- [groups.remove_member](./groups-remove-member/): Remove a person from a local group.
- [invites.create](./invites-create/): Create a single-use space invitation.
- [invites.list](./invites-list/): List and copy current space invitations.
- [invites.revoke](./invites-revoke/): Revoke an unused space invitation.
- [invites.redeem](./invites-redeem/): Redeem a space invitation as the signed-in account.
- [drafts.save](./drafts-save/): drafts save
- [drafts.list](./drafts-list/): drafts list
- [drafts.get](./drafts-get/): drafts get
- [drafts.delete](./drafts-delete/): drafts delete
- [pending.queue](./pending-queue/): pending queue
- [pending.edit](./pending-edit/): pending edit
- [pending.list](./pending-list/): pending list
- [pending.cancel](./pending-cancel/): pending cancel
- [pending.confirm](./pending-confirm/): pending confirm
- [pending.submit](./pending-submit/): pending submit
