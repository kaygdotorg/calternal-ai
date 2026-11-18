---
title: "collections.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List saved notes collections in a space

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "collections.list",
  "description": "List saved notes collections in a space",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "List saved Notes collections in a space the caller can read.",
  "properties": {
    "cursor": {
      "description": "Opaque continuation from this space and account.",
      "type": [
        "string",
        "null"
      ]
    },
    "limit": {
      "description": "Page size, clamped to 1–50. Defaults to 50.",
      "format": "uint32",
      "minimum": 0,
      "type": [
        "integer",
        "null"
      ]
    },
    "space_id": {
      "description": "Owning space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "CollectionListInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "CollectionSummary": {
      "description": "A saved Notes collection as shown in its space's list.",
      "properties": {
        "can_edit": {
          "description": "Whether the caller may replace collection membership.",
          "type": "boolean"
        },
        "id": {
          "description": "Stable collection UUID.",
          "type": "string"
        },
        "revision": {
          "description": "Current opaque content revision.",
          "type": "string"
        },
        "title": {
          "description": "Current display title.",
          "type": "string"
        }
      },
      "required": [
        "can_edit",
        "id",
        "revision",
        "title"
      ],
      "type": "object"
    }
  },
  "description": "A continuation page of saved Notes collections.",
  "properties": {
    "items": {
      "description": "Saved Notes collections in this page.",
      "items": {
        "$ref": "#/definitions/CollectionSummary"
      },
      "type": "array"
    },
    "next_cursor": {
      "description": "Opaque continuation when another page may exist.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "items"
  ],
  "title": "CollectionPage",
  "type": "object"
}
```
