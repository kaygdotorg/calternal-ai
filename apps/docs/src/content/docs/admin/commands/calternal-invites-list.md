---
title: "calternal invites list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/invites-list-width-120.txt, crates/cli/tests/golden/invites-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `invites list --space SPACE [--cursor CURSOR]` | List invitations and copy usable links. | Invite state and complete usable links. | `invites` |

## JSON data shape

```json
{
  "invites": [
    {
      "invite_id": "string",
      "role": "string",
      "created_at": "integer",
      "expires_at": "integer",
      "used_at": "any",
      "revoked_at": "any",
      "link": "string?"
    }
  ],
  "next_cursor?": "string"
}
```

## Human example

Recorded test fixture:

```text
ID                                    ROLE    CREATED                     EXPIRES                     USED  REVOKED
01912345-6789-7abc-8123-000000000003  reader  2026-07-17 10:10:00 +00:00  2026-07-24 10:10:00 +00:00  —     —

invite:  01912345-6789-7abc-8123-000000000003
https://example.test/invites/example
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "invites": [
      {
        "created_at": 1784283000000,
        "expires_at": 1784887800000,
        "invite_id": "01912345-6789-7abc-8123-000000000003",
        "link": "https://example.test/invites/example",
        "revoked_at": null,
        "role": "reader",
        "used_at": null
      }
    ]
  },
  "ok": true,
  "request_id": "people-example"
}
```
