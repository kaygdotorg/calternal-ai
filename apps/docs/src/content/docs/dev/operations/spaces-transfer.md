---
title: "spaces.transfer"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Transfer ownership to a current account in the space.

Capability: space.transfer.

## Declaration

```json
{
  "risk": "widens_access",
  "required_capability": "space.transfer",
  "target": "space",
  "id": "spaces.transfer",
  "description": "Transfer ownership to a current account in the space.",
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
      "description": "Space UUID; names are never accepted in its place.",
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
