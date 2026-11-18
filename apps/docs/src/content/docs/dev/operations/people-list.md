---
title: "people.list"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

List server accounts.

Capability: people.manage.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "people.manage",
  "target": "server",
  "id": "people.list",
  "description": "List server accounts.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "A continuation for a server directory.",
  "properties": {
    "cursor": {
      "description": "The previous page's cursor. Omit it to start at the first row.",
      "maxLength": 524288,
      "minLength": 1,
      "type": "string"
    }
  },
  "title": "DirectoryPageInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "DirectoryPerson": {
      "description": "A person in the authorized server directory.",
      "properties": {
        "account_id": {
          "description": "The account's stable UUID.",
          "type": "string"
        },
        "disabled": {
          "description": "Whether the account may sign in and exercise its grants.",
          "type": "boolean"
        },
        "name": {
          "description": "Current display name.",
          "type": "string"
        },
        "role": {
          "description": "Server role, independent of space access.",
          "type": "string"
        }
      },
      "required": [
        "account_id",
        "disabled",
        "name",
        "role"
      ],
      "type": "object"
    }
  },
  "description": "Server people visible to the operator.",
  "properties": {
    "next_cursor": {
      "description": "Continuation after the last row, omitted at the end.",
      "type": [
        "string",
        "null"
      ]
    },
    "people": {
      "description": "Current accounts, including disabled records.",
      "items": {
        "$ref": "#/definitions/DirectoryPerson"
      },
      "type": "array"
    }
  },
  "required": [
    "people"
  ],
  "title": "PeopleResult",
  "type": "object"
}
```
