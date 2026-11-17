---
title: "groups.create"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Create a local group.

Capability: groups.manage.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "groups.manage",
  "target": "server",
  "id": "groups.create",
  "description": "Create a local group.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A group's display name on creation.",
  "properties": {
    "name": {
      "description": "Printable name shown to authorized people.",
      "type": "string"
    }
  },
  "required": [
    "name"
  ],
  "title": "GroupCreateInput",
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
