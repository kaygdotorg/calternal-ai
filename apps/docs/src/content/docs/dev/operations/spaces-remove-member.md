---
title: "spaces.remove_member"
---

<!-- Generated from: contract/operations.json. Rebuild: npm run docs:site -->

> **Preview: the app is not released.** This site describes a development preview and accepted designs; it does not establish production readiness.

Remove a member

Capability: not declared in this catalogue; the host policy remains authoritative.

## Declaration

```json
{
  "id": "spaces.remove_member",
  "description": "Remove a member",
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
