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

For a shared setting, declare a key with `id`, `value_type` and `default` in
`settings`. Add `read_setting_v1` to `host_calls`. In a handler, import
`readSetting` from `@calternal/plugin-sdk` and check the returned value's
type. To put that value in the composer, declare a `composer_row` slot with
`renderer: { "type": "generated", "operation": "<your renderer operation>" }`.
Have that operation return `{ view: { genui_version: 2, source } }`. For a
text chip, build the source as `"Badge(" + JSON.stringify(label) + ")"` so
a quote in the label stays text.

With the Manager connection supplied for the trial, invoke
`plugins.settings.get` with `space_id`, `plugin_id` and `key`. Then invoke
`plugins.settings.set` with the same fields, the read result's `value` as
`base`, and the new `value`. Set `value` to null to reset. Invoke the plugin
view again and check that it reflects the value. Use `plugins.list` and
look for the contribution in `slots`.
