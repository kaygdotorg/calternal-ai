---
title: "calternal people set-profile"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/people-set-profile-width-120.txt, crates/cli/tests/golden/people-set-profile.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people set-profile NAME` | Update your own display name. | Account ID and name. | `account_id`, `name` |

## JSON data shape

```json
{
  "account_id": "string",
  "name": "string"
}
```

## Human example

Recorded test fixture:

```text
account:  01912345-6789-7abc-8123-000000000001
name:     Sam Wu
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "account_id": "01912345-6789-7abc-8123-000000000001",
    "name": "Sam Wu"
  },
  "ok": true,
  "request_id": "people-example"
}
```
