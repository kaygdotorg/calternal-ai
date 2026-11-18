---
title: "sharing.link_copy"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Retrieve an authorized public link URL

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "sharing.link_copy",
  "description": "Retrieve an authorized public link URL",
  "mutates": false
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
  "description": "Bind a grant UUID to its item.",
  "properties": {
    "grant_id": {
      "description": "The independently manageable grant UUID.",
      "type": "string"
    },
    "item": {
      "allOf": [
        {
          "$ref": "#/definitions/SharedItemRef"
        }
      ],
      "description": "The target item reference."
    }
  },
  "required": [
    "grant_id",
    "item"
  ],
  "title": "ShareGrantTarget",
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
