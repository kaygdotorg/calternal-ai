---
title: "calternal notes delete"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/notes-delete.txt, crates/cli/tests/golden/notes-delete.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `notes delete ID` | Move a note to Trash. | Note fields and state | `note` |

## JSON data shape

```json
{
  "note": {
    "id": "string",
    "space_id": "string",
    "title": "string",
    "revision": "string",
    "updated_at": "string"
  }
}
```

## Human example

Recorded test fixture:

```text
ID:        019aabb0-1100-7111-8111-111111111111
Space:     019aabb0-2200-7222-8222-222222222222
Title:     New Plan
Revision:  v2.preview
State:     In Trash
Updated:  2026-11-17 06:12:03 +00:00
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "note": {
      "id": "019aabb0-1100-7111-8111-111111111111",
      "revision": "v2.preview",
      "space_id": "019aabb0-2200-7222-8222-222222222222",
      "title": "New Plan",
      "updated_at": "2026-11-17T06:12:03.000Z"
    }
  },
  "ok": true,
  "request_id": "req-notes"
}
```
