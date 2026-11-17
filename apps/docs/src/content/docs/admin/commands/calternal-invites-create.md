---
title: "calternal invites create"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/invites-create-width-120.txt, crates/cli/tests/golden/invites-create.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `invites create --space SPACE --role ROLE [--expires-at MILLISECONDS]` | Issue one invite. The default expiry is seven days. | Invite ID and expiry. | `invite_id`, `expires_at` |

## JSON data shape

```json
{
  "invite_id": "string",
  "expires_at": "integer"
}
```

## Human example

Recorded test fixture:

```text
invite:   01912345-6789-7abc-8123-000000000003
expires:  2026-07-24 10:10:00 +00:00
Next: calternal invites list --space SPACE_UUID
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "expires_at": 1784887800000,
    "invite_id": "01912345-6789-7abc-8123-000000000003"
  },
  "ok": true,
  "request_id": "people-example"
}
```
