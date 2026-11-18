---
title: "spaces.members"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List direct account and group grants.

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "space",
  "id": "spaces.members",
  "description": "List direct account and group grants.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A continuation scoped to one space.",
  "properties": {
    "cursor": {
      "description": "The previous page's cursor for this space and operation.",
      "maxLength": 524288,
      "minLength": 1,
      "type": "string"
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceDirectoryPageInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "SpaceGroupMember": {
      "description": "A group grant visible to current members of the space.",
      "properties": {
        "group_id": {
          "description": "The group's stable UUID.",
          "type": "string"
        },
        "member_count": {
          "description": "Number of active people in the group.",
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        },
        "name": {
          "description": "Current group display name.",
          "type": "string"
        },
        "role": {
          "allOf": [
            {
              "$ref": "#/definitions/SpaceRole"
            }
          ],
          "description": "The role assigned to the group."
        }
      },
      "required": [
        "group_id",
        "member_count",
        "name",
        "role"
      ],
      "type": "object"
    },
    "SpaceMember": {
      "additionalProperties": false,
      "description": "Account membership in a space.",
      "properties": {
        "account_id": {
          "description": "Member's stable account UUID.",
          "type": "string"
        },
        "name": {
          "description": "Current account display name.",
          "type": "string"
        },
        "role": {
          "allOf": [
            {
              "$ref": "#/definitions/SpaceRole"
            }
          ],
          "description": "Current directly granted role."
        }
      },
      "required": [
        "account_id",
        "name",
        "role"
      ],
      "type": "object"
    },
    "SpaceRole": {
      "description": "Preview role exposed by `GET /api/v1/describe`; membership is enforced by the host, never by this enum alone.",
      "oneOf": [
        {
          "description": "Read authorized space content.",
          "enum": [
            "reader"
          ],
          "type": "string"
        },
        {
          "description": "Read and edit authorized space content.",
          "enum": [
            "editor"
          ],
          "type": "string"
        },
        {
          "description": "Read, edit and manage space plugins.",
          "enum": [
            "manager"
          ],
          "type": "string"
        },
        {
          "description": "Sole owner; includes Manager grants plus management of Managers, transfer and deletion.",
          "enum": [
            "owner"
          ],
          "type": "string"
        }
      ]
    }
  },
  "description": "Member directory visible to a current space reader.",
  "properties": {
    "groups": {
      "description": "Direct group grants, including names and active member counts.",
      "items": {
        "$ref": "#/definitions/SpaceGroupMember"
      },
      "type": "array"
    },
    "members": {
      "description": "Direct account grants on this page. Exactly one Owner has a direct grant.",
      "items": {
        "$ref": "#/definitions/SpaceMember"
      },
      "type": "array"
    },
    "next_cursor": {
      "description": "Continue after the last grant, omitted at the end.",
      "type": [
        "string",
        "null"
      ]
    }
  },
  "required": [
    "groups",
    "members"
  ],
  "title": "SpaceMembersResult",
  "type": "object"
}
```
