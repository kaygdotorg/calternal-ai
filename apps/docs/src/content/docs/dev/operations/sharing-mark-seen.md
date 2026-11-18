---
title: "sharing.mark_seen"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Mark a received item share as seen

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "sharing.mark_seen",
  "description": "Mark a received item share as seen",
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
  "description": "An explicit acknowledgement that the recipient has seen a share.",
  "properties": {
    "item": {
      "allOf": [
        {
          "$ref": "#/definitions/SharedItemRef"
        }
      ],
      "description": "The received item reference."
    }
  },
  "required": [
    "item"
  ],
  "title": "ShareSeenInput",
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
