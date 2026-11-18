---
title: "collections.set_members"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Replace a saved collection's explicit notes

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "risk": "widens_access",
  "id": "collections.set_members",
  "description": "Replace a saved collection's explicit notes",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Replace a saved collection's explicit note list.",
  "properties": {
    "base_revision": {
      "description": "Expected current collection revision.",
      "type": "string"
    },
    "id": {
      "description": "Target collection UUID.",
      "type": "string"
    },
    "members": {
      "description": "Explicit same-space note UUIDs.",
      "items": {
        "type": "string"
      },
      "type": "array"
    }
  },
  "required": [
    "base_revision",
    "id",
    "members"
  ],
  "title": "CollectionMembersInput",
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
