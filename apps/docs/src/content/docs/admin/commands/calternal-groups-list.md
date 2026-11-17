---
title: "calternal groups list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/groups-list-width-120.txt, crates/cli/tests/golden/groups-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `groups list [--group GROUP] [--cursor CURSOR]` | List server groups and current member IDs. | Group and member tables. | `groups` |

## JSON data shape

```json
{
  "groups": [
    {
      "group_id": "string",
      "name": "string",
      "source": "string",
      "members": [
        "string"
      ],
      "members_next_cursor?": "string"
    }
  ],
  "next_cursor?": "string"
}
```

## Human example

Recorded test fixture:

```text
ID                                    NAME    SOURCE
01912345-6789-7abc-8123-000000000002  Studio  local
MEMBER ID
01912345-6789-7abc-8123-000000000001
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "groups": [
      {
        "group_id": "01912345-6789-7abc-8123-000000000002",
        "members": [
          "01912345-6789-7abc-8123-000000000001"
        ],
        "name": "Studio",
        "source": "local"
      }
    ]
  },
  "ok": true,
  "request_id": "people-example"
}
```
