---
title: "cl notes list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/notes-list.txt, crates/cli/tests/golden/notes-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `notes list [--space ID] [--cursor CURSOR]` | List notes. | Table `ID TITLE UPDATED REVISION`, or a friendly empty result with a command to create one. | `notes[]` |

## JSON data shape

```json
{
  "notes": [
    {
      "id": "string",
      "space_id": "string",
      "title": "string",
      "revision": "string",
      "updated_at": "string"
    }
  ],
  "next_cursor?": "string"
}
```

## Human example

Recorded test fixture:

```text
ID                                    TITLE      UPDATED                     REVISION
11111111-1111-4111-8111-111111111111  Groceries  2026-11-17 07:12:03 +01:00  r-7
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "notes": [
      {
        "id": "11111111-1111-4111-8111-111111111111",
        "revision": "r-7",
        "space_id": "s",
        "title": "Groceries",
        "updated_at": "2026-11-17T06:12:03Z"
      }
    ]
  },
  "ok": true,
  "request_id": "req-1"
}
```
