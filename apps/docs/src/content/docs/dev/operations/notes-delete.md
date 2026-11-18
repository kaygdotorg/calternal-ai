---
title: "notes.delete"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Move a note to its space's Trash

Capability: content.write.

## Declaration

```json
{
  "risk": "reversible",
  "required_capability": "content.write",
  "id": "notes.delete",
  "description": "Move a note to its space's Trash",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A single note UUID targeted by a lifecycle operation.",
  "properties": {
    "id": {
      "description": "Global note UUID, resolved to its owning space by the host.",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "NoteTarget",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "NoteSummary": {
      "additionalProperties": false,
      "description": "Metadata returned by `notes.list`; excludes Markdown to bound listing payloads.",
      "properties": {
        "id": {
          "description": "Stable UUID note identity; access is checked against the accompanying space.",
          "type": "string"
        },
        "revision": {
          "description": "Opaque nonempty server revision; compare for equality, never infer order or parse it.",
          "type": "string"
        },
        "space_id": {
          "description": "Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.",
          "maxLength": 80,
          "minLength": 1,
          "pattern": "^[a-zA-Z0-9_.:-]+$",
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
        "id",
        "revision",
        "space_id",
        "title",
        "updated_at"
      ],
      "type": "object"
    }
  },
  "description": "Result of a recoverable Trash move.",
  "properties": {
    "note": {
      "allOf": [
        {
          "$ref": "#/definitions/NoteSummary"
        }
      ],
      "description": "Updated metadata including the revision and the time of the move."
    }
  },
  "required": [
    "note"
  ],
  "title": "NoteChange",
  "type": "object"
}
```
