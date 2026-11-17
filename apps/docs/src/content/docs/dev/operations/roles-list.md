---
title: "roles.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List the space roles and their capabilities.

Capability: roles.list.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "roles.list",
  "target": "account",
  "id": "roles.list",
  "description": "List the space roles and their capabilities.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "An operation scoped to the authenticated account or server with no input fields.",
  "title": "DirectoryInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "RoleDefinition": {
      "description": "A space role and its currently declared capabilities.",
      "properties": {
        "capabilities": {
          "description": "Stable dotted capability names from the authority catalog.",
          "items": {
            "type": "string"
          },
          "type": "array"
        },
        "name": {
          "description": "The stored role name.",
          "type": "string"
        }
      },
      "required": [
        "capabilities",
        "name"
      ],
      "type": "object"
    }
  },
  "description": "Space role catalog result.",
  "properties": {
    "roles": {
      "description": "Role definitions, ordered by name.",
      "items": {
        "$ref": "#/definitions/RoleDefinition"
      },
      "type": "array"
    }
  },
  "required": [
    "roles"
  ],
  "title": "RolesResult",
  "type": "object"
}
```
