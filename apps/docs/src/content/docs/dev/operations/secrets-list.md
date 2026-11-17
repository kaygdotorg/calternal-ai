---
title: "secrets.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List secret metadata and pending requests, never values

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "secrets.list",
  "description": "List secret metadata and pending requests, never values",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "List secret metadata and pending requests of one owner.",
  "properties": {
    "space_id": {
      "description": "Owning space UUID; omit for the caller's personal secrets.",
      "type": "string"
    }
  },
  "title": "SecretsListInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "definitions": {
    "SecretMetadata": {
      "additionalProperties": false,
      "description": "Everything about a stored secret except its value.",
      "properties": {
        "created_at": {
          "description": "Creation time in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "label": {
          "description": "Human label.",
          "type": "string"
        },
        "name": {
          "description": "Secret name.",
          "type": "string"
        },
        "updated_at": {
          "description": "Last change in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "version": {
          "description": "Current version, increased by each rotation.",
          "format": "uint64",
          "minimum": 0,
          "type": "integer"
        }
      },
      "required": [
        "created_at",
        "label",
        "name",
        "updated_at",
        "version"
      ],
      "type": "object"
    },
    "SecretPendingRequest": {
      "additionalProperties": false,
      "description": "A request waiting for a value.",
      "properties": {
        "created_at": {
          "description": "Request time in Unix milliseconds.",
          "format": "int64",
          "type": "integer"
        },
        "id": {
          "description": "Request UUID.",
          "type": "string"
        },
        "label": {
          "description": "Label shown in the prompt.",
          "type": "string"
        },
        "name": {
          "description": "Name the value will be stored under.",
          "type": "string"
        },
        "reason": {
          "description": "Reason shown in the prompt.",
          "type": "string"
        },
        "requested_by": {
          "description": "Who asked: account and route (session, plugin or agent thread).",
          "type": "string"
        }
      },
      "required": [
        "created_at",
        "id",
        "label",
        "name",
        "reason",
        "requested_by"
      ],
      "type": "object"
    }
  },
  "description": "Metadata and pending requests of one owner; never values.",
  "properties": {
    "requests": {
      "description": "Pending requests, oldest first.",
      "items": {
        "$ref": "#/definitions/SecretPendingRequest"
      },
      "type": "array"
    },
    "secrets": {
      "description": "Stored secrets ordered by name.",
      "items": {
        "$ref": "#/definitions/SecretMetadata"
      },
      "type": "array"
    }
  },
  "required": [
    "requests",
    "secrets"
  ],
  "title": "SecretsListResult",
  "type": "object"
}
```
