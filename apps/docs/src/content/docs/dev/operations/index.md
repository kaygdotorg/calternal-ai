---
title: "Operation catalogue"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Generated operation schemas. Capability and risk are shown only when the catalogue declares them; absence is not a grant.

- [spaces.list](./spaces-list/): List current account memberships
- [spaces.create](./spaces-create/): Create an owned space
- [spaces.members](./spaces-members/): List current members
- [spaces.set_role](./spaces-set-role/): Change an existing member role
- [spaces.remove_member](./spaces-remove-member/): Remove a member
- [spaces.leave](./spaces-leave/): Leave after transferring ownership if necessary
- [spaces.transfer](./spaces-transfer/): Transfer ownership to an existing member
- [spaces.delete](./spaces-delete/): Delete directory records; content cleanup is a later step
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
- [notes.list](./notes-list/): List up to 100 live notes, optionally under a tag
- [notes.delete](./notes-delete/): Move a note to its space's Trash
- [notes.restore](./notes-restore/): Restore a note from its space's Trash
- [trash.list](./trash-list/): List notes in the space's Trash
- [trash.purge](./trash-purge/): Permanently delete a note from Trash
- [notes.rename](./notes-rename/): Rename a note at its current revision
- [notes.lookup](./notes-lookup/): Find notes by title prefix
- [tags.list](./tags-list/): List tags and their current note counts
- [tags.lookup](./tags-lookup/): Find tags by name prefix
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
- [requests.list](./requests-list/): List requests waiting on the signed-in account
- [requests.ask](./requests-ask/): Ask the person a question from the current turn
- [requests.answer](./requests-answer/): Answer a pending question
- [requests.approve](./requests-approve/): Approve with a fresh verified passkey
- [requests.deny](./requests-deny/): Decline a pending approval
- [space.instructions](./space-instructions/): Read or replace standing space instructions
- [agent.settings](./agent-settings/): Read or set your account defaults or thread retry settings
- [machines.resume](./machines-resume/): Explicitly resume your interrupted machine lease
- [machines.set](./machines-set/): Set direct or sandboxed execution on your own machine
- [agent.check](./agent-check/): Check the selected provider installation and sign-in in your sandbox
- [agent.run](./agent-run/): Send a prompt to the selected provider in a machine sandbox
- [agent.events](./agent-events/): Read ordered live turn events and the durable checkpoint
- [agent.discard](./agent-discard/): Explicitly discard a stopped turn workspace, retaining its account home
- [agent.stop](./agent-stop/): Cancel your turn, or a turn you can manage
- [agent.resume](./agent-resume/): Explicitly resume an interrupted turn from its checkpoint
- [agent.mode](./agent-mode/): Change a thread permission mode and rotate its token
- [access.request](./access-request/): Ask for a named app scope in this thread
