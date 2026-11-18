---
title: "collections.create"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Save an explicit set of notes in one space

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "reversible",
  "id": "collections.create",
  "description": "Save an explicit set of notes in one space",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Create a saved set of notes in one space.",
  "properties": {
    "members": {
      "description": "Explicit same-space note UUIDs.",
      "items": {
        "type": "string"
      },
      "type": "array"
    },
    "space_id": {
      "description": "Owning space UUID.",
      "type": "string"
    },
    "title": {
      "description": "Display title.",
      "type": "string"
    }
  },
  "required": [
    "members",
    "space_id",
    "title"
  ],
  "title": "CollectionCreateInput",
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
