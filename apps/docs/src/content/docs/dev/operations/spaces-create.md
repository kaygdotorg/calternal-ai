---
title: "spaces.create"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Create an owned space

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "spaces.create",
  "description": "Create an owned space",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Input for creation; the authenticated account becomes owner.",
  "properties": {
    "name": {
      "description": "Display name, with the same length and control constraints as a title.",
      "type": "string"
    }
  },
  "required": [
    "name"
  ],
  "title": "SpacesCreateInput",
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
        "personal": {
          "description": "Whether this is the personal space provisioned for the caller.",
          "type": "boolean"
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
        "personal",
        "role"
      ],
      "type": "object"
    }
  },
  "description": "Newly created space.",
  "properties": {
    "space": {
      "allOf": [
        {
          "$ref": "#/definitions/StoredSpace"
        }
      ],
      "description": "Metadata for the new owned space."
    }
  },
  "required": [
    "space"
  ],
  "title": "SpaceResult",
  "type": "object"
}
```
