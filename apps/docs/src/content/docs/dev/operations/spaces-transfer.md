---
title: "spaces.transfer"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Transfer ownership to an existing member

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "spaces.transfer",
  "description": "Transfer ownership to an existing member",
  "mutates": true
}
```

## Input

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "additionalProperties": false,
  "description": "Target account for removal or explicit ownership transfer.",
  "properties": {
    "account_id": {
      "description": "Stable target account UUID, independently allocated from the space.",
      "type": "string"
    },
    "space_id": {
      "description": "Stable space UUID; development inputs temporarily accept seeded aliases.",
      "type": "string"
    }
  },
  "required": [
    "account_id",
    "space_id"
  ],
  "title": "SpaceMemberInput",
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
