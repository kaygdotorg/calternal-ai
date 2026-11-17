---
title: "groups.delete"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Delete a local group and remove its grants.

Capability: groups.manage.

## Declaration

```json
{
  "risk": "irreversible",
  "required_capability": "groups.manage",
  "target": "server",
  "id": "groups.delete",
  "description": "Delete a local group and remove its grants.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Target group UUID.",
  "properties": {
    "group_id": {
      "description": "The group's stable UUID.",
      "type": "string"
    }
  },
  "required": [
    "group_id"
  ],
  "title": "GroupInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Target group UUID.",
  "properties": {
    "group_id": {
      "description": "The group's stable UUID.",
      "type": "string"
    }
  },
  "required": [
    "group_id"
  ],
  "title": "GroupInput",
  "type": "object"
}
```
