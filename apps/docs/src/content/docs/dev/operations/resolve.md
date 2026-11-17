---
title: "resolve"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Resolve an item address with current access

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "read",
  "id": "resolve",
  "description": "Resolve an item address with current access",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for the common `/<kind>/<id>` address resolver.",
  "properties": {
    "id": {
      "description": "Globally unique item UUID; possession grants no authority.",
      "type": "string"
    },
    "kind": {
      "description": "Open item-kind identifier, independent of its rendering plugin; future kinds need no enum change.",
      "maxLength": 80,
      "minLength": 1,
      "pattern": "^[a-zA-Z0-9_.:-]+$",
      "type": "string"
    }
  },
  "required": [
    "id",
    "kind"
  ],
  "title": "ResolveInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "ItemState": {
      "description": "Stable state of an item address.",
      "oneOf": [
        {
          "description": "The item is in its ordinary collection.",
          "enum": [
            "active"
          ],
          "type": "string"
        },
        {
          "description": "The item can be restored from the space's Trash.",
          "enum": [
            "in_trash"
          ],
          "type": "string"
        },
        {
          "description": "The identity is retained after permanent deletion.",
          "enum": [
            "deleted"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "Current owning space and effective capabilities for a visible address.",
  "properties": {
    "access": {
      "description": "Current effective capability names; informational, rechecked on use.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "id": {
      "description": "Stable item UUID.",
      "type": "string"
    },
    "kind": {
      "description": "Resolved item kind.",
      "type": "string"
    },
    "space_id": {
      "description": "Owning space UUID, independent of account identity.",
      "type": "string"
    },
    "state": {
      "allOf": [
        {
          "$ref": "#/definitions/ItemState"
        }
      ],
      "description": "Lifecycle state of a Notes address, omitted for kinds without this projection."
    }
  },
  "required": [
    "access",
    "id",
    "kind",
    "space_id"
  ],
  "title": "ResolveResult",
  "type": "object"
}
```
