---
title: "invites.create"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Create a single-use space invitation.

Capability: members.invite.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "members.invite",
  "target": "space",
  "id": "invites.create",
  "description": "Create a single-use space invitation.",
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
  "description": "Create a single-use invitation to a space.",
  "properties": {
    "expires_at": {
      "description": "Absolute Unix milliseconds; omission defaults to seven days.",
      "format": "int64",
      "type": [
        "integer",
        "null"
      ]
    },
    "role": {
      "allOf": [
        {
          "$ref": "#/definitions/SpaceRole"
        }
      ],
      "description": "Role granted on redemption. Owner is never assignable by invitation."
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
  "title": "InviteCreateInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "description": "A created invitation. Use the authorized list to copy the link.",
  "properties": {
    "expires_at": {
      "description": "Absolute expiry in Unix milliseconds.",
      "format": "int64",
      "type": "integer"
    },
    "invite_id": {
      "description": "Invitation record UUID.",
      "type": "string"
    }
  },
  "required": [
    "expires_at",
    "invite_id"
  ],
  "title": "InviteCreated",
  "type": "object"
}
```
