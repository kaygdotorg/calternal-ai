---
title: "notes.rename"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Rename a note at its current revision

Capability: content.write.

## Declaration

```json
{
  "risk": "reversible",
  "required_capability": "content.write",
  "id": "notes.rename",
  "description": "Rename a note at its current revision",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Rename one note at the revision shown in the list.",
  "properties": {
    "base_revision": {
      "description": "Opaque revision selected by the caller.",
      "maxLength": 256,
      "minLength": 1,
      "type": "string"
    },
    "id": {
      "description": "Global note UUID; its current space is resolved by the host.",
      "type": "string"
    },
    "title": {
      "description": "New title, used for the first H1 and readable filename.",
      "maxLength": 256,
      "minLength": 1,
      "type": "string"
    }
  },
  "required": [
    "base_revision",
    "id",
    "title"
  ],
  "title": "NoteRenameInput",
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
    "job_id": {
      "description": "Durable link rewrite job when this write changes the note's title.",
      "type": [
        "string",
        "null"
      ]
    },
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
