---
title: "groups.rename"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Rename a local group.

Capability: groups.manage.

## Declaration

```json
{
  "risk": "reversible",
  "required_capability": "groups.manage",
  "target": "server",
  "id": "groups.rename",
  "description": "Rename a local group.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Rename a local group.",
  "properties": {
    "group_id": {
      "description": "The group's stable UUID.",
      "type": "string"
    },
    "name": {
      "description": "Its new name.",
      "type": "string"
    }
  },
  "required": [
    "group_id",
    "name"
  ],
  "title": "GroupRenameInput",
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
