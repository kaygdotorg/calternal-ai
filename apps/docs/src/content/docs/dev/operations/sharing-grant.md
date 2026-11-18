---
title: "sharing.grant"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Set view or edit access for one named audience

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "widens_access",
  "id": "sharing.grant",
  "description": "Set view or edit access for one named audience",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "ShareAccountAudience": {
      "additionalProperties": false,
      "description": "A named account audience.",
      "properties": {
        "account_id": {
          "description": "The active recipient account UUID.",
          "type": "string"
        }
      },
      "required": [
        "account_id"
      ],
      "type": "object"
    },
    "ShareAudience": {
      "anyOf": [
        {
          "allOf": [
            {
              "$ref": "#/definitions/ShareAccountAudience"
            }
          ],
          "description": "A named account."
        },
        {
          "allOf": [
            {
              "$ref": "#/definitions/ShareGroupAudience"
            }
          ],
          "description": "A group with current membership."
        }
      ],
      "description": "Exactly one identified account or server group."
    },
    "ShareGroupAudience": {
      "additionalProperties": false,
      "description": "A live group audience.",
      "properties": {
        "group_id": {
          "description": "The recipient group UUID.",
          "type": "string"
        }
      },
      "required": [
        "group_id"
      ],
      "type": "object"
    },
    "SharePermission": {
      "description": "Current operations allowed on the shared item.",
      "oneOf": [
        {
          "description": "Read the live item.",
          "enum": [
            "view"
          ],
          "type": "string"
        },
        {
          "description": "Read and edit the live item.",
          "enum": [
            "edit"
          ],
          "type": "string"
        }
      ]
    },
    "SharedItemRef": {
      "additionalProperties": false,
      "description": "A recognized kind and stable item UUID.",
      "properties": {
        "id": {
          "description": "The stable UUID of the item.",
          "type": "string"
        },
        "kind": {
          "description": "The item kind, either notes or collections in this slice.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "kind"
      ],
      "type": "object"
    }
  },
  "description": "Create or replace a named grant.",
  "properties": {
    "audience": {
      "allOf": [
        {
          "$ref": "#/definitions/ShareAudience"
        }
      ],
      "description": "The named person or group receiving access."
    },
    "item": {
      "allOf": [
        {
          "$ref": "#/definitions/SharedItemRef"
        }
      ],
      "description": "The target item reference."
    },
    "permission": {
      "allOf": [
        {
          "$ref": "#/definitions/SharePermission"
        }
      ],
      "description": "The permission to grant."
    }
  },
  "required": [
    "audience",
    "item",
    "permission"
  ],
  "title": "ShareGrantInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": true,
  "title": "Map_of_AnyValue",
  "type": "object"
}
```
