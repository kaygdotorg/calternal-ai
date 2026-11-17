---
title: "calternal people list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/people-list-width-120.txt, crates/cli/tests/golden/people-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `people list [--cursor CURSOR]` | List server accounts with the people administration capability. | ID, name, role, state. | `people` |

## JSON data shape

```json
{
  "people": [
    {
      "account_id": "string",
      "name": "string",
      "role": "string",
      "disabled": "bool"
    }
  ],
  "next_cursor?": "string"
}
```

## Human example

Recorded test fixture:

```text
ID                                    NAME    ROLE   STATE
01912345-6789-7abc-8123-000000000001  Sam Wu  admin  Active
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "people": [
      {
        "account_id": "01912345-6789-7abc-8123-000000000001",
        "disabled": false,
        "name": "Sam Wu",
        "role": "admin"
      }
    ]
  },
  "ok": true,
  "request_id": "people-example"
}
```
