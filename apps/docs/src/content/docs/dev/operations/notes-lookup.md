---
title: "notes.lookup"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Find a page of up to 100 notes by title prefix

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "id": "notes.lookup",
  "description": "Find a page of up to 100 notes by title prefix",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Space-scoped lookup of notes or tags by literal prefix.",
  "properties": {
    "cursor": {
      "description": "Opaque cursor from the preceding page with the same space and filters.",
      "maxLength": 524288,
      "minLength": 1,
      "type": "string"
    },
    "prefix": {
      "description": "Literal prefix, up to 256 Unicode scalars. Empty starts at the first name.",
      "maxLength": 256,
      "type": "string"
    },
    "space_id": {
      "description": "UUID of the space whose index to query.",
      "type": "string"
    }
  },
  "required": [
    "prefix",
    "space_id"
  ],
  "title": "NotePrefixInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "NoteMatch": {
      "description": "Metadata for one picker match, including its relative Markdown destination.",
      "properties": {
        "id": {
          "description": "Stable UUID note identity; access is checked against the accompanying space.",
          "type": "string"
        },
        "link_target": {
          "description": "Encoded relative target for a Markdown link from another note.",
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
        "link_target",
        "revision",
        "space_id",
        "title",
        "updated_at"
      ],
      "type": "object"
    }
  },
  "description": "Bounded title-matching page in folded title and UUID order.",
  "properties": {
    "next_cursor": {
      "description": "Opaque continuation for the same prefix, omitted at the end.",
      "type": [
        "string",
        "null"
      ]
    },
    "notes": {
      "description": "At most 100 live matches with their exact on-disk destinations.",
      "items": {
        "$ref": "#/definitions/NoteMatch"
      },
      "type": "array"
    }
  },
  "required": [
    "notes"
  ],
  "title": "NotesLookupResult",
  "type": "object"
}
```
