---
title: "calternal invites revoke"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/invites-revoke-width-120.txt, crates/cli/tests/golden/invites-revoke.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `invites revoke --space SPACE INVITE` | Revoke an unused link. | Invite ID and revocation confirmation. | `invite_id` |

## JSON data shape

```json
{
  "invite_id": "string"
}
```

## Human example

Recorded test fixture:

```text
Invitation revoked.
invite:  01912345-6789-7abc-8123-000000000003
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "invite_id": "01912345-6789-7abc-8123-000000000003"
  },
  "ok": true,
  "request_id": "people-example"
}
```
