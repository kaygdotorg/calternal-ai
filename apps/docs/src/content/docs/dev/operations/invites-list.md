---
title: "invites.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List and copy current space invitations.

Capability: members.invite.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "members.invite",
  "target": "space",
  "id": "invites.list",
  "description": "List and copy current space invitations.",
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
      "description": "Space UUID; names are never accepted in its place.",
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
  "definitions": {
    "DirectoryInvite": {
      "description": "An invitation visible to an authorized space manager or owner.",
      "properties": {
        "created_at": {
          "description": "Creation time in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "expires_at": {
          "description": "Expiration time in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "invite_id": {
          "description": "Invitation record UUID.",
          "type": "string"
        },
        "link": {
          "description": "Bearer link, present only for a usable invitation.",
          "type": [
            "string",
            "null"
          ]
        },
        "revoked_at": {
          "description": "Revocation time in Unix milliseconds, or null before revocation.",
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
          "description": "Role granted upon successful redemption."
        },
        "used_at": {
          "description": "Redemption time in Unix milliseconds, or null before redemption.",
          "format": "int64",
          "type": [
            "integer",
            "null"
          ]
        }
      },
      "required": [
        "created_at",
        "expires_at",
        "invite_id",
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
  "description": "Current invitation directory, including unexpired used and revoked records.",
  "properties": {
    "invites": {
      "description": "Authorized invitation records.",
      "items": {
        "$ref": "#/definitions/DirectoryInvite"
      },
      "type": "array"
    }
  },
  "required": [
    "invites"
  ],
  "title": "InvitesResult",
  "type": "object"
}
```
