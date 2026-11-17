---
title: "spaces.add_member"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Grant a person or group access to a space.

Capability: members.invite.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "members.invite",
  "target": "space",
  "id": "spaces.add_member",
  "description": "Grant a person or group access to a space.",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
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
  "description": "A role assignment to exactly one person or group.",
  "properties": {
    "account_id": {
      "description": "Account UUID when granting access directly to a person.",
      "type": [
        "string",
        "null"
      ]
    },
    "group_id": {
      "description": "Group UUID when granting access through current group membership.",
      "type": [
        "string",
        "null"
      ]
    },
    "role": {
      "allOf": [
        {
          "$ref": "#/definitions/SpaceRole"
        }
      ],
      "description": "Requested role. Ownership uses explicit transfer."
    },
    "space_id": {
      "description": "Space UUID.",
      "type": "string"
    }
  },
  "required": [
    "role",
    "space_id"
  ],
  "title": "SpaceGrantRoleInput",
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
