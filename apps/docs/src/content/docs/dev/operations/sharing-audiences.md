---
title: "sharing.audiences"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Find visible people or groups to share an item with

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "sharing.audiences",
  "description": "Find visible people or groups to share an item with",
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
  "description": "Search the currently visible directory for audience choices.",
  "properties": {
    "cursor": {
      "description": "An optional continuation issued for this operation and scope.",
      "type": [
        "string",
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
    "query": {
      "description": "A display-name search prefix or substring, or empty.",
      "type": "string"
    }
  },
  "required": [
    "item",
    "query"
  ],
  "title": "ShareAudiencesInput",
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
