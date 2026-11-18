---
title: "trash.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List a page of up to 100 notes in the space's Trash

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "id": "trash.list",
  "description": "List a page of up to 100 notes in the space's Trash",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A continuation scoped to one space.",
  "properties": {
    "cursor": {
      "description": "The previous page's cursor for this space and operation.",
      "maxLength": 524288,
      "minLength": 1,
      "type": "string"
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceDirectoryPageInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "TrashedNote": {
      "description": "One trashed note and its remaining whole-day retention, rounded up.",
      "properties": {
        "days_remaining": {
          "description": "Days until eligibility for automatic purge. Zero means eligible now.",
          "format": "uint32",
          "minimum": 0,
          "type": "integer"
        },
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
        "days_remaining",
        "id",
        "revision",
        "space_id",
        "title",
        "updated_at"
      ],
      "type": "object"
    }
  },
  "description": "One page of notes eligible for restore or purge, newest deletion first.",
  "properties": {
    "next_cursor": {
      "description": "Opaque next-page cursor, omitted when there are no more notes.",
      "type": [
        "string",
        "null"
      ]
    },
    "notes": {
      "description": "At most 100 notes with their remaining retention.",
      "items": {
        "$ref": "#/definitions/TrashedNote"
      },
      "type": "array"
    }
  },
  "required": [
    "notes"
  ],
  "title": "TrashListResult",
  "type": "object"
}
```
