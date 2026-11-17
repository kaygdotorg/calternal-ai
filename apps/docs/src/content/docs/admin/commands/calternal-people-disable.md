---
title: "calternal people disable"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/people-disable-width-120.txt, crates/cli/tests/golden/people-disable.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people disable ACCOUNT` | Disable the account and sign out its sessions. | Account ID and disable confirmation. | `account_id`, `disabled` |

## JSON data shape

```json
{
  "account_id": "string",
  "disabled": "bool"
}
```

## Human example

Recorded test fixture:

```text
Account disabled.
account:  01912345-6789-7abc-8123-000000000001
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "account_id": "01912345-6789-7abc-8123-000000000001",
    "disabled": true
  },
  "ok": true,
  "request_id": "people-example"
}
```
