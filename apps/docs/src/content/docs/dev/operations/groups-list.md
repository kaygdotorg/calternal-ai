---
title: "groups.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List server groups.

Capability: groups.manage.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "groups.manage",
  "target": "server",
  "id": "groups.list",
  "description": "List server groups.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "An operation scoped to the authenticated account or server with no input fields.",
  "title": "DirectoryInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "DirectoryGroup": {
      "description": "A server group and its active members.",
      "properties": {
        "group_id": {
          "description": "The group's stable UUID.",
          "type": "string"
        },
        "members": {
          "description": "Account UUIDs for each active member.",
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "name": {
          "description": "Current display name.",
          "type": "string"
        },
        "source": {
          "description": "Local or identity-provider managed.",
          "type": "string"
        }
      },
      "required": [
        "group_id",
        "members",
        "name",
        "source"
      ],
      "type": "object"
    }
  },
  "description": "Server group directory result.",
  "properties": {
    "groups": {
      "description": "Groups ordered by display name and UUID.",
      "items": {
        "$ref": "#/definitions/DirectoryGroup"
      },
      "type": "array"
    }
  },
  "required": [
    "groups"
  ],
  "title": "GroupsResult",
  "type": "object"
}
```
