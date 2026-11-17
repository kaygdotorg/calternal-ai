---
title: "spaces.set_role"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Change an existing member role

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "spaces.set_role",
  "description": "Change an existing member role",
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
  "description": "Change an existing member's role; assigning Owner requires transfer instead.",
  "properties": {
    "account_id": {
      "description": "Stable target account UUID.",
      "type": "string"
    },
    "role": {
      "allOf": [
        {
          "$ref": "#/definitions/SpaceRole"
        }
      ],
      "description": "Requested membership role; Owner is refused here."
    },
    "space_id": {
      "description": "Stable space UUID; development inputs temporarily accept seeded aliases.",
      "type": "string"
    }
  },
  "required": [
    "account_id",
    "role",
    "space_id"
  ],
  "title": "SpaceSetRoleInput",
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
