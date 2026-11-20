---
title: "Glossary"
---

<!-- Generated from: docs/glossary.md. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Terms used across Calternal-AI's documents, issues and reports, one line each. When a
report uses one of these, it links here. Kept by the coordinating thread; add a term the
first time it is used with a new meaning.

## People and access

- **Account**: one person's identity on a server. Signs in with a passkey. Owns nothing by itself.
- **Space**: the unit of ownership and access. Holds files, apps and members. Everything is in a space.
- **Role**: Reader, Editor, Manager or Owner in one space. A role is a set of capabilities.
- **Capability**: one named permission, such as `content.write`. Checks ask for a capability, never a role.
- **Owner**: the one account answerable for a space. Changes only by explicit transfer.
- **Group**: a named set of accounts, named after a plan. Decides entitlements such as cloud machine access. Assigned by an invite.
- **Invite**: a single-use, expiring link that creates an account and assigns its groups.

## Storage

- **appdata**: the server's own databases and keys, on SSD. Never the only copy of a user's file.
- **userdata**: real, readable files, in two trees: `userdata-ssd` for text and `userdata-hdd` for everything else.
- **Root guard**: the startup check that the three storage roots are present, correct and not swapped.
- **Recovery log**: an append-only protected file in userdata recording every change to who has access to what, so access survives losing appdata.
- **Kind**: a type of item, such as `notes` or `threads`. A plugin declares its kinds.
- **Item**: one thing of a kind, with a permanent ID and an address `/<kind>/<id>`.
- **Collection**: a stored list of items with its own ID, addressed as `/collections/<id>`.
- **Journal**: a space's ordered record of changes, which clients follow to stay in sync.

## Plugins

- A **plugin** is an app or extension a person installs and uses. Threads and Notes are plugins. Use this word in interface text, documentation, issues and user files.
- A **package** is the built archive that distributes a plugin, including its manifest and any bundled assets. This technical term is used in the contract and code when referring to the archive.
- **Manifest**: the file listing everything a plugin offers and asks for. Read without running the plugin.
- **Operation**: the only way to do anything. People, the CLI and agents call the same operations.
- **Request ID**: a client-chosen ID on a write, so a retry does not do it twice.
- **Receipt**: the stored result of a write, returned again when its request ID is retried.
- **Worker**: the confined process a plugin's code runs in, one per plugin per space.
- **Host call**: a request from a worker back to the server, checked against the manifest and the caller's access.
- **Hook**: a plugin rule that runs before an operation: it can watch, block, or rewrite content under five rules.
- **Context contribution**: labelled extra context a plugin attaches to an agent's turn. Never changes what the person typed.
- **Component**: a piece of UI described as data and drawn natively by each client. HTML in a sealed frame is the fallback.

## Agents

- **Job**: one piece of long-running work the server tracks, such as an agent's turn. Survives restarts.
- **Stream**: a job's live output. Not stored; checkpoints and results are.
- **Checkpoint**: a job's saved progress, used to resume after a restart.
- **Machine**: a computer running `calternal machine`. It connects out to the server and runs agents.
- **Pool**: the cloud machines the operator runs for people whose group allows it.
- **Sandbox**: the confined environment one agent thread runs in on a machine.
- **Lease**: a machine's claim on a piece of work. A silent machine loses it, and its late output is refused.
- **Person request**: a private question, approval or secret prompt addressed to an account.
- **Secret**: a credential stored encrypted, per account or per space.
- **Handle**: what a plugin or agent holds in place of a secret's value.
