---
title: "trash.purge"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Permanently delete a note from Trash

Capability: content.purge.

## Declaration

```json
{
  "risk": "irreversible",
  "required_capability": "content.purge",
  "id": "trash.purge",
  "description": "Permanently delete a note from Trash",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A single note UUID targeted by a lifecycle operation.",
  "properties": {
    "id": {
      "description": "Global note UUID, resolved to its owning space by the host.",
      "type": "string"
    }
  },
  "required": [
    "id"
  ],
  "title": "NoteTarget",
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
  "description": "Permanent deletion acknowledgement. Contains no note contents.",
  "properties": {
    "id": {
      "description": "Stable UUID retained for address resolution.",
      "type": "string"
    },
    "state": {
      "allOf": [
        {
          "$ref": "#/definitions/ItemState"
        }
      ],
      "description": "Deleted state of the address."
    }
  },
  "required": [
    "id",
    "state"
  ],
  "title": "PurgeResult",
  "type": "object"
}
```
