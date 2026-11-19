---
title: "collections.get"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Read a saved notes collection and its current revision

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "collections.get",
  "description": "Read a saved notes collection and its current revision",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Read one saved Notes collection.",
  "properties": {
    "id": {
      "description": "The saved collection UUID.",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "CollectionGetInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "CollectionMember": {
      "description": "A visible note in a saved collection.",
      "properties": {
        "id": {
          "description": "Canonical note UUID.",
          "type": "string"
        },
        "title": {
          "description": "Current display title.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "title"
      ],
      "type": "object"
    }
  },
  "description": "A saved Notes collection and its current visible members.",
  "properties": {
    "can_edit": {
      "description": "Whether the caller may replace collection membership.",
      "type": "boolean"
    },
    "id": {
      "description": "Stable collection UUID.",
      "type": "string"
    },
    "members": {
      "description": "Same-space live notes in collection order.",
      "items": {
        "$ref": "#/definitions/CollectionMember"
      },
      "type": "array"
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
    "members",
    "revision",
    "title"
  ],
  "title": "CollectionGetResult",
  "type": "object"
}
```
