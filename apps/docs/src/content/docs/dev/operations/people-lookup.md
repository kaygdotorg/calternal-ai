---
title: "people.lookup"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Find mentionable accounts and groups in a space.

Capability: space.read.

## Declaration

```json
{
  "risk": "read",
  "required_capability": "space.read",
  "target": "space",
  "id": "people.lookup",
  "description": "Find mentionable accounts and groups in a space.",
  "mutates": false
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Picker filter within one space.",
  "properties": {
    "prefix": {
      "default": "",
      "description": "Literal display-name prefix. Omit to browse.",
      "type": "string"
    },
    "space_id": {
      "description": "Selected space UUID.",
      "type": "string"
    }
  },
  "required": [
    "space_id"
  ],
  "title": "SpaceLookupInput",
  "type": "object"
}
```

## Output

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "definitions": {
    "PersonMatch": {
      "description": "A named account or group directly granted access to the selected space.",
      "properties": {
        "id": {
          "description": "Account or group UUID.",
          "type": "string"
        },
        "kind": {
          "description": "Subject type, either account or group.",
          "type": "string"
        },
        "name": {
          "description": "Display name.",
          "type": "string"
        }
      },
      "required": [
        "id",
        "kind",
        "name"
      ],
      "type": "object"
    }
  },
  "description": "The first page of matching mention subjects.",
  "properties": {
    "people": {
      "description": "Matching direct accounts and groups.",
      "items": {
        "$ref": "#/definitions/PersonMatch"
      },
      "type": "array"
    }
  },
  "required": [
    "people"
  ],
  "title": "PeopleLookupResult",
  "type": "object"
}
```
