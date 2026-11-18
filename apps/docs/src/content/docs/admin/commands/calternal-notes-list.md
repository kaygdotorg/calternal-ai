---
title: "calternal notes list"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/calternal-notes-list.txt, crates/cli/tests/golden/calternal-notes-list.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `notes list --space UUID [--tag TAG] [--cursor CURSOR]` | List up to 100 active notes, optionally under a tag. | Note table | `notes[]` |

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
ID                                    TITLE     UPDATED                     REVISION
019aabb0-1100-7111-8111-111111111111  New Plan  2026-11-17 06:12:03 +00:00  v2.preview

next cursor:  c-next
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "next_cursor": "c-next",
    "notes": [
      {
        "id": "019aabb0-1100-7111-8111-111111111111",
        "revision": "v2.preview",
        "space_id": "019aabb0-2200-7222-8222-222222222222",
        "title": "New Plan",
        "updated_at": "2026-11-17T06:12:03.000Z"
      }
    ]
  },
  "ok": true,
  "request_id": "req-notes"
}
```
