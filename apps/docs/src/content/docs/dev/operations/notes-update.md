---
title: "notes.update"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Update a note against an opaque base revision

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "notes.update",
  "description": "Update a note against an opaque base revision",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for `notes.update`; the base revision must match before the write can commit.",
  "properties": {
    "base_revision": {
      "description": "Expected opaque current revision, 1..256 UTF-8 bytes; mismatch produces a conflict.",
      "maxLength": 256,
      "minLength": 1,
      "type": "string"
    },
    "id": {
      "description": "Stable globally unique note UUID; the server resolves its owning space.",
      "type": "string"
    },
    "markdown": {
      "description": "UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.",
      "type": "string"
    },
    "space_id": {
      "description": "Optional compatibility space hint; omission derives the owning space from `id`.",
      "maxLength": 80,
      "minLength": 1,
      "pattern": "^[a-zA-Z0-9_.:-]+$",
      "type": "string"
    },
    "title": {
      "description": "Optional replacement title: nonblank, at most 256 Unicode scalars, no controls; omission preserves the title and null is rejected.",
      "type": "string"
    }
  },
  "required": [
    "base_revision",
    "id",
    "markdown"
  ],
  "title": "NotesUpdateInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "Note": {
      "additionalProperties": false,
      "description": "Current snapshot returned by `notes.get`, `notes.create` and `notes.update`.",
      "properties": {
        "id": {
          "description": "Stable UUID note identity; access is checked against the accompanying space.",
          "type": "string"
        },
        "markdown": {
          "description": "UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.",
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
        "markdown",
        "revision",
        "space_id",
        "title",
        "updated_at"
      ],
      "type": "object"
    }
  },
  "description": "Result for `notes.get`, `notes.create` and `notes.update`, containing one current snapshot.",
  "properties": {
    "note": {
      "allOf": [
        {
          "$ref": "#/definitions/Note"
        }
      ],
      "description": "Current note snapshot visible to the authenticated caller."
    }
  },
  "required": [
    "note"
  ],
  "title": "NoteResult",
  "type": "object"
}
```
