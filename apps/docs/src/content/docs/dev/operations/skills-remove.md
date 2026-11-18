---
title: "skills.remove"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Move a skill bundle to Trash

Capability: content.write.

## Declaration

```json
{
  "risk": "reversible",
  "required_capability": "content.write",
  "target": "space",
  "id": "skills.remove",
  "description": "Move a skill bundle to Trash",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A conditional removal of a skill bundle.",
  "properties": {
    "base_revision": {
      "description": "Revision returned by the read operation.",
      "type": "string"
    },
    "name": {
      "description": "Name of the bundle directory.",
      "type": "string"
    },
    "space_id": {
      "description": "Owning space UUID.",
      "type": "string"
    }
  },
  "required": [
    "base_revision",
    "name",
    "space_id"
  ],
  "title": "SkillRemove",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "True": {
      "const": true,
      "type": "boolean"
    }
  },
  "description": "Acknowledgement of a successful removal into Trash.",
  "properties": {
    "ok": {
      "allOf": [
        {
          "$ref": "#/definitions/True"
        }
      ],
      "description": "True after the committed removal."
    }
  },
  "required": [
    "ok"
  ],
  "title": "RemovalResult",
  "type": "object"
}
```
