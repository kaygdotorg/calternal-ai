---
title: "Spaces and roles"
---

<!-- Generated from: docs/glossary.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

- **Account**: one person's identity on a server. Signs in with a passkey. Owns nothing by itself.
- **Space**: the unit of ownership and access. Holds files, apps and members. Everything is in a space.
- **Role**: Reader, Editor, Manager or Owner in one space. A role is a set of capabilities.
- **Capability**: one named permission, such as `content.write`. Checks ask for a capability, never a role.
- **Owner**: the one account answerable for a space. Changes only by explicit transfer.
- **Group**: a named set of accounts, named after a plan. Decides entitlements such as cloud machine access. Assigned by an invite.
- **Invite**: a single-use, expiring link that creates an account and assigns its groups.
