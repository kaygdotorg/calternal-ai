---
title: "sharing.link_create"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Issue a read-only public link

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "widens_access",
  "id": "sharing.link_create",
  "description": "Issue a read-only public link",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "SharedItemRef": {
      "additionalProperties": false,
      "description": "A recognized kind and stable item UUID.",
      "properties": {
        "id": {
          "description": "The stable UUID of the item.",
          "type": "string"
        },
        "kind": {
          "description": "The item kind, either notes or collections in this slice.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "kind"
      ],
      "type": "object"
    }
  },
  "description": "Create a labeled public view link.",
  "properties": {
    "expires_at": {
      "description": "Unix milliseconds. A null value means Never.",
      "format": "int64",
      "type": [
        "integer",
        "null"
      ]
    },
    "item": {
      "allOf": [
        {
          "$ref": "#/definitions/SharedItemRef"
        }
      ],
      "description": "The target item reference."
    },
    "label": {
      "description": "The display name in the Share sheet.",
      "type": "string"
    }
  },
  "required": [
    "item",
    "label"
  ],
  "title": "ShareLinkCreateInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": true,
  "title": "Map_of_AnyValue",
  "type": "object"
}
```
