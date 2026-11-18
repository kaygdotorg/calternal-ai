---
title: "calternal search people"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/search-people.txt, crates/cli/tests/golden/search-people.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `search people --space UUID [PREFIX]` | Find accounts and groups to mention. | Name, subject type and ID. | `people` |

## JSON data shape

```json
{
  "people": [
    {
      "id": "string",
      "name": "string",
      "kind": "string"
    }
  ]
}
```

## Human example

Recorded test fixture:

```text
NAME       TYPE     ID
Account A  account  019aabb0-1100-7111-8111-111111111111
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "people": [
      {
        "id": "019aabb0-1100-7111-8111-111111111111",
        "kind": "account",
        "name": "Account A"
      }
    ]
  },
  "ok": true,
  "request_id": "req-search"
}
```
