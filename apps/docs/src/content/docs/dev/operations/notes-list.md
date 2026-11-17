---
title: "notes.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List up to 100 notes

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "notes.list",
  "description": "List up to 100 notes",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for `notes.list`; the server independently checks access to the selected space.",
  "properties": {
    "space_id": {
      "description": "Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.",
      "maxLength": 80,
      "minLength": 1,
      "pattern": "^[a-zA-Z0-9_.:-]+$",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "NotesListInput",
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
  "description": "Result for `notes.list`, containing at most 100 current note summaries.",
  "properties": {
    "notes": {
      "description": "At most 100 current summaries, ordered by descending update timestamp then resource ID.",
      "items": {
        "$ref": "#/definitions/NoteSummary"
      },
      "type": "array"
    }
  },
  "required": [
    "notes"
  ],
  "title": "NotesListResult",
  "type": "object"
}
```
