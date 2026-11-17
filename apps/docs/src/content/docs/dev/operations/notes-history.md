---
title: "notes.history"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read bounded note history

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "notes.history",
  "description": "Read bounded note history",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for `notes.history`; bounded pagination uses opaque server-issued cursors.",
  "properties": {
    "cursor": {
      "description": "Optional opaque pagination token, 1..256 UTF-8 bytes; omit for the first page, never send null.",
      "maxLength": 256,
      "minLength": 1,
      "type": "string"
    },
    "id": {
      "description": "Stable globally unique note UUID; the server resolves its owning space.",
      "type": "string"
    },
    "limit": {
      "description": "Optional page size, 1..100 snapshots; omission selects the server default, null is rejected.",
      "format": "uint32",
      "maximum": 100,
      "minimum": 1,
      "type": "integer"
    },
    "space_id": {
      "description": "Optional compatibility space hint; omission derives the owning space from `id`.",
      "maxLength": 80,
      "minLength": 1,
      "pattern": "^[a-zA-Z0-9_.:-]+$",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "NotesHistoryInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "NoteVersion": {
      "additionalProperties": false,
      "description": "Immutable historical snapshot in a `notes.history` page.",
      "properties": {
        "markdown": {
          "description": "UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.",
          "type": "string"
        },
        "revision": {
          "description": "Opaque nonempty server revision; compare for equality, never infer order or parse it.",
          "type": "string"
        },
        "title": {
          "description": "Display title, nonblank, at most 256 Unicode scalars and without control characters.",
          "maxLength": 256,
          "minLength": 1,
          "type": "string"
        },
        "updated_at": {
          "description": "Server-authored UTC RFC 3339 timestamp for this snapshot.",
          "type": "string"
        }
      },
      "required": [
        "markdown",
        "revision",
        "title",
        "updated_at"
      ],
      "type": "object"
    }
  },
  "description": "Result for `notes.history`; snapshots are paginated without exposing storage paths.",
  "properties": {
    "next_cursor": {
      "description": "Opaque token for the next history page; null marks the end.",
      "type": [
        "string",
        "null"
      ]
    },
    "versions": {
      "description": "Historical snapshots in descending server sequence order, bounded by the requested page size.",
      "items": {
        "$ref": "#/definitions/NoteVersion"
      },
      "type": "array"
    }
  },
  "required": [
    "next_cursor",
    "versions"
  ],
  "title": "NotesHistoryResult",
  "type": "object"
}
```
