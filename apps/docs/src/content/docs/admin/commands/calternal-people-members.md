---
title: "calternal people members"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/people-members-width-120.txt, crates/cli/tests/golden/people-members.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people members --space SPACE [--cursor CURSOR]` | List direct people and group grants in a space. | ID, name, role, recipient type, group member count. | `members`, `groups` |

## JSON data shape

```json
{
  "members": [
    {
      "account_id": "string",
      "name": "string",
      "role": "string"
    }
  ],
  "groups": [
    {
      "group_id": "string",
      "name": "string",
      "role": "string",
      "member_count": "integer"
    }
  ],
  "next_cursor?": "string"
}
```

## Human example

Recorded test fixture:

```text
ID                                    NAME    ROLE    TYPE    MEMBERS
01912345-6789-7abc-8123-000000000001  Sam Wu  owner   Person  —
01912345-6789-7abc-8123-000000000002  Studio  editor  Group   3
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "groups": [
      {
        "group_id": "01912345-6789-7abc-8123-000000000002",
        "member_count": 3,
        "name": "Studio",
        "role": "editor"
      }
    ],
    "members": [
      {
        "account_id": "01912345-6789-7abc-8123-000000000001",
        "name": "Sam Wu",
        "role": "owner"
      }
    ]
  },
  "ok": true,
  "request_id": "people-example"
}
```
