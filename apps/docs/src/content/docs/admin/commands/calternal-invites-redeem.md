---
title: "calternal invites redeem"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/invites-redeem-width-120.txt, crates/cli/tests/golden/invites-redeem.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `invites redeem --secret-file FILE` | Redeem a link as the signed-in person. Use `-` to read standard input. | Space ID. | `space_id` |

## JSON data shape

```json
{
  "space_id": "string"
}
```

## Human example

Recorded test fixture:

```text
Joined space.
space:  01912345-6789-7abc-8123-000000000004
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "space_id": "01912345-6789-7abc-8123-000000000004"
  },
  "ok": true,
  "request_id": "people-example"
}
```
