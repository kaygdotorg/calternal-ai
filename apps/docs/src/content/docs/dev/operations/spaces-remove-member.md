---
title: "spaces.remove_member"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Remove a direct account or group grant.

Capability: members.manage.

## Declaration

```json
{
  "risk": "irreversible",
  "required_capability": "members.manage",
  "target": "space",
  "id": "spaces.remove_member",
  "description": "Remove a direct account or group grant.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A direct space grant identifies exactly one person or group.",
  "properties": {
    "account_id": {
      "description": "Account UUID when the grant targets a person.",
      "type": [
        "string",
        "null"
      ]
    },
    "group_id": {
      "description": "Group UUID when the grant targets a group.",
      "type": [
        "string",
        "null"
      ]
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceGrantInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A completed membership mutation or directory-record deletion.",
  "title": "SpaceChangedResult",
  "type": "object"
}
```
