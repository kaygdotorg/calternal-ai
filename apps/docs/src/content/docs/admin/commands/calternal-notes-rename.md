---
title: "calternal notes rename"
---

<!-- Generated from: docs/cli.md, docs/cli-shapes.json, crates/cli/tests/golden/notes-rename.txt, crates/cli/tests/golden/notes-rename.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

| Arguments | Purpose | Human output | JSON data |
| --- | --- | --- | --- |
| `notes rename ID --base-revision REVISION --title TITLE` | Update heading, filename and links. | Note fields and Markdown | `note` |

## JSON data shape

```json
{
  "note": {
    "id": "string",
    "space_id": "string",
    "title": "string",
    "revision": "string",
    "updated_at": "string",
    "markdown": "string"
  },
  "job_id?": "string"
}
```

## Human example

Recorded test fixture:

```text
ID:        019aabb0-1100-7111-8111-111111111111
Space:     019aabb0-2200-7222-8222-222222222222
Title:     New Plan
Revision:  v2.preview
State:     Renamed
Updated:  2026-11-17 06:12:03 +00:00
Job:  019aabb0-3300-7333-8333-333333333333

# New Plan
```

## JSON example

Recorded test fixture:

```json
{
  "data": {
    "job_id": "019aabb0-3300-7333-8333-333333333333",
    "note": {
      "id": "019aabb0-1100-7111-8111-111111111111",
      "markdown": "# New Plan\n",
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
