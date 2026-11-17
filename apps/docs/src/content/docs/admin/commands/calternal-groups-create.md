---
title: "calternal groups create"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/groups-create-width-120.txt, crates/cli/tests/golden/groups-create.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `groups create NAME` | Create a local group. | Group ID. | `group_id` |

## JSON data shape

```json
{
  "group_id": "string"
}
```

## Human example

Recorded test fixture:

```text
group:  01912345-6789-7abc-8123-000000000002
Next: calternal groups list
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "group_id": "01912345-6789-7abc-8123-000000000002"
  },
  "ok": true,
  "request_id": "people-example"
}
```
