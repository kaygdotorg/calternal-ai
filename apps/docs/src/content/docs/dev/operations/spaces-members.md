---
title: "spaces.members"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List current members

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "spaces.members",
  "description": "List current members",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for members, leave and directory-record deletion.",
  "properties": {
    "space_id": {
      "description": "Stable space UUID; development inputs temporarily accept seeded aliases.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "SpaceMember": {
      "additionalProperties": false,
      "description": "Account membership in a space.",
      "properties": {
        "account_id": {
          "description": "Member's stable account UUID.",
          "type": "string"
        },
        "role": {
          "allOf": [
            {
              "$ref": "#/definitions/SpaceRole"
            }
          ],
          "description": "Current effective role."
        }
      },
      "required": [
        "account_id",
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
    "members": {
      "description": "Current account memberships, including exactly one Owner.",
      "items": {
        "$ref": "#/definitions/SpaceMember"
      },
      "type": "array"
    }
  },
  "required": [
    "members"
  ],
  "title": "SpaceMembersResult",
  "type": "object"
}
```
