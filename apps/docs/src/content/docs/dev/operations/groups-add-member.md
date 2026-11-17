---
title: "groups.add_member"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Add a person to a local group.

Capability: groups.manage.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "groups.manage",
  "target": "server",
  "id": "groups.add_member",
  "description": "Add a person to a local group.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "One membership in a local group.",
  "properties": {
    "account_id": {
      "description": "The person's account UUID.",
      "type": "string"
    },
    "group_id": {
      "description": "The group's stable UUID.",
      "type": "string"
    }
  },
  "required": [
    "account_id",
    "group_id"
  ],
  "title": "GroupPersonInput",
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
