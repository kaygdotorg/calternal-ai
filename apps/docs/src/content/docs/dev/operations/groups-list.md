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
  "description": "List groups or continue an individual group's member page.",
  "properties": {
    "cursor": {
      "description": "Use the group list cursor or, with `group_id`, its member cursor.",
      "type": [
        "string",
        "null"
      ]
    },
    "group_id": {
      "description": "Group UUID when reading one group's members.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "title": "GroupDirectoryPageInput",
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
        "members_next_cursor": {
          "description": "Continue these members with this group's UUID, omitted at the end.",
          "type": [
            "string",
            "null"
          ]
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
      "description": "Groups ordered by UUID.",
      "items": {
        "$ref": "#/definitions/DirectoryGroup"
      },
      "type": "array"
    },
    "next_cursor": {
      "description": "Continuation after the last row, omitted at the end.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "groups"
  ],
  "title": "GroupsResult",
  "type": "object"
}
```
