---
title: "spaces.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List current account memberships

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "spaces.list",
  "description": "List current account memberships",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Empty input for account-scoped space listing.",
  "title": "SpacesListInput",
  "type": "object"
}
```

## Output

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
    },
    "StoredSpace": {
      "additionalProperties": false,
      "description": "Space metadata and the caller's current effective role.",
      "properties": {
        "created": {
          "description": "Creation time in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "id": {
          "description": "Independent stable UUIDv7 space identity.",
          "type": "string"
        },
        "name": {
          "description": "Display name, never an authority key.",
          "type": "string"
        },
        "owner": {
          "description": "Account UUID of the sole owner.",
          "type": "string"
        },
        "role": {
          "allOf": [
            {
              "$ref": "#/definitions/SpaceRole"
            }
          ],
          "description": "Caller role, including the effective Owner role."
        }
      },
      "required": [
        "created",
        "id",
        "name",
        "owner",
        "role"
      ],
      "type": "object"
    }
  },
  "description": "Account's visible spaces; server administration does not add memberships.",
  "properties": {
    "spaces": {
      "description": "Current memberships only.",
      "items": {
        "$ref": "#/definitions/StoredSpace"
      },
      "type": "array"
    }
  },
  "required": [
    "spaces"
  ],
  "title": "SpacesListResult",
  "type": "object"
}
```
