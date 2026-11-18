---
name: calternal-plugin-authoring
description: Scaffold, implement, test and document a TypeScript Calternal plugin.
---

Read the current desired behavior, stable addresses and space scope. Call
`calternal-plugin-guide.explain` with the `reference` and `walkthrough` topics.
Scaffold with `calternal plugin new` in the author's checkout. Use
`calternal-plugin-guide.scaffold` if you need a starter manifest and command
before choosing a checkout.

Keep product logic in the worker. Make durable changes through declared broker
calls. Use request identities for retriable mutations. Add an accessible GenUI
view through the existing catalog where the feature needs an interface.

Document each operation and its risk. Run `calternal plugin check`,
`calternal plugin test` and, when relevant, `calternal plugin bench`. Verify
both the expected result and a denied callback using disposable fixtures.
Present the exact capabilities to the administrator for review.

For live testing, use the disposable development server and space supplied with
the task. Its server and CLI must be built with the `dev-plugin-drafts`
feature and the server must already be running with `--development` and an
enrolled worker machine. Run:

```sh
calternal plugin dev . --appdata "$APPDATA" --space "$SPACE_ID" --account "$ACCOUNT_ID" --once
calternal invoke my-plugin.hello --input "{\"space_id\":\"$SPACE_ID\"}"
```

Use the session connection supplied with the task. Confirm the response has
the expected generated view. Use continuous `plugin dev` by omitting
`--once`. A failed check or readiness response leaves the last ready draft
in place. Read its diagnostic and try the edit again.
